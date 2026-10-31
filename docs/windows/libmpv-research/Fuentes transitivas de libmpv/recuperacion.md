# Recuperacion de fuentes transitivas del libmpv GPL Windows seleccionado

Investigacion: 2026-10-21. Objetivo: conservar el DLL del commit mpv `a43ab319fdb7aec31874999e5562c9975328cbda`, run `39541010475`; no sustituirlo por otra variante. Solo consultas HTTP publicas y lectura del proyecto; no se descargaron imagenes, caches grandes ni fuentes adicionales. Los comandos siguientes son propuestas, salvo las consultas de metadatos explicitamente verificadas.

## 1. Que permite recuperar el digest OCI sin descargar la imagen completa?

### Takeaway

El digest seleccionado en la investigacion inicial dejo de estar disponible: desde el 2026-10-30 devuelve `manifest unknown`. El tag publico `latest` se resolvio al digest `sha256:572cc7bc0974ebe2e5fd5f82c49e68c07942c6858098fd04f8daa23e3dc4dd2c` (Linux amd64, Ubuntu 26.04), que ya esta fijado en `sources.lock.json`. El run candidato 41150745596 completo el build y las validaciones Windows; el DLL resultante tiene SHA-256 `6a6ac973ecb495b6799b32d221fdf357f212f9cbd65bc01599670ed6b9d13f56`. Esto verifica el reemplazo operativo, no recupera la composicion historica ni los commits transitivos de la imagen anterior.

### Cited Findings

