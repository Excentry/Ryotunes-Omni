# W23 Spotify transport lab

This is a temporary, Windows-only measurement harness. It does not change production playback. It uses one librespot session and one libmpv instance per run; select one transport per process so the same track and settings can be repeated for TCP and named pipe.

## Prerequisites

- Windows 11 with an interactive audio output device.
- Rust toolchain and the candidate `libmpv-2.dll` already validated for this checkout.
- A Spotify Premium test account. The sign-in URL is printed in the console if no cached session exists.
- A track ID or `spotify:track:` URI that can be played by that account.

Use a separate data directory for the lab so its Spotify credentials and libmpv cache are isolated from the app. Do not share or attach the directory; it contains credentials.

## Run

From the repository root in PowerShell, with the candidate DLL directory already set in `LIB` and `PATH`:

```powershell
cargo run --locked -p ryotunes-core --example spotify_w23_lab -- tcp <track-id-or-uri> "$env:TEMP\ryotune-w23-lab-tcp"
cargo run --locked -p ryotunes-core --example spotify_w23_lab -- named-pipe <track-id-or-uri> "$env:TEMP\ryotune-w23-lab-pipe"
```

To measure the failed-connection path against a closed local port with the same isolated 300-second libmpv timeout:

```powershell
cargo run --locked -p ryotunes-core --example spotify_w23_lab -- --dead-listener 49173 "$env:TEMP\ryotune-w23-lab-probe"
```

Confirm that port `49173` is not listening before the probe. It prints progress every ten seconds, reports the libmpv error and elapsed time, or exits with an explicit timeout after 310 seconds. Run this separately from the real-audio measurements.

Complete Spotify sign-in in the browser if prompted. The harness reports librespot and libmpv events and accepts console commands:

| Command | Action |
| --- | --- |
| `load <track-id-or-uri>` | Replace the current track and transport stream |
| `pause` | Pause libmpv while librespot continues producing until backpressure |
| `resume` | Resume libmpv |
| `seek <seconds>` | Seek in the Spotify track |
| `metrics` | Print bounded PCM queue byte and blocking counters |
| `stop` | Stop both playback sides and clean up the bridge |
| `quit` | Stop both playback sides and clean up the bridge |

Both transports use the same 64-chunk PCM queue, 2 MiB demuxer limit, 1 MiB back-buffer limit, raw S16LE/44.1 kHz/stereo settings, and a dedicated libmpv instance initialized with `network-timeout=300`. The timeout is not set on the production player constructor, so other providers are unaffected. The TCP candidate is HTTP over IPv4 loopback: libmpv loads `http://127.0.0.1:<port>/<random-token>`, and the bridge validates that request before returning PCM. It switches each accepted socket to blocking mode; the listener accept loop polls cancellation. The named pipe rejects remote clients and uses the extended `\\?\pipe\` media path. The separate `player` prototype test uses libmpv's `tcp://` protocol and is not the HTTP bridge exercised here.

Every Spotify and libmpv event is printed with milliseconds since harness start. The final `cleanup_ms` is the elapsed time for explicit cancellation and bridge cleanup. Use `load` to repeat track replacements within one process.

The queue counters do not measure all PCM retained by libmpv. Isa observed approximately 163 MiB before the writer stalled with the stated demuxer limits; that discrepancy remains unresolved and must be recorded rather than attributed to the bounded queue. Record process working set as well as the harness counters.

## Measurement record

Capture audible timing with **Audacity on Windows using Windows WASAPI loopback capture**: select the same output endpoint the harness is playing through and its loopback input, record to WAV at the endpoint's native rate, and mark first audible sample, pause silence, resume onset, seek arrival, and final tail against the harness event timestamps. Do not change the system default endpoint or volume for the capture. Record Audacity version, WASAPI mode, endpoint, and capture rate. This is the agreed capture method for the comparison; it has not been exercised in this checkout.

Run at least five repetitions per transport on the same Windows 11 machine, endpoint, track, volume, and account. Record Windows build, Rust version, candidate DLL SHA-256, output endpoint/format, transport, queue and cache settings, repetition number, event timestamps, audible onset/silence timing from the WAV, seek target versus audible location, produced/consumed bytes, blocked time, peak working set, and handle/thread counts. Exercise start, pause/resume, seek to 30/60/120 seconds, pause-induced backpressure, natural EOF, cancellation while the consumer is blocked, 20 track replacements using `load`, Stop, and Quit. Keep measured values separate from estimates; do not include account identifiers, tokens, or credential files in evidence.

The harness reports load and end-file errors in the console. The no-listener probe uses the same timeout and HTTP URL shape as the TCP candidate; record elapsed time and whether the error is visible. Do not leave a five-minute timeout test running unattended. The Windows 11 real-audio run, not the hosted synthetic tests, determines the W23 recommendation.
