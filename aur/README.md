# Inherited AUR packaging

This directory contains an upstream `ryotunes-bin` recipe. It is not a Ryotune-Omni package and is not maintained for Omni.

Do not use this recipe to install Omni. A distinct Omni AUR package requires a separate reviewed recipe and published Omni release.

Before publishing or updating the AUR package:

1. Confirm the matching GitHub Release asset exists.
2. Verify its SHA-256 against `PKGBUILD`.
3. Run `makepkg --printsrcinfo > .SRCINFO` on an Arch-based system.
4. Test with `makepkg -si`.
5. Commit `PKGBUILD`, `.SRCINFO`, `ryotunes-bin.install`, and `LICENSE` to the AUR package repository.

The packaged application remains GPL-3.0-or-later. The AUR packaging files in this directory are offered under 0BSD.
