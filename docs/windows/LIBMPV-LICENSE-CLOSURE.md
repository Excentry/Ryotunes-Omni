# Windows candidate license and source closure

## Outcome and scope

**This document's earlier review is scoped to the owned libmpv candidate from CI run
`39796468807` and isolated Candidate application installers. It does not clear
the different DLL selected by issue #44 (run `41150745596`, SHA-256
`6a6ac973…d13f56`).** The earlier user's GPL-enabled assumption is not a legal
certification or automatic clearance for the #44 source/binary pair. The #44
source and Windows validation status is tracked separately in
[LIBMPV-CANDIDATE-VALIDATION.md](LIBMPV-CANDIDATE-VALIDATION.md); source and
distribution gates remain open. This is an engineering record, not legal advice
or a compliance certification. Findings below do not by themselves apply to an
artifact that does not include the candidate they describe.
This is not a claim that Linux is a verified/supported Omni release target; the
repository's platform notes classify its Linux daemon/client as inherited
reference code. Any Linux release/support gates must be assessed separately.

The native DLL remains SHA-256
`a63580d610114bd039f79c72e0bd99b57c3276df77d98a0774830035ea5210c1`.
The application source snapshot includes pre-existing uncommitted application
changes. The recipe commit alone is not its complete source identity. Exact
installer and source hashes are recorded in
[LIBMPV-CANDIDATE-VALIDATION.md](LIBMPV-CANDIDATE-VALIDATION.md).

New evidence lives under
`.libmpv/owned/run-39796468807/license-closure/`, beside the frozen candidate and
the previously verified `delivery/` snapshot. Neither frozen directory is
modified to retroactively include these materials.

## Gaps addressed with preserved evidence

### Native library notices

`collect_native_notices.py` verifies the frozen candidate manifest and all 15
inner source archives against `sources.lock.json`. It preserved 6,834 complete,
unchanged source/notice files selected by notice filenames/directories or
copyright/grant markers in the first 65,536 bytes. These include embedded notices
that a root-license-only collection would miss.

This is a conservative notice/source superset, not a license classifier or a
per-object audit. Selection does not prove a file was compiled; absence from this
selection does not prove a file is notice-free. Original complete sources remain
in the native source-input archive. Notice symlinks, if encountered, are listed
rather than silently materialized or treated as absent grants.

The linked-archive record contains 63 MinGW runtime/winpthreads members: 10 from
`libmingw32.a`, 44 from `libmingwex.a` and 9 from `libpthread.a`. The recovered
MinGW source-cache archive has the same locked SHA-256 as the earlier source
review. Its generated `mingw-w64-crt/Makefile.in` and
`mingw-w64-libraries/winpthreads/Makefile.in` map **all 63** archive member names
to one exact source path each; all source hashes match the preserved source
candidates. See `toolchain-selected-source-mappings-v2.json` in the separate
supplement. This resolves the four basename collisions from the earlier
candidate list:

- `libmingwex` `misc.o` is `gdtoa/misc.c` (Lucent terms), not winpthreads `misc.c`.
- `libmingwex` `scalbn.o` is the x86 assembly implementation, not ARM C source.
- `libmingwex` `strtof.o` is `gdtoa/strtof.c` (Lucent terms), not the distinct
  Public Domain `stdio/strtof.c`.
- `libpthread` `misc.o` is `winpthreads/src/misc.c` (MIT notice plus the
  embedded Lockless-derived BSD conditions), not gdtoa.

No explicit Cephes/Moshier/Wine/LGPL marker occurs in these 63 selected source
files. That narrows, but does not settle, the runtime-notice issue: the archive
root still contains the upstream Cephes FIXME, Wine-derived LGPL headers may be
consumed as compile inputs, and this mapping does not enumerate preprocessor
headers, compile flags, or generated source. Do not extrapolate the 63-member
map to GCC's separately linked runtime archives.

