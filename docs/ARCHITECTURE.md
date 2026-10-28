# Ryotune-Omni architecture

> **Status.** Windows and Android are targets; neither has a verified release. The Windows host below is the Tauri host that exists in this repository, and its runtime has only been checked on isolated development builds (see the [Windows changelog](windows/CHANGELOG.md)). The Linux sections are the inherited Ryotunes client and daemon, kept as reference; they still use the Ryotunes name and are not Ryotune-Omni platform support.

Playback is kept independent of the visible interface: a Rust process owns playback, queue and account state, and the UI is a client of that state rather than the clock that drives it. The shared Rust core is reused by every host; each host supplies its own window, transport and operating-system integrations.

## Shared core

- `crates/core` holds `AppState`, the orchestrator, database, lyrics, local library, Discord, Last.fm, OS media controls, radio, Listen Together, cipher, PoToken, session (login) and settings. It does not depend on Tauri. A host implements three traits in `crates/core/src/host.rs`: `EventSink`, `JsBridge` and `LoginFlow`.
- `crates/player` wraps libmpv. `crates/innertube`, `crates/spotify`, `crates/soundcloud`, `crates/tidal`, `crates/listen-protocol` and `crates/sync-server` are the provider clients and the Listen Together protocol and relay.
- `crates/protocol` holds the RPC wire types used by the Linux daemon, the CLI and the Linux launcher in `src-tauri/src/main.rs`. The Windows host links the crate but does not use it.

## Hosts

| Host | Status | Process model |
|:--|:--|:--|
| Windows | Target. Built and run only as isolated development candidates; no release | One process: Rust core, player and WebView2 UI |
| Linux | Inherited reference, not Omni platform support | Two processes over a Unix socket: `ryotune-omni-daemon` and a Quickshell client |
| Android | No host exists; only a Tauri mobile entry-point annotation is present | None |

## Windows host

On non-Linux targets `src-tauri/src/main.rs` starts the Tauri application. The SvelteKit bundle built from `ui/` runs in WebView2; the Rust side owns playback and all integrations.

```text
Windows 11
       |
       +-- SMTC + media keys + tray + taskbar thumbnail toolbar
       |             |
       v             v
  Tauri / Rust ---- player crate ---- libmpv-2.dll (beside the executable)
       |
       +-- crates/core: providers, library, integrations
       |
       +-- WebView2 surfaces (SvelteKit bundle from ui/)
                    |
                    +-- main window (Home / Search / Library / Queue / Lyrics / Now Playing)
                    +-- mini player
                    +-- hidden cipher and PoToken views, and the sign-in window, on demand
```

| File in `src-tauri/src` | Role |
|:--|:--|
| `lib.rs` | Wiring of plugins, state and window events |
| `commands.rs` | The application commands the UI invokes (97 in `APP_COMMANDS` in `build.rs`) |
| `host_sink.rs` | `TauriSink`: every core event becomes a Tauri event on every window |
| `login_webview.rs` | `TauriLogin`: the Google sign-in window and cookie harvest |
| `main_window.rs`, `mini.rs` | Main-window and mini-player lifecycle |
| `tray.rs`, `taskbar.rs` | Tray menu and the Windows taskbar thumbnail toolbar |
| `webview.rs` | Hidden-webview bridge for the cipher and PoToken runtimes |

### Windows lifecycle

- Closing the main window with close-to-tray enabled hides it. The WebView stays resident, unlike Linux, where it is destroyed. Playback and queue continue.
- With close-to-tray disabled, closing the window quits. The tray `Quit` item uses the same `request_quit`, which waits for `AppState::shutdown_for_quit` before the process exits.
- A second launch is handled by `tauri-plugin-single-instance`, which re-shows the running instance instead of starting another player.
- Closing the mini player closes only that surface.
- OS media controls are created from the main window's HWND; if there is no HWND the optional integration is disabled instead of panicking. The taskbar toolbar is driven from the same `playback-state` event the UI receives.

### Windows data and native dependencies

