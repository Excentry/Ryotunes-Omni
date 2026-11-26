# Visual parity lab

Captures the Linux QML client and the Windows Svelte/WebView2 UI in the same states, at 1376×820 and scale 1, so the two can be compared for W26 (`docs/windows/W26-VISUAL-BASELINE.md`). Both sides run without an account and use the same public data: the search "Daft Punk Get Lucky", the album *Random Access Memories* and the artist Daft Punk.

Each side produces the same 20 PNG files (`dark-00-home-loading.png` … `light-10-nowplaying-queue.png`) plus `playback.json`.

## Linux reference (Docker)

The image is Arch Linux with Quickshell, the `Ryoku.Ui` QML module cloned from `Ryoku-dev/ryoku`, Weston in headless kiosk mode at 1376×820, PulseAudio with its automatic null sink, and the Ryoku skin fonts (Space Grotesk 2.0.0 and Fraunces 1.000 checked against the hashes Ryoku pins, SpaceMono Nerd Font from Arch).

From the repository root:

```powershell
docker build -t ryotunes-visual-parity scripts/dev/visual-parity
docker run -d --name ryotunes-visual-parity --shm-size=1g -v ryotunes-visual-parity-target:/home/ryo/target ryotunes-visual-parity sleep infinity
docker exec -u root ryotunes-visual-parity chown ryo:ryo /home/ryo/target
git archive --format=tar -o $env:TEMP\ryotunes-src.tar HEAD
docker cp $env:TEMP\ryotunes-src.tar ryotunes-visual-parity:/home/ryo/src.tar
docker exec ryotunes-visual-parity sh -c "mkdir -p /home/ryo/src && tar -xf /home/ryo/src.tar -C /home/ryo/src"
docker exec ryotunes-visual-parity /home/ryo/lab.sh build
docker exec ryotunes-visual-parity /home/ryo/capture.sh
```

`capture.sh` prints the output directory under `/home/ryo/shots/`; copy it out with `docker cp`. It deletes the container's own daemon and client profiles before each run so that "no track" is a clean state. `lab.sh` also exposes `up`, `daemon`, `client`, `ctl` (the `RYOTUNES_CTL` commands documented in `client/shell.qml`) and `shot` for manual work.

The Weston and PulseAudio setup only exists inside the container. Do not point these scripts at a live desktop session.

## Windows (WebView2 over CDP)

Requires `src-tauri/libmpv-2.dll` prepared by `scripts/windows/prepare-libmpv.ps1`, plus the directory holding `mpv.lib` and the DLL for linking and loading.

```powershell
pwsh scripts/dev/visual-parity/run-windows.ps1 -LibmpvDir <libmpv directory>
```

The script builds the app with `tauri.visual-parity.conf.json`, whose identifier `dev.excentry.ryotune-omni.visual-parity` keeps a separate data directory and WebView profile. It deletes only that profile before launching, so the normal app profile is never touched. It enables the WebView2 debugging port on `127.0.0.1` for this process only, drives the main WebView, writes the screenshots and `fonts.json` (the platform fonts WebView2 used for the Home heading and body text), and closes the app. `-SkipBuild` reuses the last build.

The capture overrides the viewport to 1376×820 at device scale 1, so it does not depend on the monitor's DPI.

## Side-by-side images

```powershell
pwsh scripts/dev/visual-parity/compare.ps1 -LinuxDir <linux shots> -WindowsDir <windows shots> -OutDir <output>
```

Writes one JPEG per pair, Linux on the left.

## Limits

- No sign-in: authenticated pages, Spotify and other providers are not covered.
- Home recommendations and play counts are live data and can differ between runs and between the two captures.
- The two UIs map some states differently: on Windows, Q opens the tabbed Now Playing view instead of a side panel, and "rail collapsed" keeps an icon rail. The file names follow the protocol state, not the control used.
