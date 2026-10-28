# Developer guide

Technical reference and planning for Ryotune-Omni, an independently maintained project based on Ryotunes. For the public introduction, see the [README](../README.md). Repository paths in code spans are relative to the repository root.

> **Current status:** Windows and Android are planned targets, not supported releases. Source presence, build success, and runtime compatibility are separate milestones.

### In this guide

| Topic | Reference |
|:--|:--|
| Project baseline and platform status | [Technical baseline](#technical-baseline) · [Platform support](#platform-support) |
| Existing code and porting strategy | [Architecture and portability](#architecture-and-portability-strategy) |
| Feature status and constraints | [Compatibility reference](COMPATIBILITY.md) |
| Versioning policy and release cautions | [Versioning and releases](#versioning-and-releases) |
| Attribution and distribution | [Licensing guide](LICENSING.md) |
| How to help | [Contributing](../CONTRIBUTING.md) |

---

## Technical baseline

Windows and Android are planned targets, not supported releases. This guide separates source inspection from runtime validation and does not represent a completed portability or licensing audit.

The inspected repository contains the inherited Linux-first application, its Rust workspace, a primary Quickshell/QML client, and an older Svelte/Tauri client. Some Windows-specific code and a Tauri mobile entry-point annotation already exist, but neither establishes a functioning Ryotune-Omni port.

### What has and has not been validated

- No Ryotune-Omni releases were published on this project's GitHub repository.
- CI runs on pushes to `main` and on pull requests, on Windows, for the Svelte frontend and selected portable Rust crates. It does not compile the full desktop application or validate playback; no Android CI job exists.
- This documentation review did not itself build or run an application. Isolated Windows candidate builds have since been compiled and run (see the [Windows changelog](windows/CHANGELOG.md)); none is a release, and Android has no build.
- UI framework selection, target OS versions, Android ABIs, native-library packaging, and feature parity remain open engineering decisions.

Inherited version numbers still describe the source baseline, not a Ryotune-Omni release. Application identity is now isolated as `dev.excentry.ryotune-omni`; its Linux data/config/cache and package names are separate from Ryotunes. Existing profiles are not migrated automatically.

## Vision and principles

These are project goals, not completed achievements:

- **Feature parity:** Preserve desktop functionality where feasible and document unavoidable platform differences.
- **Code reuse:** Retain compatible Rust logic, provider integrations, data models, and persistence rather than rebuilding them unnecessarily.
- **Platform independence:** Put OS-specific behavior behind explicit interfaces instead of spreading platform assumptions through shared code.
- **Original identity:** Carry forward the application's visual language, themes, artwork-centered presentation, and listening experience, adapting interaction for desktop or mobile rather than creating an unrelated player.
- **Maintainability:** Track upstream improvements while keeping port-specific divergence small and understandable.
- **Transparency:** Publish limitations, incomplete features, technical decisions, and evidence behind compatibility claims.
- **Open-source collaboration:** Welcome community contributions while preserving upstream attribution and complying with applicable licenses.

## Platform support

| Platform | Existing reference | Ryotune-Omni status |
|:--|:--|:--|
| Linux, especially Ryoku / Arch-based desktops | Original [Ryotunes repository](https://github.com/Ryoku-dev/ryotunes): Quickshell/QML, Rust daemon, libmpv and Linux packaging; Void installation documentation is also inherited | Linux releases and maintenance belong to the original Ryotunes project; reference implementation for parity, not newly tested by this project's documentation review |
| Windows | Inherited Tauri launch path, Windows taskbar code, and media-control integration points | Planned port; no verified build, installer, minimum OS version, or parity guarantee |
| Android | Tauri v2 dependencies and a mobile entry-point annotation, but desktop-oriented hosting and dependencies | Planned port; no verified Android build, APK, background service, or supported device list |

The original application is **audio-only**. Ryotune-Omni does not announce video playback or additional provider capabilities.

## Architecture and portability strategy

> **Reading this section:** It describes the inherited implementation and likely porting work. Existing code is a starting point, not evidence that a target works.

### What is implemented in the inherited code

The primary Linux execution path is:

```text
ryotunes launcher -> ryotunesd -> shared Rust core -> player -> libmpv
                         ^
                         | newline-delimited JSON requests and events
                         v
                 Quickshell / QML client
```

The alternate Tauri host embeds the Svelte frontend and calls the shared core through Tauri commands. On Linux, the current launcher uses the daemon/native client only: a failed daemon connection does not fall back to Tauri.

| Area | Source and role | Portability assessment |
| --- | --- | --- |
| Primary UI | `client/shell.qml`, `App.qml`, pages and components; Qt Quick/QML hosted by Quickshell, with `Quickshell.Wayland` and `Ryoku.Ui.Singletons` imports | Not a standalone, platform-neutral Qt app. Quickshell, shell tokens, and window surfaces need adaptation or replacement; ordinary Qt platform support does not establish client portability. |
| Alternate UI/host | `ui/`: Svelte 5, SvelteKit, TypeScript, Tailwind and Vite, statically built into `ui/build`; `src-tauri/`: Tauri v2 host, commands and desktop integrations | A candidate for reuse, not a selected migration. Its feature coverage differs from the QML client, and desktop plugins/window behavior need target-specific review. |
| Shared backend | `crates/core`: `AppState`, orchestration, SQLite persistence, local library, lyrics, provider bridges and integrations; Tokio, Serde, reqwest/rustls, rusqlite and other dependencies | Candidate logic for reuse, not a certified portable crate. It still depends on playback, media controls, Discord integration, and Linux-oriented updater behavior. |
| Host interfaces | `crates/core/src/host.rs`: `EventSink`, `JsBridge`, `LoginFlow`, and host-supplied paths | Useful existing seams. Provider JavaScript execution, interactive authentication, and event delivery still need implementations on each target. |
| Playback | `crates/player`: `libmpv2` wrapper, dedicated event thread, audio-only playback, queue lookahead, disk cache and audio filters | Requires native libmpv and its dependency chain. Linkage, output devices, lifecycle, codecs and effects must be validated per target. Pitch shifting depends on the libmpv build's rubberband support. |
| Providers | `crates/innertube`, `spotify`, `soundcloud`, `tidal`, and core bridge modules | Request/model/parsing logic may be reusable. Authentication, native dependencies, audio transport, service restrictions, and live behavior require separate validation. |
| IPC | `crates/protocol` wire types; daemon `server.rs` / `methods.rs`; `crates/ryotunes-cli`; `client/Daemon.qml` | JSON contracts are reuse candidates. The current daemon and CLI use Unix sockets; daemon locking and runtime paths are OS-specific. Protocol path discovery also reads Linux `/proc`. |
| Downloads | Daemon-owned queue/history, external yt-dlp and FFmpeg, metadata/lyrics/artwork enrichment | Not simply portable backend code: process groups, signals, Unix priority handling, hard-link publication and desktop folder opening need review or replacement. |

### State, storage, and desktop integration

The Ryotune-Omni Linux daemon resolves data under `$XDG_DATA_HOME/dev.excentry.ryotune-omni` and audio cache under `$XDG_CACHE_HOME/dev.excentry.ryotune-omni/audio`, with home-directory defaults. Its application setup opens `ryotunes.sqlite`; download settings/history use `downloads.json`. The QML client separately stores UI preferences under `$XDG_CONFIG_HOME/ryotune-omni/client.json`. Tauri uses the same new identifier for its platform data/cache resolvers. The profile starts fresh; no existing Ryotunes data is copied.

The normal daemon socket is `$XDG_RUNTIME_DIR/ryotune-omni/daemon.sock`, with its single-instance lock beside it. Spotify audio currently passes from librespot into libmpv through a Unix FIFO; `crates/spotify/src/stream.rs` explicitly rejects this transport on non-Unix platforms. Authentication data, cookies, settings, local library paths, artwork and provider caches also need a target-specific storage and privacy review. Existing XDG/HOME assumptions, Unix permissions, filesystem URLs and external-process behavior must not be copied unchanged to Windows or Android.

Linux integrations include MPRIS/D-Bus media controls, StatusNotifierItem tray support, systemd user socket activation, Hyprland window rules, Ryoku live palette integration, and GTK/WebKitGTK authentication and JavaScript hosts. These are not portable application requirements. The daemon owns playback and active downloads independently of the visible client; equivalent lifecycle behavior is a porting goal, not a reason to reproduce systemd or a desktop daemon on mobile.

### Windows: planned approach

The existing Tauri path is worth evaluating before choosing a replacement UI. `src-tauri/src/main.rs` invokes the Tauri application on non-Linux targets, and `src-tauri/src/taskbar.rs` contains Windows thumbnail-toolbar integration. These are inherited implementation points, **not tested Windows support**. The Tauri setup now obtains the `main` HWND and passes a non-null opaque handle to the souvlaki owner thread; missing HWND disables that optional integration instead of panicking. Isolated Windows 11 runtime checks of the HWND and SMTC wiring are recorded in the [Windows changelog](windows/CHANGELOG.md); a clean-machine installer rehearsal and distribution validation remain pending. WebView2 itself is supplied by Tauri/Wry; this repo has no WebView2-specific API or integration harness.

Windows work must investigate:

- Building selected compatible Rust packages and native dependencies with a validated Windows toolchain rather than assuming the Unix-oriented workspace builds as a whole.
- Reusing/adapting the Svelte/Tauri interface, or another validated UI host, while preserving QML-era features, skins and design. Running the current Quickshell shell unchanged is not the plan.
- Retaining libmpv if a reproducible, license-reviewed Windows build can meet playback requirements; otherwise evaluating an audio abstraction and alternative engine without promising identical codec/effect behavior.
- Replacing Unix socket/locking and Spotify FIFO transport where needed. Windows named pipes or an in-process host are options requiring design and security review, not implemented decisions.
- Adapting system media controls, tray, launch behavior, authentication/browser callbacks and window management instead of carrying over MPRIS, systemd or Hyprland.
- Resolving Windows application-data/cache locations, Unicode paths, file URLs, permissions, local-library access and download publication safely.
- Packaging the app, libmpv/native libraries and any downloader dependencies; validating runtime discovery, updates, signing and redistribution obligations. If Tauri is retained, its Windows C++ build-tool and WebView2 requirements also apply.

### Android: planned approach

Desktop QML, Quickshell, the existing daemon, systemd activation, and Linux desktop APIs are not an Android application architecture. Tauri v2 supports Android at the framework level, but that does not make this application's native libraries or desktop plugins Android-compatible.

Options requiring validation include a mobile-adapted Svelte/Tauri frontend with native plugins, or an Android-native interface connected to compatible Rust components through JNI/FFI. Neither is selected. A libmpv-based Android engine versus an Android-native playback engine must be assessed against stream support, effects, lifecycle, resource use and licensing.

Android work must investigate:

- SDK/NDK, Rust targets, native ABIs, reproducible builds, and which backend components can actually compile and run.
- Touch navigation, responsive layouts and accessibility while preserving the original visual identity, artwork, themes and feature intent.
- A platform-appropriate playback service, media session, foreground-service behavior, notifications and lock-screen controls, including audio focus, headphone/disconnection events and interruption recovery.
- Activity/service lifecycles and background limits; keeping playback independent of the visible activity without assuming desktop process lifetimes or IPC. In-process bindings and Android service interfaces are candidates.
- App-private configuration/cache and protected credentials; shared music and downloads through appropriate MediaStore or Storage Access Framework access. Handle content URIs and request only permissions required by the selected access method and Android version, rather than treating every document as a normal path.
- Replacing desktop authentication assumptions and validating provider flows, playlists, lyrics, downloads and offline local playback individually.
- APK packaging and, if a distribution channel requires it, an Android App Bundle, with signing, native libraries, source availability and license notices reviewed before release.

Reference guidance: [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/), [Tauri v2 mobile support and plugin limitations](https://v2.tauri.app/blog/tauri-20/), [Android background playback](https://developer.android.com/media/media3/session/background-playback), and [Android document/storage access](https://developer.android.com/training/data-storage/shared/documents-files). These describe platform capabilities, not completed Ryotune-Omni work.

## Feature compatibility

The [compatibility reference](COMPATIBILITY.md) contains the feature matrix, provider restrictions, download behavior, and validation-reporting criteria. No Windows or Android feature is currently verified working.

## Versioning and releases

Ryotune-Omni uses its own release ledger, separate from Ryotunes. The inherited `1.2.3` manifests and `v1.*` changelog entries describe the source baseline; they are not Ryotune-Omni releases. The release process and tag-triggered validation are defined in [RELEASING.md](RELEASING.md).

- Use SemVer and tags in the form `vMAJOR.MINOR.PATCH`; while pre-1.0, releases stay on the `v0.*` line. Never reuse or recreate upstream tags.
- Reserve `v0.1.0` for the first validated Ryotune-Omni release; the current release workflow refuses to publish until a validated build target is enabled.
- Increment patch for compatible fixes and minor for compatible features while pre-1.0. Follow `docs/RELEASING.md` for prereleases and future 1.0 policy.
- Documentation-only changes stay under `Unreleased`; they do not create tags or releases.
- Create tags only through an explicitly reviewed Ryotune-Omni release. A tag must identify the exact source and artifacts for that release.

Application manifests and active Arch metadata use Ryotune-Omni identity. The self-updater checks this repository and will install only a package owned as `ryotune-omni`; it never updates upstream `ryotunes`. The release workflow validates synchronized versions and publishes nothing until a real, validated target is enabled. Do not enable a target until runtime support is validated.

### Windows and Android

> **Windows and Android:** Platform-specific setup, build, and installation instructions await successful port validation. There are no verified executables, installers, or APKs to recommend.

The existing Tauri configuration, mobile annotation, DLL ignore entries and Windows code are starting points for investigation, not installation instructions. No minimum Windows/Android version or supported CPU/ABI list has been established.

### Inherited Linux development reference

The existing build system is a Cargo workspace plus a separate pnpm project in `ui/`. CI uses Windows Server 2025, stable Rust, Node 22 and pnpm 11 to check the frontend, Rust formatting, and selected portable crates. The Spotify crate uses Rust edition 2024, so the older `rust-version` declared by the Tauri package alone is not a sufficient workspace toolchain guide. The release validator still runs on Ubuntu and Linux builds require native dependencies including GTK/WebKitGTK and libmpv; the QML runtime additionally requires Quickshell and Ryoku modules.

On an Arch-based Linux development machine, run `make run-omni` from the repository root to build the Svelte frontend and both Omni binaries, then launch the standalone app through the native Quickshell client without copying any binary path. Use `make build-omni` to build only. For options such as `--release` or `--skip-frontend`, call `scripts/dev/omni.sh` directly. The script does not install files, alter upstream Ryotunes, or migrate its profile. Running requires `qs`, the Ryoku QML runtime, and the native runtime dependencies in `packaging/arch/PKGBUILD`.

Use the following existing documents and executable configurations as **upstream-oriented references**, not verified Windows/Android instructions:

- [Arch/Ryoku setup and source builds](INSTALL-ARCH.md) and [Void installation](INSTALL-VOID.md).
- [Contributing](../CONTRIBUTING.md), [CI workflow](../.github/workflows/ci.yml), [Cargo workspace](../Cargo.toml), [frontend scripts](../ui/package.json), and [Arch dependency/build recipe](../packaging/arch/PKGBUILD).
- [Release checklist](RELEASE-CHECKLIST.md) and [release checks](../scripts/release-check.sh).
- [Architecture](ARCHITECTURE.md), [design language](DESIGN.md), [skins](SKINS.md), and [troubleshooting](TROUBLESHOOTING.md).

These documents contain historical Linux assumptions and some stale lifecycle descriptions. Consult current code when they conflict: the Linux launcher has no Tauri fallback, and the daemon's default idle grace is 60 seconds, subject to activity and subscribers. Do not run separate playback hosts against the same application data.

Existing test sources include Rust unit tests, Node-run `ui/src/lib/*.check.ts` regressions, and separate QtTest files in `client/tests`. Windows CI runs only selected portable Rust crate tests; it does not validate the inherited Linux daemon, libmpv playback, or QML client. The tag-release validator still runs Linux checks, and QML tests are not currently a CI job. Windows CI passing is not proof that the Windows application port works.

> **Release safety:** The upstream Ryotunes workflow is archived in `.github/legacy-workflows/ryotunes-release.yml`. The active Ryotune-Omni tag-triggered workflow validates releases but refuses to publish until at least one platform build target is implemented and validated. See [RELEASING.md](RELEASING.md).

## Licensing and attribution

See [licensing and distribution](LICENSING.md) for provenance, GPL obligations, third-party components and trademarks. Preserve the original license and copyright notices. A complete dependency and artifact review remains a release prerequisite.

## Relationship with upstream

- **Direct upstream: [Ryotunes](https://github.com/Ryoku-dev/ryotunes)** - the inherited Linux-first application and its implementation baseline.
- **Ryoku: [Ryoku-dev/ryoku](https://github.com/Ryoku-dev/ryoku)** - the original desktop/distribution environment and shell/design integration. Older `ryoku-arch` repository links redirect here.
- **Earlier origins:** LiMusic and Sonora, credited in the [licensing reference](LICENSING.md) and retained provenance documentation.

Ryotune-Omni is based on the original Ryotunes repository and maintained independently. These upstream projects remain authoritative for their own codebases, features, development and contribution guidelines. Their releases, branding and community channels do not represent Ryotune-Omni.

The goal is to follow relevant upstream fixes and improvements while diverging only where Windows/Android support or maintainability requires it. Port-specific decisions and parity gaps should be documented so upstream work can be integrated without misrepresenting compatibility.

## Contributing

See [CONTRIBUTING.md](../CONTRIBUTING.md) for contribution areas, verification expectations, product contracts and bug-report guidance. A successful compile alone does not establish playback or feature parity.

## Disclaimer and acknowledgments

Ryotune-Omni is experimental software, provided under the warranty and liability terms of its licenses. Port feasibility, feature parity, provider availability, release dates and distribution approval are not guaranteed.

Thanks to the Ryotunes, Ryoku, LiMusic and Sonora authors and contributors, and to the maintainers of Rust, Tauri, Svelte, Qt/Quickshell, mpv, librespot, FFmpeg, yt-dlp and the other projects on which this work depends. Their work remains credited to its original authors.
