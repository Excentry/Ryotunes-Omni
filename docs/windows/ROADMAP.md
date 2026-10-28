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

### Documentos relacionados

| Documento | Contenido |
| --- | --- |
| [CHANGELOG.md](CHANGELOG.md) | Evidencia, resultados y hallazgos ya registrados; este roadmap conserva solo lo pendiente. |
| [PARITY-MATRIX.md](PARITY-MATRIX.md) | Funciones que Windows debe conservar de Linux y superficies de contrato (W04). |
| [CONTRACT-MATRIX.md](CONTRACT-MATRIX.md) | Cotejo exhaustivo de comandos y eventos Tauri/daemon (W14). |
| [W26-VISUAL-BASELINE.md](W26-VISUAL-BASELINE.md) | Baseline visual Ryotunes y protocolo de comparación (W26). |
| [W10-CARGO-WINDOWS-INVENTORY.md](W10-CARGO-WINDOWS-INVENTORY.md) | Inventario del grafo Cargo en Windows (W10). |
| `LIBMPV-*.md` | Auditoría, validación, licencias y build propio de libmpv (W12, W20, W85). |

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
| ◐ W04 | @IsaVonxz-type | Matriz por función y host ([PARITY-MATRIX.md](PARITY-MATRIX.md)). *Pendiente: validar en runtime Linux autenticado la suscripción de canal y la escritura Spotify (ver «Siguiente responsable» abajo).* | Contratos, superficies QML/Svelte/Tauri/daemon, pruebas y prioridad visibles; validar en runtime autenticado la suscripción de canal y la escritura Spotify antes de congelar |

**Siguiente responsable — Brayan, desde su distro Ryoku:** usar checkout actualizado y cuentas de prueba propias, sin compartir credenciales ni cookies; registrar commit, distro y resultados sin datos sensibles. (1) Ejecutar daemon y `qs -p client` con el módulo Ryoku disponible; en ArtistPage probar suscribir y desuscribir un artista y confirmar cada cambio en la cuenta YouTube. (2) Iniciar Spotify desde Settings, completar OAuth en navegador y comprobar estado autenticado; añadir un track de prueba a playlist de prueba, confirmar que aparece remotamente y registrar valor/error devuelto por `add_to_playlist`. (3) Adjuntar evidencia reproducible (pasos, resultado y logs redactados) y dejar W04 parcial hasta comprobar ambos flujos. Los defectos de dispatch ya se corrigieron en PR #4; reportar cualquier regresión nueva con reproducción mínima. El resultado de WSL no implica paridad Windows.

**Salida de fase:** alcance y ruta de integración aprobados. No empezar refactor amplio antes de separar publicación de CI.

### Fase 1 — Build reproducible y auditoría por target

