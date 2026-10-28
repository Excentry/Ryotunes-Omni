# Ryotune-Omni | Roadmap del port a Windows 11

**Plan de ejecución para dos contribuidores**

> **Objetivo:** entregar una aplicación Windows 11 instalable, verificable y mantenible, reutilizando el núcleo donde sea viable, preservando el carácter visual de Ryotunes y evitando regresiones en Linux.

| Campo | Decisión/estado |
| --- | --- |
| Plataforma inicial | Windows 11 |
| CPU inicial | x64 confirmado para el prototipo y la Alpha |
| Windows 10 | Fuera del alcance inicial |
| Interfaz Windows | Svelte + Tauri integrado en un proceso; runtime aún por validar |
| Motor de audio | Adoptar libmpv GPL x64 oficial de mpv `v0.41.0-1274-ga43ab319f` como dependencia objetivo de distribución Windows; el build/paquete y cumplimiento de fuentes/avisos siguen pendientes de gate |
| Diseño | Mantener Ryotunes como experiencia predeterminada y permitir personalización gradual |
| Android | Fuera de este roadmap |
| Base técnica | [Arquitectura actual del repositorio](../ARCHITECTURE.md) y [guía de portabilidad](../DEVELOPMENT.md#architecture-and-portability-strategy) |

## Índice

1. [Decisiones y alcance](#1-decisiones-y-alcance)
2. [Responsabilidades](#2-responsabilidades)
3. [Reglas de colaboración e integración](#3-reglas-de-colaboración-e-integración)
4. [Entregas y Definition of Done](#4-entregas-y-definition-of-done)
5. [Fases y tareas](#5-fases-y-tareas)
6. [Validación y operación](#6-validación-y-operación)
7. [Riesgos, dependencias y decisiones abiertas](#7-riesgos-dependencias-y-decisiones-abiertas)

## 1. Decisiones y alcance

### Decisiones cerradas para desbloquear el trabajo

| Tema | Decisión | Límite / siguiente validación |
| --- | --- | --- |
| Plataforma | Windows 11 x64. Windows 10 y ARM64 quedan fuera de esta entrega. | W11 y CI deben usar target MSVC x64; ampliar plataformas requiere aprobación explícita. |
| Host | Reutilizar Svelte + Tauri dentro de un proceso; no crear un daemon Windows separado ahora. | W24 valida que ocultar/cerrar la ventana no detenga playback o descargas y que Quit sí libere recursos. Reabrir arquitectura solo si falla esa aceptación o aparece evidencia nueva. |
| Alcance del producto | Paridad con Linux sigue siendo objetivo del producto; ninguna función se elimina por quedar fuera de Alpha. Las funciones no implementadas se declaran pendientes/no disponibles. | La matriz W04 asigna funciones a Alpha o fases posteriores y conserva pruebas por función. |
| Alpha interna | Corte vertical: YouTube browse/login/reproducción; playback y cola con persistencia; biblioteca local; ventana/cierre/reapertura y segunda instancia; diagnóstico mínimo. | Requiere instalación en Windows 11 limpio y flujo end-to-end W45. Proveedores, descargas, integraciones y paridad restante continúan en fases posteriores. |
| Motor de audio | Adoptar el artefacto oficial Windows x64 GPL de mpv como dependencia objetivo; no esperar a una DLL estable que upstream no publica. | El artefacto actual es un dev build, no un release estable. Antes de distribuirlo, Excentry debe fijar descarga/hash, entregar fuentes correspondientes y build scripts, inventariar dependencias/avisos GPLv3 y validar paquete/runtime en W20/W85. |
| Datos | Datos y caché en ubicaciones por usuario resueltas por Tauri; no guardar estado en el directorio de instalación ni compartirlo con Linux. | W33 valida ACL, migración y persistencia. Definir identificador único del fork antes de instalar junto a upstream o distribuir. |
| Actualización/publicación | Sin updater ni publicación pública en Alpha. No usar repositorio, identidad o credenciales de upstream. | Resolver identidad del fork, endpoint, firma, licencias y recuperación antes de cualquier distribución externa (W84–W86). CI rutinaria no publica. |
| Diseño | Ryotunes predeterminado y reconocible en Alpha. Skins/temas/presets avanzados se implementan en Fase 6; editor libre queda opcional y aplazado. | W26 fija baseline visual; W60–W64 cubren la personalización posterior. |

Estas decisiones de producto y arquitectura fueron confirmadas por @IsaVonxz-type el 2026-10-19 para desbloquear trabajo en paralelo. No convierten tareas de validación en tareas completadas ni reemplazan la revisión cruzada requerida.

### Alcance inicial

- Port de escritorio para Windows 11 x64.
- Usar el host Svelte/Tauri existente en un proceso; build/runtime, ciclo de vida y motor libmpv aún deben validarse.
- Reutilizar lógica de dominio y contratos Rust, sin duplicar lógica de proveedores entre hosts.
- Conservar Linux como referencia funcional y ejecutar sus checks para cambios compartidos.
- Usar el cliente QML/Quickshell de Linux como referencia de paridad funcional y visual; no portarlo ni convertirlo en runtime Windows.
- Mantener la identidad de Ryotunes: jerarquía visual, comportamiento centrado en artwork, tratamiento de tipografía, color y reproductor. Adaptar controles e interacción a Windows cuando corresponda.
- Introducir personalización mediante un sistema acotado y extensible, no un editor arbitrario desde la primera entrega.

### Fuera del alcance por ahora

- Windows 10 y ARM64, salvo que se aprueben expresamente.
- Portar Quickshell, Wayland, systemd, MPRIS, Hyprland o integraciones Ryoku literalmente.
- Reemplazar libmpv antes de medir una alternativa y documentar el bloqueo que motiva el cambio.
- Rehacer toda la interfaz antes de comprobar la cobertura de funciones existente.
- Publicar actualizaciones usando repositorio, identidad o credenciales de upstream.

### Niveles de evidencia

No usar “compatible” como sinónimo de “compila”. Registrar cada función como:

1. **Inspeccionada:** evidencia estática, sin build target.
2. **Compila:** target y comando identificados.
3. **Prueba automatizada:** test concreto pasó.
4. **Verificada en runtime:** escenario y Windows 11 registrados.
5. **Distribuible:** artefacto instalado, ejecutado, actualizado y revisado.

## 2. Responsabilidades

El reparto refleja la instrucción del equipo. No presupone experiencia exclusiva: el segundo integrante revisa cambios críticos y hay transferencia explícita de propiedad cuando haga falta.

| Responsable | Áreas primarias | Archivos/áreas de propiedad principal |
| --- | --- | --- |
| **@Excentry** | Backend, audio, adaptación Windows, dependencias nativas, CI/CD, empaquetado y operación | `crates/`, manifests Cargo, `Cargo.lock`, `.github/workflows/`, scripts de build/paquete |
| **@IsaVonxz-type** | Frontend, Tauri, UX, autenticación desde host, pruebas de interfaz, accesibilidad y documentación de producto | `ui/`, `src-tauri/src/`, `src-tauri/build.rs`, capabilities/permissions, configuraciones Tauri, docs de UI y compatibilidad |
| **Ambos** | Contratos entre host y core, alcance/paridad, seguridad, criterios de release | Un implementador dueño por cambio; el otro realiza revisión cruzada |

### Propiedad de archivos compartidos

| Archivo/grupo | Dueño de edición | Coordinación requerida |
| --- | --- | --- |
| `Cargo.toml`, manifests Cargo, `Cargo.lock` | @Excentry | @IsaVonxz-type solicita dependencias; una sola actualización de lockfile por PR |
| `crates/core/src/host.rs`, contratos y estado compartido | @Excentry | Publicar propuesta de interfaz antes de implementarla; revisión de @IsaVonxz-type |
| `src-tauri/src/lib.rs`, `commands.rs`, `build.rs` | @IsaVonxz-type | Integrar contratos del core publicados por @Excentry |
| `ui/package.json`, `ui/pnpm-lock.yaml` | @IsaVonxz-type | @Excentry solicita tooling; no actualizar versiones como trabajo incidental |
| `.github/workflows/` y empaquetado Windows | @Excentry | @IsaVonxz-type especifica casos funcionales que deben cubrirse |
| Documentación pública de producto | @IsaVonxz-type | Mantener hallazgos técnicos fieles al estado reportado por @Excentry |

Si ambos necesitan modificar un archivo con dueño único, dividir el trabajo por PR o serializarlo. No editarlo simultáneamente.

## 3. Reglas de colaboración e integración

1. Crear una tarea y asignar un dueño antes de editar.
2. Anunciar archivos previstos y contrato afectado; declarar dependencias y criterio de aceptación.
3. Usar ramas cortas y PR pequeñas; integrar cambios de negocio, refactors, dependencias y release por separado.
4. Cada PR tiene un solo implementador y una revisión del otro integrante para contratos compartidos, autenticación, procesos, datos o publicación.
5. Antes de modificar un archivo, comprobar su estado y preservar trabajo no relacionado.
6. No regenerar lockfiles, formatear directorios completos ni actualizar dependencias sin necesidad ligada a la tarea.
7. Integrar primero las interfaces compartidas; después, el consumidor de Tauri. Mientras espera, frontend puede avanzar con fixtures, sin afirmar que validó backend real.
8. Si los contratos cambian, actualizar pruebas de ambos lados en la misma secuencia de integración.
9. Cambios compartidos deben seguir pasando la suite Linux. Una limitación ambiental se informa como no verificada, no como aprobada.
10. No fusionar a la rama por defecto mientras su push active publicación automática de upstream. Primero separar y verificar CI de release.

### Plantilla mínima de tarea

```text
ID / objetivo:
Responsable / revisor:
Dependencias:
Archivos bajo propiedad:
Contrato o datos que cambian:
Aceptación observable:
Pruebas, entorno y resultados:
Riesgo y recuperación:
```

Estados: `Pendiente -> Lista -> En curso -> Revisión -> Verificación -> Hecha`. Usar `Bloqueada` con causa y siguiente acción. Una tarea solo se marca hecha después de revisar la evidencia.

## 4. Entregas y Definition of Done

| Entrega | Mínimo exigible | No implica |
| --- | --- | --- |
| **Prototipo técnico** | Builds reproducibles de host y audio; pruebas de riesgos técnicos; ejecución interactiva | Soporte de usuario, paridad o publicación |
| **Alpha interna** | Instalación Windows 11 limpia; recorrido de reproducción; persistencia; cierre/reapertura y segunda instancia | Paridad completa de proveedores o instalador público |
| **Beta funcional** | Funciones acordadas para la primera entrega, personalización prevista, descarga/control donde estén comprometidos | Ausencia de límites de proveedores |
| **Release candidate** | Seguridad, rendimiento, recuperación, licencias, actualización y prueba de instalación aprobados | Publicación automática |
| **Estable** | Mismo artefacto validado y firmado; notas, fuente correspondiente, límites y soporte publicados | Compatibilidad con versiones fuera del alcance |

### Invariantes funcionales

- Un solo propietario del motor de audio y estado persistente.
- Cerrar la ventana no interrumpe reproducción ni descargas activas.
- Quit explícito detiene trabajo y libera recursos propios.
- Una segunda instancia enfoca la existente sin competir por DB o audio.
- Fallos de proveedor no inutilizan reproducción local.
- Funciones ausentes se muestran como no disponibles; nunca se finge éxito.
- Credenciales, cookies y URLs firmadas no se escriben en logs ni artefactos de diagnóstico.
- No ampliar permisos de WebViews remotas u ocultas para resolver errores del port.
- Los cambios compartidos no introducen regresiones conocidas en Linux.

### Requisito de diseño y personalización

**Diseño predeterminado:** debe reconocerse como Ryotunes, no como un dashboard genérico. Preservar composición, jerarquía, artwork y lenguaje visual en Svelte; adaptar ventanas, navegación, teclado y controles nativos a Windows 11.

**Personalización progresiva:**

1. **Primera entrega:** tema claro/oscuro, acento, colores y tipografía mediante el sistema de skins existente o una extensión compatible; valores seguros por defecto.
2. **Siguiente entrega:** densidad compacta/cómoda y presets de navegación/reproductor; preferencias persistentes y restaurables.
3. **Futuro opcional:** edición libre de layout, solo después de validar costos de mantenimiento, accesibilidad, escalabilidad de UI y compatibilidad de skins.

No cambiar estructura arbitraria de páginas como requisito de MVP. Mantener separación entre tokens de tema, presets de layout y estado de usuario para que la personalización no bifurque el frontend.

## 5. Fases y tareas

Las fases son puertas de avance, no fechas. El equipo estima duración después de las pruebas de viabilidad. W0 evita riesgos de publicación; W1 y W2 permiten trabajo paralelo limitado.

**Estado de tareas:** ☑ Hecha (criterio/evidencia registrados); ◐ Parcial; ⊘ Bloqueada; ☐ Sin verificar. `Sin verificar` significa que el roadmap no registra evidencia suficiente para cerrarla; no afirma que nadie haya trabajado en ella. Actualizar el estado al registrar evidencia.

### Fase 0 — Proteger integración y confirmar alcance

| Estado / ID | Responsable | Tarea | Aceptación |
| --- | --- | --- | --- |
| ☑ W00 | @IsaVonxz-type | Acordar Alpha/Beta, matriz de paridad y preservar identidad Ryotunes — **decisiones cerradas** | El alcance de Alpha y la paridad por fases quedan acordados; baseline visual sigue en W26 |
| ☑ W01 | @Excentry | Revisar publicación automática de `.github/workflows/release.yml` — **revisión estática hecha** | Push/merge normal no publica; CI de PR separada. Solo tags `v*` activan release; sin targets, tag falla antes de publicar y trigger manual valida sin publicar. Ver workflow y `docs/RELEASING.md`. |
| ☑ W02 | Ambos | Confirmar x64, host Svelte/Tauri, actualización y aislamiento de datos — **decisiones cerradas** | Windows 11 x64, Tauri integrado, datos por usuario, sin updater ni publicación pública en Alpha; gates restantes asignados |
| ☑ W03 | @Excentry | Proponer branch/CI/deployment rules seguras para equipo de dos — **propuesta documentada** | [Procedimiento y reglas recomendadas](../RELEASING.md#branch-and-tag-protection), mínimo privilegio y revisión por el otro maintainer. No afirma que GitHub Settings ya estén aplicadas; verificarlas solo con autorización del dueño |
| ◐ W04 | @IsaVonxz-type | Matriz por función y host — **baseline estática y correcciones contractuales completadas; runtime Linux pendiente** | Contratos, superficies QML/Svelte/Tauri/daemon, pruebas y prioridad visibles; validar en runtime autenticado la suscripción de canal y la escritura Spotify antes de congelar |

#### W04 — Matriz de paridad funcional (baseline Linux)

**Política acordada:** Windows debe conservar las funciones de la versión Linux. Las integraciones propias del sistema se reemplazan por equivalentes Windows; no se eliminan del alcance. Windows reutiliza Svelte/Tauri; QML/Quickshell documenta el comportamiento Linux, no es una opción de host Windows. La Alpha puede avanzar por cortes verticales, pero cada función aún pendiente debe declararse como pendiente y la paridad completa sigue siendo criterio de producto.

La matriz toma como fuentes de verdad `ui/src/lib/api.ts`, `src-tauri/src/commands.rs`, `src-tauri/build.rs`, las rutas/componentes de `ui/src/`, `crates/core` y `crates/ryotunesd`. Las referencias describen capacidades presentes en el código Linux, no soporte Windows verificado. El estado inicial de todas las filas es **Inspeccionada; Windows no verificado en runtime**.

| Área / funciones que deben conservarse | Baseline Linux (evidencia principal) | Equivalente o adaptación Windows | Prueba de aceptación Windows |
| --- | --- | --- | --- |
| Inicio, navegación y descubrimiento: Home, recomendaciones, chips, historial/Shortcuts, búsqueda categorizada y paginada | `ui/src/routes/+page.svelte`, `search/+page.svelte`; `get_home*`, `search*`, `get_browse_grid` | Reutilizar Svelte y contratos; conservar carga incremental, navegación y estados vacíos/error | Recorrer Home y todas las categorías de búsqueda; paginar resultados y abrir artist/album/playlist desde tarjetas |
| Proveedores y cuentas: YouTube Music, Spotify, SoundCloud, TIDAL; login/logout, selección y cambio de cuenta, catálogo, favoritos y errores recuperables | `crates/core/src/state.rs`, `spotify.rs`, bridges de proveedores; `get_account*`, `login_webview`, `switch_account`, `rate`, `subscribe` | Host de autenticación compatible con WebView2/browser; preservar requisitos, restricciones por cuenta/región y errores de cada proveedor | Ciclo login/logout/cambio de cuenta y browse/play por proveedor con cuentas de prueba; fallar un proveedor no debe romper local ni los demás |
| Playback y sonido: play/pause, seek, volumen, next/previous, shuffle/repeat, stop-after-current, gapless/autoplay, tempo, pitch y efectos | `crates/player`, `crates/core/src/orchestrator.rs` y `state.rs`; comandos de playback y eventos en `api.ts` | libmpv para Windows con DLL, codecs y filtros requeridos; mantener un único dueño de motor/estado | Archivo local y streams: controles, EOF, gapless, error/skip, efectos, cierre y liberación de handles; comprobar pitch/loudness |
| Cola y transporte: play next, add/clear/remove/reorder, contexto de álbum/lista, persistencia y restauración | `QueueList.svelte`; `play_*`, `add_to_queue`, `move_in_queue`, `get_queue`, snapshots/eventos | Mismo contrato y semántica; restaurar desde almacenamiento por usuario | Manipular cola, reiniciar/cerrar/reabrir y comprobar orden, índice actual y contexto sin duplicados |
| Catálogo y biblioteca: Library, canciones/álbumes/artistas guardados, playlists, álbumes, artistas, listas inteligentes On Repeat/Recently Played/Rediscover, estadísticas y rating | Rutas `library`, `playlist`, `album`, `artist`, `list`; `get_library*`, `get_playlist*`, `get_album`, `get_artist`, `listening_stats`, `play_counts` | Mantener API/modelos y persistencia; adaptar rutas de datos y cualquier picker | Comparar contenido/acciones con Linux; validar signed-out, paginación, smart playlists, estadísticas y persistencia |
| Edición y transferencia de playlists: crear, ordenar, editar nombre/descripción/privacidad, añadir/quitar, borrar, portada, importar/exportar JSON; playlists locales | `commands.rs` playlist actions; `EditPlaylistDialog.svelte`, `PlaylistMenu.svelte` | Pickers nativos Windows; almacenamiento seguro por usuario; preservar validación/formato transferible | CRUD en playlist remota y local, cancelación de picker, import/export y portada; comprobar que no se aceptan archivos inválidos |
| Música local: carpetas vigiladas, escaneo, tags, artistas/álbumes, artwork, archivos ausentes y reproducción offline | `crates/core/src/local.rs`; `get_local_library`, `add_local_folder`, `remove_local_folder` | Paths Windows, Unicode, permisos/ACL, enlaces y cambios del filesystem; selector de carpetas nativo | Escanear, reproducir y reescanear carpetas con Unicode/espacios; archivo eliminado se retira sin bloquear UI |
| Letras: sincronizadas/no sincronizadas, palabras con timing, búsqueda/caché, seek, ajuste manual, sidecar/embedded y fallback de proveedor | `crates/core/src/lyrics.rs`; `LyricsView.svelte`; `get_lyrics` | Mantener datos y sincronización; confirmar lectura de sidecars/metadatos en rutas Windows | Comparar letras, sincronización, seek, ajuste, caché y estados sin resultados/error |
| Radio: radio generada desde canción/artista/álbum/playlist e Internet Radio en vivo | `crates/core/src/radio.rs`; `start_radio`, `radio_stations`, `play_radio_station` | libmpv/HTTP en Windows y cancelación de streams en vivo | Generar radio desde cada tipo compatible; buscar estaciones, reproducir, pausar, terminar y recuperar de URL caída |
| Descargas/offline: cola e historial persistentes, preferencias de formato, progreso, cancelar/reintentar, metadata, artwork/letras incrustadas | `crates/ryotunesd/src/downloads.rs`, `download_media.rs`; UI de Downloads/Settings | Gestionar procesos Windows y árboles de procesos; ubicar/validar yt-dlp y FFmpeg; cancelar sin hijos huérfanos | Descargar fuentes permitidas, reiniciar y recuperar cola, cancelar/reintentar; verificar archivo y enriquecimiento; probar cierre de UI |
| Personalización e identidad visual: skins, tema claro/oscuro, acento/colores/tipografía, densidad/presets, artwork y escala | `client/Skin.qml`, `client/Style.qml`, `docs/SKINS.md`, `ui/src/lib/theme.svelte.ts`, `ryoku_theme_tokens` | Desacoplar Ryoku/Matugen/Follow System Linux; conservar el diseño predeterminado y adaptar tema a Windows; skin inválida revierte al default | Aplicar/restaurar skin y tema, reiniciar, DPI/ventana mínima; comparar visualmente con baseline Linux |
| Mini-player y ventanas: mini con now-playing/cola/letras; cerrar/minimizar/restaurar, segunda instancia y estado de ventanas | `src-tauri/src/mini.rs`, `main_window.rs`; `MiniPlayer.svelte`; `open_mini`/`close_mini` | Ventanas Tauri nativas Windows; segundo lanzamiento enfoca instancia existente; respetar cierre sin detener tareas | Abrir/cerrar/restaurar mini y ventana principal; segunda instancia enfoca la primera; playback/download sobreviven a ocultar/cerrar ventana |
| Controles del sistema: tray, teclas multimedia, metadatos y controles externos de reproducción | `crates/core/src/media.rs`, `src-tauri/src/tray.rs`, `taskbar.rs`; MPRIS en Linux | Tray de Windows + SMTC/media keys; traducir lifecycle/metadatos, no portar MPRIS literalmente | Controlar play/pause/next/previous y seek desde controles Windows; validar arte y metadatos tras ocultar/restaurar |
| Integraciones: Last.fm scrobbling, Discord Rich Presence, Listen Together (crear/unirse, roles, aprobaciones, cola/sugerencias/sync) | `crates/core/src/lastfm.rs`, `discord.rs`, `listentogether/`; comandos/eventos `lastfm_*`, `lt_*` | Browser auth, IPC/SDK y ciclo de vida compatibles con Windows; aislar fallos opcionales | Conectar/desconectar; scrobble con umbral correcto; probar sala host/guest, sync, reconexión y errores; ninguna credencial en logs |
| Preferencias y accesibilidad de uso: volumen/quality/proxy/historial/autoplay/duplicados/recursos/escala, atajos y estados UI | `SettingsDialog.svelte`, `KeyboardShortcuts.svelte`, `UI_SETTINGS`, validadores de `commands.rs` | Persistencia por usuario; autostart con mecanismo Windows; adaptar DPI, teclado, foco y lector de pantalla | Cada setting sobrevive reinicio y aplica efecto real; probar atajos, teclado, Narrator y escalas 100–200 % |
| Actualización, diagnóstico y operación: comprobar/instalar updates, logs/diagnósticos y fallos explicables | `crates/core/src/update.rs`; comandos `check_for_updates`, `install_update`; páginas Diagnostics/Logs | Sustituir actualización Linux (pacman/xbps/pkexec) por mecanismo Windows aprobado; logs locales rotados y exportables | Instalar/actualizar/recuperar en Windows limpio; comprobar hash/procedencia y que diagnóstico no incluye secretos/cookies/URLs firmadas |

**Asignación inicial a fases/prioridades:**

- **P0 — Fase 4 / Alpha:** playback, cola, recuperación, browse/login YouTube, biblioteca local, ventanas/segunda instancia y diagnóstico mínimo (`W40`–`W45`). Son la primera ruta vertical usable.
- **P1 — Fase 5 / paridad por bloques:** Spotify, SoundCloud y TIDAL; descargas; tray/SMTC/media keys; audio/letras/radio e integraciones Last.fm/Discord/Listen Together (`W50` y bloques 1–5). No se consideran descartados por no estar en Alpha.
- **P1 — Fase 6 / identidad y personalización:** skins, tema y presets/densidad (`W60`–`W64`); la identidad Ryotunes y tema predeterminado se conservan desde Alpha como parte de la UI.
- **P0 transversal — Fase 0–3:** contratos UI-host, eventos, almacenamiento por usuario, seguridad y no-regresión Linux (`W00`–`W35`); habilitan cada bloque y no son tareas pospuestas a paridad.
- **P0 para release — Fases 7–8:** accesibilidad, rendimiento, robustez, actualización, empaquetado, licencias y recuperación (`W70`–`W86`).

**Cierre de W04:** cada función visible en Linux queda asociada a contrato (comando/evento), superficies UI, adaptación Windows, prioridad/fase y prueba. El cotejo estático y los tests contractuales de W14 cubren registro de comandos, API, eventos y los defectos corregidos de suscripción y resultado Spotify. W04 sigue parcial hasta confirmar en runtime Linux autenticado la suscripción/cancelación desde ArtistPage y una escritura Spotify exitosa. Registrar excepciones solo como diferencias de integración del sistema, no como recortes funcionales. No marcar W04 hecha antes de registrar ambos resultados de runtime.

#### Auditoría estática de contratos y superficies (W04)

**Decisión de host:** conservar Svelte + Tauri para Windows y aprovechar el trabajo existente en `ui/` y `src-tauri/`. No portar Quickshell. **Flujos de ejecución confirmados en código:** `src-tauri/src/main.rs` abre Tauri en Windows y el launcher QML/daemon en Linux. Por tanto, `ui/src/lib/api.ts` no representa por sí solo la superficie Linux vigente: la referencia de paridad debe incluir también `client/` y `crates/ryotunesd/src/methods.rs`.

**Superficie Tauri compartida que debe cotejarse por nombre, parámetros, resultado/error y payload** (registro en `src-tauri/build.rs`; wrappers en `ui/src/lib/api.ts`; implementación en `src-tauri/src/commands.rs`):

- Búsqueda y catálogo: `search`, `search_page`, `search_page_more`, `search_all`, `search_all_more`, `search_cards`, `search_cards_page`, `search_cards_more`, `get_home`, `get_home_more`, `get_library`, `get_library_albums`, `get_library_artists`, `get_playlist`, `get_playlist_more`, `get_album`, `get_artist`, `get_browse_grid`.
- Playback/cola: `play`, `prefetch_stream`, `play_index`, `remove_from_queue`, `clear_queued`, `add_to_queue`, `move_in_queue`, `play_next`, `next_track`, `prev_track`, `toggle_shuffle`, `set_repeat`, `set_stop_after_current`, `toggle_pause`, `seek`, `set_volume`, `set_playback_params`, `get_queue`, `get_playback`.
- Biblioteca/edición: `playlist_index`, `sync_playlist_index`, `play_counts`, `listening_stats`, `play_playlist`, `start_radio`, `radio_stations`, `play_radio_station`, `export_playlist_file`, `import_playlist_file`, `rate`, `set_album_saved`, `add_to_playlist`, `add_to_local_playlist`, `remove_from_playlist`, `create_playlist`, `edit_playlist_details`, `set_playlist_cover`, `set_playlist_sort`, `delete_playlist`, `subscribe`, `get_local_library`, `add_local_folder`, `remove_local_folder`.
- Preferencias/cuenta/host: `get_settings`, `set_setting`, `discord_status`, `ryoku_theme_tokens`, `get_personal`, `set_personal`, `get_stream_clients`, `clear_caches`, `get_account`, `get_account_identities`, `switch_account`, `sign_out`, `login_webview`, `frontend_ready`, `open_mini`, `close_mini`, `open_external`.
- Integraciones y operación expuestas por Tauri: `lt_get_state`, `lt_set_server_url`, `lt_create_room`, `lt_join_room`, `lt_leave`, `lt_approve_join`, `lt_reject_join`, `lt_kick`, `lt_transfer_host`, `lt_suggest`, `lt_approve_suggestion`, `lt_reject_suggestion`, `lt_request_sync`, `get_lyrics`, `lastfm_connect`, `lastfm_disconnect`, `lastfm_status`, `check_for_updates`, `install_update`.

**Superficie adicional usada por el cliente Linux QML/daemon** (no asumir que existe en Tauri Windows; cada capacidad requiere equivalente en el host Windows):

| Grupo | Métodos/eventos Linux adicionales relevantes | Superficie Linux | Trabajo de paridad Windows |
| --- | --- | --- | --- |
| Proveedores | Métodos `set_provider`, `get_provider`, `spotify_status`, `spotify_sign_in`, `spotify_sign_out`, `soundcloud_status`, `soundcloud_sign_in`, `soundcloud_sign_out`, `tidal_status`, `tidal_sign_in`, `tidal_sign_out`, `tidal_set_credentials`; eventos `provider-changed`, `spotify-auth`, `soundcloud-auth`, `tidal-auth` | `Playback.qml`, `TitleBar.qml`, `SettingsPage.qml`, flujo de selector/login en QML | Agregar contrato/handlers y UI equivalente a Tauri; conservar OAuth/browser/device flow, estados de error y restricciones por proveedor |
| Efectos y waveform | `get_audio_fx`, `set_audio_fx`, `get_waveform`; evento `audio-fx` | `SoundDialog.qml`, `Playback.qml` y waveform de SoundCloud | Exponer controles y eco de estado; probar efectos reales con build Windows de libmpv y waveform para SoundCloud |
| Descargas | `get_download_settings`, `set_download_settings`, `get_downloads`, `enqueue_download`, `enqueue_collection`, `cancel_download`, `retry_download`, `clear_download_history`, `open_download`, `open_download_folder`; evento `downloads-changed` | `DownloadsPage.qml`, `DownloadSettings.qml`, `DownloadButton.qml`, `Downloads.qml` | Tauri Windows necesita API, UI y worker/proceso Windows equivalentes; validar que cierre de ventana no detiene jobs y que cancelación termina árbol de procesos |
| Diagnóstico | `get_logs`, `log_client_error`, `open_logs_folder` | `DiagnosticsPage.qml`, `Logs.qml` | Añadir diagnóstico/logs locales con rotación y redacción; probar export/abrir ubicación y ausencia de secretos |
| Perfil de SoundCloud | `get_sc_user` | `ArtistPage.qml` y utilidades de SoundCloud | Conservar metadata y navegación de artista, o mapear a endpoint equivalente con las mismas reglas de error |
| Lifecycle y sincronización | `hello`, `subscribe`, `show`, `quit`, `sign_in`; snapshot incluye `audioFx`, `provider`, `spotify`, `soundcloud`, `tidal`, `downloads` además de playback/settings/auth | `Daemon.qml`, `Playback.qml`, `shell.qml`, daemon y tray | Tauri Windows mantiene single-instance/Show, snapshot/resync y playback/download supervivientes al cerrar ventana; adaptar lifecycle, no reutilizar socket Unix |

**Eventos de referencia:** el reducer QML `client/lib/playback.js` maneja `now-playing`, `rating`, `queue-changed`, `queue-index`, `position`, `duration`, `playback-state`, `stop-after-current`, `volume`, `audio-fx`, `provider-changed`, `spotify-auth`, `tidal-auth`, `soundcloud-auth`, `playback-error`, `playback-notice`, `cover-error`, `auth-changed`, `lt-state` y `lt-notice`. Otros consumidores QML manejan `personal-changed`, `library-changed`, `downloads-changed` y `show`; `Daemon.qml` re-suscribe y vuelve a aplicar snapshots tras reconexión. En Svelte/Tauri, los listeners de `api.ts` no cubren los eventos específicos de proveedor/descargas de la superficie QML. Crear una tabla payload-origen-consumidor-prueba por evento al implementar cada contrato.

**Diferencias/gaps observados, sin resolverlos por suposición:**

1. `ui/src/lib/api.ts` y los comandos Tauri no exponen los métodos de proveedor, descargas, diagnóstico ni waveform listados arriba, aunque sí forman parte del cliente Linux actual.
2. Import/export, pickers de carpeta/portada y acceso a archivos usan contratos distintos entre Tauri (diálogo nativo en el comando) y daemon/QML (ruta explícita). Windows debe preservar selección explícita del usuario con picker nativo y no abrir acceso irrestricto a paths desde WebView.
3. `ryoku_theme_tokens` es una integración Linux/Ryoku; QML obtiene tokens directamente. Windows necesita conservar tema/skins y preferencias con fuente de tokens Windows, no el acoplamiento Ryoku.
4. Linux usa `xdg-open`, GTK/WebKit oculto, socket Unix, Quickshell, systemd y MPRIS/tray; Windows requiere browser/WebView2, transporte/lifecycle Tauri, autostart y controles Windows. No son funciones que se puedan quitar.
5. La lista de comandos/eventos es contrato estático; no demuestra que el target Windows compile ni que los flujos funcionen. El estado de lo específico de Windows sigue sujeto a sus gates de build/runtime; W20 y W21 tienen evidencia runtime registrada, mientras W13, W15, W22–W25 y W45 siguen pendientes/parciales.
6. Qt/QML como tecnología puede ejecutarse en Windows con un host Qt propio, pero Quickshell no es un host Windows viable para este cliente. Crear un host Qt nuevo duplicaría el host existente sin aportar ventaja clara; queda fuera de alcance.

**Estado W04:** matriz estática completada como baseline de planificación; W14 completó cotejo e inventario de métodos/eventos y añadió tests contractuales. En Ubuntu 26.04 WSL se instalaron temporalmente Rust/Cargo 1.93.1 y dependencias GTK/WebKitGTK/libmpv; `cargo build --locked -p ryotunesd -p ryotunes-cli` pasó. Se instaló Quickshell 0.3.2 desde `ppa:avengemedia/danklinux`, pero no se pudo arrancar `client/shell.qml`: checkout carece de `Ryoku.Ui.Singletons`. Al terminar la prueba se retiraron Quickshell, PPA, toolchain, dependencias de desarrollo y 427 paquetes automáticos huérfanos; no se eliminó Ubuntu WSL ni el cache Cargo externo `/home/isaac/.cache/ryotune-omni-target`. La prueba socket daemon aislado original mostró que `subscribe({channelId,subscribed:true})` devolvía snapshot sin confirmar cambio remoto; Spotify respondió `spotify_signed_out` porque no había sesión. Después, PR #4 corrigió la colisión con `set_channel_subscribed`/`subscribe_events` y la respuesta `true` de Spotify. Los tests de `ryotunesd` cubren parámetros, resultado de la rama Spotify, errores y delegación del dispatch (`cargo test --locked -p ryotunesd`: 69 pasaron, 1 ignorado); no prueban escrituras remotas ni sesión real. W04 permanece parcial hasta validar ambos flujos autenticados en runtime.

**Siguiente responsable — Brayan, desde su distro Ryoku:** usar checkout actualizado y cuentas de prueba propias, sin compartir credenciales ni cookies; registrar commit, distro y resultados sin datos sensibles. (1) Ejecutar daemon y `qs -p client` con el módulo Ryoku disponible; en ArtistPage probar suscribir y desuscribir un artista y confirmar cada cambio en la cuenta YouTube. (2) Iniciar Spotify desde Settings, completar OAuth en navegador y comprobar estado autenticado; añadir un track de prueba a playlist de prueba, confirmar que aparece remotamente y registrar valor/error devuelto por `add_to_playlist`. (3) Adjuntar evidencia reproducible (pasos, resultado y logs redactados) y dejar W04 parcial hasta comprobar ambos flujos. Los defectos de dispatch ya se corrigieron en PR #4; reportar cualquier regresión nueva con reproducción mínima. El resultado de WSL no implica paridad Windows.

**Salida de fase:** alcance y ruta de integración aprobados. No empezar refactor amplio antes de separar publicación de CI.

### Fase 1 — Build reproducible y auditoría por target

| Estado / ID | Responsable | Tarea | Aceptación |
| --- | --- | --- | --- |
| ◐ W10 | @Excentry | Resolver grafo Cargo Windows por target/features — **inventario estático hecho; check MSVC bloqueado por recurso ausente** | Paquetes activos, build dependencies, APIs Unix y DLL requeridas documentadas; [inventario W10](W10-CARGO-WINDOWS-INVENTORY.md) registra el intento de build y el bloqueo |
| ☐ W11 | @Excentry | Definir entorno MSVC x64 en Windows 11 | Versiones Rust/Node/pnpm/VS C++/SDK registradas; pasos reproducibles desde máquina limpia |
| ☑ W12 | @Excentry | Auditar libmpv Windows — **candidato/fuentes emparejados y entrega futura documentada; sin promoción ni publicación** | Run `39901704769` pasó build, comprobación Windows de igualdad byte-a-byte del source bundle, smoke y regresión MSVC; DLL SHA-256 `a63580d6…5210c1`, source archive SHA-256 `9733a763…adc0f47`. La distribución futura se propone como companion source asset de la misma GitHub Release, nunca como artifact temporal de Actions. Se conserva la limitación técnica de mapeo exacto de 101 miembros GCC-family, sin infracción confirmada ni afirmación de clearance legal. |
| ☑ W13 | @IsaVonxz-type | Ejecutar baseline Svelte/Tauri — **hecha en checkout local** | `pnpm check`, build y nueve checks puros pasaron; toolchain/resultados registrados abajo. No equivale a validar Tauri en runtime Windows |
| ☑ W14 | @IsaVonxz-type | Comparar comandos/eventos Tauri con daemon — **inventario y tests contractuales Linux completados** | Matriz por método/evento y tests Linux registrados en [el apéndice W14](CONTRACT-MATRIX.md); integración UI-host y runtime Windows siguen como gates W21/W34 |
| ☐ W15 | @Excentry | Añadir CI Windows no-publicadora | Compila solo paquetes seleccionados; permisos de workflow mínimos; no firma ni publica |

#### W13 — Baseline local ejecutada

- Entorno: Windows (`win32`), Node `v24.12.0`, pnpm `12.8.1`, Rust `1.94.0`; el repo/CI documenta Node 22 y pnpm 11, así que esta ejecución no sustituye la baseline exacta de CI.
- Dependencias frontend principales resueltas: Svelte `5.56.4`, SvelteKit `2.70.3`, Vite `8.1.3`, TypeScript `6.0.3`, Tauri API `2.11.1`, plugin-dialog `2.7.2`.
- `pnpm check` (desde `ui/`): **PASS**, 0 errores, 0 warnings.
- `pnpm build` (desde `ui/`): **PASS**, adapter-static escribió `ui/build`.
- Nueve checks puros (desde raíz, Node `--experimental-strip-types`): `dnd`, `localsearch`, `menu`, `personal`, `queue`, `rows`, `shortcut-match`, `sort`, `ytlink`: **PASS**.
- `git diff --check`: **PASS**. `git status --short` conservó solo `?? AGENTS.md`; no aparecieron cambios versionados ni artefactos nuevos sin rastrear.
- No se probó Tauri/WebView2 instalado ni IPC/runtime; corresponde a W21. W13 valida el baseline frontend y sus checks, no compatibilidad Windows completa.

#### W14 — Cotejo estático Tauri/daemon

Matriz exhaustiva de comandos, parámetros, resultados, errores y eventos (payload/productor/consumidor/prueba): [apéndice W14](CONTRACT-MATRIX.md).

- Inventario mecánico: `ui/src/lib/api.ts` contiene **96 invocaciones**; todas tienen handler `#[tauri::command]`. `commands.rs` añade `lt_suggest` sin wrapper en ese archivo; registro total 97. Hay 92 arms daemon con nombre exacto, cuatro nombres agrupados en un arm que responde `client_side`, `ryoku_theme_tokens` responde también `client_side`, y `subscribe` tiene contrato semánticamente distinto. Ver conteo y firmas completas en el apéndice.
- Métodos comunes cotejados por área (los nombres son exactos; parámetros posicionales Rust se exponen como camelCase según `api.ts`): búsqueda/paginación `search`, `search_page`, `search_page_more`, `search_all`, `search_all_more`, `search_cards`, `search_cards_page`, `search_cards_more`; playback/cola `play`, `prefetch_stream`, `play_index`, `remove_from_queue`, `move_in_queue`, `play_next`, `add_to_queue`, `clear_queued`, `next_track`, `prev_track`, `toggle_shuffle`, `set_repeat`, `set_stop_after_current`, `toggle_pause`, `seek`, `set_volume`, `set_playback_params`, `get_queue`, `get_playback`; settings/estado `get_settings`, `set_setting`, `discord_status`, `get_stream_clients`, `clear_caches`, `get_personal`, `set_personal`; cuenta `get_account`, `get_account_identities`, `switch_account`, `sign_out`; catálogo `get_home`, `get_home_more`, `get_library`, `get_library_albums`, `get_library_artists`, `get_playlist`, `get_playlist_more`, `playlist_index`, `sync_playlist_index`, `play_counts`, `listening_stats`, `get_album`, `get_artist`, `get_browse_grid`; reproducción de colecciones/radio `play_playlist`, `start_radio`, `radio_stations`, `play_radio_station`; transferencia `export_playlist_file`, `import_playlist_file`; biblioteca local `get_local_library`, `add_local_folder`, `remove_local_folder`; escrituras `rate`, `add_to_playlist`, `add_to_local_playlist`, `remove_from_playlist`, `create_playlist`, `edit_playlist_details`, `set_playlist_cover`, `delete_playlist`, `subscribe`, `set_album_saved`; letras/Last.fm `get_lyrics`, `lastfm_status`, `lastfm_connect`, `lastfm_disconnect`; Listen Together `lt_get_state`, `lt_set_server_url`, `lt_create_room`, `lt_join_room`, `lt_leave`, `lt_approve_join`, `lt_reject_join`, `lt_kick`, `lt_transfer_host`, `lt_approve_suggestion`, `lt_reject_suggestion`, `lt_request_sync`; Tauri/Ryoku `ryoku_theme_tokens`.
- Para los métodos comunes, las formas de argumentos/resultados están tipadas en los wrappers `invoke<T>(name, params)` de `ui/src/lib/api.ts`; daemon deserializa los mismos nombres camelCase mediante `arg<T>`. Resultado común es el tipo `T` indicado por el wrapper (o `null` wire para `void`). Los errores de dominio son `Result<T,String>` en Tauri frente a `{code,message}` en daemon; errores de deserialización daemon usan `bad_params`. Los 92 arms con nombre coincidente no garantizan equivalencia semántica; excepciones y candidatos están desglosados en el apéndice.
- Comandos propios del daemon necesarios para paridad Linux y no registrados en Tauri: `hello`, `subscribe`, `show`, `quit`; APIs de providers (`set_provider`, `get_provider`, `*_status`, `*_sign_in`, `*_sign_out`, `tidal_set_credentials`); efectos/waveform (`get_audio_fx`, `set_audio_fx`, `get_waveform`, `get_sc_user`); descargas (`get/set_download_settings`, `get_downloads`, `enqueue_download`, `enqueue_collection`, `cancel_download`, `retry_download`, `clear_download_history`, `open_download`, `open_download_folder`); diagnóstico (`get_logs`, `log_client_error`, `open_logs_folder`). No son gaps de compilación del daemon: son funciones a exponer en el host Windows.
- Diferencias de parámetros por picker: `add_local_folder` en Tauri abre picker y no recibe `path`; daemon exige `path`. `import_playlist_file` en Tauri no recibe parámetros y devuelve cancelación como `null`; daemon exige `path`. `export_playlist_file` en Tauri recibe `title/items` y abre save picker; daemon además exige `path`. `set_playlist_cover` en Tauri recibe `playlistId/pick`; daemon exige también `path` cuando `pick=true`. El contrato Windows debe usar picker nativo y validar la selección; no permitir que WebView envíe paths arbitrarios. `edit_playlist_details` recibe `playlistId` y campos opcionales planos `name/description/public` en ambos hosts: wrapper Svelte aplana `changes` al construir params.
- `subscribe` Tauri recibe `{channelId, subscribed}` para escribir la suscripción; el antiguo `subscribe` del daemon solo activaba el stream y devolvía snapshot. Corregido en PR #4: `set_channel_subscribed` realiza la escritura, `subscribe_events` registra el stream y `subscribe` conserva alias de compatibilidad. Tests de parámetros/errores; falta confirmar escritura remota con sesión YouTube en runtime Linux.
- `add_to_playlist` Tauri anuncia `boolean`; la antigua rama Spotify del daemon terminaba con `null`. Corregido en PR #4: éxito Spotify devuelve `true`; la ruta de producción completa se prueba con escritor fake y test de regresión de delegación del dispatch. Falta validar una escritura remota con sesión Spotify; errores de dominio siguen siendo errores, nunca `false`.
- Diferencias condicionadas por provider: varios métodos con misma forma seleccionan YouTube/Spotify/SoundCloud/TIDAL en daemon (`search_page`, `search_all`, `search_cards`, `get_home`, biblioteca, álbum/artista, `rate` y escrituras). Tauri tiene handlers de proveedor adicionales o menores y no todos mantienen el mismo comportamiento multifuente. Registrar estas como diferencias de backend, no de wire schema.
- Códigos de error: `invoke` Tauri rechaza con strings; socket daemon responde `{ code, message }`. El adaptador de frontend Windows debe normalizar los errores sin perder mensaje/categoría; probar bad params, proveedor desconectado, archivo inexistente y cancelación de diálogo.
- Snapshots: Tauri obtiene estado con handshake `frontend_ready` y comandos separados (`get_playback`, `get_queue`, `get_settings`, `get_account`). `subscribe` del daemon entrega esos datos juntos y además `audioFx`, provider/status de Spotify/SoundCloud/TIDAL y downloads. Windows necesita una sincronización de primera carga y reanudación que incluya todos los estados equivalentes.
- Eventos: payloads tipados de Tauri están declarados junto a listeners de `api.ts`: `now-playing: NowPlaying`; `rating: {videoId,rating}`; `queue-changed: QueueState`; `queue-index: {currentIndex,playedFrom?,shuffle?,repeat?,sourceName?,current}`; `position:{position:number}`; `duration:{duration:number}`; `volume:number`; `playback-state:'playing'|'paused'`; `stop-after-current:boolean`; errores/notices/cover `{message}`; `auth-changed:Account`; `account-selection-required` y `login-done` sin payload usado; `local-changed:{removed:string[]}`; `login-error:string`; `ryoku-theme-changed:RyokuThemeTokens`; `lastfm-state:LastfmState`; `lt-state:LtState`; `lt-notice:string`. Los tipos son contrato de consumo, no exhaustividad de productores. Daemon agrega eventos no expuestos por Tauri: `audio-fx` (AudioFx), `provider-changed:{provider}`, `spotify-auth`/`soundcloud-auth`/`tidal-auth` (máquinas de estado con URL/código/name/message), `personal-changed` (blob), `library-changed:{id}`, `downloads-changed` (snapshot), `show` (activación); verificar cada payload con productor antes de implementar bridge Windows. `playback-error` puede incluir `videoId` adicional; el consumidor tipado lee `message`. `ryoku-theme-changed` es integración Tauri/Ryoku; Windows debe sustituirla por fuente de tema propia.
- Los nombres no bastan para cerrar contrato: usar `ui/src/lib/api.ts`/`commands.rs` como schema Tauri y `methods.rs` como schema RPC; implementar tests de contrato para parámetros/resultados y payloads en W21/W34 antes de declarar integración.

**Inventario explícito de eventos (nombre contractual):**

| Canal | Nombres | Nota de compatibilidad |
| --- | --- | --- |
| Tauri `api.ts` listeners | `now-playing`, `rating`, `queue-changed`, `queue-index`, `position`, `duration`, `volume`, `playback-state`, `stop-after-current`, `playback-error`, `playback-notice`, `cover-error`, `auth-changed`, `account-selection-required`, `local-changed`, `login-error`, `login-done`, `ryoku-theme-changed`, `lastfm-state`, `lt-state`, `lt-notice` | Contrato tipado de referencia; payloads definidos junto a cada listener en `ui/src/lib/api.ts`. |
| Consumidos por QML/daemon además de ese listado | `audio-fx`, `provider-changed`, `spotify-auth`, `tidal-auth`, `soundcloud-auth`, `personal-changed`, `library-changed`, `downloads-changed`, `show` | Mantener cada flujo/actualización visible en Windows; cada evento debe tener consumidor o mecanismo equivalente documentado. |

**Estado W14:** inventario estático y tests contractuales/IPC en Linux completados. Los defectos de contrato de `subscribe` y `add_to_playlist` están corregidos y cubiertos por tests; la confirmación de escrituras remotas autenticadas permanece pendiente en W04. W14 no afirma compatibilidad ni runtime Windows; los contratos UI-host/fixtures y runtime Windows siguen como gates W21/W34.

**Salida:** build repetible, limitaciones demostradas, y no hay dependencias nativas críticas desconocidas.

### Fase 2 — Prototipos de viabilidad

| Estado / ID | Responsable | Tarea | Mínimo de aceptación |
| --- | --- | --- | --- |
| ☑ W20 | @Excentry | Prueba libmpv Windows — **runtime con DLL candidata aislada verificado** | Audio local, pause/resume, seek, volume, EOF, stop/Quit, ruta/hash y dependencias probados; evidencia y límites en `LIBMPV-CANDIDATE-VALIDATION.md` |
| ☑ W21 | @IsaVonxz-type | Prueba Tauri/WebView2 y configuración por plataforma — **criterios runtime verificados en Windows 11 con candidato aislado** | UI instalada, IPC exitoso/rechazado, evento, SMTC/controles incl. seek, fallo fail-soft sin HWND, hide/reopen, Quit y reinicio sin procesos residuales; evidencia y límites registrados abajo |
| ☑ W22 | @IsaVonxz-type | Probar bridge JavaScript y login YouTube — **login, cancelación, timeout con toast y reintento verificados en la app real (WebView2); cookies verificadas por nombre y llamada autenticada; hallazgo abierto: el primer login con 2FA falla por redirect a `accounts.google.com.co` rechazado por la allowlist (issue #25, @Excentry); mitigación de reintento en el host sin verificar en runtime** | Harness correcto; timeout, cancelación, cookies esperadas y fallo recuperable demostrados |
| ◐ W23 | @Excentry | Comparar transporte Spotify — **Windows actual rechaza FIFO Unix; Spotify se recomienda aplazado hasta medir prototipos** | Named pipe vs sink/alternativa; latencia, pausa, seek, backpressure, EOF, cancelación y handles medidos |
| ☐ W24 | Ambos | Validar el host integrado Tauri acordado | Prueba demuestra continuidad al ocultar/cerrar ventana, Quit limpio, single-instance y dueño único de audio/datos; reabrir arquitectura solo si falla |
| ◐ W25 | @Excentry | Construir prototipo ejecutable — **candidato instalado inicia fuera del checkout y carga DLL aislada; logs/clean-checkout pendientes** | Ejecuta fuera del shell de desarrollo, con dependencias nativas descubiertas y logs útiles |
| ◐ W26 | @IsaVonxz-type | Elaborar baseline visual Ryotunes — **inventario estático inicial** | [Baseline y diferencias por validar](W26-VISUAL-BASELINE.md); falta comparación con capturas/runtimes equivalentes para cerrar paridad, sin rediseño accidental |

**Salida:** reproducción local en runtime, WebView funcional y decisión explícita para Spotify. Si Spotify se aplaza, publicarlo como limitación aceptada; no marcarlo como portado.

#### W21 — Pruebas Tauri/WebView2 Windows (estado propio)

- Entorno detectado: Rust `1.94.0`, target `x86_64-pc-windows-msvc`; Visual Studio 2022 Build Tools con MSVC `14.44.35207`; Windows SDK `10.0.26100.0`; WebView2 Runtime `155.0.4283.62`.
- Auditoría de este cambio confirmó el panic: `crates/core/src/media.rs` pasaba `hwnd: None` y souvlaki 0.8.3 hace `expect` porque el campo es obligatorio en Windows. Ahora `src-tauri/src/lib.rs` obtiene `main.hwnd()` en setup, lo reduce a un valor opaco no nulo y lo entrega al thread que crea/posee `MediaControls`; si no hay handle, ese backend opcional se desactiva sin panic. El thread conserva el owner de SMTC; no hay mutex de HWND. En Windows el cierre normal oculta `main`, y Quit espera `AppState::shutdown_for_quit`/`MediaHandle::shutdown` antes de `app.exit`; el host Windows no recrea `main`.
- Verificación portable ejecutada ahora: `cargo test --locked -p ryotunes-core media::tests` (Linux 1/1: nulo rechazado y valor opaco no nulo preservado), `cargo test --locked -p ryotunes-core` (159 pass, 2 ignored), `cargo test --locked -p ryotunesd` (70 pass, 1 ignored), `cargo test --locked -p ryotunes-protocol` (10 pass), `cargo fmt --all -- --check` y `cargo build --locked -p ryotunes --package ryotunesd` (Linux pass; warning preexistente `unused variable: win`). El test cfg Windows que comprueba que HWND ausente desactiva SMTC está añadido pero no ejecutado; ninguna prueba portable valida un HWND real, COM/SMTC ni WebView2.
- El flujo Windows actual es Tauri integrado: Svelte `invoke`/eventos → `src-tauri/src/commands.rs`/`TauriSink` → `AppState` en proceso. `crates/protocol` y `ryotunesd` usan Unix sockets y no están en el lanzamiento Windows. Wry/Tauri aporta WebView2; no hay API WebView2 propia ni harness de integración WebView2/invoke en este repo.
- `pnpm check`, `pnpm build` y `cargo fmt --all -- --check`: **PASS**. `cargo test --locked -p ryotunes-core media::tests -- --nocapture`: **1/1 PASS** en Windows.
- `cargo build --locked -p ryotunes --bin ryotune-omni`: **PASS** en Windows x64, con MSVC `14.44.35207`, usando el recurso ignorado local `src-tauri/libmpv-2.dll` del artefacto oficial de CI de mpv `libmpv-v0.41.0-dev-ga43ab319f-39541010475-x86_64-w64-mingw32-gpl.zip`. SHA-256 del archivo `7f63ce43997b0c0d3d16465162200a4cdafcc21f73a122ba3a75b55032c64b9f`; DLL SHA-256 `5816ca7aeab4d07774f3b26c465ee6691c41660ba7288381949413c9844bcdf7`. El artefacto es PE x64 Windows, sin dependencias DLL externas al sistema; el import library `libmpv.dll.a`, renombrado a `mpv.lib` en Temp, enlaza con MSVC. El release identifica esta variante como GPLv3 y el build oficial activa `rubberband`, necesario para pitch. El ZIP incluye `LICENSE.GPL` en texto GPLv2, no basta para documentar el binario combinado GPLv3. La release estable `v0.41.0` publica ejecutables mpv Windows pero no una DLL libmpv; esta variante dev queda seleccionada como objetivo Windows, con descarga/hash, fuentes correspondientes y avisos completos como gate obligatorio antes de distribuir.
- `scripts/windows/prepare-libmpv.ps1` fija la URL y verifica SHA-256 del ZIP, DLL e import library; extrae el `.a` como `mpv.lib`, coloca la DLL y la licencia upstream en rutas consumidas por el bundle. Para compilar desde PowerShell, ejecutar el script y luego `$env:LIB = "$(Resolve-Path .libmpv\v0.41.0-dev-ga43ab319f-x64-gpl);$env:LIB"`. El Windows bundle declara GPL-3.0-or-later e incluye `LICENSE`, el texto GPL upstream y aviso de procedencia.
- `scripts/windows/collect-libmpv-sources.ps1` recopila y verifica fuentes mpv y FFmpeg de los commits registrados por el run. [Auditoría de fuentes](LIBMPV-SOURCE-AUDIT.md) registra además digest de contenedor y versiones visibles del run.
- Límite de redistribución: la colección es parcial. El run clona LuaJIT `v2.1`, libsixel y subrandr desde referencias móviles y usa dependencias estáticas del contenedor sin adjuntar manifiesto de fuentes/licencias. Falta un paquete completo de fuentes correspondientes e inventario/avisos transitivos; no publicar un instalador hasta cerrar ese gate.
- Smoke separado con `APPDATA`/`LOCALAPPDATA` en perfil temporal y la DLL oficial: proceso Windows responde y Tauri/WebView2 carga Svelte. La búsqueda UI `Daft Punk` y hide/reopen se probaron en el smoke anterior con el artefacto tercero, no se repitieron con este binario. No se ejecutó desde instalador ni VM limpia; no se probó invoke rechazado, eventos de control, SMTC/medios ni Quit explícito.
- Entorno registrado: Windows 11 Pro build `26300`, Rust `1.94.0`, Node `v24.12.0`, pnpm `12.11.2`, Visual Studio `17.14.37710.0`, WebView2 Runtime `155.0.4283.62`, target MSVC instalado. PR #11 reemplaza el wiring local `spawn_with_hwnd` por `MediaWindowHandle` no nulo y `media::spawn`; el attach/controles SMTC con HWND real aún no se verificaron con esa implementación.
- `cargo tauri build --bundles msi,nsis -- --locked` con la ruta del import library en `LIB`: **PASS** también tras integrar PR #11; generó MSI y NSIS locales. `cargo test --locked -p ryotunes-core media::tests -- --nocapture`: **2/2 PASS** en Windows con el wiring integrado. Sin esa ruta falla con `LNK1181: mpv.lib`. La DLL como recurso de bundle no añade la ruta del import library al linker. El run CI [39878029733](https://github.com/Excentry/Ryotunes-Omni/actions/runs/39878029733) instaló/desinstaló bundles de prueba reconstruidos con la DLL candidata y verificó el hash; no ejecutó la UI instalada ni los instaladores locales congelados.
- Verificación de paquetes tras el merge: extracción MSI/NSIS con 7-Zip correcta; DLL contenida en ambos coincide con SHA-256 `5816ca7aeab4d07774f3b26c465ee6691c41660ba7288381949413c9844bcdf7`. El ejecutable extraído de NSIS arranca y muestra Svelte en WebView2 desde fuera del checkout. Windows expone la sesión SMTC `ryotune-omni.exe` y metadatos; solicitudes dirigidas `TryPlayAsync`/`TryPauseAsync` verificaron transiciones a `Playing` y `Paused`. La reproducción quedó pausada. No se probó audio de forma auditiva, next/previous, seek externo ni Quit/reinicio en este smoke.
- La extracción local no ejecuta el instalador ni verifica registro, shortcuts, upgrade o desinstalación. CI probó instalación/desinstalación en runner efímero, pero no los instaladores locales congelados ni un perfil interactivo Windows 11; el test tampoco arranca la app. El smoke usa el perfil Windows actual: Tauri resuelve datos mediante Known Folders; cambiar `APPDATA`/`LOCALAPPDATA` no demuestra aislamiento.
- Validación runtime aislada posterior, Windows 11 Pro x64 build `26300.9711`, WebView2 `155.0.4283.62`, Rust `1.94.0`, Node `v24.12.0`, pnpm `12.8.1`: `cargo tauri build --config $env:TEMP\opencode\tauri-w21-validation.json --bundles nsis -- --locked` pasó en worktree `ryotune-w21-8f558b1`; el instalador se instaló en `%TEMP%\opencode\ryotune-w21-installed-8f558b1` bajo identificador/perfil `dev.excentry.ryotune-omni.w21-validation`. DLL instalada: SHA-256 `a63580d610114bd039f79c72e0bd99b57c3276df77d98a0774830035ea5210c1`; la DLL activa del checkout permaneció en `5816ca7aeab4d07774f3b26c465ee6691c41660ba7288381949413c9844bcdf7`. Build emitió solo warning no bloqueante `static BUILDING_MAIN is never used`.
- La app instalada abrió la UI Svelte en WebView2 (`tauri.localhost`, ventana respondió) y registró `OS media controls attached`. Con dos WAV sintéticos silenciosos de 45 s se confirmó `get_local_library` por IPC (3 canciones indexadas), `get_playback` (metadata y duración) y rechazo de `set_personal({ personal: null })` con `personal store must be a JSON object`, sin mutar datos personales. Un listener Tauri registrado en WebView2 recibió `playback-state` con payload `playing` al activar Play/Pause externo.
- Teclas multimedia Windows enviadas por `user32.keybd_event` cambiaron Play/Pause, Next y Previous; Next/Previous alternaron correctamente entre las dos pistas y Play/Pause dejó el contador detenido. El comando Tauri `seek({ position: 20 })` fijó posición en `20.000 s`; esto no verifica seek originado desde SMTC/Windows. No se hizo escucha audible porque las pistas son silencio sintético.
- La API Windows `GlobalSystemMediaTransportControlsSession` confirmó metadata independiente del app: título `W21 silent test track one`, artista `Ryotune validation`, álbum `W21 synthetic test set`; indicó seek habilitado. `TryChangePlaybackPositionAsync(200000000)` devolvió `true`; timeline SMTC mostró ~20.2 s y `get_playback` del app avanzó desde 20 s, confirmando seek externo absoluto.
- Para teardown, se cambió `close_to_tray=false` solo en el perfil aislado y se cerró la ventana. La rama Windows llamó `main_window::request_quit`, mismo helper que la opción tray `Quit`; desaparecieron proceso y sesión SMTC. El proceso hijo WebView2 permaneció ~8 s y ya no existía tras ~23 s; no se forzó terminación. El click exacto del item visual del menú tray no se automatizó.
- `cargo test --locked -p ryotunes-core media::tests::missing_main_window_handle_disables_smtc_without_panicking -- --nocapture`: **1/1 PASS en Windows**. La primera ejecución terminó `STATUS_DLL_NOT_FOUND` porque `LIB` resuelve el linker, no el loader; repetir con la carpeta candidata también en `PATH` permitió ejecutar el binario sin recompilar ni cambiar código. Warning no bloqueante: `arch_package` sin uso en tests.
- Tras Quit, se lanzó de nuevo el ejecutable instalado como PID `10836`; la ventana y una sesión SMTC del identificador aislado aparecieron. Un segundo cierre por `request_quit` terminó el proceso, el hijo WebView2 y la sesión SMTC; las tres ausencias se comprobaron tras 25 s. No se usó terminación forzada. El item visual `Quit` del menú tray no se pulsó; su callback estático apunta al mismo `request_quit` probado.
- Los criterios runtime W21 quedaron verificados en este Windows 11 y candidato aislado; no es una VM limpia ni certificación de distribución. Issue #18 se cierra con esta evidencia; no se promovió ni publicó el candidato. El artefacto oficial GPL queda seleccionado como dependencia objetivo de distribución Windows; su condición de dev build y los gates de fuentes/avisos siguen bloqueando W12 de @Excentry, no el resultado runtime W20.

#### W22 — Harness de bridge/login (estado propio)

- El bridge de autenticación ahora se prueba con `ui/src/lib/auth-bridge.check.ts`, usando transporte IPC/listeners inyectados: invocación `login_webview`, entrega de eventos de autenticación/error/fin y rechazo recuperable de IPC.
- Deadline absoluto de cinco minutos ahora incluye espera de eventos, lectura de cookies en el hilo principal y demoras de reintento; timeout cierra la ventana y devuelve `LoginError::Failed`. Error al despachar/leer cookies también cierra y falla de forma recuperable; cierre de ventana conserva cancelación silenciosa. Tests cubren callback exitoso/fallido/estancado junto a `login_webview.rs`, más timeout/cancelación/eventos y allowlist/cookies.
- `cargo test --locked -p ryotunes --lib login_webview::tests -- --nocapture`: **10/10 PASS en Windows**, con DLL candidata en `PATH`; incluye callback cookie exitoso/fallido/estancado, evento/cancelación, UA de WebView por plataforma, notificación de navegación rechazada y allowlist/jar sintético. Warning preexistente no bloqueante: `BUILDING_MAIN` sin uso en tests. No prueba WebView2 ni cookies reales.
- En la app instalada de prueba con cuenta autorizada, el usuario reportó que cerrar la ventana WebView2 canceló el primer intento y la app principal siguió viva. En un intento separado, la WebView quedó negra/abierta al esperar cinco minutos. Después, PID `21404` seguía vivo y Windows enumeró la ventana principal, pero UI Automation solo devolvió el árbol raíz vacío; no se observó ventana WebView2 de login en esa enumeración. La captura disponible estaba cubierta por otra app, así que no confirma el estado visual posterior.
- En un intento posterior con la misma app candidata, tras introducir credenciales la ventana de login quedó blanca. El usuario confirmó que la app principal seguía funcional y respondía, pero no se autenticó; PID `15672` corresponde al ejecutable CI aislado y seguía respondiendo al comprobarlo. El usuario cerró y volvió a abrir la app entre intentos; esto solo acredita recuperación por reinicio, no recuperación in-place del login.
- Tras reconstruir con UA nativo de WebView2 y el manejador de redirects bloqueados, el usuario reportó que el login se completó. En PID `15040` se observó que el proceso seguía respondiendo y que solo quedaba la ventana principal; no se inspeccionó contenido de cuenta, cookies ni credenciales. Es resultado manual reportado, no captura independiente del estado de autenticación.
- Retest de cancelación en runtime aislado: configuración con identificador distinto y perfil WebView2 temporal limpio, mismo código de login corregido y DLL candidata. Cerrar la X del WebView quitó la ventana de login, devolvió ventana principal y el proceso `23300` siguió respondiendo. No se introdujeron credenciales. La acción GUI fue seguida de verificación de postestado; no prueba timeout.
- En ese perfil limpio, el WebView permaneció en el formulario vacío de Google sin interacción durante el límite configurado de cinco minutos. Después desapareció la ventana de login, volvió la principal y PID `23300` siguió respondiendo. La observación de accesibilidad no capturó el toast `Sign-in timed out`; el mensaje visible de error queda sin verificar. Se volvió a abrir el login y se pudo cerrar otra vez, confirmando reintento in-place sin reiniciar la app.
- Retest aislado con timeout temporal de 15 s: build con identificador `dev.excentry.ryotune-omni.login-toast-short-20261023` y perfil `%TEMP%\opencode\pr21-login-toast-short-profile-20261023`, sin credenciales. En PID `11324`, el WebView desapareció, volvió la ventana principal y el proceso siguió respondiendo. Un monitor capturó 40 cuadros de pantalla completa inmediatamente después del cierre; no se observó el toast en los cuadros inspeccionados. Esta captura no demuestra que el toast nunca apareciera. El timeout/perfil temporales se restauraron en el código del worktree; el binario de prueba continúa usando timeout de 15 s.
- La corrección candidata conserva el UA Safari solo en hosts WebKit y deja Windows WebView2 con su UA nativo; una navegación que rechace la allowlist ahora finaliza con error genérico en vez de dejar el login esperando en blanco. La causa observada no quedó distinguida entre incompatibilidad del UA y redirect bloqueado; no se amplió la allowlist.
- `cargo fmt --all -- --check`, `cargo check --locked -p ryotunes --bin ryotune-omni` y los 10 tests anteriores pasan. El paquete raíz se descartó para pruebas porque incorporó la DLL activa `5816ca…`, no la candidata. MSI/NSIS se reconstruyeron en worktree aislado con `src-tauri/libmpv-2.dll` SHA-256 `a63580d610114bd039f79c72e0bd99b57c3276df77d98a0774830035ea5210c1`; login manual reportado exitoso en NSIS corregido. Se desinstaló para preparar el ciclo MSI y luego se restauró en `%TEMP%\opencode\pr21-login-installed-7990133`; DLL verificada. PID `15040` permanece cerrado; runtime-test PID `23300` usa identificador y perfil separados.
- El ciclo local MSI/NSIS combinado pasó en tercer intento con UAC aprobado: los dos instaladores desplegaron 13 archivos con DLL SHA-256 candidata; el script verificó registro, archivos de licencia y shortcuts y limpió ambos paquetes. El primer MSI del build raíz incorrecto se había rechazado por hash de DLL; dos intentos posteriores no iniciaron por UAC cancelado. NSIS corregido quedó restaurado en `%TEMP%\opencode\pr21-login-installed-7990133` con DLL verificada y sin lanzar. No se inspeccionaron ni registraron cookies/credenciales. El toast de timeout y las cookies se verificaron después (ver bullets siguientes). No usar cookies en logs ni artefactos.
- Verificación del toast de timeout en la app real (2026-10-24, HEAD `57b41c3` sin cambios de código): `cargo tauri build --no-bundle` con identificador aislado `dev.excentry.ryotune-omni.w22-toast-20261023` (config temporal fuera del repo, `beforeBuildCommand` vacío tras `pnpm build` manual), `LIB` hacia `.libmpv\v0.41.0-dev-ga43ab319f-x64-gpl`, DLL local `5816ca…`, Windows 11 Pro `26300`, WebView2 `155.0.4283.62`. Se lanzó el exe copiado fuera del checkout con `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222` y se leyó el DOM real de `tauri.localhost` por CDP. Con un `MutationObserver` instalado, se abrió el login desde el menú `Sign in` → `Sign in with Google`, sin introducir credenciales ni interactuar con la ventana de login, y se esperó el `LOGIN_TIMEOUT` de 300 s. A 300.7 s del clic `.ryo-toast` apareció con `data-kind="error"` y texto `sign-in failed: Sign-in timed out. Please try again.` (el prefijo viene de `LoginError` en `crates/core/src/host.rs`). La app siguió respondiendo. Cierre forzado con `Stop-Process`; a los 20 s no quedaban procesos de la app ni `msedgewebview2` del perfil. No se inspeccionaron cookies ni cuentas. El toast queda verificado en WebView2; la evidencia previa con IPC simulado en Chrome (cadena `startGoogleLogin` → `toast.error` → `+layout.svelte`) coincide. No se observó visualmente con captura, solo en el DOM; la causa de las capturas fallidas anteriores sigue sin confirmar (probable ventana de 2.5 s de `show()`).
- Verificación de cookies y cuenta en la app real (2026-10-24, mismo exe y perfil aislado, login manual del usuario con cuenta propia, observado por CDP imprimiendo solo claves, flags y nombres; ningún valor de cookie ni dato de cuenta). Primer intento, con 2FA por teléfono: `login-error` `sign-in failed: Sign-in redirected outside Google or YouTube. Please try again.`, `get_account` con `signedIn:false`, aunque el jar ya contenía `SAPISID` y `__Secure-*` en `.youtube.com`. Segundo intento, con la sesión de Google ya presente en el perfil: navegación `accounts.google.com/ServiceLogin` → `accounts.youtube.com/accounts` → `music.youtube.com/` (todo `https` y dentro de la allowlist), la ventana de login se cerró sola y `get_account` pasó a `signedIn:true` con una identidad (`get_account_identities`: 1), sin selector (`account-selection-required` no se disparó: no aplica a una cuenta de un solo canal; el caso multicanal queda sin probar). `get_library` autenticado respondió sin error (5 secciones). Cookies `.youtube.com` presentes por nombre: `SAPISID`, `SID`, `HSID`, `SSID`, `APISID`, `__Secure-1/3PAPISID`, `__Secure-1/3PSID`, `__Secure-1/3PSIDTS`, `__Secure-1/3PSIDRTS`, `__Secure-1/3PSIDCC`, `SIDCC`, `LOGIN_INFO`, `YSC`, `VISITOR_INFO1_LIVE`, entre otras. No se verificó el contenido del header cosechado ni que excluya cookies de `google.com`; eso lo cubre el test unitario `youtube_cookie_pairs` y no se repitió en runtime. No se observó el evento `login-done` ni el toast `Signed in` (el observador de eventos no estaba activo en el segundo intento).
- Hallazgo (no bloquea el criterio de W22): el primer login con 2FA falla. Reproducido el 2026-10-24 con las cookies del perfil aislado borradas y observando navegaciones por CDP (solo esquema, host y primer segmento de ruta): tras aprobar el 2FA, `accounts.google.com/CheckCookie` → `accounts.youtube.com/accounts` → **`accounts.google.com.co/accounts`**, rechazado por `allowed_login_navigation` (`crates/core/src/session.rs`), que solo acepta `google.com`/`.google.com`. La ventana se cierra y el usuario debe pulsar Sign in otra vez; el segundo intento, con sesión ya creada, va `ServiceLogin` → `accounts.youtube.com` → `music.youtube.com` y entra sin credenciales. El sufijo regional depende de la cuenta, así que no es exclusivo de un país. La corrección de la allowlist es de @Excentry: issue [#25](https://github.com/Excentry/Ryotunes-Omni/issues/25). No se amplió la allowlist.
- Mitigación del lado del host (`src-tauri/src/login_webview.rs`, sin commit): tras una navegación rechazada se reabre la ventana de login una vez, con el mismo deadline absoluto de 300 s; si el segundo intento también es rechazado, falla con el mismo error. Se registra con `tracing::warn!` el esquema y el host rechazados (sin ruta ni query). La allowlist no cambia. Verificado: `cargo fmt --all -- --check` y `cargo test --locked -p ryotunes --lib login_webview::tests` (11/11 PASS en Windows, con la DLL candidata en `PATH`, incluido el test nuevo de la decisión de reintento). No verificado en runtime: no se reconstruyó el exe ni se repitió el 2FA con esta mitigación; el test cubre la decisión de reintento, no la reapertura real de la ventana.
- Cierre forzado de la app de prueba con `Stop-Process`: sin procesos de la app ni `msedgewebview2` del perfil, puerto de depuración liberado.

#### W23 — Transporte Spotify Windows

- `crates/spotify/src/stream.rs` usa backend `pipe`, almacena bytes S16 en FIFO creada con `mkfifo` y devuelve error inmediato bajo `cfg(not(unix))`; el motor además referencia `XDG_RUNTIME_DIR`. Esto prueba que el transporte actual no está portado a Windows, no compara candidatos.
- `cargo test --locked -p ryotunes-spotify --lib stream::tests -- --nocapture` pasó 2/2 en Windows; son tests de URI/event translation, no miden un transporte ni implican streaming Spotify funcional en Windows.
- Recomendación provisional: Spotify queda aplazado/no soportado en Windows hasta que el owner de playback/native pruebe candidatos Windows. La limitación visible es que Spotify no reproduce en Windows; la creación del stream devuelve `Spotify streaming needs a Unix FIFO; this platform is unsupported` (no se verificó aún cómo lo presenta la UI). No anunciarlo como portado. W23 permanece parcial: faltan prototipos y mediciones comparables de latencia, pause/resume, seek, backpressure, EOF, cancelación y limpieza.

#### W25 — Ejecutable aislado y dependencias

- El NSIS instalado aislado de W21 se ejecutó desde `%TEMP%`, fuera del checkout/dev shell, y cargó `libmpv-2.dll` desde su propio directorio con hash registrado en `LIBMPV-CANDIDATE-VALIDATION.md`. La inspección de imports/dependencias del candidato y el build offline están en esa evidencia; stdout no se recopiló como paquete diagnóstico repetible.
- Startup, UI/WebView2, Quit y ausencia posterior de proceso/WebView2 se observaron; un log de runtime informa attach de media controls, sin credenciales/cookies. W25 sigue parcial: falta reproducir desde clean checkout/documentar setup integral y entregar logs útiles privacy-safe para fallos de dependencia/startup/shutdown. No implica release readiness.

### Fase 3 — Fronteras portables y contratos compartidos

| Estado / ID | Responsable | Tarea | Aceptación |
| --- | --- | --- | --- |
| ☐ W30 | @Excentry | Extraer servicios de negocio necesarios desde daemon | Core/servicio testeable sin GTK ni APIs Unix; Linux conserva comportamiento |
| ☐ W31 | @Excentry | Definir adaptadores de rutas, credenciales, media y procesos | Interfaz acotada con tests de éxito/error/cancelación; sin `cfg` disperso en lógica de negocio |
| ☐ W32 | @IsaVonxz-type | Adaptar host y comandos Tauri a contratos publicados | Cada comando está registrado, capability acotada y conectada a servicio real |
| ☐ W33 | @Excentry | Adaptar almacenamiento y directorios Windows | Datos/cache en directorio por usuario; ACL y migración comprobadas; fallos reportados |
| ☐ W34 | @IsaVonxz-type | Contratos UI-host y fixtures | Tests de errores, cargas y eventos; fixtures no se etiquetan como backend integrado |
| ☐ W35 | Ambos | Revisar compatibilidad Linux de extracciones | Suite afectada corre; owner registra checks y runtime no verificado |

**Regla:** primero merge de interfaz y pruebas del core; después PR del consumidor Tauri.

### Fase 4 — Corte vertical Alpha

| Estado / ID | Responsable | Tarea | Aceptación |
| --- | --- | --- | --- |
| ☐ W40 | @Excentry | Playback, cola y recuperación | Play/pause/seek/volume/next/error; no quedan procesos/handles después de Quit |
| ☐ W41 | @IsaVonxz-type | Recorrido YouTube en interfaz | Login, browse/search, reproducción, errores y restauración visual probados |
| ☐ W42 | @Excentry | Biblioteca local Windows | Unicode, espacios, directorios borrados y metadata; reescaneo no bloquea UI |
| ☐ W43 | @IsaVonxz-type | Ventanas, tray y segunda instancia | Cerrar/restaurar/relanzar evita segunda sesión de audio y conserva estados esperados |
| ☐ W44 | @Excentry | Diagnóstico/telemetría local | Logs rotados y exportables; sin credenciales, cookies ni URL firmadas |
| ☐ W45 | Ambos | Prueba Alpha en Windows 11 limpio | Instalación y recorrido end-to-end, commit/OS/toolchain y resultados registrados |

**Salida:** Alpha instalada sin herramientas del desarrollador; reproducción local y YouTube funcional; ventana puede cerrarse y reabrirse sin perder trabajo activo.

### Fase 5 — Paridad por bloques funcionales

Cada bloque se implementa de extremo a extremo y se revisa antes de iniciar otro. @Excentry domina backend; @IsaVonxz-type domina host/UI; revisión cruzada obligatoria.

| Estado / Orden | Bloque | @Excentry | @IsaVonxz-type | Criterio mínimo de cierre |
| --- | --- | --- | --- | --- |
| ☐ 1 | Proveedores | Cliente, sesión, audio y recuperación | Flujos de login, selector, estados y errores | Por proveedor comprometido: login/logout, catálogo, reproducción y fallo recuperable |
| ☐ 2 | Spotify | Transporte elegido, seek, pausa y EOF | UI de estado, controles y errores | Pruebas de audio y ciclo de vida pasan; Premium/restricciones documentadas |
| ☐ 3 | Descargas | Procesos Windows, cancelación de árbol, queue y publicación | Settings, progreso, cancelar/reintentar, empty/error states | Cerrar UI conserva jobs; cancelar no deja procesos; reinicio recupera el estado |
| ☐ 4 | Controles Windows | HWND, SMTC y ciclo de vida del owner | Tray, taskbar, mini-player y metadatos | Controles funcionan tras ocultar/restaurar; no se confunde taskbar con SMTC |
| ☐ 5 | Audio, letras e integraciones | Filtros, Last.fm/Discord/Listen Together comprometidos | Ajustes, superficies, seek y opt-in | Sin pérdida de estado; integración opcional falla aislada |
| ☐ 6 | Personalización UI | Contrato/formato de tokens si requiere backend | Skins, temas, presets y preferencias | Skin válida aplica/revierte; valores por defecto fieles a Ryotunes |

**Dependencia:** cada función de la matriz debe ser `Implementada`, `Aplazada` o `No soportada`, con prueba y evidencia en el registro de compatibilidad.

### Fase 6 — Diseño fiel y personalización segura

| Estado / ID | Responsable | Tarea | Aceptación |
| --- | --- | --- | --- |
| ☐ W60 | @IsaVonxz-type | Portar tokens y tema predeterminado Ryotunes | Jerarquía, artwork, tipografía y reproductor reconocibles respecto al baseline W26 |
| ☐ W61 | @IsaVonxz-type | Reutilizar/adaptar formato de skins | Colores y tipografías configurables; skin inválida revierte a default y no rompe startup |
| ☐ W62 | @IsaVonxz-type | Añadir densidad compacta/cómoda y presets UI | Estados persistentes, navegación accesible, compatibilidad con ventana mínima y DPI |
| ☐ W63 | Ambos | Definir estabilidad del contrato de personalización | Tokens separados de layout y estado; migración/versionado del formato documentados |
| ☐ W64 | @IsaVonxz-type | Evaluar editor libre de layout | ADR con casos de uso, accesibilidad, costo de pruebas y criterio de aplazamiento; no implementarlo sin aprobación |

### Fase 7 — Robustez, seguridad y rendimiento

| Estado / ID | Responsable | Tarea | Aceptación |
| --- | --- | --- | --- |
| ☐ W70 | @Excentry | Resistencia de servicios/procesos | Timeouts/cancelación; cargas acotadas; no quedan hijos en escenarios de fallo |
| ☐ W71 | @IsaVonxz-type | UI WebView2 en carga | p95 de búsqueda/controles, scroll, listas y artwork documentados en 1k/10k/50k |
| ☐ W72 | @Excentry | Test prolongado | 8 h en hardware acordado; memoria/handles/procesos sin crecimiento sostenido inexplicado |
| ☐ W73 | @IsaVonxz-type | Accesibilidad y ventanas Windows | DPI 100–200 %, monitores, teclado, Narrator, foco y restauración comprobados |
| ☐ W74 | @Excentry | Revisión seguridad/dependencias nativas | DLL search order, orígenes de binarios, ACL, secretos y tools revisados; críticos resueltos |
| ☐ W75 | Ambos | Actualización fallida y recuperación de datos | Reintento/rollback probado con estado anterior; pérdida de datos no aceptada explicada |

**Presupuestos de rendimiento pendientes:** controles locales p95 <100 ms; búsqueda p95 <100 ms y sin bloqueo perceptible del hilo UI; recursos absolutos después de baseline. Acordar hardware y método antes de medir W71; no bloquea el inicio de W10–W12.

### Fase 8 — Empaquetado, operación y release

| Estado / ID | Responsable | Tarea | Aceptación |
| --- | --- | --- | --- |
| ☐ W80 | @Excentry | Verificar `tauri.windows.conf.json` efectivo | NSIS/MSI no asumen compatibilidad; arrays de config merge revisados; recurso libmpv resuelto en instalador |
| ☐ W81 | @Excentry | Seleccionar formato inicial de paquete | Propuesta: NSIS por usuario; MSI solo si requisito justifica duplicar QA |
| ☐ W82 | @Excentry | Pipeline candidato Windows | CI produce artefacto no publicador, commit/version/hash y logs; permisos mínimos |
| ◐ W83 | @IsaVonxz-type | Ensayo instalar/actualizar/desinstalar — **test CI fortalecido; validación pendiente en VM Windows 11 limpia** | Script verifica registro, DLL, contenido completo del directorio aislado y shortcuts con target al ejecutable; sintaxis PowerShell pasa. Falta ejecutar en runner limpio y registrar identidad/hash/upgrade/datos de usuario |
| ◐ W84 | @Excentry | Firma, actualizador y procedencia | Identidad del fork, endpoint, Authenticode y verificación del paquete definidos; upstream no usado accidentalmente — **identidad y marca visible `Ryotunes Omni` implementadas en la UI, backend y metadatos Linux**; identidad técnica `dev.excentry.ryotune-omni`, binarios/socket/datos/paquete aislados de Ryotunes, gates de release actualizados y build Linux verificado (`5d30f76`; revisión de marca en progreso); **faltan**: endpoint del actualizador, Authenticode y verificación de paquete Windows/runtime Windows |
| ☐ W85 | @Excentry | SBOM y revisión de licencias | DLL, codecs, FFmpeg, yt-dlp, fuentes y skins inventariados; obligaciones atendidas antes de publicar |
| ☐ W86 | Ambos | Aprobación del release candidate | Artefacto exacto instalado y verificado por ambos; limitaciones y rollback documentados |

**Puerta de release:** ningún push rutinario debe crear versión/tag/release pública. Separar workflow de CI y workflow de publicación; publicar solo con aprobación explícita y permisos mínimos. Firma ausente significa build interno/no estable, no “firmado después”.

## 6. Validación y operación

### Checks por pull request

- Formato Rust y tests focalizados.
- `pnpm check`, `pnpm build` y nueve suites puras de frontend.
- Build Windows de los paquetes seleccionados; no usar `--all-features` sin motivo.
- Pruebas de core/contrato y regresiones Linux para cambios compartidos.
- Comprobación de permisos Tauri y registro de comandos al cambiar host.
- Chequeos de seguridad/licencia para nuevas DLL y ejecutables.
- No exponer secretos de firma ni permisos de publicación a PRs sin confianza.

### Checks de candidato

- Ejecución interactiva en Windows 11; WebView2 y audio real.
- Proveedores con cuentas autorizadas y limitaciones registradas.
- Prueba de carga prolongada, suspensión/reanudación y dispositivos de audio.
- Installer desde Windows limpio sin Rust/Node/mpv en PATH.
- Upgrade, rollback, uninstall y recuperación de estado.

Runner CI no reemplaza una máquina Windows 11 con sesión interactiva y dispositivos de audio. Si una prueba depende de eso, reportarla como **pendiente de laboratorio**, no como pass.

### Métricas requeridas

Registrar commit, build, Windows 11 edition/version, CPU/RAM, toolchain, WebView2, libmpv/DLL, proveedor/cuenta de prueba sin datos personales, configuración, estado de caché, repeticiones y resultado.

Medir el árbol completo: aplicación, WebView2 renderer/GPU, mpv y downloader. Working set/private bytes Windows no se comparan directamente con PSS Linux.

## 7. Riesgos, dependencias y decisiones abiertas

| Riesgo/decisión | Dueño | Resolver en | Condición de salida |
| --- | --- | --- | --- |
| Publicación Linux automática al push | @Excentry | Fase 0 | Pull requests/merges rutinarios no publican artefactos |
| Linkage/DLL y licencia de libmpv | @Excentry | Fases 1–2 | Build y redistribución reproducibles |
| Transporte Spotify no Unix | @Excentry | Fase 2 | Transporte probado o exclusión aprobada y visible |
| Bridge/login WebView2 | @IsaVonxz-type | Fase 2 | Flujo real de login probado, timeout y cancelación |
| Paridad entre métodos daemon/Tauri | @IsaVonxz-type | Fases 0–3 | Matriz contractual y funciones asignadas |
| Acceso SoundCloud/browser auth en Windows | @Excentry + @IsaVonxz-type | Fase 5 | Flujo soportado aprobado; no copiar import Linux sin análisis |
| Identidad/actualizador del fork | Ambos | Antes de distribución externa | Updater y publicación pública fuera de Alpha; antes de distribuir, identidad/ID de app/endpoint/licencias definidos y datos aislados de upstream |
| Scope de personalización | @IsaVonxz-type | Fases 0 y 6 | MVP de tokens y presets; editor libre aplazado por defecto |
| UI predeterminada conserva Ryotunes | @IsaVonxz-type | Fase 6 | Baseline visual comparado y aceptación de ambos |
| Presupuesto de rendimiento absoluto | Ambos | Fase 1/7 | Hardware de referencia y umbrales aprobados |

### Primer sprint de planificación

**@Excentry:** puede iniciar W01 y W10–W12; W20 continúa siendo su prueba de reproducción/controles. La selección/receta de libmpv no está declarada completada por este roadmap. Primero inspeccionar workflows/estado y fijar entorno/grafo; no cambiar lockfiles ni empaquetado sin evidencia.

**@IsaVonxz-type:** decisiones W00/W02 cerradas y baseline estática W04 publicada; W04 espera validar en runtime Linux los dos flujos autenticados, además de W21/W26 y el diseño de UI. No sobrescribir cambios documentales existentes.

**Ambos:** x64, host integrado, alcance Alpha y política de no publicación externa en Alpha ya acordados. Identidad/updater y requisitos de release siguen siendo gate antes de distribución externa.

### Criterio general de tarea completada

- Cambios limitados a archivos bajo propiedad anunciada.
- Criterios de aceptación demostrados, no inferidos de comentarios o compilación parcial.
- Checks y entorno exactos registrados; omisiones o fallos no se ocultan.
- Revisión cruzada terminada.
- Compatibilidad y limitaciones actualizadas, sin presentar lo planeado como soportado.

## Nota de estado

Este documento es un roadmap propuesto. No confirma que toolchain, audio, login, CI Windows, configuraciones de Tauri o instaladores estén funcionando. Antes de iniciar cada tarea, volver a comprobar el estado del repositorio y respetar los cambios existentes.
