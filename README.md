<picture>
  <source media="(max-width: 720px)" srcset="docs/assets/ryotune-omni-banner-mobile.svg">
  <img src="docs/assets/ryotune-omni-banner.svg" alt="Ryotune-Omni - Your music. New horizons. Experimental. Windows and Android ports planned.">
</picture>

<div align="center">

[![CI](https://github.com/Excentry/Ryotune-Omni/actions/workflows/ci.yml/badge.svg)](https://github.com/Excentry/Ryotune-Omni/actions/workflows/ci.yml)
[![Security audit](https://github.com/Excentry/Ryotune-Omni/actions/workflows/security.yml/badge.svg)](https://github.com/Excentry/Ryotune-Omni/actions/workflows/security.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/github/license/Excentry/Ryotune-Omni)](LICENSE)
[![Release: none verified](https://img.shields.io/badge/release-none%20verified-lightgrey)](docs/COMPATIBILITY.md)
[![Windows: planned](https://img.shields.io/badge/Windows-planned-yellow)](docs/COMPATIBILITY.md)
[![Android: planned](https://img.shields.io/badge/Android-planned-yellow)](docs/COMPATIBILITY.md)
[![Independent project](https://img.shields.io/badge/project-independent-blue)](docs/COMPATIBILITY.md)
[![Audio only](https://img.shields.io/badge/media-audio%20only-blueviolet)](docs/COMPATIBILITY.md)
[![Rust core](https://img.shields.io/badge/core-Rust-orange)](docs/DEVELOPMENT.md)
[![Playback engine: under validation](https://img.shields.io/badge/playback%20engine-under%20validation-lightgrey)](docs/COMPATIBILITY.md)

</div>

# Ryotune-Omni

> **Your music. New horizons.**

Ryotune-Omni is an independent, community-driven project based on [Ryotunes](https://github.com/Ryoku-dev/ryotunes), with the goal of bringing its desktop music-player experience to **Windows and Android**.

The aim is to preserve the original application's design, themes and music experience while adapting it to each platform wherever technically and legally possible.

> **Independent project:** Not affiliated with, endorsed by, or an official project of Ryotunes or Ryoku.

## Status

> **Experimental / work in progress.** Windows and Android support is planned, not ready for everyday use. No verified Ryotune-Omni installers or APKs are available.

| Platform | Availability |
|:--|:--|
| Windows | Planned; no validated release. Isolated development builds have been built and run; evidence in [docs/windows/CHANGELOG.md](docs/windows/CHANGELOG.md) |
| Android | Planned; no validated release |
| Linux | Inherited reference code in this repository, not Ryotune-Omni platform support; the original app is in the [Ryotunes repository](https://github.com/Ryoku-dev/ryotunes), maintained separately |

Existing Ryotunes version numbers and Linux packages do not indicate Windows or Android compatibility.

## Start here

| If you want to | Read |
|:--|:--|
| Understand status and limits | [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md) |
| Contribute or run checks | [CONTRIBUTING.md](CONTRIBUTING.md), [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) |
| Follow the Windows port | [docs/windows/ROADMAP.md](docs/windows/ROADMAP.md) for what is left, [docs/windows/CHANGELOG.md](docs/windows/CHANGELOG.md) for what was verified |
| Release policy and security | [docs/RELEASING.md](docs/RELEASING.md), [SECURITY.md](SECURITY.md) |
| Every document and its role | [docs/README.md](docs/README.md) |

Quick frontend check, on Windows or Linux (it does not build or validate the full app):

```sh
cd ui
pnpm install --frozen-lockfile
pnpm check
pnpm build
```

## What the app aims to offer

The original application includes the following features. Bringing them to Windows and Android is a goal, **not a claim of working support**:

- **Music discovery:** browsing and search across YouTube/YouTube Music, Spotify, SoundCloud and TIDAL integrations.
- **Your collection:** queues, playlists, favorites, local music, metadata and album artwork.
- **Listening:** native audio playback, synchronized lyrics and Internet Radio.
- **Personalization:** themes, skins, listening history and artwork-centered presentation.
- **Offline files:** downloads and local playback where supported and authorized.
- **Integrations:** media controls, Last.fm, Discord Rich Presence and optional Listen Together, where appropriate for each platform.

Ryotune-Omni is **audio-only**. Availability varies by provider and platform; see [compatibility and limitations](docs/COMPATIBILITY.md). Download only content you are authorized to save.

## Downloads

Windows and Android installation instructions will be published when usable builds have been validated. No executable or APK download is currently advertised here.

## Contributing

Help is welcome with Windows and Android development, audio integration, testing, documentation and accessibility. Small, reviewable contributions and clear reports of limitations are especially useful.

See [CONTRIBUTING.md](CONTRIBUTING.md) to get involved and the [developer guide](docs/DEVELOPMENT.md) for architecture and development references.

## Credits and license

An independently maintained modified version of [Ryotunes](https://github.com/Ryoku-dev/ryotunes), originally developed for [Ryoku](https://github.com/Ryoku-dev/ryoku), with earlier work from [LiMusic](https://github.com/SimoHypers/limusic) and [Sonora](https://github.com/sonorahq/sonora). Original authors retain credit for their work.

Licensed under **GPL-3.0-or-later**; see [LICENSE](LICENSE), [UPSTREAM.md](UPSTREAM.md) and [licensing and distribution](docs/LICENSING.md) for attribution, source obligations and third-party notices. Third-party components and trademarks retain their respective terms and ownership. Provided under the warranty and liability terms of its licenses.
