# Releasing Ryotune-Omni

This document defines the release process for Ryotune-Omni, the independent
fork of [Ryotunes](https://github.com/Ryoku-dev/ryotunes) that brings it to
Windows and Android. Version numbers, tags and GitHub releases produced by this
process belong to Ryotune-Omni only. Upstream Ryotunes releases (`v1.x`, `v2.x`
tags, Arch packages named `ryotunes-*`) are a separate distribution and must
never be republished here as Ryotune-Omni releases.

Ryotune-Omni is in initial development: **no platform build of this fork has
been validated and no Ryotune-Omni release has been published yet.** The
infrastructure below is the validated path for when that changes.

## Versioning policy

- Semantic Versioning: `MAJOR.MINOR.PATCH`.
- While the fork is pre-1.0, all releases live on the `0.x.y` line. Feature
  work bumps `MINOR`, fixes bump `PATCH`.
- Pre-release identifiers (`alpha`, `beta`, `rc`) are appended as
  `-alpha.1`, `-rc.2`, … The workflow enforces the `v0.` line in
  `.github/workflows/release.yml` (the tag regex); when the fork declares
  1.0.0, that single constant is the only gate to widen.
- Git tags are prefixed with `v`, matching the repository's inherited
  convention: `v0.1.0`, `v0.1.0-alpha.1`.
- Tags are immutable. Never re-point, force-update or reuse a published tag;
  fix problems with a new `PATCH` or pre-release tag instead.

The application version lives in one source, the Cargo workspace
(`Cargo.toml`, `[workspace.package].version`), and is mirrored by
`scripts/sync-version.sh` into `Cargo.lock`, `src-tauri/tauri.conf.json`,
`ui/package.json`, `client/version`, `packaging/arch/PKGBUILD` (hyphens
stripped, since `pkgver` forbids them) and
`packaging/linux/dev.excentry.ryotune-omni.metainfo.xml`. `scripts/release-check.sh`
and `scripts/check-release-invariants.py` fail any tree where these disagree.
A release tag must equal the workspace version of the tagged commit; the
release workflow verifies this.

Identity checks enforce Tauri/AppStream ID `dev.excentry.ryotune-omni` and
Arch package `ryotune-omni`. Package metadata has no `epoch`, `replaces`, or
`conflicts` relation with upstream `ryotunes`; release assets use the Omni
package name. Identity migration does not create a release or change version.

To prepare a version bump:

```bash
scripts/sync-version.sh 0.1.0 --release
```

This also rolls the `## Unreleased` section of `CHANGELOG.md` into
`## v0.1.0 - <date>`. Commit the result before tagging.

## Workflow architecture

Validation, artifact production and publishing are separate jobs; publishing
is impossible unless every earlier stage succeeded.

### CI (`.github/workflows/ci.yml`)

Runs on pull requests and pushes to `main` with `contents: read` only, on a
Windows Server 2025 runner. It checks the Svelte frontend, Rust formatting, and
tests for the currently portable crates (`innertube`, `listen-protocol`,
`ryotunes-protocol`, `ryotunes-soundcloud`, `ryotunes-tidal`, and `sync-server`).
It does not compile the full app, daemon, player, or Windows Tauri host, and is
not evidence of a working Windows port. Linux-specific playback, daemon, and
release-guard checks are not part of daily CI. CI never publishes anything and
must stay that way.

### Release (`.github/workflows/release.yml`)

Triggered by pushing a `v*` tag, or manually via `workflow_dispatch` with an
explicit existing `tag` input (for validating or re-publishing). Jobs:

1. **preflight** — validates the tag against the `v0.` SemVer pattern,
   requires the tag to resolve to a commit reachable from the default branch
   (release history integrity), and requires the tagged tree's workspace
   version to equal the tag. It records the resolved commit SHA for downstream
   jobs, and release publishing fails if the tag no longer points to that
   validated commit. It then determines the enabled release-target matrix.
   With zero enabled targets (the current state):
   - a tag push **fails** with a clear error, before anything is published;
   - a manual dispatch runs the documented **non-publishing validation
     path**: everything is checked, nothing is released.
2. **validate** — runs the Linux release-validation suite against the tagged
   tree. This remains separate from daily Windows CI while the release
   packaging and platform target are not yet ported.
3. **build** — one job per enabled target from the matrix in `preflight`. The
   matrix is currently empty; a target may only be added together with its
   real, verified build steps. A target added without steps fails loudly (the
   job is a deliberate tripwire, not a placeholder artifact).
4. **publish** — requires `contents: write` only here; requires all previous
   jobs; refuses to publish an empty artifact set; verifies artifacts are
   non-empty; generates `SHA256SUMS.txt`; attaches every regular file in
   `dist/` except `NOTES.md`; and verifies the release asset list matches.
   It also attaches the reproducible
   `git archive` source tarball and its SHA-256 sidecar; generates notes via
   `scripts/omni-release-notes.py`; creates or updates the GitHub Release for
   the exact tag. While the `v0.` line is in force every release is marked
   **pre-release**, so it never becomes "latest".

A tag never produces a release automatically: without a passing `validate`
and at least one passing `build`, `publish` cannot run.

`workflow_dispatch` does not bypass any check: the same preflight, validate
and target rules apply; it only changes the preflight treatment of the
"no enabled target" case, from hard failure to explicit non-publishing
validation.

### Manual trigger — when to use it

- After a tag-push failed at the target gate, to run the non-publishing
  validation path and prove the tagged tree is sound.
- Once real targets exist, to re-publish or repair a release for an existing
  tag after a partial failure (artifacts are re-uploaded with `--clobber`).

## Platform readiness

| Platform | State | What is missing before it can be a release target |
|---|---|---|
| Linux (Ryoku/Arch, x86_64) | Inherited from upstream; buildable, **not validated for this fork** | Deliberate validation of the fork's packaging and identity, and a decision on the artifact name/identity (the inherited packaging ships upstream's `ryotunes-*` identity) |
| Windows (x86_64) | Not implemented | A verified Windows build of this tree (the Tauri host has Windows-only taskbar/media code inherited from upstream, but no Windows build has ever been produced or tested), runtime dependency list (WebView2, libmpv), architecture decision, startup + playback smoke validation, packaging format decision (ZIP/portable/installer — not chosen prematurely), checksums |
| Android | Not implemented | An actual Android port (no `gen/android` project exists), APK-first packaging, package identity and version metadata, correct signing configuration with keys held as GitHub secrets only (never committed), install/startup/playback/lifecycle validation. Add AAB only if a store-distribution process is adopted |

