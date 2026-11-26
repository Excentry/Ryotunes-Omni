# Licensing and distribution

> **Before shipping:** This page is a practical summary, not legal advice or a compliance certification. Review the actual licenses and the exact artifacts being distributed.

### On this page

- [Project license and provenance](#project-license-and-provenance)
- [Distribution checklist](#what-distribution-requires)
- [Third-party components](#third-party-components)
- [Trademarks and services](#trademarks-and-third-party-services)

---

## Project license and provenance

The root [LICENSE](../LICENSE) contains the GNU GPL version 3 text. The workspace manifest explicitly declares **`GPL-3.0-or-later`**, and the verified Ryotunes upstream manifest and [UPSTREAM.md](../UPSTREAM.md) confirm that grant. Ryotune-Omni retains this licensing basis for the inherited application and derivative work; this guide does not relicense third-party components.

Ryotune-Omni includes and modifies code from the original [Ryotunes repository](https://github.com/Ryoku-dev/ryotunes). It is an independently maintained modified version, not a claim of original authorship; the original copyright notices, attribution and applicable license obligations remain in force.

Attribution must include the original Ryotunes/Ryoku contributors and the earlier **[LiMusic project by SimoHypers](https://github.com/SimoHypers/limusic)** described in `UPSTREAM.md`. Additional Spotify implementation provenance is recorded in [crates/spotify/README.md](../crates/spotify/README.md) and source headers: code was adapted from **[Sonora](https://github.com/sonorahq/sonora)**, historically `nolight132/sonora`, commit `695242e`, under GPL-3.0-or-later. Preserve those notices and the original authors' credits.

## What distribution requires

Use this checklist when preparing a source or binary release:

- Preserve copyright, attribution, licensing and warranty notices, and retain the required license text. Clearly identify modifications rather than presenting this modified version as the original work.
- Distribute covered derivative work under the applicable GPL terms, preserving recipients' rights to use, study, modify and redistribute it. Do not add incompatible restrictions.
- When conveying binaries, meet GPL section 6's corresponding-source requirements through an applicable compliant distribution method. Source must correspond to the shipped build and include required build/install scripts and modifications, not merely a link to unrelated upstream HEAD.
- Where GPLv3's User Product provisions apply, provide required Installation Information so recipients can install and run modified versions.
- Preserve required legal notices in interfaces and distributions, and satisfy each bundled dependency's own license requirements.

Upstream license texts must remain unmodified. `scripts/windows/collect-libmpv-sources.ps1` prepares mpv and FFmpeg notices from hash-verified source archives; the Windows bundle installs these notices and the project GPLv3 text as files under `licenses/`. See the [source and notice audit](windows/LIBMPV-SOURCE-AUDIT.md) for the exact available files and unresolved gaps. Installer license display alone is not the complete installed notice package.

## Third-party components

The initial review identified these distinct licensing considerations; this is **not a complete dependency/license inventory**:

| Component | License / review note |
|:--|:--|
| `aur/` packaging material | [aur/LICENSE](../aur/LICENSE) is BSD Zero Clause, with a 2026 ashmitvoid copyright notice; do not replace it with the application license. |
| Spotify code adapted from Sonora | GPL-3.0-or-later, with provenance above and in the crate's own documentation. |
| Pinned librespot source revision | MIT; its license retains Paul Lietar's copyright and notice requirements. The actual source revision is pinned in `Cargo.toml` / `Cargo.lock`. |
| `libmpv2` 4.1.0 and `libmpv2-sys` 4.0.1 | Registry metadata declares LGPL-2.1 for these Rust bindings; do not confuse the wrapper's license with the native engine's. |
| Native mpv/libmpv | Windows selects the GPL-enabled x64 libmpv candidate from run `41150745596`, DLL SHA-256 `6a6ac973…d13f56`. Its binary ZIP and corresponding-source archive are paired in the public [libmpv-6a6ac973-r2 asset release](https://github.com/Excentry/Ryotune-Omni/releases/tag/libmpv-6a6ac973-r2); `prepare-libmpv.ps1` pins the ZIP, DLL and import-library hashes. The source archive contains the pinned native inputs, MinGW-w64 `7213d719`, GCC 16.2.0 sources and crosstool-NG patch source. The GCC map has documented limits and is not a byte-reproducibility attestation or legal certification. This standalone library asset release is not an application release; review the [candidate validation](windows/LIBMPV-CANDIDATE-VALIDATION.md) and [license/source closure](windows/LIBMPV-LICENSE-CLOSURE.md) before distributing a combined application. Stable mpv `v0.41.0` has Windows player executables but no libmpv DLL asset. |
| FFmpeg | [Licensing depends on build configuration and enabled components](https://www.ffmpeg.org/legal.html); a bundled binary needs its own source/notice and compatibility review. |
| yt-dlp | Source is under the Unlicense, but [bundled release executables can have additional licensing obligations](https://github.com/yt-dlp/yt-dlp#licensing), including GPLv3+ code. Review the artifact, not just the source repository license. |

Before shipping, inventory the resolved Rust/frontend dependencies, native codecs/libraries, Qt/Quickshell if retained, fonts, icons and skins. Verify license compatibility, required notices and source/relinking obligations for the chosen binaries and distribution channel. No blanket legal guarantee is made for Windows installers or Android packages.

The separate [owned libmpv build candidate](windows/LIBMPV-OWNED-BUILD.md) fixes a smaller source closure and preserves its inputs before compilation. It is distinct from the selected `6a6ac973` DLL and is not approved by this asset release.

## Trademarks and third-party services

Software licensing does not grant ownership of music, provider accounts, service APIs, artwork or trademarks, and does not imply upstream endorsement. Third-party names and trademarks remain the property of their respective owners. Ryotune-Omni is not affiliated with, authorized by, sponsored by, or endorsed by YouTube or Google; YouTube and YouTube Music are trademarks of Google LLC. Provider terms and applicable law remain separate obligations.

Ryotune-Omni is not an official Ryotunes or Ryoku project. See the [README](../README.md) for the public introduction and the [developer guide](DEVELOPMENT.md) for development and release cautions.
