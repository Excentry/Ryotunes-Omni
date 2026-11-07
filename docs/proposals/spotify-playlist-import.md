# Proposal: import Spotify playlists and play them through YouTube Music

Status: proposal, not scheduled. No owner yet.

## Summary

The user links a Spotify account, picks playlists (or Liked Songs) from their Spotify library, and Ryotune-Omni copies them as playlists whose tracks are matched to YouTube Music. The copy can be kept as a local playlist, written to the user's YouTube Music account, or downloaded. Playback goes through the existing InnerTube path (`crates/core/src/orchestrator.rs`). Spotify supplies only metadata.

## Why

- Spotify playback is deferred on Windows (`docs/COMPATIBILITY.md`, [Windows changelog](../windows/CHANGELOG.md#w23--spotify-aplazado-en-windows)). The inherited transport is a Unix FIFO (`crates/spotify/src/stream.rs` returns `Spotify streaming needs a Unix FIFO; this platform is unsupported`), librespot received a refused audio key for the Premium test account, and Spotify Soloist exits shortly after playback starts on the test build. Without this feature a Windows user, Premium or not, cannot listen to their Spotify library in Ryotune-Omni.
- Spotify sign-in requires Premium today, so free accounts cannot bring their library either.

## Rules

- Spotify audio is never extracted, saved or re-encoded. librespot is used only for the authenticated session and library metadata. Downloads always come from the matched YouTube source and keep the existing label "YouTube match for Spotify" (`crates/ryotunesd/src/downloads.rs`), never a claim of the Spotify recording.
- No anonymous or impersonated Spotify tokens. Everything goes through the user's own sign-in.
- No capture of another Spotify client's audio output either; the W23 deferral already discarded it on terms grounds.

## Current state

- Sign-in: PKCE OAuth and librespot (`crates/spotify/src/auth.rs`). `sign_in` and `restore` call `premium()`, which fails with `SignInProblem::Premium` for non-Premium accounts.
- The Tauri host does not expose the daemon's Spotify sign-in commands, and has no native downloads (`docs/COMPATIBILITY.md`). Both exist only in the inherited Linux daemon (`crates/ryotunesd`).
- Library reads already exist in `crates/spotify/src/client.rs`: `playlists(limit)`, `playlist(id)`, `playlist_tracks(id)`, `liked(page)`, `saved_albums(limit)`, `album_tracks(id)`. `Track` carries `id`, `name`, `artists`, `album`, `duration` and `cover` (`crates/spotify/src/models.rs`).
- InnerTube search: `search_songs` and `search_all` (`crates/innertube/src/endpoints.rs`).
- Local playlists need no YouTube sign-in: `Db::create_local_playlist` and `Db::add_local_playlist_track` (`crates/core/src/db.rs`).
- Spotify downloads resolve the source with `ytsearch1:{artists} {title} official audio` (`crates/ryotunesd/src/downloads.rs`): first result, no scoring, no duration check.
- The only playlist import reads a Ryotunes Omni JSON file (`import_playlist_file`) and writes into a YouTube Music playlist (`ui/src/routes/library/+page.svelte`).

## User flow

1. Settings or Library: "Link Spotify". The existing OAuth flow opens; the user approves.
2. Library: "Import from Spotify" lists the user's playlists, Liked Songs and saved albums, with track counts.
3. The user selects sources and a destination:
   - **Local playlist** (default, no YouTube sign-in);
   - **YouTube Music playlist** (only when signed in to Google);
   - **Download** the matched tracks (only where native downloads are available).
4. Progress shows per source (`loading`, `matching i/n`, `saving` or `downloading`) with a cancel button.
5. A summary lists tracks that were not matched or matched with low confidence. For each one the user can search a replacement or skip it.
6. The result plays like any other playlist, through YouTube Music.

## Design

### Matching: `catalog_match`

New module in `crates/core`, shared by the import and by Spotify downloads, replacing the `ytsearch1` query.

Search sequence per track, stopping at the first accepted candidate:

1. `search_songs("<title> <primary artist>")`
2. `search_all` with the same query
3. `search_songs("<title>")`

Score each candidate on:

- normalized title similarity, after removing featured artists, bracketed noise and remaster tags;
- best artist overlap against all source artists;
- duration difference, rejecting candidates beyond a fixed tolerance;
- penalties for version markers (live, remix, sped up, cover, instrumental) present on one side only.

Outcome per track: `matched`, `low_confidence` (below the acceptance threshold but not rejected) or `not_found`. Only `matched` tracks are added automatically.

Limits: bounded concurrency; cancellation checked between searches; a cache table keyed by Spotify track id with the chosen `video_id` and score, so overlapping or repeated imports do not search again. A replacement chosen by the user overwrites the cache entry. Throttling is reported as a retryable error, not as `not_found`.

### Reading the source

- `playlist_tracks` for playlists, `album_tracks` for albums, `liked` paged until empty for Liked Songs.
- Skip local files and podcast episodes. Keep each track's Spotify id, title, artists, album, duration and position.

### Destinations

- **Local:** one local playlist per source, matched tracks in source order. The Spotify id of the source is stored so a re-import refreshes the playlist instead of duplicating it. Two-way sync is out of scope.
- **YouTube Music:** `create_playlist` and `add_to_playlist`, as the file import does today.
- **Download:** queue the matched `video_id`s through the existing download path, labelled as YouTube matches.

### Commands and events

| Name | Kind | Purpose |
| --- | --- | --- |
| `spotify_import_sources` | command | List playlists, Liked Songs and saved albums with counts |
| `spotify_import_start { ids, destination }` | command | Start importing; returns a job id |
| `spotify_import_cancel { job }` | command | Stop the job; no partial playlist is kept |
| `spotify_import_progress` | event | `{ job, source, phase, done, total }` |
| `spotify_import_done` | event | `{ job, results: [{ source, playlist_id, matched, low_confidence, not_found }] }` |

Register them on every surface the contract requires: `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src-tauri/build.rs` (`APP_COMMANDS`), `src-tauri/permissions/`, `crates/ryotunesd/src/methods.rs`, `crates/protocol` wire types and `ui/src/lib/api.ts`. Add the rows to `docs/windows/CONTRACT-MATRIX.md`.

## Phases

### Phase 0: prerequisites on Windows

- Verify that `playlists`, `playlist_tracks`, `liked` and `album_tracks` return data on Windows 11. OAuth sign-in and session restore already worked there in the W23 lab with a Premium account; the refused audio key affects playback only, but library reads have not been measured.
- Expose Spotify sign-in, sign-out and session state in the Tauri host, with the same registration rules as any command.

### Phase 1: matching

- Build `catalog_match` and measure it on three or four real playlists before any UI. Record the measured match rate in `docs/windows/CHANGELOG.md`.
- Switch Spotify downloads in `crates/ryotunesd/src/downloads.rs` from `ytsearch1` to `catalog_match`.

### Phase 2: import to a local playlist

- Commands, events, progress, cancel, summary and manual replacement, with the local destination only.

### Phase 3: optional

- YouTube Music destination.
- Download destination; on Windows it needs native downloads ported to the Tauri host first.
- Library-only sign-in for free accounts: `sign_in` and `restore` take a requirement, `Playback` (Premium enforced, current behaviour) or `Library` (not enforced), and Spotify playback stays gated on Premium. Spotify may refuse free accounts on its side regardless of our check; verify with a free test account on librespot 0.8.0 before building it.

## Acceptance criteria

- On Windows 11, a linked Spotify account lists its playlists, Liked Songs and saved albums.
- A playlist of more than 1,000 tracks imports completely into a local playlist, in source order, with no YouTube sign-in, and plays through InnerTube with no Spotify stream opened.
- Every source track appears in the summary as `matched`, `low_confidence` or `not_found`; none are dropped silently.
- Cancelling stops further searches and leaves no partial playlist.
- Re-importing the same source refreshes the existing playlist and reuses cached matches.
- Spotify downloads use `catalog_match` and keep the "YouTube match for Spotify" label.
- No code path writes Spotify audio to disk.

## Tests

- Unit: title and artist normalization; scoring cases (same recording, live against studio, remix, wrong artist, duration out of tolerance); cache hit and manual override; throttling reported as retryable.
- Contract: Tauri and daemon expose the same commands, parameters and event shapes.
- Manual on Windows 11: sign-in; a small playlist, a playlist over 1,000 tracks, Liked Songs, an album; cancel mid-import; re-import; YouTube Music and download destinations once available.

## Risks and open questions

- Match quality decides whether the feature is useful; thresholds must be tuned on real playlists.
- Library reads on Windows are unmeasured; Phase 0 decides whether the rest can start.
- Large imports produce many InnerTube searches; concurrency limits and the cache keep the rate bounded.
- Free-account linking may be refused by Spotify.
- Spotify's terms on using library metadata to play or download from another service need a review before release.