| Estado / ID | Responsable | Tarea | Aceptación |
| --- | --- | --- | --- |
| ◐ W10 | @Excentry | Resolver grafo Cargo Windows por target/features — **inventario estático hecho; check MSVC bloqueado por recurso ausente** | Paquetes activos, build dependencies, APIs Unix y DLL requeridas documentadas; [inventario W10](W10-CARGO-WINDOWS-INVENTORY.md) registra el intento de build y el bloqueo |
| ☐ W11 | @Excentry | Definir entorno MSVC x64 en Windows 11 | Versiones Rust/Node/pnpm/VS C++/SDK registradas; pasos reproducibles desde máquina limpia |
| ☑ W12 | @Excentry | Auditar libmpv Windows — **candidato/fuentes emparejados y entrega futura documentada; sin promoción ni publicación** | Run `39901704769` pasó build, comprobación Windows de igualdad byte-a-byte del source bundle, smoke y regresión MSVC; DLL SHA-256 `a63580d6…5210c1`, source archive SHA-256 `9733a763…adc0f47`. La distribución futura se propone como companion source asset de la misma GitHub Release, nunca como artifact temporal de Actions. Se conserva la limitación técnica de mapeo exacto de 101 miembros GCC-family, sin infracción confirmada ni afirmación de clearance legal. |
| ☑ W13 | @IsaVonxz-type | Ejecutar baseline Svelte/Tauri. [Evidencia](CHANGELOG.md#w13--baseline-local-ejecutada) | `pnpm check`, build y nueve checks puros pasaron; toolchain/resultados registrados en el changelog. No equivale a validar Tauri en runtime Windows |
| ☑ W14 | @IsaVonxz-type | Comparar comandos/eventos Tauri con daemon. [Matriz](CONTRACT-MATRIX.md) y [evidencia](CHANGELOG.md#w14--cotejo-estático-tauridaemon) | Matriz por método/evento y tests Linux registrados en [el apéndice W14](CONTRACT-MATRIX.md); integración UI-host y runtime Windows siguen como gates W21/W34 |
| ☐ W15 | @Excentry | Añadir CI Windows no-publicadora | Compila solo paquetes seleccionados; permisos de workflow mínimos; no firma ni publica |

**Salida:** build repetible, limitaciones demostradas, y no hay dependencias nativas críticas desconocidas.

### Fase 2 — Prototipos de viabilidad

| Estado / ID | Responsable | Tarea | Mínimo de aceptación |
| --- | --- | --- | --- |
| ☑ W20 | @Excentry | Prueba libmpv Windows — **runtime con DLL candidata aislada verificado** | Audio local, pause/resume, seek, volume, EOF, stop/Quit, ruta/hash y dependencias probados; evidencia y límites en `LIBMPV-CANDIDATE-VALIDATION.md` |
| ☑ W21 | @IsaVonxz-type | Prueba Tauri/WebView2 y configuración por plataforma. [Evidencia](CHANGELOG.md#w21--pruebas-tauriwebview2-windows-estado-propio) | UI instalada, IPC exitoso/rechazado, evento, SMTC/controles incl. seek, fallo fail-soft sin HWND, hide/reopen, Quit y reinicio sin procesos residuales; evidencia y límites en el changelog |
| ☑ W22 | @IsaVonxz-type | Probar bridge JavaScript y login YouTube. [Evidencia](CHANGELOG.md#w22--harness-de-bridgelogin-estado-propio). *Login, cancelación, timeout/toast y llamada autenticada verificados; el fix de allowlist regional de #25 está en revisión, pero su 2FA aún requiere retest Windows.* | Harness correcto; timeout, cancelación, cookies esperadas y fallo recuperable demostrados |
| ◐ W23 | @Excentry | Comparar transporte Spotify — **Windows actual rechaza FIFO Unix; Spotify se recomienda aplazado hasta medir prototipos** | Named pipe vs sink/alternativa; latencia, pausa, seek, backpressure, EOF, cancelación y handles medidos |
| ☐ W24 | Ambos | Validar el host integrado Tauri acordado | Prueba demuestra continuidad al ocultar/cerrar ventana, Quit limpio, single-instance y dueño único de audio/datos; reabrir arquitectura solo si falla |
| ☑ W25 | @Excentry | Construir prototipo ejecutable — **clean checkout compilado y ejecutado fuera del checkout con DLL aislada; dependencias inventariadas, fallo por DLL ausente y logs de arranque/cierre registrados; sin VM limpia, revisión cruzada pendiente** | Ejecuta fuera del shell de desarrollo, con dependencias nativas descubiertas y logs útiles |
| ◐ W26 | @IsaVonxz-type | Elaborar baseline visual Ryotunes ([W26-VISUAL-BASELINE.md](W26-VISUAL-BASELINE.md)). *Pendiente: capturas Linux y Windows comparables.* | [Baseline y diferencias por validar](W26-VISUAL-BASELINE.md); falta comparación con capturas/runtimes equivalentes para cerrar paridad, sin rediseño accidental |

**Salida:** reproducción local en runtime, WebView funcional y decisión explícita para Spotify. Si Spotify se aplaza, publicarlo como limitación aceptada; no marcarlo como portado.
#### W23 — Transporte Spotify Windows

- `crates/spotify/src/stream.rs` usa backend `pipe`, almacena bytes S16 en FIFO creada con `mkfifo` y devuelve error inmediato bajo `cfg(not(unix))`; el motor además referencia `XDG_RUNTIME_DIR`. Esto prueba que el transporte actual no está portado a Windows, no compara candidatos.
- `cargo test --locked -p ryotunes-spotify --lib stream::tests -- --nocapture` pasó 2/2 en Windows; son tests de URI/event translation, no miden un transporte ni implican streaming Spotify funcional en Windows.
- Recomendación provisional: Spotify queda aplazado/no soportado en Windows hasta que el owner de playback/native pruebe candidatos Windows. La limitación visible es que Spotify no reproduce en Windows; la creación del stream devuelve `Spotify streaming needs a Unix FIFO; this platform is unsupported` (no se verificó aún cómo lo presenta la UI). No anunciarlo como portado. W23 permanece parcial: faltan prototipos y mediciones comparables de latencia, pause/resume, seek, backpressure, EOF, cancelación y limpieza.

#### W25 — Ejecutable aislado y dependencias

- El NSIS instalado aislado de W21 se ejecutó desde `%TEMP%`, fuera del checkout/dev shell, y cargó `libmpv-2.dll` desde su propio directorio con hash registrado en `LIBMPV-CANDIDATE-VALIDATION.md`. La inspección de imports/dependencias del candidato y el build offline están en esa evidencia; stdout no se recopiló como paquete diagnóstico repetible.
- Startup, UI/WebView2, Quit y ausencia posterior de proceso/WebView2 se observaron; un log de runtime informa attach de media controls, sin credenciales/cookies. La clean checkout y los logs se registraron después (ver abajo). No implica release readiness.
- Clean checkout (2026-10-24, issue #16): `git worktree add --detach` de `57b41c3` en `%TEMP%\w25\src` (ruta corta por el límite de 260 caracteres), árbol limpio, en la misma máquina Windows 11 Pro `26300` con Rust `1.94.0` (`x86_64-pc-windows-msvc`), VS 2022 Build Tools MSVC `14.44.35207`, Node `v24.12.0`, pnpm `12.8.1`, `cargo-tauri 2.12.4`, WebView2 `155.0.4283.62`. Receta usada, en este orden: `pnpm --dir ui install --frozen-lockfile`; `scripts/windows/prepare-libmpv.ps1 -ArchivePath <zip cacheado>` (sin `-ArchivePath` descarga el ZIP fijado; el script verificó ZIP `7f63ce43…4b9f` y DLL `5816ca7a…bcdf7`); `$env:LIB = "<repo>\.libmpv\v0.41.0-dev-ga43ab319f-x64-gpl;$env:LIB"`; y, **desde la raíz del repo**, `cargo tauri build --no-bundle --config <json con identificador aislado> -- --locked`. El hook `beforeBuildCommand` (`pnpm build`) funcionó desde la raíz; lanzado desde `src-tauri` falla con `ERR_PNPM_RECURSIVE_EXEC_FIRST_FAIL Command "build" not found`. Compilación: 10 min 49 s, `ryotune-omni.exe` de 32 428 544 bytes, SHA-256 `4a74506c…5e17f`. La receta no estaba reunida en ningún documento; queda aquí.
- Dependencias nativas (`dumpbin /dependents`, solo nombres): el exe importa `libmpv-2.dll` además de librerías del sistema (`kernel32`, `user32`, `gdi32`, `shell32`, `ole32`, `oleaut32`, `comctl32`, `dwmapi`, `shlwapi`, `advapi32`, `crypt32`, `bcrypt`, `ws2_32`, UCRT `api-ms-win-crt-*`) y **`VCRUNTIME140.dll` y `VCRUNTIME140_1.dll`** (CRT de VC++ enlazado dinámicamente). `libmpv-2.dll` es PE x64 y solo importa DLL del sistema (UCRT, `OPENGL32`, `d2d1`, `DWrite`, `AVRT`, `WS2_32`, etc.), sin DLL de terceros. WebView2 Runtime es requisito externo. No se verificó en una máquina sin el redistribuible de VC++ ni sin WebView2 Runtime.
- DLL ausente: el exe solo, sin `libmpv-2.dll`, termina en menos de 6 s con código `0xC0000135` (`STATUS_DLL_NOT_FOUND`) y sin una línea en stdout/stderr; el único diagnóstico es el código de salida. Con la DLL junto al exe, `Process.Modules` mostró `libmpv-2.dll` cargada desde la carpeta del exe.
- Arranque y cierre con `RUST_LOG=info` y puerto de depuración de WebView2: ventana `Ryotunes Omni` respondiendo, UI cargada en `tauri.localhost`, 6 procesos `msedgewebview2` del perfil. Se fijó `close_to_tray=false` en el perfil aislado y se cerró con `CloseMainWindow`: el proceso salió con código `0`; a los 25 s no quedaban procesos de la app ni de WebView2 y el puerto de depuración estaba libre. Logs completos: stdout con dos líneas `INFO` (`ryotunes_core::media: OS media controls attached`, `app_lib::taskbar: taskbar thumbnail toolbar attached`) y stderr con una línea de WebView2 (`Failed to unregister class Chrome_WidgetWin_0. Error = 1412`, no fatal). Sin coincidencias de `SAPISID`, `cookie`, `token`, `Bearer`, `password`, `googlevideo`, `sig=` ni correo. Sin `RUST_LOG` el nivel por defecto es silencioso (`src-tauri/src/lib.rs`).
- Límites de W25: misma máquina de desarrollo, no una VM limpia; el ZIP de libmpv vino del caché local, no de una descarga nueva; los logs no incluyen versión, identificador ni ruta de la DLL, y un fallo de DLL no deja traza; el candidato sigue siendo un dev build sin promover. Revisión cruzada de @Excentry pendiente.

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
| ◐ W83 | @IsaVonxz-type | Ensayo instalar/actualizar/desinstalar. *Pendiente (aplazado): ejecutar en una VM Windows 11 limpia y registrar identidad, hash, upgrade y datos de usuario.* | Script verifica registro, DLL, contenido completo del directorio aislado y shortcuts con target al ejecutable; sintaxis PowerShell pasa. Falta ejecutar en runner limpio y registrar identidad/hash/upgrade/datos de usuario |
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
