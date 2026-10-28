# Contributing to Ryotune-Omni

Ryotune-Omni is an independently maintained project based on Ryotunes, exploring Windows and Android support. Contributions are reviewed for behavior, platform integration and steady-state cost, not only whether they compile. Neither target currently has a validated release.

> **New to the project?** Start with the [developer guide](docs/DEVELOPMENT.md), check [feature compatibility](docs/COMPATIBILITY.md), then choose a small, testable contribution.

### Contribution areas

- **Portability:** dependency and license audits, backend boundaries, Windows hosting and packaging.
- **Android:** storage, permissions, media services, lifecycle, and audio integration.
- **Quality:** tests, accessibility, documentation, and reproducible compatibility reports.

## Start here

Read the [developer guide](docs/DEVELOPMENT.md) for architecture and build boundaries, [compatibility reference](docs/COMPATIBILITY.md) for feature status, and [licensing notes](docs/LICENSING.md) before distributing code or binaries.

Windows and Android build instructions are pending validation. The commands below describe inherited Linux/web checks, not a working Windows or Android setup. Linux prerequisites are documented in [docs/INSTALL-ARCH.md](docs/INSTALL-ARCH.md); the QML client also needs Quickshell and Ryoku modules.

### Frontend checks

```bash
cd ui
pnpm install --frozen-lockfile
pnpm check
pnpm build
```

### Native checks

Run from the repository root on Linux:

```bash
cargo fmt --all -- --check
cargo test --workspace --locked -- --skip mpv_keeps_the_gain_through_pitch_changes_and_failures
cargo check --workspace --locked
```

The skipped real-libmpv regression is environment-sensitive and needs separate validation; see `.github/workflows/release.yml` and `.github/workflows/libmpv-candidate.yml`. Report a timeout as unverified, not passed. For a focused Rust test, use `cargo test --locked -p <package-name> <test-filter>`.

### Run the primary Linux client

Start the daemon and client in separate terminals from the repository root:

```bash
cargo run --locked -p ryotunesd
```

```bash
qs -p client
```

QML changes require restarting the client; building `ui/` does not check QML. Tests are separate QtTest sources in `client/tests`, run with `qmltestrunner -input client/tests` in a suitable Qt environment. Do not run separate playback hosts against the same application-data directory.

## Product contracts

Preserve these inherited contracts where applicable, and document intentional platform differences:

- audio-only native playback through the backend/libmpv;
- playback and active downloads surviving closure of the visible client;
- background WebKit hibernation without stopping playback where the desktop WebView host is used;
- tray/background playback staying alive, with bounded idle exit when no work or client remains;
- explicit Quit fully stopping playback and unregistering media state;
- event-driven playback without a permanent high-frequency frontend transport timer;
- stable Home scrolling and incremental models, without rebuilding the entire model on pagination;
- independent `Ryotunes Mini` geometry;
- replacement packaging that never removes `ryoku-desktop`.

Linux-specific contracts such as MPRIS, Hyprland geometry and replacement packaging describe the reference implementation, not requirements to reproduce those APIs on Windows or Android. Keep playback/provider behavior behind the backend boundary, and let each platform supply appropriate lifecycle and media integration.

## UI and design conventions

- Preserve the original design language and skin semantics while adapting layout and input for each platform.
- On the inherited Linux UI, treat Ryoku live semantic tokens as the Follow System source of truth; do not create a competing palette layer.
- Artwork may be colourful; functional chrome stays restrained.
- Prefer one-pixel structure, compact geometry and existing UI primitives.
- Use existing iconography before introducing another library.
- Motion should communicate state, not run as ambient decoration.
- Honor reduced-motion settings and, on Linux, Ryoku motion settings.
- Do not add custom wheel physics.
- Keep network/provider logic behind the Rust command boundary.

See [docs/DESIGN.md](docs/DESIGN.md) for the visual language.

## Performance review

If a change touches playback UI, lyrics, artwork, Home, visualizers, WebViews or timers, include a quick steady-state check.

Check for:

- new intervals/timeouts that repeat forever;
- requestAnimationFrame loops;
- hidden WebKit work;
- repeated image decoding;
- unnecessary compositor animation;
- work that continues after the main WebView is hibernated.

## Pull requests

### Title format

Use this format for pull request titles:

```text
<type>(<scope>): <short description>
```

Use a scope when it clarifies the affected area; examples include `player`, `spotify`, `ui`, `windows`, `android`, `ci`, and `docs`. Keep the title concise, lowercase, and without a trailing period.

Common types:

| Type | Use for |
| --- | --- |
| `feat` | New user-facing behavior |
| `fix` | Bug fixes |
| `refactor` | Behavior-preserving restructuring |
| `perf` | Performance or resource-use improvements |
| `test` | Tests and test infrastructure |
| `docs` | Documentation |
| `build` | Build and packaging changes |
| `ci` | Continuous integration and automation |
| `chore` | Other maintenance |
| `revert` | Reverting a previous change |

For an incompatible API or behavior change, add `!` before the colon and explain migration requirements in the PR description, for example `feat(protocol)!: change playback event payload`. Do not use release titles for ordinary changes; releases follow the Ryotune-Omni release policy.

Examples:

- `feat(player): add device reconnect`
- `fix(spotify): handle Windows cancellation`
- `docs(portability): update compatibility status`
- `ci(windows): build selected Rust packages`

### Description

Keep each PR focused. Explain what changed and how it was validated; add screenshots for UI changes.

For large architectural changes, propose the scope and alternatives before implementation. Use the repository's available pull-request mechanism; use Issues or Discussions only if enabled. Do not assume an upstream community channel represents Ryotune-Omni.

> Provider and network features should fail softly. An external service going down must not make local playback or the application shell unusable.

## Bug reports and validation

Report the commit, OS/device, toolchain and relevant native-library versions, provider/account prerequisites, reproduction steps, expected and actual behavior, and checks actually run. Include screenshots or measurements where useful; remove credentials and personal data from logs.

Keep source inspection, successful compilation and runtime verification distinct. Record target-specific results and limitations in [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md); do not label planned features as working. Report Ryotune-Omni-specific port failures here rather than presenting them to upstream as confirmed upstream defects.

## Issue labels

Use the repository labels defined in [`.github/labels.yml`](.github/labels.yml): one `type:` label, the applicable `platform:` label, and a `priority:` or `status:` label when useful. Keep the label set small and focused on triage. The GitHub issue tracker must be enabled before contributors can open issues.

## Attribution and release safety

Preserve original copyright and license notices, identify modifications, and retain required third-party attribution. Read [docs/LICENSING.md](docs/LICENSING.md) for corresponding-source and distribution considerations; this documentation is not a compliance certification.

The upstream Ryotunes release workflow is archived and must not publish from this repository. Review [versioning and release policy](docs/DEVELOPMENT.md#versioning-and-releases) before creating an Omni tag or distributing artifacts.
