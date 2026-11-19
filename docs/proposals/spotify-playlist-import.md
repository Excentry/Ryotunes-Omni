# Proposal: import Spotify playlists and play them through YouTube Music

Status: feasibility experiments, the supervised local-import contract and the core/storage prototype are assigned to @IsaVonxz-type by explicit user request. Additive storage, in-memory jobs and an isolated Tauri import laboratory are implemented; @Excentry review is pending. The laboratory has native metadata authorization, manual candidate search, restricted IPC and a review dialog. Production integration, scored matching, remote playlist writes, downloads and release clearance remain pending. This work does not change Spotify's Windows playback deferral. Findings are recorded in the [Windows changelog](../windows/CHANGELOG.md#spotify-playlist-import--phase-0-feasibility).

Phase 0 evidence now includes official Web API reads on Windows of three selected owned playlists, Liked Songs, saved-album listing and one album's tracks. A user-authorized test playlist with 1,251 occurrences (370 distinct track URIs) passed two complete paginated reads with stable snapshot, order and repetitions. Playlist discovery remains unresolved: with a declared total of seven, size-50 pages returned six distinct playlist IDs; size-1 and size-2 traversals returned seven entries but the same six distinct IDs, with a repeated ID. Each page size was stable across two traversals, but order differed between sizes. Smaller pages did not recover an additional distinct playlist. This is partial technical feasibility, not approval of the import, proof of access to other users' playlists, or distribution clearance.

## Summary

The user links a Spotify account, picks playlists (or Liked Songs) from their Spotify library, and Ryotune-Omni copies them as playlists whose tracks are matched to YouTube Music. The copy can be kept as a local playlist, written to the user's YouTube Music account, or downloaded. Playback goes through the existing InnerTube path (`crates/core/src/orchestrator.rs`). Spotify supplies only metadata.

## Why

