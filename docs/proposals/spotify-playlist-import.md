# Proposal: import Spotify playlists and play them through YouTube Music

Status: proposal only; not scheduled or approved for implementation. No owner yet. Merging this document does not change Spotify's Windows playback deferral or authorize implementation. Phase 0 is a go/no-go gate.

## Summary

The user links a Spotify account, picks playlists (or Liked Songs) from their Spotify library, and Ryotune-Omni copies them as playlists whose tracks are matched to YouTube Music. The copy can be kept as a local playlist, written to the user's YouTube Music account, or downloaded. Playback goes through the existing InnerTube path (`crates/core/src/orchestrator.rs`). Spotify supplies only metadata.

## Why

- Spotify playback is deferred on Windows (`docs/COMPATIBILITY.md`, [Windows changelog](../windows/CHANGELOG.md#w23--spotify-aplazado-en-windows)). The inherited transport is a Unix FIFO (`crates/spotify/src/stream.rs` returns `Spotify streaming needs a Unix FIFO; this platform is unsupported`), librespot received a refused audio key for the Premium test account, and Spotify Soloist exits shortly after playback starts on the test build. Without this feature a Windows user, Premium or not, cannot listen to their Spotify library in Ryotune-Omni.
- Spotify sign-in requires Premium today, so free accounts cannot bring their library either.

## Rules

- Spotify audio is never extracted, saved or re-encoded. librespot is used only for the authenticated session and library metadata. Downloads always come from the matched YouTube source and keep the existing label "YouTube match for Spotify" (`crates/ryotunesd/src/downloads.rs`), never a claim of the Spotify recording.
- No anonymous or impersonated Spotify tokens. Everything goes through the user's own sign-in.
- Request only OAuth permissions needed for library metadata: `playlist-read-private`, `playlist-read-collaborative` and `user-library-read`. Do not request `streaming` or playback-control scopes for this feature. Separate library authorization from playback authorization; do not weaken the existing Premium gate for Spotify playback. Confirm the exact scopes with the supported Spotify authorization flow before implementation.
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
4. Progress shows per source (`loading`, `matching i/n`, `saving` or `downloading`) with a cancel button. Each source occurrence keeps its original ordinal and is tracked independently, even when the same Spotify track appears more than once.
5. A summary accounts for every source occurrence, including unsupported entries, source/metadata failures, unmatched tracks and low-confidence matches. For each matchable item the user can search a replacement or skip it.
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

Outcome per source occurrence: `matched`, `low_confidence` (below the acceptance threshold but not rejected), `not_found`, `unsupported` (for example a local file or podcast), or `source_error` (the entry or its metadata could not be read). Only `matched` tracks are added automatically. These outcomes are distinct so a skipped or unreadable item is never reported as a search miss.

Limits: bounded concurrency; cancellation checked between searches; a candidate cache keyed by Spotify track id with the chosen `video_id` and score, so overlapping imports do not search again. Keep user-selected replacements separate and scoped to the local source/account; one user's preference must not overwrite a shared candidate for other users. Throttling is reported as a retryable error, not as `not_found`.

### Reading the source

- The current `playlist_tracks` and `album_tracks` return `Vec<Track>` and can omit source entries whose URI or metadata is unsupported or missing; `Track` has no source position. Do not use those lossy results as the import ledger. Add import-specific source readers that preserve each raw source occurrence and its ordinal before resolving metadata.
- Read `playlist_tracks` for playlists, `album_tracks` for albums, and `liked` page-by-page until empty. Keep each occurrence's source id when present, ordinal, URI/type and any available title, artists, album and duration. Local files, podcast episodes and entries with unavailable metadata remain in the ledger with `unsupported` or `source_error`; they are not silently filtered out.
- Spotify permits the same track to occur more than once in a playlist. Treat occurrences as distinct even when Spotify track ids or matched YouTube `video_id`s repeat. The current local playlist key `(playlist_id, video_id)` deduplicates identical videos, so a schema/API change is required before local import can claim complete fidelity. Preserve order using occurrence/position identity, not `video_id` uniqueness.

### Destinations

- **Local:** one imported local playlist per source, matched occurrences in source order, including repeated matches. Store source identity and occurrence identity separately from `video_id`; re-import replaces the imported contents without duplicating the playlist. Two-way sync is out of scope. Build the new contents in staging and atomically replace the existing imported contents only after a successful job; cancellation or failure before commit leaves the previous playlist unchanged and removes staging.
- **YouTube Music:** `create_playlist` and `add_to_playlist`, as the file import does today.
- **Download:** queue the matched occurrences through the existing download path, labelled as YouTube matches. Repeated occurrences may resolve to the same downloaded file; report each occurrence while avoiding redundant downloads.

### Commands and events

| Name | Kind | Purpose |
| --- | --- | --- |
| `spotify_import_sources` | command | List playlists, Liked Songs and saved albums with counts |
| `spotify_import_start { ids, destination }` | command | Start importing; returns a job id |
| `spotify_import_cancel { job }` | command | Stop further work. Local imports discard staging and preserve the previous playlist; remote YouTube Music changes and completed downloads are not rolled back and are reported as partial results |
| `spotify_import_progress` | event | `{ job, source, phase, done, total }`; `done` counts source occurrences, including unsupported/error entries |
| `spotify_import_done` | event | `{ job, results: [{ source, playlist_id, entries: [{ ordinal, source_uri?, spotify_id?, outcome, video_id? }], matched, low_confidence, not_found, unsupported, source_error, partial }] }` |

Register them on every surface the contract requires: `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src-tauri/build.rs` (`APP_COMMANDS`), `src-tauri/permissions/`, `crates/ryotunesd/src/methods.rs`, `crates/protocol` wire types and `ui/src/lib/api.ts`. Add the rows to `docs/windows/CONTRACT-MATRIX.md`.

## Phases

### Phase 0: prerequisites on Windows

- Verify that playlist listing, playlist contents, Liked Songs and saved-album contents return complete, occurrence-preserving data on Windows 11. OAuth sign-in and session restore worked there in the W23 lab with a Premium account; library reads have not been measured. Do not infer that the audio-key refusal affects only playback until these reads are tested.
- Expose library-only Spotify sign-in, sign-out and session state in the Tauri host with the minimum library scopes above. Existing `sign_in` and `restore` enforce Premium; free-account linking is not promised by this proposal and requires a separate verified authorization path before Phase 3.

### Phase 1: matching

- Build `catalog_match` and measure it on three or four real playlists before any UI. Record the measured match rate in `docs/windows/CHANGELOG.md`.
- Switch Spotify downloads in `crates/ryotunesd/src/downloads.rs` from `ytsearch1` to `catalog_match`.

### Phase 2: import to a local playlist

- Commands, events, progress, cancel, summary and manual replacement, with the local destination only.

### Phase 3: optional

- YouTube Music destination.
- Download destination; on Windows it needs native downloads ported to the Tauri host first.
- Optional research: library-only sign-in for free accounts. Do not build it as part of the initial import unless Spotify allows the least-scope session; first verify a free test account on librespot 0.8.0. If supported, separate `Library` authorization from `Playback` (Premium enforced, current behaviour) and keep Spotify playback gated on Premium.

## Acceptance criteria

- On Windows 11, a linked Spotify account lists its playlists, Liked Songs and saved albums.
- A playlist of more than 1,000 source occurrences imports completely into a local playlist, in source order and with repeated tracks preserved, with no YouTube sign-in, and plays through InnerTube with no Spotify stream opened.
- Every source occurrence appears in the summary as `matched`, `low_confidence`, `not_found`, `unsupported` or `source_error`; none are dropped silently.
- Cancelling a local import stops further searches, discards staging and leaves the existing playlist unchanged. Cancelling a YouTube Music or download job stops future work and reports already completed remote side effects; it does not claim rollback.
- Re-importing the same source refreshes the existing playlist and reuses cached matches.
- Spotify downloads use `catalog_match` and keep the "YouTube match for Spotify" label.
- No code path writes Spotify audio to disk.

## Tests

- Unit: title and artist normalization; scoring cases (same recording, live against studio, remix, wrong artist, duration out of tolerance); cache hit and manual override; throttling reported as retryable.
- Contract: Tauri and daemon expose the same commands, parameters and event shapes.
- Manual on Windows 11: least-scope sign-in; a small playlist, a playlist over 1,000 occurrences with repeated tracks, Liked Songs, an album; source items with unavailable metadata; cancel before local commit; re-import; and verify partial-result reporting for YouTube Music/download cancellation once those destinations are available.

## Risks and open questions

- Match quality decides whether the feature is useful; thresholds must be tuned on real playlists.
- Library reads on Windows are unmeasured; Phase 0 decides whether the rest can start.
- Large imports produce many InnerTube searches; concurrency limits and the cache keep the rate bounded.
- Free-account linking may be refused by Spotify.
- The current Spotify OAuth scope list includes streaming and playback/profile scopes. A metadata-only implementation must request the minimum library scopes and verify that the supported librespot authorization/session flow works without playback scopes; do not quietly reuse the broad current scope set.
- Existing local playlist storage deduplicates by `video_id`; occurrence-preserving import needs a schema/API migration and compatibility tests for existing playlists.
- Remote YouTube Music playlist writes and completed downloads cannot be assumed to roll back on cancellation; the UI and result event must disclose partial side effects.
- Spotify's terms on using library metadata to play or download from another service need a review before release.