- Data comes from Tauri's path resolver, scoped by the application identifier `dev.excentry.ryotune-omni`. In the isolated runs the SQLite database and the cipher cache were under `%APPDATA%\<identifier>`, and the audio cache and the WebView2 profile (`EBWebView`) under `%LOCALAPPDATA%\<identifier>`. Nothing is shared with Linux or with Ryotunes, and there is no migration.
- `libmpv-2.dll` must sit beside the executable. It is the candidate build described in the [libmpv documents](windows/LIBMPV-OWNED-BUILD.md) and is not promoted for distribution. The executable also links the Visual C++ runtime dynamically, and the WebView2 Runtime is an external requirement; the [W25 record](windows/ROADMAP.md#w25--ejecutable-aislado-y-dependencias) lists the imports and the failure when the DLL is missing.
- Not yet available on the Windows host: native downloads, the daemon's Spotify, SoundCloud and TIDAL sign-in commands, its audio-effect commands, and diagnostics and logs. Google sign-in does exist. Spotify playback depends on a Unix FIFO and is unsupported. See the [parity matrix](windows/PARITY-MATRIX.md) and the [roadmap](windows/ROADMAP.md).

## Linux host (inherited reference)

Ryotunes is split so that playback survives independently of the visible interface. This is especially important on Ryoku laptops, where keeping a WebKit view active for a song that is already playing wastes CPU, memory and power.

### Process model

The Tauri/Rust process is authoritative for playback and desktop integration. The SvelteKit WebView is a client of that state rather than the clock that drives it.

```text
Ryoku / Hyprland
       │
       ├── MPRIS + media keys + tray
       │             │
       ▼             ▼
  Tauri / Rust ── player crate ── libmpv
       │
       ├── Innertube / library / integrations
       │
       └── visible WebKit UI (SvelteKit)
                    │
                    ├── Home / Search / Library
                    ├── Queue / Lyrics / Now Playing
                    └── Mini player
```

### Crates and binaries

The workspace separates the playback core from whoever renders it. Phase 1 of the native-client plan moved everything in `src-tauri/src` that is not a window, a webview or a Tauri command into `crates/core`, behind three host traits; the daemon, CLI and QML rows below are the targets later phases add.

| Path | Role | Origin |
|---|---|---|
| `crates/innertube`, `crates/player`, `crates/listen-protocol`, `crates/sync-server` | unchanged | existing |
| `crates/core` | `AppState`, orchestrator, db, lyrics, local, discord, lastfm, media (MPRIS), radio, listentogether, cipher, potoken, session (login), settings. No Tauri. Talks to the outside through three traits (4.2) | moved from `src-tauri/src` |
| `crates/protocol` | request/response/event types, `serde` only, shared by daemon, CLI, tests | new |
| `crates/ryotunesd` | binary `ryotune-omni-daemon`: socket server, single-instance lock, systemd-friendly lifecycle, GTK thread hosting hidden WebKitGTK views, tray | inherited implementation with Omni-specific runtime identity |
| `crates/ryotunes-cli` | `ryotune-omni-cli <method> [json]` and `events`; local skin tooling | inherited implementation with Omni-specific executable |
| `client/` | QML client run by Quickshell (shipped as `/usr/share/ryotune-omni/client`), plus `client/mini/` | inherited UI with isolated Omni config and runtime paths |
| `src-tauri/` | Tauri host. On Linux `main.rs` launches the daemon and client and never falls back to Tauri; on non-Linux targets it is the application | active (Windows target) |

### Daemon

`ryotune-omni-daemon` hosts `crates/core` behind a Unix socket so the interface can be closed without stopping playback. It is a systemd user service, socket-activated by `ryotune-omni-daemon.socket` on the first client connection, and exits on a bounded idle after the configured grace period.

- Socket: `$XDG_RUNTIME_DIR/ryotune-omni/daemon.sock`, in a `0700` directory, created under `umask 077`. A single instance is enforced with an `flock` on `daemon.sock.lock`; a second launch connects to the incumbent, asks it to `show`, and exits 0. This path is isolated from upstream Ryotunes.
- Framing: newline-delimited JSON, one object per line, both directions. Request `{"id":12,"method":"play","params":{"videoId":"…"}}`; response `{"id":12,"result":…}` or `{"id":12,"error":{"code":"…","message":"…"}}`; event `{"event":"position","data":{"position":12.3}}`.
- Methods: every `#[tauri::command]` name and JSON shape, plus the control methods `hello` (returns `{"protocol":1,"daemon":"2.4.1"}`), `subscribe` (opts into events and replies with the current `playback`/`queue`/`settings`/`auth` snapshot), `show` (raise a subscribed client's window, or launch the client when none is listening), `quit` (stop playback, unregister MPRIS, exit), and `sign_in` (open the Google login window).
- `ryotunes-cli <method> [json]` runs one method and prints its `result`; `ryotunes-cli events [name…]` subscribes and prints one event per line until interrupted.

The Omni Tauri app and daemon share `$XDG_DATA_HOME/dev.excentry.ryotune-omni`; the original Ryotunes app uses a different profile. No data migration occurs.

#### Native downloads

`crates/ryotunesd/src/downloads.rs` owns a persistent download queue independently of the client. `client/Downloads.qml` mirrors the `downloads` subscription snapshot and `downloads-changed` events; the player button, queue/history page and settings use dedicated `enqueue_download`, `get_downloads`, `cancel_download`, `retry_download`, `clear_download_history`, `open_download`, `open_download_folder`, `get_download_settings` and `set_download_settings` socket methods.

- Settings and the latest 200 terminal records live in `downloads.json` under the daemon's data directory. Writes are serialized with queue mutations and use a synced temporary file plus atomic rename. Each job captures its file preferences at enqueue time.
- Worker admission is bounded to 1–4 jobs (default 1), with at most 200 active/queued tracks. A reserved slot is held through cancellation until the subprocess is reaped; retries get a fresh attempt identity so an older task cannot complete a newer attempt.
- yt-dlp runs via direct argv in a separate, lower-priority process group, with one fragment downloader and constrained FFmpeg threading. Config/plugin execution is disabled, output/error buffers are bounded, and a job has a finite timeout. Quit and SIGTERM stop admission, terminate whole process groups and await teardown. Queued/active downloads inhibit daemon idle exit even without a client.
- Downloads go to private staging directories beneath the destination. A successful exit and a nonempty completed audio file are required before a same-filesystem hard link atomically publishes the final file without replacing an existing file. Generated artist directories cannot resolve outside the chosen folder. Clearing history never removes music files.
- With metadata embedding on, `crates/ryotunesd/src/download_media.rs` enriches staged audio via `lofty` and saves image and lyric companions before publication. Artwork uses a trusted thumbnail or a strictly matched iTunes fallback; lyrics reuse the core cache/providers with a bounded lookup. Providers receive track details including title, artist, duration and provider identifiers. For local playback, core lyrics checks `.lrc`, embedded lyrics, then `.txt` before network/cache, enabling offline lyrics. The worker retains its slot throughout enrichment; cancelled jobs never publish. Audio and companions share the final filename stem and collision suffix, using no-overwrite hard links and validated regular staging files. Missing metadata produces non-fatal per-job warnings.
- Restart recovers interrupted jobs as retryable failures, not automatic new network activity. Spotify downloads are explicitly labelled YouTube search matches; they do not export the Spotify stream or promise the same recording. SoundCloud links come from the existing provider client; protected/unavailable content produces a visible job error.
- `client/Daemon.qml` recreates its socket while disconnected: Quickshell retains a failed initial native socket, so setting `connected = true` repeatedly on that object is insufficient. The reconnect timer stops entirely while connected.

### Background playback

Closing the main window is not the same operation as quitting the application.

On Linux, the Quickshell client and `ryotunesd` are separate processes. Closing or hibernating the client does not stop active daemon playback, MPRIS or tray controls. Reopening subscribes to the daemon's current state rather than creating a second player.

A tray-only session with no playback has a bounded idle lifetime and exits automatically. Explicit Quit stops the playback session, unregisters MPRIS and shuts integrations down immediately.

### UI update model

Playback state is event-driven. Ryotunes avoids a permanent high-frequency frontend transport clock and avoids heavy requestAnimationFrame/FFT loops for ordinary idle playback.

The primary QML Home uses a reusable `ListView` with an asynchronous header. `client/components/HomeFeed.qml` pins `contentY` to `originY` until interaction, with `Binding.RestoreNone`: restoring the old pre-layout value (usually zero) when the binding is released skips a tall header whose origin is negative. The QML regression exercises a real wheel event after header growth.

Continuation pages append shelves to a persistent `ListModel`; only an explicit feed reload clears it. Reassigning a JavaScript array as `ListView.model` resets the view to its header, so pagination must not rebuild the entire model or compensate by writing `contentY` afterward. This follows Qt's [incremental ListModel API](https://doc.qt.io/qt-6/qml-qtqml-models-listmodel.html).

Home's personal state is a mirror of the existing daemon `personal_json` store, not another database. `client/Personal.qml` hydrates before applying local mutations, serializes immediate saves, and ignores its own outstanding write echoes. Songs are recorded from genuine `now-playing` events; album/playlist controls capture their context before playback and record it after the command succeeds. Subscription snapshots restore the UI without recording another listen.

`client/lib/recommendations.js` builds a provider-scoped candidate pool from the existing Home response. A lightweight relevance score combines provider order, artist affinity, recency, shortcut affinity and recent-play fatigue; a maximum-marginal-relevance pass adds variety. Candidate count is capped at 300 and output at 12. The diversity pass caches each candidate's maximum similarity as picks are added, so its work is O(candidates × picks), with no training, new network endpoint or timer. Header/feed and personal-data changes trigger recomputation; additional card artwork uses the existing image pipeline.

We considered [implicit](https://github.com/benfred/implicit) (MIT) and [LightFM](https://github.com/lyst/lightfm) (Apache-2.0), but their trained models add machinery that one user's sparse local history does not justify. Neither is a dependency and no implementation was copied. The local diversity pass follows the standard [MMR relevance/diversity principle](https://www.elastic.co/search-labs/blog/maximum-marginal-relevance-diversify-results).

Search loads results incrementally in bounded pages, deduplicates them and preserves query, selection and scroll state when navigating back.

### Artwork

Large artwork paths reuse already available thumbnails, prepare higher-resolution images before swapping them into view, reject stale track requests, and keep the cache bounded. The goal is to avoid blank artwork and decode spikes during queue/Now Playing changes.

Device playlists carry no provider thumbnail, so the daemon derives one from their songs (`crates/ryotunesd/src/methods.rs`): the first four *distinct* non-empty song covers in playlist order. Four distinct covers become one thumbnail string — the literal `ryotunes-collage:` prefix followed by a JSON array of exactly four URLs; one to three collapse to the first cover; an empty playlist carries none. That marker is rendered only by the shared native `Artwork.qml` as a 2x2 grid — it is never a real URL, never sent to a provider, and download artwork resolution rejects it as an untrusted host. A user's custom cover overrides the automatic art, and clearing it returns the automatic art without contacting YouTube Music. The art is recomputed on every fetch, so add/remove/reorder shows up on the next `get_library`/`get_playlist`; a `library-changed` event refreshes cards, the page header and the sidebar live.

### Linux / Ryoku lifecycle

The Linux `ryotunes` executable is a native-only launcher. It requests `show` on the daemon socket, starting the user socket unit or the daemon directly for a cold launch. Connection or protocol failures never open the legacy Tauri player.

The primary window is a Quickshell surface (`org.quickshell`, title `Ryotunes`). Cold launch, second-instance launch, tray reopen and mini-player-to-main restoration converge on the daemon's show event and the client's visibility lifecycle. The package exposes one desktop entry and requires Quickshell.

### Packaging invariants

The Ryoku replacement package is intentionally conservative:

- never uninstall the `ryoku-desktop` package;
- preserve genuine stock entry points for rollback;
- expose one active `/usr/bin/ryotunes` route and one desktop launcher;
- migrate old custom Ryotunes generations only after the new generation is active;
- install/remove only the Ryotunes-managed Hyprland rule;
- verify binary ownership and active routes after installation.

### Release gates

`scripts/release-check.sh` runs source-level structural and regression checks. A distributable build must additionally pass Svelte/TypeScript semantic checking, locked Cargo fetch/test/check, the native Tauri release build and final pacman package ownership validation on an Arch/Ryoku build machine.