The same linked-archive record has 101 GCC-family members: 8 `libgcc.a`, 2
`libgcc_eh.a` and 91 `libstdc++.a`. GCC release sources and crosstool-NG patches
are preserved, and the original Runtime Library Exception is present. The
effective GCC/crosstool-NG checkout and patch application were not pinned by the
image recipe; no exact source/patch map or per-file exception applicability has
been proven for those 101 members. This is a technical confidence gap, not a
confirmed exception failure or standalone release blocker. Continue using the
GPL-enabled candidate and retain this limitation for follow-up; do not describe
the mapping as complete.

### Rust package notices

The Windows-filtered dependency inventory has 482 packages: nine workspace
packages and 473 external packages. `collect_rust_notices.py` checks copied
metadata/notices against each vendored package's Cargo file checksums and
preserves named notices, license directories and explicit `license-file`
references. Thirty-three external packages have no root notice in their
vendored directory. This count is not a count of confirmed license violations:
some have embedded grants or a shared repository-level license.

Repository evidence is tied to crate-recorded revisions or the exact pinned Git
source, not current upstream HEAD. In particular, the librespot fork's pinned
source takes precedence over the older repository URL in its manifest.
Fourteen repository snapshots were preserved. Three additional lofty-family
snapshots exceeded the collector's size limit; their MIT and Apache root notices
were instead recovered individually at each crate-recorded revision. The failed
large-archive attempts remain recorded and are not presented as successful.
The original vendored crate sources are still available for all three packages.

`rust-repository-code-comparison.json` records a separate code-file comparison
for the 33 packages. Most match their referenced repository files. The
`rquickjs-core` package records `dirty: true`, but its 70 compared code files
match the recorded commit. That comparison does not certify its manifest or
every generated input. The rquickjs-sys repository archive omits its QuickJS
submodule contents; the vendored crate itself includes those sources and a
separate QuickJS license. Five WebView2 crate files have byte differences from
their referenced repository. A separate line-ending review verified that all
five become identical after CRLF-to-LF normalization. Original files and raw
hashes remain unchanged; this is not represented as byte identity.

The MPL-2.0 grants in selectors and priority-queue are embedded in source files;
their missing root LICENSE is not permission to discard those grants. The
priority-queue source explicitly offers LGPL-3.0-or-later or MPL-2.0. Original
source headers and a full MPL text, including the one supplied by the pinned
Symphonia repository, must remain available with the source package. Review
applicable secondary-license conditions before describing a GPL combination as
fully cleared.

### Frontend packages missing root notices

The original npm distributions for the six initially identified packages were
downloaded and checked against registry SHA-512 integrity metadata. Every file
in the preserved build-input snapshot matches its counterpart in those
distributions. The original tarballs and unmodified registry metadata are
retained separately. A later review also checked `saxes@6.0.0` against the
registry integrity in `ui/pnpm-lock.yaml`; its tarball has no LICENSE file, but
the exact `gitHead` source revision contains the ISC license text recorded in
the supplemental notice. `saxes` is a development-only transitive dependency
and is absent from the built static UI assets.

