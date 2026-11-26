# Windows libmpv 6a6ac973 distribution notice

This Windows x64 GPL-enabled libmpv DLL is the candidate confirmed as the official Windows libmpv build in Ryotunes-Omni issue #44. Its DLL SHA-256 is `6a6ac973ecb495b6799b32d221fdf357f212f9cbd65bc01599670ed6b9d13f56`; it was produced by Actions run `41150745596`. The DLL and corresponding-source archive are published together in the prerelease `libmpv-6a6ac973-r2`. This is a standalone native-library asset release, not an application release or approval to distribute a Ryotunes Omni application package.

The recipe builds GPL-enabled mpv and FFmpeg with GPL version 3 enabled, plus Rubber Band. The immutable source-input archive from the original run has SHA-256 `a9e613b351b8519ff0a0328e46913072032dbd3ac1e7f1482eeaae7ec7b1ea35`; it is retained unchanged as historical run evidence and does not by itself contain the complete MinGW source used by the pinned image. The published companion source archive contains a corrected source-input bundle with MinGW-w64 commit `7213d719fe344553aa24fef7cc7dedee4940d594`, the GCC 16.2.0 source archive, the crosstool-NG source and patch set at `b1a94f65c5187d99037a4e2f57aefbdf995a9677`, and the GCC runtime source map. Its exact SHA-256 is recorded in the same release's `SHA256SUMS.txt`.

The source companion README gives the extraction and preparation steps. The recipe uses the toolchain image pinned in `sources.lock.json`; it does not establish that the included GCC/crosstool-NG sources alone recreate that container or reproduce the DLL byte-for-byte.

Other compiled components include libplacebo, libass, FreeType, FriBidi, HarfBuzz and libxml2. Source snapshots of libplacebo's fixed submodules are also preserved, including code-generation inputs. Original upstream notices are copied under `licenses/`; original per-file grants remain in the source archives. Automated filename-based notice collection is not a complete per-file legal review. Preserve all applicable copyright, permission and warranty notices.

See `REVIEW.md` for the native input inventory, source-delivery record and remaining compiler/runtime confidence limits. Known embedded grants and full texts under upstream `LICENSES/` directories are also copied under `licenses/`. The GCC map covers 101 linked GCC-family members and records a source path with the Runtime Library Exception text for each; the objects were not rebuilt and compared, and patch 0004 changes headers that may have been compile inputs. No runtime-exception incompatibility was confirmed. Distribution of an application containing this DLL requires a separate combined-work review, corresponding application sources and any applicable installation information; the DLL source archive alone is not sufficient.

Original GCC GPLv3/Runtime Library Exception 3.1 and MinGW-w64/winpthreads notices are supplied under `licenses/toolchain/`, with revision URLs and hashes in `notice-inputs.lock.json`. These texts are also preserved in the source archive's recipe. They do not constitute a blanket exception for unrelated GPL components.

For this candidate, FreeType is used under the FreeType Project License, with its original `docs/FTL.TXT` preserved. Portions of this software are copyright © 2026 The FreeType Project (www.freetype.org). All rights reserved.

FFmpeg includes code derived from the Independent JPEG Group. This software is based in part on the work of the Independent JPEG Group. No modifications to those source files are made by this recipe.

LuaJIT, MuJS, libsixel, subrandr, DVD/Blu-ray integrations and optional mpv graphics backends are not enabled by this audio-focused recipe. Tauri/souvlaki, not this libmpv instance, owns application SMTC. Disabled functionality must still be checked against the application's runtime requirements before promotion.

The toolchain container is pinned for build tools only. Target dependencies are rebuilt into an isolated prefix; preinstalled BtbN target libraries must not be linked accidentally. Windows system libraries and compiler runtime exceptions require separate verification, as does the final PE dependency list.

The companion source asset is durably hosted next to the binary in the same GitHub release; expiring Actions artifacts are not used as the delivery method. The review records remaining technical confidence limits and does not constitute legal advice or a compliance certification.