- Spotify playback is deferred on Windows (`docs/COMPATIBILITY.md`, [Windows changelog](../windows/CHANGELOG.md#w23--spotify-aplazado-en-windows)). The inherited transport is a Unix FIFO (`crates/spotify/src/stream.rs` returns `Spotify streaming needs a Unix FIFO; this platform is unsupported`), librespot received a refused audio key for the Premium test account, and Spotify Soloist exits shortly after playback starts on the test build. Without this feature a Windows user, Premium or not, cannot listen to their Spotify library in Ryotune-Omni.
- Spotify sign-in requires Premium today, so free accounts cannot bring their library either.

## Rules

- Spotify audio is never extracted, saved or re-encoded. The feasibility probe uses the official Spotify Web API with PKCE, independent of the inherited librespot playback client. A production metadata-only session remains to be implemented. Any future downloads must come from the matched YouTube source and keep the existing label "YouTube match for Spotify" (`crates/ryotunesd/src/downloads.rs`), never a claim of the Spotify recording.
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

Outcome per source occurrence: `matched`, `low_confidence` (ambiguous or insufficient confidence), `not_found`, `unsupported` (for example a local file or podcast), `source_error` (the entry or its metadata could not be read), or `search_error` (the search failed). The supervised prototype requires explicit acceptance or skipping of every occurrence, including `matched` proposals; nothing is saved automatically. These outcomes are distinct so an unreadable item or network failure is never reported as a search miss.

Limits: bounded concurrency; cancellation checked between searches; a candidate cache keyed by Spotify track id with the chosen `video_id` and score, so overlapping imports do not search again. Keep user-selected replacements separate and scoped to the local source/account; one user's preference must not overwrite a shared candidate for other users. Throttling is reported as a retryable error, not as `not_found`.

### Reading the source

- The current `playlist_tracks` and `album_tracks` return `Vec<Track>` and can omit source entries whose URI or metadata is unsupported or missing; `Track` has no source position. Do not use those lossy results as the import ledger. Add import-specific source readers that preserve each raw source occurrence and its ordinal before resolving metadata.
- Read `playlist_tracks` for playlists, `album_tracks` for albums, and `liked` page-by-page until empty. Keep each occurrence's source id when present, ordinal, URI/type and any available title, artists, album and duration. Local files, podcast episodes and entries with unavailable metadata remain in the ledger with `unsupported` or `source_error`; they are not silently filtered out.
- Spotify permits the same track to occur more than once in a playlist. Treat occurrences as distinct even when Spotify track ids or matched YouTube `video_id`s repeat. The current local playlist key `(playlist_id, video_id)` deduplicates identical videos, so a schema/API change is required before local import can claim complete fidelity. Preserve order using occurrence/position identity, not `video_id` uniqueness.

### Destinations

- **Local:** one imported local playlist per source, matched occurrences in source order, including repeated matches. Store source identity and occurrence identity separately from `video_id`; re-import replaces the imported contents without duplicating the playlist. Two-way sync is out of scope. Build the new contents in staging and atomically replace the existing imported contents only after a successful job; cancellation or failure before commit leaves the previous playlist unchanged and removes staging.
- **YouTube Music:** `create_playlist` and `add_to_playlist`, as the file import does today.
- **Download:** queue the matched occurrences through the existing download path, labelled as YouTube matches. Repeated occurrences may resolve to the same downloaded file; report each occurrence while avoiding redundant downloads.

### Supervised local-import contract

The first prototype handles one owned/collaborative Spotify playlist per job, selected by URL, and one local destination. Liked Songs, albums, remote destinations and downloads are not part of this increment. The pure UI contract and selection checks live in `ui/src/lib/spotify-import.ts`; they are not a security boundary or a registered host implementation.

#### Identity and review

- The host assigns `jobId`, binds it to the initiating app window/session and Spotify account, and resolves the source playlist, snapshot and local target. A WebView cannot provide a different account or arbitrary replacement target at commit time.
- Only the authorized local main WebView can control these jobs. Account disconnect/switch invalidates that account's uncommitted jobs and cached review metadata; restoring credentials must not revive a stale job. No import command is granted to remote or hidden WebViews.
- Source identity is `(accountKey, playlistId)`. Source `snapshotId` identifies this captured revision, not the playlist itself; it must not be part of the uniqueness key for re-import.
- Each raw source occurrence gets an opaque `occurrenceId` and a contiguous zero-based `ordinal`. Repeated Spotify IDs, null items and repeated matched YouTube videos retain independent occurrences. An occurrence ID identifies a row of one job, not a song across imports.
- Candidate IDs are issued by the host and scoped to `(jobId, occurrenceId)`. A candidate includes YouTube metadata for review; acceptance sends only the candidate ID. The host reloads the stored candidate, never accepts client-supplied video IDs, metadata, paths or scores as authoritative.
- All occurrences start undecided. A user must explicitly accept an offered candidate or skip each occurrence. No bulk skip is provided. Any outcome, including `unsupported` and `source_error`, can accept a host-offered manual replacement; the UI does not search these entries automatically, and the user can leave any decision pending.
- A review has a monotonically increasing revision. New candidate searches invalidate older commit requests. The connected session can enumerate its review job IDs and retrieve each full snapshot, so closing the review dialog does not discard prepared jobs. Disconnecting or closing the isolated app invalidates in-memory jobs. Validation rejects duplicate occurrence IDs, ordinal gaps, decisions for another occurrence, duplicate decisions and unoffered candidates.
- Summary has separate outcome counts and decision counts: `total = accepted + skipped + pending`. A skipped unavailable occurrence remains in the summary. Empty acceptance is rejected so a failed or accidentally cleared selection cannot replace an existing playlist with an empty one.

#### Job lifecycle and commands (proposed, not registered)

`loading -> matching -> review -> committing -> committed`. Before committing, cancellation leads to `cancelled`; operational failures lead to `failed`. Only `review` can request commit. Search errors stay attached to their occurrences for retry/skip; they are not silently converted to `not_found`.

| Name | Proposed arguments | Result |
| --- | --- | --- |
| `spotify_import_start` | `{ playlistUrl }` | Host-owned job ID; always a local target, regardless of Google sign-in |
| `spotify_import_snapshot` | `{ jobId }` | Full phase/revision, source binding, target revision, occurrence ledger and candidate metadata |
| `spotify_import_search` | `{ jobId, occurrenceId, expectedReviewRevision, query }` | Bounded metadata search; stores candidates in that job and returns an updated review |
| `spotify_import_commit` | `{ jobId, expectedReviewRevision, expectedPlaylistRevision, decisions }` | Commit receipt for the local playlist |
| `spotify_import_cancel` | `{ jobId }` | Authoritative terminal/current phase, not an optimistic acknowledgement of rollback |
| `spotify_import_changed` | event with `{ jobId, revision, phase }` | Notification to retrieve a snapshot; no tokens, cookies or authorization URLs |

Commit decisions are `{ occurrenceId, kind: "skip" }` or `{ occurrenceId, kind: "accept", candidateId }`. Their request order is not playback order: the host builds accepted rows in source ordinal order. The UI planner produces a preview, but the host independently validates the complete source ledger, account/window binding, phase, revisions, candidate ownership and selected metadata.

The host serializes commit/cancel for a job. If cancellation wins before the transaction starts, no destination data changes. Once commit is admitted, cancellation cannot claim rollback; it returns the resulting committed/failed state. If the UI loses the response, it reads job status rather than starting another import.

#### Storage and compatibility (migration proposal)

Current `local_playlist_tracks` uses primary key `(playlist_id, video_id)`, `INSERT OR IGNORE`, and removal by video ID. `create_playlist` in the Tauri host chooses YouTube when Google is signed in. The importer must not call either existing path to simulate a complete local import.

Prefer additive occurrence-aware storage for imported playlists rather than changing the semantics of existing device playlists in place:

- `local_playlist_imports`: one provenance row per imported local playlist, with a unique `(account_key, source_playlist_id)` mapping, captured source snapshot and a destination content revision. Its playlist ID references `local_playlists`.
- `local_playlist_import_entries`: playlist ID, host-generated stored entry ID, source ordinal, dense playback position, chosen video ID and sanitized YouTube `song_json`. Primary key `(playlist_id, entry_id)` and unique `(playlist_id, position)`; video ID is indexed but not unique. Preserve source ordinals even when skipped occurrences create gaps. Stored entry IDs need not survive a complete re-import.
- `local_playlist_import_receipts`: unique job ID, source/account binding, decision digest, resulting playlist ID/revision and count summary. This allows a repeated commit after response loss to return its receipt without applying writes again. A different decision digest for an already committed job is a conflict.
- Read imported contents from the occurrence table when provenance exists; otherwise retain the legacy playlist path. Do not merge both tables and accidentally duplicate rows. Keep membership queries set-like per playlist even when occurrences repeat.
- Update library counts, artwork lookup, rename/delete and membership readers to understand imported rows. Deletion must remove provenance and occurrence rows in the same transaction as the playlist, with deliberate receipt-retention semantics; do not assume foreign-key cascading exists in the current connection configuration. An imported playlist must not appear empty because a legacy count reads only `local_playlist_tracks`.
- Introduce an additive local entry ID in playlist row DTOs and occurrence-aware remove/reorder commands before exposing imported playlists for editing. Never route their removal through legacy `DELETE ... WHERE video_id = ?`; existing commands must reject that unsupported operation or use the new occurrence-aware path. Queue/render keys must preserve multiple appearances as well.

The user explicitly authorized @IsaVonxz-type to implement this feature's core/storage increment, with @Excentry review pending. `Db::open` now initializes the three additive tables; it does not rewrite legacy playlist rows. Tests use in-memory databases and isolated temporary files, not the user's database. This exception does not transfer ownership of other backend work or change other owners' roadmap rows.

#### Implemented core increment

`crates/core/src/spotify_import.rs` provides host-owned staging, owner checks, review revisions, scoped candidate validation, explicit decisions, cancellation/invalidation and commit planning. The service accepts host-provided metadata; it does not perform Spotify authorization, source reads or matching. A single jobs mutex serializes job mutations and remains held through database commit, so cancellation cannot report a rollback after a successful commit. Failed storage operations restore the prior review phase for retry.

`crates/core/src/db/spotify_import.rs` adds transactional initial import/replacement, durable receipts, occurrence reads, remove-one and reorder. Replacement checks the captured destination revision and preserves its local title. Removal/reorder require the expected revision and leave dense playback positions without changing source ordinals. Legacy video-based add/remove operations reject imported destinations. Existing library readers, counts, artwork lookup, membership, rename and deletion recognize imported storage; receipt retention is deliberate when deleting a destination.

Limits: 32 retained jobs, 10,000 source occurrences per job and 100 candidates per occurrence. The host must release terminal jobs and invalidate owner sessions on disconnect/window teardown. Uncommitted jobs are not recoverable after restart. A stored receipt can be queried by account/job ID after restart, but the job service cannot resume or replay a missing job. The `failed` phase is reserved; source-loading/matching failure transitions and event delivery remain host-integration work.

Local checks cover 1,251 repeated video occurrences, scoped decisions, unavailable entries, stale revisions, cancellation/commit races, reimport, conflicting source commits, legacy database reopen, persisted receipt replay and injected SQLite failures during initial insert, replacement receipt insertion and remove-one compaction. These are storage/service tests, not recording or playback validation. The Tauri local reader now preserves stored occurrence IDs in `set_video_id`; its legacy video-based edit commands still reject imported playlists. Occurrence-aware editing IPC remains pending.

#### Isolated Tauri import laboratory

`src-tauri/tauri.import-lab.conf.json` selects application identifier `dev.excentry.ryotune-omni.import-lab`. This separates its data directory and WebView profile from the normal app. The host initializes import state only for that identifier, checks the local main WebView at runtime and grants the dedicated permission only through the main capability. The build manifest, handler, permission and generated ACL schemas include the laboratory commands. Normal-profile status returns `enabled: false`.

`crates/core/src/spotify_metadata.rs` implements metadata-only PKCE using the existing developer app and exact loopback redirect `http://127.0.0.1:8989/login`. Tokens and verifier stay in native memory; no refresh token is retained. The listener checks state, path and duplicate parameters, bounds HTTP input and times out. API responses are bounded, redirects are disabled and pagination uses host-constructed Spotify URLs rather than following arbitrary `next` destinations. Reads require a stable snapshot, consistent total and complete occurrence count. The laboratory now accepts complete playlists of 1 to 1,000 entries, so all three selected sources fit without sampling; the 1,251-entry fixture remains rejected. The host exposes the limit through status for the UI. Closing/disconnecting invalidates jobs and cancels pending authorization, reads and searches.

`src-tauri/src/spotify_import.rs` wires authorization, start/snapshot, occurrence-scoped candidate search, commit, cancel and readback. Windows authorization opens the system HTTPS handler through `ShellExecuteW` on the main thread and checks the result; the inherited `explorer.exe` opener opened Documents on the test machine and is not used for this flow. The library's `SpotifyImport.svelte` dialog is shown only in the laboratory. Search offers up to five host-owned results per occurrence, without automatic acceptance or production scoring. Every occurrence needs an explicit accept or omit decision before saving; source errors and unsupported entries can be searched manually, but remain outside automatic batch search. The user can clear a decision to leave it pending, navigate to the next undecided occurrence, and save remains disabled until none are pending. Reload status and durable receipts recover a lost commit response. Readback compares accepted count, dense order, video IDs, source ordinals and distinct stored entry IDs against the reviewed plan.

Batch search proposes candidates for all unsearched, undecided, supported occurrences in source order. Each request uses the latest returned review revision. It stops on the first error without automatic retries, supports stopping after the in-flight request, and never accepts or skips an occurrence. After verified save, "Import another playlist" clears the prior client review while keeping the same Spotify authorization. The user requested complete imports of the three previously selected playlists (63, 33 and 117 occurrences; 213 total), still in the isolated profile and with candidate review before commit. Their live preparation is separate from the earlier three-song proof.

All 213 source occurrences were captured in native jobs; 207 initially had offered candidates. Structured artist-credit recovery and 62 bounded additional metadata searches produced 161 provisional proposals under the existing experimental rules, with 41 rejected comparisons, seven ambiguities and four unavailable/unsupported source entries. No decisions or full-playlist commits were sent. These remain three staged reviews, not three completed imports. Candidates found only in additional searches must be offered within the corresponding host job before their IDs can be accepted. Any omission or selection of an uncertain alternative needs an explicit user decision.

Build the UI with `pnpm build` from `ui/`, then run `cargo tauri build --debug --no-bundle --config src-tauri/tauri.import-lab.conf.json -- --locked` from the repository root. The lab config disables the inherited build hook, so the frontend build is a separate required step. On the test Windows machine, the build and launch used the existing libmpv cache via process-local `LIB`/`PATH`; no native dependency was installed or replaced. The resulting executable is `target/debug/ryotune-omni.exe`. This is a local test build, not an installer or a release. Before launching it, set `RYOTUNES_SPOTIFY_CLIENT_ID` to the client ID of a Spotify app registered with the callback `http://127.0.0.1:8989/login`; the lab refuses to start authorization without it, and no client ID is committed.

Native PKCE authorization, a complete 33-entry source read and three live candidate searches were observed in the application. After reconnecting, the user-authorized selection of three candidates and explicit skipping of 30 completed through the review UI. The application reported successful commit and readback; the normal `get_playlist` reader independently returned the three expected video IDs in order, with distinct occurrence IDs ending in source ordinals 0, 1 and 2. This verifies the small supervised storage path, not importing all 33 tracks, recording accuracy, playback or a full reimport. Closing the dialog discards its session. Repeated-video preservation and commit/replay remain covered by temporary-database tests, and response-loss/readback handling by fixture-host UI tests. Selected row/candidate foregrounds use the accent foreground for both title and metadata, including hover; the user confirmed the contrast correction in the live dialog.

#### Commit, re-import and failure guarantees

- Keep the captured ledger, candidate results and review in isolated host staging. They are not visible as playlist contents. A crash or cancellation before commit leaves the destination unchanged; persistent job recovery is not promised by the initial prototype.
- In a single database transaction, verify the expected destination revision, create the local playlist/provenance if needed or replace its imported contents, insert accepted occurrences in source order, update source snapshot/content revision and save the idempotency receipt. Any database error rolls back all these writes.
- Re-import the same account/source refreshes its existing imported playlist rather than creating another playlist. Preserve the user's local playlist title. A concurrent edit or another import that changed the destination revision causes conflict, not last-writer-wins replacement.
- If that imported playlist was deleted after review started, fail rather than recreating it silently. A job for a new import with no prior destination can create one only after its source mapping is checked inside the transaction.
- Publish library/playlist change events only after commit. A failed event delivery does not undo a committed transaction; snapshots and receipts are the recovery path.
- Scope candidate caches and manual overrides to the account/source. No tokens or secrets in tables, receipts, events or logs. Reuse a recording candidate across repeated occurrences only as a lookup optimization, never by dropping the occurrences themselves.

Required storage tests before integration: old databases remain readable; repeated videos survive insert/read/remove-one/reorder; two concurrent replacements conflict; failed insert and interrupted commit roll back; cancellation before admission leaves the prior playlist unchanged; re-import keeps the same playlist ID; duplicate commit returns one receipt and performs no new writes; a Google-signed-in host still writes only to local storage. Pure UI tests do not establish any of those database guarantees.

### Earlier multi-destination sketch

The following sketch is retained for later phases; the supervised local contract above takes precedence for the current increment.

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

- Evaluate the official Spotify Web API with PKCE and a project-authorized registered application before choosing librespot for metadata access. Do not reuse the inherited client ID without confirming authorization. Development-mode applications require an app owner with Premium and allowlisted test users; this is an application-access requirement, not evidence that every metadata reader needs Premium.
- Keep the initial probe read-only and isolated: no playback engine, downloads, remote playlist writes, production credential cache or logged authorization/token responses. Check OAuth state, bind the callback before opening authorization, and apply request deadlines and bounded pagination. Report only aggregate results in committed evidence.
- Validate a small owned playlist, a playlist with repeated entries and more than 1,000 occurrences, Liked Songs and saved-album contents. Compare source counts, order and repeated positions; retain unsupported or unavailable entries. A denied or unavailable endpoint is a failed capability check, not an empty successful library.
- Assess policy separately from technical access. Spotify's Developer Policy III.9 permits user-directed transfer of personal data or playlist metadata, but III.5 restricts integration with another service's streams/content. The playlist-transfer exception does not establish permission for integrated playback or downloads. Keep those destinations gated pending review.
- For playlist discovery, distinguish source entries from distinct playlist IDs and report discrepancies rather than promising a complete library. Display a playlist once per ID, without deduplicating track occurrences inside its contents. Direct URL selection can be evaluated independently for an owned/collaborative source; it does not establish complete discovery or bypass Spotify access restrictions.
- Verify that playlist listing, playlist contents, Liked Songs and saved-album contents return complete, occurrence-preserving data on Windows 11. OAuth sign-in and session restore worked there in the W23 lab with a Premium account; library reads have not been measured. Do not infer that the audio-key refusal affects only playback until these reads are tested.
- Expose library-only Spotify sign-in, sign-out and session state in the Tauri host with the minimum library scopes above. Existing `sign_in` and `restore` enforce Premium; free-account linking is not promised by this proposal and requires a separate verified authorization path before Phase 3.

### Phase 1: matching

- An isolated `innertube` example, `catalog_match_lab`, now supports bounded live search and offline replay. The first fifteen live samples produced eleven heuristic proposals; after explicit collaborator-credit handling, offline replay of the retained candidates produced fourteen. This is not measured recording accuracy or independent validation. The matcher is not integrated into production or downloads, and still requires a broader independent sample and review of version/credit edge cases.
- A subsequent held-out selection from the same three playlists, excluding baseline title/artist identities with rules unchanged, produced twelve proposals, one ambiguous result, one candidate rejection and one unsearchable source out of fifteen occurrences. Metadata review found no visible title/artist/version contradictions in the proposals, but did not verify recordings, album editions or explicit/clean status. These results support a supervised prototype, not unattended import or a general accuracy claim. See the changelog for exact limitations.
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