| Package | Evidence obtained | Remaining limitation |
| --- | --- | --- |
| `sirv@3.0.2` | Complete MIT notice at registry-recorded commit `1135207e92c40354543cbd15c763c7a61d79d432` | Build/server tooling; do not infer static-client inclusion solely from installation. |
| `@polka/url@1.0.0-next.29` | Complete MIT notice at `02cbdb529ddca0a9f3d225e2abb2931924219cc3` | Same inclusion distinction. |
| `@rolldown/binding-win32-x64-msvc@1.1.4` | MIT root notice and separately referenced `THIRD-PARTY-LICENSE` at `6cbd2330dc5ca973b90444973ee04c2dc7ee2f2d` | The root notice alone does not exhaust the native bundler's third-party terms. It is build tooling, not proof of runtime shipping. |
| `is-reference@3.0.3` | Exact package, author metadata, README MIT declaration and repository snapshot at `8bb053129bfabe2f6a7d7ed050159d67ebe82829`; exact commit tree has no LICENSE path | `pnpm why` places it under Svelte/development tooling, not a direct UI import; its name is absent from static output. Its package source is included in the separately delivered frontend package archive. No full notice found; do not invent a copyright year or substitute another project's notice. |
| `locate-character@3.0.0` | Exact package, author metadata, README MIT declaration and GitHub mirror snapshot at registry-recorded commit `4f08a59ec248121f7002abd02ee7b94e8eda06bc`; exact mirror tree has no LICENSE path | Same build-tooling/source-archive distinction as `is-reference`. Package metadata points to GitLab; the preserved GitHub mirror is identified explicitly. No full notice found; do not invent a copyright year or substitute another project's notice. |
| `@hugeicons/core-free-icons@4.2.3` | Exact original distribution, package MIT declaration, preserved original README and licensor's explicit free-package MIT statement | Actually imported by the UI. The original version lacks its advertised LICENSE; a later `4.3.5` notice is preserved as later-version evidence only, not relabeled as a `4.2.3` file. |
| `saxes@6.0.0` | Exact npm tarball integrity matches the lockfile; registry metadata declares ISC; source revision `211fa0ebec9b628affc09219199639887174bfc3` contains the ISC notice | Transitive dependency of development-only `jsdom`; absent from static UI output. Include its upstream ISC notice if the frontend package-source archive is distributed; it is not evidence of runtime code shipping. |

The Hugeicons distribution contains one repeated normalized path for
`dist/cjs/index.js`; both entries have identical bytes. The collector records
that duplicate and rejects conflicting duplicates. Two declaration files,
`dist/types/loader.node.d.ts` and `dist/types/loader.web.d.ts`, were absent from
the earlier installed-package snapshot. Both are preserved in the newly retained
original npm tarball. No existing snapshot, package version or binary was
rewritten to hide the discrepancy.

On the isolated frontend build, `pnpm why` places `is-reference` and
`locate-character` under Svelte and related development tooling; neither is a
direct UI import, and neither package name occurs in the built static assets.
It places `saxes` under development-only `jsdom` (also used by Vitest), and its
package name is absent from the built static assets.
This is evidence about the frontend bundle, not a full binary code-provenance
proof. Their exact package source trees are nonetheless included in
`frontend-package-inputs.tar.gz`, so a source package distributed with the
installer still needs a notice treatment. Hugeicons differs: the UI imports its
icons directly and its code is part of the static frontend.

The licensor's agreement explicitly excludes the free icons from Pro
restrictions. Neither Pro restrictions nor the Svelte wrapper's distinct notice
are substituted for the free core package's license. Explicit attribution and
notice treatment is prepared in
[LIBMPV-FRONTEND-SUPPLEMENTAL-NOTICES.md](LIBMPV-FRONTEND-SUPPLEMENTAL-NOTICES.md),
using original author attribution and clearly labeled standard MIT text, without
invented dates or claimed upstream copyright statements. Acceptance and actual
distribution integration of this treatment still require a recorded resolution
before release; preparing it does not manufacture a missing upstream grant.

The five Fontsource variable-font packages retain their original OFL notices in
the earlier frontend inventory. Preserve the fonts' own terms and any reserved
font names; the application GPL does not relicense the fonts. Presence in the
installed package set alone does not establish which fonts the executable embeds.

### Rust standard library and Microsoft WebView2 loader

Fifteen original Rust toolchain/library notice files were copied, including
`COPYRIGHT-library.html`, `COPYRIGHT.html` and the accompanying license texts.
The installed compiler reports revision
`4a4ef493e3a1488c6e321570238084b38948f6db`, matching the isolated build's
`target/.rustc_info.json`. The complete toolchain notice set is a superset, not a
claim that every listed compiler component is embedded in the application.

`webview2-com-sys@0.38.2/src/lib.rs` requests static linkage of
`WebView2LoaderStatic.lib` on MSVC. The vendored x64 loader matches byte-for-byte
the loader in Microsoft.Web.WebView2 SDK `1.0.3650.58`, the version recorded by
that crate's changelog. The original NuGet package, its nuspec, `LICENSE.txt` and
`NOTICE.txt` were preserved. The nuspec designates `LICENSE.txt`, whose text has
Microsoft's BSD-3-Clause-style redistribution, attribution, non-endorsement and
disclaimer conditions. This is separate from the Rust wrapper's MIT license.