- Consulta verificada: `GET https://ghcr.io/token?service=ghcr.io&scope=repository:btbn/ffmpeg-builds/win64-gpl:pull`, seguida de una consulta al manifiesto viejo, ahora devuelve 404 `MANIFEST_UNKNOWN`. La etiqueta `latest` responde con el digest nuevo indicado arriba. No imprimir ni persistir el bearer temporal. [Endpoint publico de autorizacion](https://ghcr.io/token?service=ghcr.io&scope=repository:btbn/ffmpeg-builds/win64-gpl:pull); [manifiesto viejo](https://ghcr.io/v2/btbn/ffmpeg-builds/win64-gpl/manifests/sha256:666d6b74870f232bbc9ea29acc608c91b6b1c74f1effd8129cd39ed2332d1287)
- Configuracion recuperada: `sha256:9be0c3e1fb2588b4282379366d4c45bf6441fef0387fd04aa391210653fadafb`, 17.620 bytes. Contiene `TARGET=win64`, `VARIANT=gpl`, `FF_CONFIGURE`, variables del compilador y `history`; las etiquetas visibles son heredadas de Ubuntu, sin revision BtbN ni inventario SPDX/CycloneDX. [Blob de configuracion, HTTP 200 verificado](https://ghcr.io/v2/btbn/ffmpeg-builds/win64-gpl/blobs/sha256:9be0c3e1fb2588b4282379366d4c45bf6441fef0387fd04aa391210653fadafb)
- La capa que corresponde a `COPY /opt/ffbuild/. /opt/ffbuild` es `sha256:9cdcad5ce64aee65123a9ddeb2dd385579666e72d3cf5b49f14a65ae180eb5e8`, comprimida de 118.098.147 bytes. Correspondencia obtenida emparejando, en orden, las entradas `history` que NO tienen `empty_layer: true` con `layers` del manifiesto. Permite inspeccionar headers, `.pc`, bibliotecas y avisos instalados sin bajar las capas grandes del compilador/base. No se descargo ni se inspecciono su contenido. [Manifiesto](https://ghcr.io/v2/btbn/ffmpeg-builds/win64-gpl/manifests/sha256:666d6b74870f232bbc9ea29acc608c91b6b1c74f1effd8129cd39ed2332d1287); [history](https://ghcr.io/v2/btbn/ffmpeg-builds/win64-gpl/blobs/sha256:9be0c3e1fb2588b4282379366d4c45bf6441fef0387fd04aa391210653fadafb)
- `GET /v2/btbn/ffmpeg-builds/win64-gpl/referrers/<digest>` devolvio HTTP 404 `MANIFEST_UNKNOWN`. Esto NO demuestra ausencia universal de SBOM/atestaciones: solo ese endpoint no las entrego. La receta BtbN consultada ademas configura `provenance: false` en los pasos `docker/build-push-action`. [Consulta referrers](https://ghcr.io/v2/btbn/ffmpeg-builds/win64-gpl/referrers/sha256:666d6b74870f232bbc9ea29acc608c91b6b1c74f1effd8129cd39ed2332d1287); [workflow BtbN fijado](https://github.com/BtbN/FFmpeg-Builds/blob/937868360a21d711ec94a33f5591d345ad9b7754/.github/workflows/build.yml)
- La imagen final copia el prefijo instalado desde etapas intermedias. Las descargas se montan como `/cache.tar.xz`, no se copian al resultado final; `run_stage` elimina el directorio de trabajo de cada etapa. Por tanto, el digest final no es en si un archivo de fuentes, aunque identifica bibliotecas exactas que sirven para contrastar una recuperacion. [vars.sh fijado](https://github.com/BtbN/FFmpeg-Builds/blob/937868360a21d711ec94a33f5591d345ad9b7754/util/vars.sh); [generador consultado](https://github.com/BtbN/FFmpeg-Builds/blob/master/generate.sh); [run_stage consultado](https://github.com/BtbN/FFmpeg-Builds/blob/master/util/run_stage.sh)

### Inferences

- Descargar solamente la capa de 118 MB es una opcion razonable para extraer versiones/avisos y comparar hashes de bibliotecas estaticas con candidatos; NO recupera automaticamente todos los commits ni prueba la composicion final del DLL. El script mpv recompila FFmpeg y agrega otras dependencias encima de ese prefijo. [Receta mpv exacta](https://github.com/mpv-player/mpv/blob/a43ab319fdb7aec31874999e5562c9975328cbda/ci/build-mingw64-full.sh)
- Las fechas OCI no deben usarse como revision: `created` declara 2026-10-17, pero una entrada `COPY /opt/ffbuild` declara 2026-10-21T13:00:36Z. Hay capas cacheadas con fechas mezcladas. [Configuracion observada](https://ghcr.io/v2/btbn/ffmpeg-builds/win64-gpl/blobs/sha256:9be0c3e1fb2588b4282379366d4c45bf6441fef0387fd04aa391210653fadafb)

### Gaps

- No se inspeccionaron capas ni caches BuildKit; no se afirma que contengan un manifiesto de fuentes oculto. El manifiesto/configuracion examinados no aportan una correspondencia digest -> commit BtbN verificable por si solos.

## 2. Existe una via publica concreta para rescatar los fuentes exactos de BtbN?

### Takeaway

SI: BtbN publica un artefacto `download-cache` con los arboles descargados, independiente de los binarios FFmpeg. Se localizaron dos caches vivos del dia del build, uno con enlace directo, hash y caducidad; falta vincular por log de exportacion el digest exacto al run correcto antes de declararlo correspondiente. [Artefactos del candidato principal, pagina 2](https://api.github.com/repos/BtbN/FFmpeg-Builds/actions/runs/39601828709/artifacts?per_page=100&page=2)

### Cited Findings

- Run candidato principal: `39601828709`, commit BtbN `937868360a21d711ec94a33f5591d345ad9b7754`, iniciado 2026-10-21T12:43:03Z. Job exacto Windows GPL: `119234548334`, nombre `Build target-variant image (ubuntu-latest, win64, gpl)`. Es candidato, NO vinculacion probada al digest seleccionado. [Runs de ese dia](https://api.github.com/repos/BtbN/FFmpeg-Builds/actions/runs?created=2026-10-21&per_page=20); [job](https://github.com/BtbN/FFmpeg-Builds/actions/runs/39601828709/job/119234548334)
- Cache observado del candidato principal: ID `12346534162`, nombre `download-cache`, `expired: false`, 2.210.375.083 bytes, SHA-256 publicado `ea73dfe500a66c3b9f545954f0c9935d3eb8e241c60449dd1ae574e5d43e464d`, caduca `2026-11-04T12:43:03Z`. Creado `2026-10-21T12:55:38Z`. Hay 124 artefactos: revisar solo pagina inicial omite este cache. [Metadatos exactos](https://api.github.com/repos/BtbN/FFmpeg-Builds/actions/runs/39601828709/artifacts?per_page=100&page=2); [descarga ZIP](https://api.github.com/repos/BtbN/FFmpeg-Builds/actions/artifacts/12346534162/zip)
- Segundo candidato del mismo dia: run `39598082585`, commit `63ac3265b0789ea16c7b1f8351cd2ad195a28722`; cache ID `12345526726`, 2.210.375.462 bytes, `expired: false`, SHA-256 `7e6411ad422597a673ccf2d984f2d59547dcd7bfcfd24519269c49e3955a2d18`. No asumir que ambos caches son identicos por tamano parecido. [Artefactos](https://api.github.com/repos/BtbN/FFmpeg-Builds/actions/runs/39598082585/artifacts?per_page=100); [descarga](https://api.github.com/repos/BtbN/FFmpeg-Builds/actions/artifacts/12345526726/zip)
- El workflow ejecuta `./download.sh`, limpia cache y empaqueta `.cache/downloads/*` en `.cache/cache.tar.gz`, subido como `download-cache`. El script de descarga ejecuta la receta `ffbuild_dockerdl` por dependencia y crea `<etapa>_<hash-de-receta>.tar.xz`. Ese hash nombra la RECETA, no certifica el contenido del fuente. [Workflow fijado](https://github.com/BtbN/FFmpeg-Builds/blob/937868360a21d711ec94a33f5591d345ad9b7754/.github/workflows/build.yml); [download.sh consultado](https://github.com/BtbN/FFmpeg-Builds/blob/master/download.sh)
- La descarga git por defecto llama `git-mini-clone REPO REF DEST`; ese helper hace `git init`, `fetch --depth=1` y `checkout FETCH_HEAD`. El tar incluye `.` completo del directorio descargado, por lo que los paquetes que usan este camino pueden conservar `.git` y el commit resuelto, incluso si la receta indicaba una rama. Hay que verificarlo en el cache real. [dl_functions](https://github.com/BtbN/FFmpeg-Builds/blob/master/util/dl_functions.sh); [git-mini-clone](https://github.com/BtbN/FFmpeg-Builds/blob/master/images/base/git-mini-clone.sh); [empaquetado](https://github.com/BtbN/FFmpeg-Builds/blob/master/download.sh)
- Consulta anonima al endpoint de logs del job `119234548334`: HTTP 403, mensaje `Must have admin rights to Repository.` La pagina HTML del job es publica pero no incluia el digest buscado en el HTML obtenido. No se puede afirmar que el candidato ya este demostrado. [Logs API](https://api.github.com/repos/BtbN/FFmpeg-Builds/actions/jobs/119234548334/logs); [pagina job](https://github.com/BtbN/FFmpeg-Builds/actions/runs/39601828709/job/119234548334)

### Inferences

- La mejor via de rescate no es reconstruir versiones a ojo: preservar cache de descargas + revision BtbN + parches/recetas y demostrar el digest de salida del job. Esto podria recuperar gran parte de los fuentes estaticos exactos sin cambiar el DLL. El cache incluye mas plataformas/variantes que las necesarias: habra que filtrar con las recetas `ffbuild_enabled` y la configuracion realmente utilizada. [Workflow](https://github.com/BtbN/FFmpeg-Builds/blob/937868360a21d711ec94a33f5591d345ad9b7754/.github/workflows/build.yml); [generador](https://github.com/BtbN/FFmpeg-Builds/blob/master/generate.sh)
- Los fuentes de Rust/vendoring, submodulos, descargas adicionales dentro de cada etapa y parches requieren auditoria separada; un cache descargable no equivale por si solo a Corresponding Source completo. [Descarga](https://github.com/BtbN/FFmpeg-Builds/blob/master/download.sh); [recetas por dependencia](https://github.com/BtbN/FFmpeg-Builds/tree/937868360a21d711ec94a33f5591d345ad9b7754/scripts.d)

### Gaps

- No se descargo el ZIP de 2,21 GB ni se comprobo acceso anonimo a sus bytes. La API publica confirma su existencia, no que cualquier cliente pueda descargar sin sesion. Si exige autenticacion, usar una sesion propia ya autorizada sin compartir credenciales.
- Falta el log que dice `exporting manifest sha256:666d...`, o metadatos equivalentes de BuildKit, para seleccionar inequívocamente run/cache. Las fechas por si solas no bastan.

## 3. Que ejecutar ahora para cerrar la recuperacion manteniendo este DLL?

### Takeaway

Primero rescatar la vinculacion digest -> job/cache antes de la caducidad; despues recuperar solo el cache correcto y los fuentes adicionales de mpv. Ningun paso exige instalar Linux localmente ni reconstruir ahora el DLL. El empaquetado upstream no incorpora un source bundle, pero eso no elimina la via de rescate del cache BtbN. [Packaging mpv](https://github.com/mpv-player/mpv/blob/a43ab319fdb7aec31874999e5562c9975328cbda/ci/build-mingw64-full.sh); [cache vivo](https://api.github.com/repos/BtbN/FFmpeg-Builds/actions/runs/39601828709/artifacts?per_page=100&page=2)

### Cited Findings

- El run mpv corresponde al commit indicado y tiene `run_attempt: 3`, iniciado `2026-10-21T15:37:46Z`; conservar intento/job correcto, no mezclar automaticamente logs de los tres intentos. [Run mpv](https://api.github.com/repos/mpv-player/mpv/actions/runs/39541010475)
- La receta exacta de mpv recompila FFmpeg con flags del contenedor, pero anula `librav1e`, `librsvg` y `openal`. Por tanto, `FF_CONFIGURE` de la imagen es inventario potencial, no prueba de que esas tres bibliotecas esten enlazadas al DLL. Tambien construye LuaJIT, libsixel, subrandr, cppwinrt/windows-rs y Meson subprojects fuera del cache BtbN. [Receta mpv](https://github.com/mpv-player/mpv/blob/a43ab319fdb7aec31874999e5562c9975328cbda/ci/build-mingw64-full.sh)
- El paquete libmpv upstream copia DLL, import library, headers mpv, `LICENSE.GPL` y jar libbluray. No hay paso para copiar archivos fuente ni un manifiesto transitivo completo. [Packaging](https://github.com/mpv-player/mpv/blob/a43ab319fdb7aec31874999e5562c9975328cbda/ci/build-mingw64-full.sh)

### Inferences

Pasos minimos propuestos, no ejecutados salvo GET de metadatos:

1. Abrir el [job BtbN principal](https://github.com/BtbN/FFmpeg-Builds/actions/runs/39601828709/job/119234548334), buscar el digest completo en `Build target base image`/exportacion. Si no coincide, revisar run `39598082585` y anteriores; no descargar 2 GB para un candidato sin vincular. Si la interfaz no entrega logs, solicitar al mantenedor solamente la revision BtbN y el cache asociados al digest, no rehacer el build.
2. Si coincide, preservar el [ZIP cache 12346534162](https://api.github.com/repos/BtbN/FFmpeg-Builds/actions/artifacts/12346534162/zip) antes de `2026-11-04T12:43:03Z`, verificar SHA-256 publicado y conservar junto al snapshot de [recetas/parches fijados](https://github.com/BtbN/FFmpeg-Builds/tree/937868360a21d711ec94a33f5591d345ad9b7754). Guardarlo fuera del repositorio si se autoriza la descarga grande. No se propone instalar herramientas.
3. Inspeccionar el listado del ZIP y del `cache.tar.gz`; extraer solo tarballs de etapas Windows GPL habilitadas, con manejo seguro de rutas. Para paquetes git, ejecutar `git -C <arbol-extraido> rev-parse HEAD` y `git -C <arbol-extraido> submodule status --recursive`; registrar URL, commit, SHA-256 del tarball, licencia y parches BtbN. Son comandos propuestos, no resultados observados.
4. Si siguen faltando versiones/evidencia binaria, obtener exclusivamente la capa de prefijo de 118 MB, verificar su SHA-256 y listar/extraer `.pc`, headers y avisos. El endpoint es `/v2/btbn/ffmpeg-builds/win64-gpl/blobs/sha256:9cdcad5ce64aee65123a9ddeb2dd385579666e72d3cf5b49f14a65ae180eb5e8`; usar bearer anonimo efimero como en la seccion 1. No necesita `docker pull`, WSL ni ejecutar la imagen. [Capa referida por manifiesto](https://ghcr.io/v2/btbn/ffmpeg-builds/win64-gpl/manifests/sha256:666d6b74870f232bbc9ea29acc608c91b6b1c74f1effd8129cd39ed2332d1287)
5. Completar por separado el lado mpv: recuperar commits resueltos LuaJIT/libsixel/subrandr y sus subdependencias del intento correcto; preservar wraps MuJS, checksums y subproyectos Meson. Solicitud upstream acotada si logs no bastan: arboles de esos clones, `subprojects`, `Cargo.lock`, fuentes de crates usadas, parches y scripts del run exacto. [Receta y puntos de descarga](https://github.com/mpv-player/mpv/blob/a43ab319fdb7aec31874999e5562c9975328cbda/ci/build-mingw64-full.sh)

Consultas PowerShell pequenas reproducibles para los pasos ya verificados; no imprimen el bearer y no descargan capas:

```powershell
$registry = 'https://ghcr.io/v2/btbn/ffmpeg-builds/win64-gpl'
$digest = 'sha256:666d6b74870f232bbc9ea29acc608c91b6b1c74f1effd8129cd39ed2332d1287'
$auth = Invoke-RestMethod 'https://ghcr.io/token?service=ghcr.io&scope=repository:btbn/ffmpeg-builds/win64-gpl:pull'
$headers = @{ Authorization = "Bearer $($auth.token)"; Accept = 'application/vnd.oci.image.manifest.v1+json' }
$manifest = Invoke-RestMethod "$registry/manifests/$digest" -Headers $headers
$config = Invoke-RestMethod "$registry/blobs/$($manifest.config.digest)" -Headers $headers
$manifest.layers | Select-Object digest, size
$config.config.Labels
$config.history | Select-Object created, created_by, empty_layer
Invoke-RestMethod 'https://api.github.com/repos/BtbN/FFmpeg-Builds/actions/runs/39601828709/artifacts?per_page=100&page=2' |
    Select-Object -ExpandProperty artifacts |
    Where-Object name -EQ 'download-cache' |
    Select-Object id, size_in_bytes, digest, expired, expires_at, archive_download_url
```

### Gaps

- No se demuestra aun Corresponding Source completo ni se autoriza redistribucion. Resultado nuevo y util: existen caches publicamente identificables que pueden rescatar fuentes, no una imposibilidad demostrada de recuperarlos.
- No se modificaron los avisos ni la auditoria del proyecto. La frase local que descarta reconstruccion fiable merece reevaluarse DESPUES de vincular y examinar el cache, no eliminarse ahora basandose solo en su existencia.
