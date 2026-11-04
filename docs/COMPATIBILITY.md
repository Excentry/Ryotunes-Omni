# Feature compatibility

Ryotune-Omni aims to preserve the experience of the original [Ryotunes application](https://github.com/Ryoku-dev/ryotunes) on Windows and Android. Linux releases and maintenance belong to that original project.

> **Status key**
> - **Existing:** Present in inherited source; not runtime-tested by this review.
> - **Planned:** Porting objective, not implemented or verified support.
> - **Investigate:** Platform or implementation decision remains open.
>
> **No Windows or Android feature below is verified working.**

| Feature area | Existing upstream implementation | Windows | Android |
|:--|:--|:--|:--|
| Discovery and search | Home feeds, songs/albums/artists/playlists, incremental results, shortcuts and local history-based recommendations | Planned parity | Planned; adapt navigation and bounded loading |
| Music providers | YouTube/YouTube Music, Spotify, SoundCloud and TIDAL provider paths | Planned; **Spotify playback is deferred** (see limitations below); investigate auth and native/audio transport for the others | Planned; investigate auth, native compatibility and service restrictions |
| Playback and audio controls | Native audio-only libmpv, seek/volume, shuffle/repeat, queue continuation and audio effects; behavior varies by provider | Planned; validate libmpv, filters and output | Investigate engine and effects parity |
| Queue, playlists and favorites | Queue editing, provider likes/saves, persistent device playlists, playlist import/export | Planned parity | Planned; adapt file import/export and interactions |
| Local library and metadata | Local-folder indexing, metadata and artwork handling | Planned; paths and access need adaptation | Planned; investigate URI-based media access and permissions |
| Album/playlist artwork | Provider/local artwork, caches, device-playlist collages and custom covers | Planned parity | Planned; validate memory/cache costs |
| Lyrics | Plain and synchronized lyrics, timing/seek UI, local sidecar/embedded lyrics and network providers | Planned parity | Planned; provider availability and mobile presentation need validation |
| Downloads and offline files | Persistent queue/history, format preferences, metadata and companion artwork/lyrics; downloaded/local files can be used offline | Investigate downloader packaging, process control and filesystem behavior | Investigate mobile downloader, background execution, storage and permissions |
| Themes, skins and customization | Light/dark, Ryoku Follow System, skin manifests, matugen integration and mini-player styling | Planned identity/skins; replace Linux shell theme coupling | Planned identity/skins; investigate mobile theme mapping |
| History and personalization | Recent listening, shortcuts, local Home ranking and listening statistics | Planned parity | Planned parity; lifecycle and persistence need validation |
| Desktop/background controls | Daemon-owned playback/downloads, mini-player, tray, media keys and MPRIS; inherited Windows host code also exists | Investigate Windows media controls, tray and mini-player parity | Investigate foreground media service, notifications, audio focus and lock-screen controls |
| External integrations | Last.fm scrobbling, Discord Rich Presence, optional Listen Together relay | Investigate credentials, host integration and lifecycle | Investigate applicability; desktop integration may have no mobile equivalent |
| Internet Radio | Demand-driven station discovery and native live audio playback | Planned; validate engine/network behavior | Planned; validate engine and background playback |
| Installation and updates | Linux package/release scripts and pacman/xbps-oriented update handling | New packaging/update strategy required | New packaging/update strategy required |

## Provider and download limitations

- Feature availability depends on account state, region, subscription, provider responses and service terms; integrations are not service guarantees. Spotify sign-in includes a Premium eligibility gate.
- **Spotify playback is deferred on Windows.** The inherited transport is a Unix FIFO, which Windows rejects, and neither playback engine tried works on the Windows 11 test machine. librespot receives a refused audio key for the test Premium account, an open upstream problem ([librespot#1649](https://github.com/librespot-org/librespot/issues/1649)). Spotify Soloist exits shortly after playback starts on Windows 11 build 26300.x ([spotify/soloist#28](https://github.com/spotify/soloist/issues/28)). Other providers are not affected. Measurements and reopening conditions are in the [Windows changelog](windows/CHANGELOG.md#w23--spotify-aplazado-en-windows).
- TIDAL integration requires configured client credentials and sign-in. The implementation explicitly refuses protected stream assets; Ryotune-Omni does not promise DRM bypass or playback of every catalogue item.
- Current downloads use yt-dlp and FFmpeg. YouTube and public SoundCloud sources are supported paths; Spotify downloads use a **labelled YouTube search match**, not exported Spotify audio or a guarantee of the same recording.
- Downloads do not establish unrestricted offline access to any provider catalogue. Local files, live radio, protected or unavailable items are not downloadable through that workflow. Save only content you are authorized to download, in accordance with applicable law and service terms.
- Accounts, lyrics, artwork, scrobbling and download metadata lookups can involve third-party services. Preserve clear disclosures and protect credentials when adapting these features.

## How to report compatibility results

Keep planned, implemented and runtime-verified behavior distinct. Future results should identify the commit, OS/device, relevant dependency versions, provider/account prerequisites, checks actually run and remaining limitations. A successful compile does not prove playback or feature parity.

See the [developer guide](DEVELOPMENT.md) for architecture and development references, and [contribution guide](../CONTRIBUTING.md) for reporting and contribution guidance.
