# Security and privacy

> **Scope and status.** Ryotune-Omni targets Windows and Android; neither has a verified release. This document covers the Tauri host, which is the Windows target, and the inherited Linux host. The IPC design below was inherited from Ryotunes and was not fully re-audited on Windows. The Windows-specific section separates what comes from source, from isolated development runs and what is still open. The dedicated Windows review is W74 in the [Windows roadmap](docs/windows/ROADMAP.md).

The Tauri host renders the UI in the operating system WebView (WebView2 on Windows), while playback, account state, filesystem access and network integrations live in Rust. Treat both sides as part of the security boundary. The inherited Linux host instead runs a daemon with a Quickshell client.

## Tauri host: WebView and IPC design

- The normal `main` and `mini` surfaces load the bundled Ryotune-Omni frontend, not a remote web app.
- All 97 application commands are registered with Tauri's runtime authority through an explicit
  `AppManifest` and one `allow-ui-commands` permission granted only to the `main` and `mini`
  surfaces. The remote Google login page and hidden cipher/PoToken JavaScript runtimes therefore
  cannot invoke Ryotune-Omni application commands even though they exist inside the same process.
- The Google sign-in WebView has a separate `login` label and is not included in the main/mini
  capability files.
- The bundled surfaces do not use `core:default`; they receive only the Tauri app/event APIs and
  explicit window/WebView operations Ryotune-Omni actually uses. In particular, renderer-side core image/path,
  tray, menu and resource defaults are not exposed.
- Neither bundled renderer has file-dialog permission. Playlist import/export, artwork selection
  and local-folder selection open native pickers from Rust commands, so the WebView cannot silently
  choose arbitrary filesystem paths.
- Portable playlist files accept YouTube Music track metadata only. Local-file identifiers contain
  filesystem paths for native local playback, so they are deliberately refused by portable
  export/import rather than leaking machine paths into a shareable JSON file.
- Renderer-writable settings, external URLs, Listen Together endpoints and media parameters are
  validated again in Rust. The frontend is never treated as an authorization boundary.
- Authenticated proxy URLs are rejected, and an invalid legacy proxy setting is discarded before
  the networking stack starts, so proxy credentials are not returned through renderer-visible
  settings.
- Internet Radio playback accepts only an opaque station id from the WebView. Native code resolves the
  cached/Radio Browser station record and rejects literal localhost/private/link-local stream
  addresses rather than accepting a renderer-supplied URL.
- Radio Browser discovery accepts only official `*.api.radio-browser.info` mirrors and bounds
  each directory response before parsing it.
- Persisted radio-station records are re-normalized through the same URL policy when a queue is
  restored, so records written by an older release cannot bypass current stream validation.
- The Google sign-in WebView can navigate only to HTTPS Google/YouTube hosts and has no main/mini
  capability set.
- External links are opened by passing a validated HTTP(S) URL directly to the OS opener. They are
  never interpolated into a shell command.
- The Tauri asset protocol has an empty static scope, plus a runtime scope for the owned covers directory and individual cover files. Local cover art is copied into Ryotune-Omni-owned
  storage; watched music directories are not recursively exposed to the renderer.
- Local track ids contain a native path for offline playback, but Rust verifies that exact path is
  present in the native-scanned `local_tracks` database before it can be handed to mpv. A forged
  `LOCAL:` id from the WebView therefore cannot open an arbitrary file.
- Account cookies, delegated YouTube identity values, visitor data, queue internals and stream URLs
  are deliberately excluded from the settings IPC API.
- The optional Listen Together relay bounds WebSocket frame/message sizes, room count, queue length,
  pending suggestions and each client's outbound queue. It binds to localhost by default; use a TLS
  reverse proxy when deliberately exposing it as a public `wss://` endpoint.

### CSP note

The global Tauri CSP remains unset because the cipher and PoToken extraction stack uses isolated,
hidden `data:` WebViews that require inline/dynamic JavaScript. Tauri injects a configured global
CSP into those data documents as well, which breaks the extraction harness. Those hidden WebViews are not granted the main/mini capability set or the application-command
permission, so their dynamic JavaScript cannot cross into Ryotune-Omni commands. A future move of the harness to a dedicated custom
protocol can allow a strict per-application CSP without weakening playback.

That harness runs YouTube's `player.js` and BotGuard JavaScript, which is third-party remote code, inside the hidden views.

## Windows host

