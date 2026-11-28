#!/usr/bin/env bash
set -u

# Intentionally excludes usernames, home paths, hostnames, account data, cookies, URLs and app logs.
line() { printf '%-22s %s\n' "$1" "$2"; }

printf '%s\n' 'Ryotune-Omni diagnostics (sanitized)'
printf '%s\n' '--------------------------------'
if command -v xbps-query >/dev/null 2>&1; then
  pkg_version='source/unpackaged'
else
  pkg_version="$(pacman -Q ryotune-omni 2>/dev/null | awk '{print $2}' || printf 'source/unpackaged')"
fi
line 'Ryotune-Omni package:' "$pkg_version"
line 'Kernel:' "$(uname -sr 2>/dev/null || printf 'unknown')"
line 'Architecture:' "$(uname -m 2>/dev/null || printf 'unknown')"
line 'Session:' "${XDG_SESSION_TYPE:-unknown}"
line 'Desktop:' "${XDG_CURRENT_DESKTOP:-unknown}"
line 'Wayland display:' "${WAYLAND_DISPLAY:+present}"
webkit_version="$(pkg-config --modversion webkit2gtk-4.1 2>/dev/null || printf 'unknown')"
line 'WebKitGTK:' "$webkit_version"
if [[ "$webkit_version" != 'unknown' ]]; then
  oldest="$(printf '%s\n' '2.52.6' "$webkit_version" | sort -V | head -n 1)"
  if [[ "$oldest" != '2.52.6' ]]; then
    line 'WebKit security:' 'UPDATE RECOMMENDED (2.52.6+ baseline)'
  else
    line 'WebKit security:' 'current for 2.52.6 baseline'
  fi
fi
line 'mpv:' "$(mpv --version 2>/dev/null | head -n 1 | sed 's/^mpv //')"
line 'Rust:' "$(rustc --version 2>/dev/null || printf 'not installed')"
line 'Node:' "$(node --version 2>/dev/null || printf 'not installed')"
line 'pnpm:' "$(pnpm --version 2>/dev/null || printf 'not installed')"
line 'Hyprland:' "$(hyprctl version -j 2>/dev/null | sed -n 's/.*"tag":"\([^"]*\)".*/\1/p' | head -n 1 || true)"
line 'GPU classes:' "$(lspci 2>/dev/null | grep -Ei 'VGA|3D controller' | sed -E 's/^[0-9a-f:.]+ //' | paste -sd ';' - || printf 'unknown')"

find_ryotune_omni_pid() {
  local expected='/usr/bin/ryotune-omni' p exe
  for p in /proc/[0-9]*; do
    [[ -e "$p/exe" ]] || continue
    exe="$(readlink -f "$p/exe" 2>/dev/null || true)"
    [[ "$exe" == "$expected" ]] && { basename "$p"; return 0; }
  done
  return 1
}

ryo_pid="$(find_ryotune_omni_pid || true)"
if [[ -n "$ryo_pid" ]]; then
  line 'Ryotune-Omni running:' "yes (pid $ryo_pid)"
else
  line 'Ryotune-Omni running:' 'no'
fi

printf '\n%s\n' 'Dependency status'
if command -v xbps-query >/dev/null 2>&1; then
  deps=(libwebkit2gtk41 libayatana-appindicator mpv openssl librsvg xdg-utils quickshell)
  have() { xbps-query "$1" >/dev/null 2>&1; }
else
  deps=(webkit2gtk-4.1 libappindicator-gtk3 mpv openssl librsvg xdg-utils)
  have() { pacman -Q "$1" >/dev/null 2>&1; }
fi
for p in "${deps[@]}"; do
  if have "$p"; then printf '  [ok] %s\n' "$p"; else printf '  [!!] %s missing\n' "$p"; fi
done
# The QML client loads cover art over HTTPS through Qt, which needs a TLS plugin.
if compgen -G '/usr/lib/qt6/plugins/tls/*' >/dev/null; then
  printf '  [ok] %s\n' 'Qt TLS plugin'
else
  printf '  [!!] %s\n' 'Qt TLS plugin missing (no cover art)'
fi

printf '\n%s\n' 'This report does not include Ryotunes logs, account details, cookies, tokens, local file paths, or network endpoints.'