Enabling a platform = adding its job to the `build` matrix and steps in
`.github/workflows/release.yml`, after the row above is checked off. Do not
enable a target speculatively: an unbuildable or unverified target must not
produce a release.

### Future Android signing requirements

- Keystores, keystore passwords and key passwords live only in GitHub
  Actions secrets (or an external secret store), never in the repository.
- Production-signed packages are produced only from the release workflow on a
  validated tag; no signed package is published from pull-request context.
- Debug-only signing is acceptable for early direct-install APKs, clearly
  labelled as such, until release signing exists.

## Artifact integrity

- Every published artifact is non-empty; the workflow fails otherwise.
- `SHA256SUMS.txt` covers the validated build artifacts; the source tarball
  carries its own `.sha256` sidecar. `sha256sum -c` verifies both.
- Build jobs must upload their artifacts under the name pattern
  `release-assets-<target>`; the publish job consumes exactly that pattern and
  refuses an empty set.
- Names are deterministic and versioned: `ryotune-omni-<version>.tar.gz` for
  sources; per-target names must embed version and architecture when targets
  land.
- The source tarball is `git archive` of the exact tag: reproducible bytes.
- Workflow permissions are least-privilege: CI is `contents: read`; only the
  publish job holds `contents: write`; the default workflow token setting in
  this repository is already read-only.

## GitHub Releases vs Actions artifacts

- **Actions artifacts** are workflow-associated, short-lived outputs for CI
  and testing (retention is bounded by repository settings). They are not
  releases.
- **GitHub Releases** are the versioned public distribution points: a tag,
  notes, and verified assets. A green CI run is not a release; only the
  `release.yml` publish job creates one, only from a `v0.*` tag, only with a
  passing validation chain and non-empty verified artifacts.

