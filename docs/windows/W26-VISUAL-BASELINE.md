# W26 — Baseline visual Ryotunes para Windows

Estado: primera comparación en ejecución hecha el 2026-11-19, sin sesión iniciada (ver [resultados](#resultados-de-la-comparación-2026-11-19)). La paridad visual con Linux es el objetivo; no se propone rediseño. Las diferencias de composición siguen abiertas, así que W26 no está cerrada.

## Autoridad visual

- El cliente Linux Quickshell/QML (`client/shell.qml`, `client/App.qml`, `client/Style.qml` y `client/chrome/`) es la referencia visual.
- `docs/DESIGN.md` define el lenguaje compartido: Ryoku-native, cálido, arquitectónico, denso sin agobio, artwork como foco, hairlines, radios compactos y movimiento con propósito.
- La UI Windows existente en `ui/` es el destino a comparar, no una autoridad que cambie la referencia.
- Windows puede adaptar integración de ventana e interacción que dependa del sistema; no debe cambiar composición, jerarquía ni carácter por conveniencia del framework.

## Rasgos que deben conservarse

1. **Marco de aplicación:** barra superior, rail de navegación, contenido, panel Queue/Lyrics y transporte persistente; separaciones y orden visual coherentes con `client/App.qml`.
2. **Artwork como centro:** portada dirige el foco; fondos/backdrop y acento acompañan, sin competir con el contenido.
3. **Sistema visual:** colores semánticos Ryoku cuando estén disponibles; oscuro casi tinta, claro cálido y contenido; acento derivado de artwork/proveedor según política.
4. **Tipografía:** los roles UI, display y mono de `client/Style.qml`; conservar sus jerarquías, escalas y etiquetas mono, sin sustituciones que cambien personalidad o densidad.
5. **Geometría:** cuadrícula de 4 px escalada, bordes finos, radios compactos y densidades de navegación/listas; evitar estética de tarjetas redondeadas genéricas.
6. **Reproductor:** barra inferior y Now Playing pertenecen al mismo sistema; transporte legible, controles centrados, artwork y estados consistentes.
7. **Movimiento:** transiciones como respuesta a navegación/acción; respetar reducción de movimiento y evitar animación ambiental añadida.

## Diferencias estáticas por validar visualmente

| Zona | Evidencia QML/Linux | Evidencia Svelte actual | Validación necesaria |
| --- | --- | --- | --- |
| Marco y medidas | `Style.qml`: titlebar 44 px, rail 248 px, panel 340 px, player 88 px y padding de página 32 px a escala 1. `App.qml` organiza rail, contenido, panel y transporte. | `layout.css` fija titlebar a 32 px; `+layout.svelte` calcula overlay para rail de 64/268 px y reserva 60 px para player. | Confirmar tamaño de ventana/DPI comparables y decidir si diferencias son intencionales o rompen silueta, alineación y espacio útil. |
| Colores y temas | `Style.qml` consume `Skin`/`Tokens` de Ryoku; acento puede seguir artwork, proveedor o skin. | `ryotunes.css` define paletas dark/light locales; `ryoku-live.ts` conecta tokens del host. | Verificar en runtime tema del sistema, paleta Ryoku, fallback y acento dinámico; no basta con que existan variables CSS. |
| Tipografía | QML selecciona `Skin.type.ui`, `display` y `mono`, además de fuente CJK. | `ryotunes.css` declara Space Grotesk, Fraunces y SpaceMono Nerd Font; las fuentes instaladas/cargadas se deben confirmar en Windows. | Comparar familia realmente renderizada, métricas, saltos/elipsis y títulos CJK con la misma escala. |
| Navegación | `Sidebar.qml`: grupos con micro-etiquetas mono, hairlines y filas de 40 px; rail nominal de 248 px. | `Sidebar.svelte`: grupos equivalentes y marca Ryotunes; anchura expandida 16.75 rem y rail compacto de 4 rem. | Comparar densidad, grupos, estados activos, marca y alineación; confirmar que colapsar no cambia identidad. |
| Home y páginas | `HomePage.qml` documenta saludo/hero, mood chips, pinned grid y shelves con paginación incremental. | `+page.svelte` usa `HomeHero`, `Shortcuts`, `RecentRail`, `Shelf`, favoritos y layout propio. | Comparar contenido equivalente, proporciones de hero/artwork, orden, márgenes y scroll con estados/datos iguales. |
| Transporte y Now Playing | `PlayerBar.qml`: tres zonas, transporte centrado en ventana, seek hairline; `NowPlaying.qml` es artwork-first con lyrics y wash. | `PlayerBar.svelte` y `NowPlaying.svelte` implementan equivalentes, pero conservan estilos y dimensiones propios. | Comparar altura, centrado real, portada, jerarquía, paneles, seek y estados playing/paused/vacío. |
| SvelteKit / WebView | No aplica a QML. | `ui/` es SvelteKit + adapter-static y Tauri API. El layout inicia estado, temas y bridge en cliente. | El framework puede servir la SPA local; validar CSS/fonts, rutas/fallback, almacenamiento, diálogos, APIs y Tauri IPC en WebView2. Build no equivale a compatibilidad runtime. |

Las diferencias de dimensiones son hallazgos de código, no defectos visuales confirmados. Resolverlas comparando capturas, no suponiendo que números idénticos siempre sean necesarios.

## Protocolo de comparación y cierre

Capturar Linux y Windows en resolución y escala equivalentes, con el mismo tema, proveedor, artwork, cuenta/datos y estado de reproducción. Cubrir como mínimo:

- Home con contenido cargado y ventana vacía/loading;
- Search con resultados;
- Library o playlist con filas/artwork;
- detalle de álbum/artista;
- Queue y Lyrics abiertos;
- Now Playing abierto;
- reproductor paused/playing y ausencia de pista;
- rail expandido/colapsado y tema dark/light.

Para cada par, registrar ventana y DPI, diferencias de composición/medidas, color, tipografía, recorte de artwork, scroll y estados; adjuntar capturas. Clasificar cada diferencia como `corregir para paridad`, `adaptación necesaria de Windows` (con motivo) o `pendiente de evidencia`. Cerrar W26 solo cuando las diferencias visuales no justificadas estén resueltas o aceptadas explícitamente y WebView2 haya sido comprobado aparte.

## Resultados de la comparación (2026-11-19)

Ambos lados desde `main` en `55efe72`, a 1376×820 y escala 1, sin cuenta y con los mismos datos públicos; entorno y comandos en el [changelog](CHANGELOG.md#w26--comparación-visual-en-ejecución-linuxwindows). Cada imagen muestra Linux a la izquierda y Windows a la derecha.

| Zona | Diferencia observada | Clasificación | Capturas |
| --- | --- | --- | --- |
| Tipografía | Windows renderizaba Times New Roman y Arial; Linux usa Fraunces, Space Grotesk y SpaceMono. | `corregir para paridad`, resuelto en PR #75 | [home](w26/dark-01-home-no-track.jpg) |
| Álbum | El botón Play aparecía sin texto en oscuro y casi ilegible en claro. | `corregir para paridad`, resuelto en PR #74 | [oscuro](w26/dark-04-album.jpg), [claro](w26/light-04-album.jpg) |
| Álbum | Windows usa cabecera a sangre con fondo de artwork y filas de dos líneas sin columnas; Linux usa portada a la izquierda y tabla con columnas Artist, Plays y Length. | `corregir para paridad` | [oscuro](w26/dark-04-album.jpg) |
| Artista | Windows usa banner a sangre y no tiene botón Play; Linux usa avatar circular, Play y tabla «Popular». | `corregir para paridad` | [oscuro](w26/dark-05-artist.jpg) |
| Search | Windows añade un panel «instrumento» en la cabecera y divide resultados y vista previa; Linux muestra secciones Songs y Albums en tabla. | `corregir para paridad` | [oscuro](w26/dark-02-search.jpg), [claro](w26/light-02-search.jpg) |
| Library | Windows añade panel «instrumento», contadores y pestañas propias; Linux usa pestañas Songs, Albums, Artists, Playlists, Local e Insights con tabla y subnavegación en el rail. | `corregir para paridad` | [oscuro](w26/dark-03-library.jpg) |
| Home | Mismo saludo, buscador y chips; Windows ordena los chips de otra forma, añade «Edit home» y muestra «Trending community playlists» donde Linux muestra «Picked for you». | `pendiente de evidencia`: el contenido es en vivo y debe compararse con los mismos datos | [oscuro](w26/dark-01-home-no-track.jpg), [claro](w26/light-01-home-no-track.jpg) |
| Barra de título | Windows mide unos 37 px y no tiene selector de proveedor, búsqueda global ni botones de panel; Linux mide 44 px y los tiene. | Proveedores: `adaptación necesaria de Windows` mientras sigan aplazados. Búsqueda y botones: `corregir para paridad` | [oscuro](w26/dark-02-search.jpg) |
| Rail | Windows muestra marcas kana a la derecha, rótulo «04PLAYLISTS列» sin separación y no tiene Downloads. | Kana y rótulo: `corregir para paridad`. Downloads: pendiente de la fase 3 | [oscuro](w26/dark-01-home-no-track.jpg) |
| Rail colapsado | Windows deja un rail de iconos de unos 56 px; Linux lo oculta del todo. | `corregir para paridad`: decidido el 2026-11-20 ocultarlo del todo como Linux | [oscuro](w26/dark-06-rail-collapsed.jpg) |
| Tema claro | El rail de Windows tiene un tinte verdoso; en Linux es un neutro cálido como el contenido. | `corregir para paridad`; falta localizar el token | [claro](w26/light-01-home-no-track.jpg) |
| Cola | Q en Windows abre Now Playing con pestañas (`tabbedPlayer`); Linux abre el panel lateral Queue/Lyrics junto al contenido. | `corregir para paridad`: decidido el 2026-11-20 que Q y el botón de cola abran el panel lateral; Now Playing queda en Ctrl+E | [cola](w26/dark-09-panel-queue.jpg) |
| Now Playing | Windows muestra artwork en un panel «Live preview» y la cola o letras en una columna; Linux muestra artwork, título grande y letras sobre fondo difuminado. | `corregir para paridad` | [letras](w26/dark-11-nowplaying-lyrics.jpg) |
| Reproductor | Misma estructura de tres zonas y transporte centrado; Windows añade un indicador a la izquierda, botones + y ⋯, y la línea de seek tiene otra forma. | `corregir para paridad`, menor | [reproduciendo](w26/dark-07-playing.jpg) |
| Carga inicial | A 1,5 s Linux muestra esqueletos y Windows ya tenía contenido de la caché de página. | `pendiente de evidencia` | [oscuro](w26/dark-00-home-loading.jpg) |

La captura de Linux para «Now Playing con cola» muestra el panel lateral vacío en vez de la vista Now Playing; hay que repetirla antes de comparar ese estado.

## Alcance y límites

- W26 no autoriza mejoras de navegación, accesibilidad, nuevas funciones, cambio de marca ni rediseño; esas propuestas requieren trabajo separado.
- La tabla de diferencias estáticas se basa en el código fuente; la comparación en ejecución de 2026-11-19 cubre solo los estados sin sesión listados arriba.
- `pnpm check`/`pnpm build` comprueban frontend, no fidelidad visual ni WebView2. Con sesión, la ventana mini, otras resoluciones y escalas y proveedores distintos de YouTube Music siguen sin comparar.