This identifies an exact binary and its supplied terms; it is not proof that
the loader's preferred-form source is included, nor a blanket GPL System Library
classification. The SDK's broad third-party NOTICE must not be used as proof
that all its listed .NET/tooling components were linked. Any separately installed
WebView2 Runtime or runtime bootstrapper needs its own artifact/terms review.

The application-side `libmpv2-sys` build script emits `rustc-link-lib=mpv` without
requesting static linkage. Thus the Rust FFI/wrapper code is compiled into the
application, while the native libmpv remains a replaceable DLL. Review the
wrapper crate's LGPL-2.1 terms for the distributed application separately from
the candidate DLL's GPL-enabled license; neither the DLL nor its source archive
answers obligations for the Rust wrapper.

The application workspace declares `GPL-3.0-or-later` in the root Cargo
manifest; the candidate's own review records GPL-enabled mpv/FFmpeg with a GPLv3
combined-distribution baseline; and the exact vendored `libmpv2-sys@4.0.1`
declares LGPL-2.1 and contains its full license text. This review found no
confirmed term conflict or violation in that combination. Proceed with the
GPL-enabled build and keep its GPL/LGPL notices and applicable source materials
in the companion distribution; this is a working compliance assumption, not a
legal opinion.

For this review, the user's selected configuration is GPL-enabled, matching the
candidate `NOTICE.md` and `REVIEW.md`. Do not treat an LGPL-only candidate as an
alternative or continue investigating whether GPL was enabled. The remaining
question is the distribution treatment of this actual GPL-enabled DLL together
with the LGPL-2.1 wrapper in the application; that is a legal/distributor review,
not uncertainty about the candidate's build option. No incompatibility or
license violation has been confirmed in the evidence reviewed. Proceed on this
working assumption, preserve the applicable notices and source materials, and do
not present the assumption as legal certification.

## Corresponding-source reconstruction requirements

The previously assembled delivery contains the original native source-input
bundle, preferred-form application source snapshot, Rust vendor supplement,
compiler/runtime source inputs and frontend package snapshots. New notices and
source evidence are supplementary; they are not inside the already-built MSI or
NSIS packages.

For a reconstruction rehearsal, use a new isolated directory, not the regular
application's data or checkout:

1. Verify the original delivery manifest and the separate supplement manifest.
2. Extract `sources/application-source.tar.gz` into a new application root.
3. Overlay `sources/application-dependency-sources-verified.tar.gz` at that root
   so `vendor/` and `.cargo/config.toml` keep their recorded relative layout.
   Preserve the original application Cargo.lock and resolve any configuration
   collision explicitly; do not overwrite unrelated configuration blindly.
4. Copy the intact `native-candidate/` directory to `src-tauri/candidate/`.
   Its DLL, notices, native sources and import library are referenced by the
   experimental build configuration. Supply its import-library directory through
   the isolated build's recorded MSVC LIB search path.
5. Copy the original application source archive itself to
   `src-tauri/application-source.tar.gz`, as required by the experimental bundle
   resource map. Do not generate a different archive under the same name and
   claim it is the original installer input.
6. Preserve `ui/pnpm-lock.yaml`. The installed-package snapshot is not a pnpm
   offline store. Arrange and verify a lock-matching store before claiming a
   complete offline frontend rebuild; the six additional original npm tarballs
   alone are not such a store.
7. Use the recorded Rust/MSVC and native toolchain requirements. Keep the
   original native source/recipe archive paired with its DLL. A successful
   dependency-metadata resolution is not an application rebuild.

These are reconstruction requirements derived from the actual resource map, not
a claim that the full rehearsal has passed.

