# Windows libmpv distribution notice

Ryotunes Omni's Windows x64 target selects the GPL-enabled native libmpv candidate from CI run `41150745596`:

- DLL SHA-256: `6a6ac973ecb495b6799b32d221fdf357f212f9cbd65bc01599670ed6b9d13f56`
- mpv source commit: `a43ab319fdb7aec31874999e5562c9975328cbda`
- FFmpeg source commit recorded in the build: `ca85613d10cd523dd05780f48dc6ecac142bd560`
- Toolchain image: `ghcr.io/btbn/ffmpeg-builds/win64-gpl@sha256:572cc7bc0974ebe2e5fd5f82c49e68c07942c6858098fd04f8daa23e3dc4dd2c`
- Binary ZIP and corresponding source archive: [libmpv-6a6ac973-r2](https://github.com/Excentry/Ryotune-Omni/releases/tag/libmpv-6a6ac973-r2)
- Source archive: [libmpv-corresponding-source-6a6ac973.tar.gz](https://github.com/Excentry/Ryotune-Omni/releases/download/libmpv-6a6ac973-r2/libmpv-corresponding-source-6a6ac973.tar.gz)

The binary release metadata identifies this combined build as GPLv3, including FFmpeg built with `--enable-gpl --enable-version3`. The project-level GPL-3.0-or-later license text is installed as `licenses/project/LICENSE`. The candidate package also preserves the upstream `LICENSE.GPL` and `LICENSE.LGPL` texts as notices; these multiple upstream texts do not change the selected binary's build configuration.

The source archive contains the pinned mpv, FFmpeg and native library inputs, the build recipe and notices, the complete MinGW-w64 source at `7213d719fe344553aa24fef7cc7dedee4940d594`, GCC 16.2.0 source, crosstool-NG source and its GCC patch set, and the reviewed GCC runtime source map. Its release checksum is listed in the release's `SHA256SUMS.txt`. Individual third-party notices and their source files are retained in the archive; the recipe's notice collection is a conservative superset, not a complete per-file legal certification.

The GCC map covers 101 linked GCC-family members, each mapped to a source file carrying the Runtime Library Exception text. The map is not an object-by-object rebuild: libstdc++ objects lack DWARF, patch 0004 changes headers that may have been compilation inputs, and the toolchain image was not reconstructed from the source archive alone. No exception incompatibility was confirmed. These limits are engineering evidence, not legal advice or a compliance certification.

This notice records a standalone DLL/source asset release. It does not by itself provide the corresponding application source, build/install scripts or any required Installation Information for a combined Ryotunes Omni application distribution. Those obligations and the application's release approval remain separate.
