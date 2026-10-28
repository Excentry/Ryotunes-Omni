# Upstream and licensing

Ryotune-Omni is an independent, community-maintained modified work, licensed GPL-3.0-or-later. It is derived from Ryotunes, which is in turn derived from the open-source project originally published by SimoHypers as LiMusic. It is not affiliated with or endorsed by Ryotunes or Ryoku.

## Lineage

| Layer | Project | Role |
|:--|:--|:--|
| 1 | [LiMusic](https://github.com/SimoHypers/limusic) | The original Rust/Tauri/YouTube Music playback stack. |
| 2 | [Ryotunes](https://github.com/Ryoku-dev/ryotunes) | Its own product identity and UI, adapting and retaining parts of the LiMusic stack for Ryoku. |
| 3 | Ryotune-Omni (this repository) | Windows and Android port work, with its own application identity and release policy. |

The history of this repository begins with the import of the Ryotunes v2.2 source on 2026-08-27. Spotify code adapted from [Sonora](https://github.com/sonorahq/sonora) is described in [docs/LICENSING.md](docs/LICENSING.md) and in the crate's own documentation.

## Ryotunes layer

Ryotunes incorporates the functional upstream changes published after upstream v0.5.7 through v0.5.10: watch-history fallback/cache tracking, WebKitGTK dialog centering, Discord artist-status behavior, collaborative playlist handling, virtual-list scroll anchoring, artwork colour extraction, wheel volume control, settings-structure cleanup, and authenticated private-upload playback. Visual and theme changes are adapted to Ryoku rather than copied wholesale.

## Ryotune-Omni layer

Ryotune-Omni uses the application identifier `dev.excentry.ryotune-omni` and keeps its binaries, data directories and packages separate from Ryotunes, so that the two can coexist. Its Windows port work is documented in [docs/windows/ROADMAP.md](docs/windows/ROADMAP.md) and [docs/windows/CHANGELOG.md](docs/windows/CHANGELOG.md). Inherited version numbers are not Ryotune-Omni releases; see [docs/RELEASING.md](docs/RELEASING.md).

## License

The GPL license in `LICENSE` applies to this redistributed modified work. Do not remove required copyright or license notices from code you redistribute. See [docs/LICENSING.md](docs/LICENSING.md) for third-party components and distribution obligations.