The frontend portion has now passed an isolated offline build rehearsal: its
`package.json`, `pnpm-lock.yaml`, Cargo lock and Tauri Windows config match the
corresponding bytes in the archived application source. `pnpm install
--offline --frozen-lockfile` reused all 95 locked packages from the existing
host pnpm store with zero downloads under pnpm `12.8.1`; `pnpm check` reported
zero errors/warnings and `pnpm build` completed with the static adapter. This is
not the project CI's pinned pnpm 11. That global pnpm store is not
contained in the frozen delivery or supplement. Thus the build proves the exact
locked frontend source can build offline in this host environment, not that a
recipient can reproduce it offline from the delivered files alone. See
`frontend-rebuild-verification.json` for input/output hashes. The native
executable, installer, source-hosting durability and license clearance remain
separate.

The isolated workspace contains all 834 regular files from the frozen
application-source archive. A corrected path-aware comparison found one
byte-level difference in generated `src-tauri/gen/schemas/desktop-schema.json`;
the parsed JSON values are equal, and four frontend/build input files were
individually SHA-256 matched. See `application-source-snapshot-comparison-v2.json`.
An earlier comparison report incorrectly removed each archive path's first
component and is superseded; its 819 reported differences were a comparison
script error, not missing source evidence.

## Windows candidate distribution review and next-release checklist

This checklist applies to a Windows installer or release artifact containing
this candidate DLL or the isolated Candidate application. It is not a blanket
gate for unrelated artifacts and does not certify another platform. The current
MSI and NSIS are frozen test candidates, not release installers.

No confirmed infringement or license incompatibility was identified. The user
has directed this review to proceed with GPL enabled as the intended candidate
configuration. License/source review is complete for proceeding on that basis;
the table records package contents and ordinary release assembly, not unresolved
confirmed violations. Exact archive-member mapping and clean-machine testing
remain assurance evidence, not independent legal requirements in every
distribution model.

| Topic | Finding / next step |
| --- | --- |
| Actual candidate payload | Both frozen installers contain the same 75 verified files: candidate DLL, 72 license/notice files, native source-input archive and application-source archive. They are test candidates; do not retrofit or relabel them. |
| Companion sources and notices for a future release | Use a companion corresponding-source asset attached to the same versioned GitHub Release as the Windows installer, with `SHA256SUMS.txt` and a direct release-notes link; do not rely on expiring Actions artifacts. Include the exact candidate `source-inputs.tar.gz` (all locked native sources and recipe), original native/toolchain notices and review, toolchain/compiler source inputs and applicable patches, application preferred-form source and build/install scripts, Rust dependency sources, frontend package inputs and notice supplement. These materials are preserved across candidate/review evidence but are not all in the frozen installers. This is the proposed delivery arrangement, not an upload or legal-compliance certification. The offline Cargo build now passes. |
| Frontend MIT notices | Proceed using the prepared distributor supplement as the working notice treatment for the three MIT-declared packages lacking a complete original notice. It preserves original metadata and does not invent years or claim to be upstream text. Include it in a future release's notice bundle; this is not a confirmed license violation. |
| GPL/LGPL distribution assumption | The app declares GPL-3.0-or-later, the candidate selects GPL-enabled mpv/FFmpeg with GPLv3 baseline, and vendored `libmpv2-sys@4.0.1` declares LGPL-2.1. No term conflict or violation is confirmed. Proceed with this combination and preserve GPL/LGPL notices and applicable source materials; this is a working assumption, not legal certification. |
| GCC runtime exception evidence | Exact MinGW mapping covers 63/63 members. The 101 GCC-family members still lack exact effective-patch/source mapping (15 unresolved names, 27 ambiguous). No exception failure is confirmed; this remains a technical confidence gap, not a reason by itself to block the Windows candidate. |
| Public distribution | No upload is authorized or performed. Before a public release, choose a durable location and pair the binary with applicable source and notice materials. |

Do not turn this record into a blanket compatibility assertion. Installer
installation/uninstallation and provider runtime tests remain separate product
validation gates, not substitutes for license/source closure. No upstream issue,
maintainer contact, public upload, dependency change, commit or release is made
by this review.
