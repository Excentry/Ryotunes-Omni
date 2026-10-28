# Documentation index

Ryotune-Omni targets Windows and Android; neither has a verified build or release. This page says what each document is for, whether it describes Ryotune-Omni or the inherited Ryotunes Linux client, and which language it uses.

## Where things go

| Information | Document |
| --- | --- |
| Work still to do on the Windows port: tasks, owners, acceptance, open decisions | [windows/ROADMAP.md](windows/ROADMAP.md) |
| Verification evidence, results and findings already obtained | [windows/CHANGELOG.md](windows/CHANGELOG.md) |
| Release notes of the inherited Ryotunes product (not the Windows port) | [../CHANGELOG.md](../CHANGELOG.md), [upstream/RELEASE_NOTES.md](upstream/RELEASE_NOTES.md) |
| Platform status and known limits | [COMPATIBILITY.md](COMPATIBILITY.md) |
| Release and publication policy | [RELEASING.md](RELEASING.md) |

## Naming

| Form | Use |
| --- | --- |
| Ryotune-Omni | The project and repository, in prose. |
| `ryotune-omni`, `dev.excentry.ryotune-omni` | Binaries, packages and the application identifier; always lowercase and literal. |
| Ryotunes Omni | The product name shown in the application and in installer metadata; keep it where it is a literal string. |
| Ryotunes | The inherited upstream project and its Linux client. |

The canonical GitHub repository is `Excentry/Ryotunes-Omni`; `Ryotune-Omni` URLs currently redirect to it.

## Ryotune-Omni

| Document | Role | Language |
| --- | --- | --- |
| [../README.md](../README.md) | Public introduction and status | English |
| [../CONTRIBUTING.md](../CONTRIBUTING.md) | How to contribute, checks, pull request format | English |
| [COMPATIBILITY.md](COMPATIBILITY.md) | What is supported, planned or unverified | English |
| [DEVELOPMENT.md](DEVELOPMENT.md) | Developer guide, portability strategy, CI description | English |
| [RELEASING.md](RELEASING.md) | Release policy and branch/tag protection | English |
| [LICENSING.md](LICENSING.md) | Licensing, source obligations, third-party notices | English |
| [INSTALL-ARCH.md](INSTALL-ARCH.md), [INSTALL-VOID.md](INSTALL-VOID.md) | Linux package status (no release) | English |
| [assets/ryotune-omni-design.md](assets/ryotune-omni-design.md) | Logo and banner notes | English |
| [../UPSTREAM.md](../UPSTREAM.md) | Provenance of the code | English |
| [../SECURITY.md](../SECURITY.md) | Security and privacy model: Tauri/Windows host, with the inherited Linux host as reference | English |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Shared core and hosts: Windows (Tauri) and the inherited Linux daemon and client | English |

## Windows port

Working notes are in Spanish; the libmpv documents are in English.

| Document | Role |
| --- | --- |
| [windows/ROADMAP.md](windows/ROADMAP.md) | Pending work, owners and acceptance criteria |
| [windows/CHANGELOG.md](windows/CHANGELOG.md) | Evidence and findings already recorded |
| [windows/PARITY-MATRIX.md](windows/PARITY-MATRIX.md) | Functions Windows must keep from Linux and contract surfaces |
| [windows/CONTRACT-MATRIX.md](windows/CONTRACT-MATRIX.md) | Command and event comparison, Tauri against daemon |
| [windows/W26-VISUAL-BASELINE.md](windows/W26-VISUAL-BASELINE.md) | Visual baseline and comparison protocol |
| [windows/W10-CARGO-WINDOWS-INVENTORY.md](windows/W10-CARGO-WINDOWS-INVENTORY.md) | Cargo dependency graph on Windows |
| `windows/LIBMPV-*.md` | libmpv candidate: source audit, component review, license closure, owned build, validation |

[windows/libmpv-research/](windows/libmpv-research/) holds Spanish research notes on libmpv source recovery.

## Inherited Linux and Ryotunes reference

These describe the inherited Ryotunes client and daemon. They are reference material, not Ryotune-Omni platform documentation, and still use the Ryotunes name.

| Document | Role |
| --- | --- |
| [DESIGN.md](DESIGN.md) | Design language; kept as the default look, see [windows/W26-VISUAL-BASELINE.md](windows/W26-VISUAL-BASELINE.md) |
| [SKINS.md](SKINS.md), [../skins/README.md](../skins/README.md) | Skin format and gallery |
| [TROUBLESHOOTING.md](TROUBLESHOOTING.md) | Linux package troubleshooting |
| [RELEASE-CHECKLIST.md](RELEASE-CHECKLIST.md) | Legacy upstream release flow |
| `superpowers/` | Historical plans and specs of the native client (2026-09-05) |
| [../aur/README.md](../aur/README.md), [../integrations/quickshell/README.md](../integrations/quickshell/README.md) | Inherited packaging and MPRIS widget |
