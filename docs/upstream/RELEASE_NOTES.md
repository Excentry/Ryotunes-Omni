# Ryotunes v2.4.1 — Security hardening for the v2.4 release

v2.4.1 keeps the Radio, signed-out device-playlist and custom Discord-presence features introduced
in v2.4, while tightening the native/WebKit trust boundary and the release supply chain. The native
libmpv playback lifecycle, background WebKit hibernation, MPRIS/tray behavior and event-driven idle
architecture remain unchanged.

## WebView and IPC hardening

- All Ryotunes application commands are registered with Tauri's runtime authority through an
  explicit `AppManifest`.
- One application permission, `allow-ui-commands`, is granted only to the bundled `main` and
  `mini` surfaces. The remote Google login page and hidden cipher/PoToken JavaScript runtimes
  cannot invoke Ryotunes application commands.
- The bundled surfaces no longer use broad `core:default`. They receive only the app/event APIs
  and explicit window/WebView operations the UI actually uses; renderer-side core image/path, tray,
  menu and resource defaults are not exposed.
- Main and mini no longer have direct file-dialog permission. Local-folder selection, playlist
  import/export and playlist artwork selection stay behind native Rust pickers.
- Renderer-writable settings, media parameters, external links, proxy URLs and Listen Together
  endpoints are validated again in native code.
- Authenticated proxy URLs are rejected, preventing proxy credentials from crossing into the
  renderer-visible settings object.
- Google sign-in top-level navigation is restricted to HTTPS Google/YouTube hosts.
- External links are opened as validated HTTP(S) arguments directly through the OS opener, never
  through shell interpolation.

## Local files and portable playlists

- A forged `LOCAL:` media id is rejected unless its exact path is present in Ryotunes' native
  scanned `local_tracks` database.
- Watched music folders are not recursively exposed through Tauri's asset protocol. Cover images are
  copied into Ryotunes-owned storage and only approved cover files are renderer-visible.
- Portable playlist import/export accepts YouTube Music tracks only, so local filesystem paths and
  live-radio records cannot leak into shareable JSON.

## Internet Radio hardening

- Radio playback accepts only an opaque station id from WebKit; the native backend resolves the
  actual Radio Browser record.
- Radio Browser mirror discovery accepts only official `*.api.radio-browser.info` hosts and every
  directory response is size-bounded before parsing.
- Radio stream URLs reject credentials, localhost, mDNS/single-label LAN names, literal private,
  loopback, link-local, metadata-service, multicast/reserved addresses and IPv4-mapped IPv6 forms.
- Persisted station records are re-normalized under the current URL policy before a restored queue
  can reach libmpv.
- Radio remains demand-driven: no startup fetch and no permanent polling loop.

## Listen Together relay hardening

- WebSocket message and frame sizes are bounded.
- Room count, participants, shared queue length, pending suggestions and each client's outbound
  queue are bounded.
- The relay still binds to localhost by default; public deployments should remain behind a TLS
  reverse proxy and use `wss://`.

## Dependency and release security

- Tauri is pinned above the affected origin-confusion range; the current lockfile resolves Tauri
  2.11.5.
- GitHub Dependabot tracks Rust, frontend and Actions dependencies.
- A scheduled Security audit runs RustSec and a production frontend dependency audit.
- Release invariants pin the application-command ACL, capability scope, settings boundary,
  radio/local-file guards, package identity and the existing lifecycle/performance invariants.

## v2.4 feature set retained

### Internet Radio
- Dedicated Radio surface under Discover using the community Radio Browser directory.
- Bounded popular/search pagination and explicit **Load more**.
- Native libmpv stream playback and best-effort Radio Browser click registration.
- Radio remains excluded from YouTube-only actions, lyrics, Last.fm, On Repeat and Listen Together.

### Device playlists
- **New playlist** works while signed out.
- Device playlists persist in SQLite independently of Google sign-in.
- **+ New playlist** / **Create + add** remove the old Add-to-playlist dead end.
- Device playlists support add/remove/reorder/rename/delete and local custom artwork.

### Discord Rich Presence
- The default activity title is **Ryotunes**.
- Users can set a local 2–128 character vanity title in Settings.
- Ryotunes' fixed Discord application/client identity does not change.

## Performance and lifecycle preserved

- Audio playback remains native through libmpv.
- Main WebKit hibernation during background playback remains intact.
- MPRIS, hardware media keys, Ryoku shell controls and tray playback remain native.
- Five-minute tray-only idle exit remains unchanged when nothing is playing.
- Explicit Quit still tears down playback, MPRIS and integrations immediately.
- No permanent high-frequency frontend timer was added.
- Home retains its stable non-virtualized section architecture and bounded artwork pipeline.
- Live Ryoku theme updates remain event-driven.

## Packaging

The v2.4 replacement package identity is **`ryotunes-v2.4 2.4.1-1`**.

It keeps `ryoku-desktop` installed, preserves the genuine stock Ryotunes entry points for rollback,
installs the custom binary under `/usr/lib/ryotunes-v2.4/ryotunes`, exposes one normal
`/usr/bin/ryotunes` route and desktop launcher, and installs the managed Ryoku window rule.