Statements marked *source* come from reading the code; *observed* comes from isolated development runs recorded in the [Windows changelog](docs/windows/CHANGELOG.md) and the [W25 record](docs/windows/ROADMAP.md#w25--ejecutable-aislado-y-dependencias).

### System WebView

- The application does not bundle a browser engine. WebView2 is the Evergreen runtime supplied by the system and updated outside the application (*source*: no engine is packaged). How the installer behaves when the runtime is missing has not been verified, and no roadmap item covers it; W80 covers only the effective installer configuration.
- `Cargo.lock` pins `tauri` 2.11.5, while the manifest requirement is `2`, so nothing enforces a minimum version. An inherited note says releases from 2.11.1 contain the fix for GHSA-7gmj-67g7-phm9 (origin confusion allowing some remote pages to be mistaken for trusted local origins); that was not re-verified.

### Sign-in window

- *Source*: the sign-in window has its own `login` label and is not in the `main` or `mini` capability files, so the remote Google page cannot invoke application commands.
- *Source*: it navigates only to `https` hosts under `google.com` or `youtube.com`. A rejected navigation closes the window; the host retries once and logs only the scheme and host of the rejected URL. Google's regional sign-in hosts, such as `accounts.google.com.co`, are currently rejected ([issue 25](https://github.com/Excentry/Ryotunes-Omni/issues/25)).
- *Source*: only cookies whose domain is `youtube.com` or a subdomain are read back; `google.com` cookies are dropped. The whole sign-in has a five-minute deadline and failures return a recoverable error.
- *Source*: the window uses a persistent WebView2 profile on purpose, so the Google session survives between sign-ins. That profile is sensitive account data.
- *Unverified*: the sign-in window most likely shares the default WebView2 user-data folder with the main UI, because no data directory is set; and popups and iframes inside it are not specifically handled, since `on_navigation` governs top-level navigations.
- *Observed*: after a successful sign-in the harvested cookies are applied by the core and the window closes by itself.

### Local data and files

- *Source*: the 0700/0600 modes for the data directory, caches and SQLite database are set only under `cfg(unix)` in `src-tauri/src/lib.rs`. On Windows the application sets no explicit ACLs; the directories inherit those of their parent profile folder. This is open (W33).
- *Source*: the session cookie is stored in plain text in the `settings` table of the SQLite database (`session_cookie`), so the database file is the sensitive artifact.
- *Observed*: the SQLite database and the cipher cache live under `%APPDATA%\<identifier>`, and the audio cache and the WebView2 profile under `%LOCALAPPDATA%\<identifier>`. Both folders can contain session material.
- *Observed*: `libmpv-2.dll` is loaded from the executable's own directory. DLL search-order hardening and the ACLs of the install directory have not been reviewed (W74).

### Local debugging surface

- *Observed*: WebView2 honored `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port>` in the environment that launched the application. The DevTools protocol then exposed the DOM and `window.__TAURI_INTERNALS__.invoke` of the `main` surface to any local process, which could call application commands. This needs control of the launch environment, so it is a local-attacker concern rather than a remote one. Whether the application should clear or reject this variable is open (W74).

### Diagnostics and logs

- *Source*: release builds use the Windows GUI subsystem (`windows_subsystem = "windows"`), so there is no console and output is lost unless the process is started with redirected handles.
- *Observed*: logs go to stdout and stderr only; the default level is quiet and `RUST_LOG=info` adds two informational lines at startup. In one clean run, a scan of that output for cookie, token, `SAPISID`, bearer, password, signed-URL and email patterns found no matches. That is one run, not a guarantee.
- There is no log file, rotation or export on Windows, and `scripts/diagnostics.sh` is a Linux shell script. Privacy-safe diagnostics are W44.

### Other surfaces

- *Source*: `tauri-plugin-autostart` is registered, so the application can create a Windows startup entry; its permissions are not granted to the renderers.
- *Source*: the Last.fm API key and secret are compiled in from a gitignored `lastfm.keys` at build time (`src-tauri/build.rs`), so they are recoverable from a shipped binary if they were present at build.

### Open items

Per-user data ACLs (W33), the diagnostics export (W44), the installer's WebView2 handling (no roadmap item yet; related to W80), updater, signing and rollback (W84, W75) and the review of DLL search order, binary origins and secrets (W74). There is no Windows updater: `check_for_updates` still contacts the GitHub API, and `install_update` is refused outside Linux packages (pacman or xbps).

## Linux host (inherited reference)

### Keep the system WebView current

The Linux host does **not** bundle its own WebKit engine. On Arch/CachyOS/Ryoku, WebKitGTK is supplied and
patched by the system package manager. Keep the machine fully updated with `pacman -Syu`.

At the time of the v2.4.1 hardening pass (September 2026), WebKitGTK **2.52.6** is the stable
security baseline. WebKitGTK advisory WSA-2026-0005 lists vulnerabilities affecting releases before
2.52.6. `./scripts/diagnostics.sh` reports the installed WebKitGTK version without exposing private
paths or account information.

### Local secrets and files

Session state is stored in the Tauri application-data directory. On Unix, Ryotune-Omni sets its app data
and cache directories to mode `0700` and its SQLite state database to `0600`.

Do **not** attach the application-data directory, SQLite database, browser session data, or raw
application logs to public bug reports. They may contain account/session material or local media
paths.

For support, prefer:

```sh
./scripts/diagnostics.sh
```

The diagnostics script intentionally excludes account details, cookies, tokens, local media paths,
hostnames and configured network endpoints.

## Dependency monitoring

GitHub Dependabot monitors Rust, frontend and GitHub Actions dependencies weekly. The
`Security audit` workflow also runs RustSec (`cargo audit`) and a production frontend dependency
audit (`pnpm audit`) on the hardening branch/main and on a weekly schedule.

## Reporting a vulnerability

If you discover a bug that exposes credentials or session data, do not post the secret publicly.
Revoke or sign out the affected session first, then report the issue with sanitized reproduction
steps through GitHub's private vulnerability-reporting channel when available.
