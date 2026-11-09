# Ryotunes × Ryoku / Quickshell

> **Upstream reference.** Ryotune-Omni publishes its MPRIS player as identity `Ryotunes Omni` with
> D-Bus name `ryotune-omni` (`crates/core/src/media.rs`). `RyotunesBarWidget.qml` still matches the
> upstream identity `ryotunes` exactly, so it may not find the Omni player until the widget is adapted.
> This note was written from the sources and the widget was not run.

Upstream Ryotunes publishes a native Linux MPRIS player named `ryotunes`. The optional
`RyotunesBarWidget.qml` consumes that service directly through Quickshell's MPRIS module, so it
adds **no second WebKit window, polling script, or background helper**.

Controls:
- left click: play / pause
- middle click: previous
- right click: next
- wheel: volume

Copy the component into the module folder used by your Ryoku/Quickshell configuration and instantiate
`RyotunesBarWidget {}` in the bar layout. Ryoku installations differ in how their shell tree is
structured, so V22 deliberately does not overwrite `~/.config/quickshell` automatically.

If your installed Quickshell version exposes `Mpris.players` as a model rather than `.values`, adapt
the `player` lookup to the local shell's existing MPRIS helper. The app-side contract is stable:
standard MPRIS identity `Ryotunes` / dbus name `ryotunes` for upstream Ryotunes only.
