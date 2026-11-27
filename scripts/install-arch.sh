#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v pacman >/dev/null 2>&1; then
  echo 'This installer is for Arch/CachyOS and other pacman-based systems.' >&2
  exit 2
fi
if [[ ${EUID:-$(id -u)} -eq 0 ]]; then
  echo 'Run this script as your normal user. It will call sudo only for package installation.' >&2
  exit 2
fi

printf '%s\n' 'Installing Ryotunes build/runtime dependencies…'
sudo pacman -S --needed --noconfirm \
  base-devel nodejs pnpm webkit2gtk-4.1 appmenu-gtk-module \
  libappindicator-gtk3 librsvg xdotool mpv openssl desktop-file-utils hicolor-icon-theme xdg-utils

# Ryoku systems commonly use rustup. Installing Arch's `rust` package on top of it conflicts, so
# keep an existing toolchain and bootstrap rustup only when Cargo is genuinely absent.
if ! command -v cargo >/dev/null 2>&1; then
  printf '%s\n' 'Rust toolchain not found; installing rustup…'
  sudo pacman -S --needed --noconfirm rustup
  rustup default stable
fi

printf '%s\n' 'Building an Arch package from this source tree…'
cd "$repo_root/packaging/arch"
makepkg -si --needed

printf '%s\n' 'Ryotune-Omni installed. Launch it from your app menu or run: ryotune-omni'