## Release notes and changelog

`CHANGELOG.md` (inherited format, `## vX.Y.Z - date` sections under
`## Unreleased`) is the authored source for notes; roll it with
`scripts/sync-version.sh <version> --release`. At publish time,
`scripts/omni-release-notes.py` combines: the changelog section for the tag,
commits since the previous `v0.*` tag grouped into new features /
improvements / bug fixes / breaking changes, current platform availability,
known limitations, install and upgrade guidance, and upstream attribution.
Notes must never present unimplemented platforms or features as done, and
upstream history is credited to upstream, not claimed as fork work.

## Branch and tag protection

Configurable only in GitHub repository settings (not through repository
files); the maintainer should enable:

- Protect `main` with a branch ruleset: require pull requests, one approval
  from the other maintainer, approval of the most recent push, and the CI
  status check. Do not allow force pushes or branch deletion. In a two-person
  team, the PR author cannot provide the required review; if the other
  maintainer is unavailable, defer the merge rather than bypassing protection.
- Require the CI check after it has completed successfully at least once, and
  select the exact check name shown by GitHub for `.github/workflows/ci.yml`.
  Revisit the required-check list when CI jobs are renamed or split. Do not
  require Windows runtime or Linux desktop checks until corresponding runners
  actually execute them.
- Protect `v*` tags with a tag ruleset: limit tag creation to maintainers and
  prohibit update/deletion. Create release tags only from reviewed commits on
  `main`; the release workflow separately rejects tags outside default-branch
  history.
- Keep release publishing maintainer-only. The workflow token remains
  read-only by default; only the `publish` job receives `contents: write`.
  Restrict who can run `workflow_dispatch` through repository access, and
  require approval for first-time contributors' workflow runs.
- Do not configure a production deployment environment or enable a release
  target until that platform has a reproducible validated build. If a protected
  release environment is introduced, restrict deployment branches to `v*`
  tags and require approval from the other maintainer.

These settings are not changed by this repository. The maintainer must review
and apply them in GitHub repository settings; record the ruleset/branch
protection name and date here or in the operations log once verified. Until
then, treat them as recommendations, not active controls.

## Scripts map

| Script | Role |
|---|---|
| `scripts/sync-version.sh` | Single writer of every version field; rolls the changelog with `--release` |
| `scripts/release-check.sh` | Release guard: version identity, secret scan, structural invariants, JSON/TOML parse, skins, TS checks |
| `scripts/check-release-invariants.py` | Deep release invariants: version consistency, ACL/capability policy, Tauri security floor |
| `scripts/omni-release-notes.py` | Ryotune-Omni release notes from changelog + `v0.*` tag history |
| `scripts/make-source-release.sh` | Local reproducible source tarball + SHA-256 sidecar |
| `scripts/release-notes.py`, `scripts/next-version.sh`, `scripts/release.sh`, `scripts/build-arch-package.sh`, `scripts/build-replacement-package.sh` | Upstream Ryotunes tooling, retained unchanged for upstream parity; **not** part of the Ryotune-Omni release flow (the upstream workflow that used them was replaced by the tag-triggered `release.yml`) |

## Licensing

All releases distribute under the inherited `GPL-3.0-or-later` license
(`LICENSE`, `UPSTREAM.md`). Source archives include the full license and
attribution files by construction (`git archive` of the whole tree).
Binary artifacts, when targets exist, must retain required copyright and
license notices. Never redistribute upstream Ryotunes binaries as
Ryotune-Omni releases.

## Checklist for the first real Ryotune-Omni release

1. A platform port exists with a verified, documented build (see the table).
2. CI is green on `main`; required status checks and branch/tag protection
   are configured in GitHub settings.
3. `scripts/sync-version.sh 0.1.0(-alpha.N) --release`; changelog written;
   commit merged.
4. Enable exactly the validated target(s) in `release.yml`.
5. Tag that commit `v<version>` and push the tag.
6. Watch preflight → validate → build → publish; verify the release page:
   notes, assets, `SHA256SUMS.txt` verified, pre-release flag set.
