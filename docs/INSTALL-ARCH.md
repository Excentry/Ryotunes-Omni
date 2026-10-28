# Install Ryotune-Omni on Arch

Ryotune-Omni's Arch package is named `ryotune-omni` and is designed to coexist with the original `ryotunes` package. No Omni release package is published yet; do not install upstream Ryotunes packages as Omni builds.

## Precompiled package

When an Omni release becomes available, its x86_64 package will be named
`ryotune-omni-X.Y.Z-1-x86_64.pkg.tar.zst` with a `.sha256` sidecar. Verify the
checksum, then install with pacman:

```bash
package=ryotune-omni-X.Y.Z-1-x86_64.pkg.tar.zst # Use the filename you downloaded.
sha256sum -c "$package.sha256"
sudo pacman -U "./$package"
```

The package has no `epoch`, `replaces`, or `conflicts` relation with `ryotunes`.
Both packages may remain installed; uninstalling `ryotune-omni` leaves Ryotunes
files and profile untouched. Omni uses a fresh profile and does not migrate
Ryotunes data.

New profiles default downloads to `Ryotunes Omni` under the user's Music
directory. Existing configured download paths and downloaded files are not
renamed or moved automatically.

This fork does not integrate with Ryoku's upstream package update commands.

Removing the package stops only its own daemon and activation socket. It does not
delete Ryotune-Omni data, or alter the separate Ryotunes installation and profile.

## Update inside Ryotune-Omni

Open **Settings → About → Check for new version** in the client. About shows the
app version (and the connected daemon version in the native client), the next
available release, **View changelog**, and **Update** when installation is supported.
It credits [ashmitvoid](https://github.com/ashmitvoid) and
[neur0map](https://github.com/neur0map), and links the original
[LiMusic project](https://github.com/SimoHypers/limusic).

Self update downloads only from this repository, checks SHA-256 and package
identity, then asks polkit to install `ryotune-omni`. It refuses to update
unmanaged builds and never targets package `ryotunes`. Void/AUR recipes are
inherited upstream artifacts and are not supported Omni distribution paths.
There is no background update polling. Keep the app open during installation,
then **Quit and reopen** it to reload both the interface and background daemon.

No Ryotune-Omni release has been published. The inherited version in manifests is
not a release and no tag or version policy changes are part of the identity migration.

## AUR binary package

The inherited `ryotunes-bin` AUR recipe is not an Omni package and is not maintained here.

```bash
paru -S ryotunes-bin
```

Do not run this command for Ryotune-Omni; it installs the upstream `ryotunes` package.

An Omni AUR package requires a separate reviewed recipe and release.

## Primary QML client

On Linux, Ryotune-Omni runs the native [Quickshell](https://quickshell.org) client with `ryotune-omni-daemon`. Its desktop entry and launcher use:

```bash
ryotune-omni
```

The package requires `quickshell`; the client also uses the Ryoku QML runtime (`Ryoku.Ui.Singletons`). The daemon starts `ryotune-omni-qml`, which runs `qs -p /usr/share/ryotune-omni/client`.

The launcher asks `ryotune-omni-daemon` to show its own client or open one. Its socket, lock and data directories are distinct from upstream Ryotunes; the apps can run simultaneously.

## Build from source

Developers can build the repository directly. On Arch-based systems you need the normal Rust/Tauri frontend toolchain plus the runtime dependencies listed in `packaging/arch/PKGBUILD`.

From the repository root, build and launch the standalone Ryotune-Omni app with:

```bash
make run-omni
```

This builds the frontend and binaries, then opens the native Quickshell client;
you do not need to locate or copy a binary path. `make build-omni` builds
without launching. Quickshell (`qs`) and the Ryoku QML runtime are required to
run. For options such as `--release`, call `scripts/dev/omni.sh` directly.

The development launcher and daemon use the fork identity
`dev.excentry.ryotune-omni`, socket `$XDG_RUNTIME_DIR/ryotune-omni/daemon.sock`,
and profile `$XDG_DATA_HOME/dev.excentry.ryotune-omni` (default:
`~/.local/share/dev.excentry.ryotune-omni`). They do not replace, invoke, or
share a profile with upstream Ryotunes. A fresh profile requires signing in
again; there is no automatic data migration. The build script does not install
system files or create a package.

```bash
cd ui
pnpm install --frozen-lockfile
pnpm check
pnpm build

cd ..
cargo fmt --all -- --check
cargo test --workspace --locked
cargo tauri build --no-bundle
```

For a pacman package from the local checkout:

```bash
cd packaging/arch
makepkg -si
```

Before publishing a build, follow `docs/RELEASE-CHECKLIST.md` rather than treating a successful local compile as a release gate.
