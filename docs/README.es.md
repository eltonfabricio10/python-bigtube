<p align="center">
  <img src="https://raw.githubusercontent.com/eltonfabricio10/bigtube/main/assets/banner.png" alt="BigTube Banner" width="100%">
</p>

<p align="center">
  <a href="../README.md">English</a> · <a href="README.pt-BR.md">Português (BR)</a> · <b>Español</b> · <a href="README.fr.md">Français</a>
</p>

# 🎬 BigTube

> **El descargador multimedia definitivo para Linux**

**BigTube** es una aplicación de escritorio moderna, rápida y elegante creada en **Rust** con **GTK4**, **Libadwaita** y **GStreamer**. Diseñada para quienes no aceptan nada menos que la perfección al descargar contenido de internet, BigTube convierte la complejidad de `yt-dlp` en una herramienta intuitiva y potente: un binario nativo y veloz.

---

## 📸 Capturas de pantalla

#### 🔍 Administrador de búsqueda
<p align="center">
  <img src="screenshots/01-main.png" alt="BigTube — Administrador de búsqueda" width="80%">
</p>

#### 🎚️ Selector de formato &nbsp;·&nbsp; ⚙️ Ajustes
<p align="center">
  <img src="screenshots/04-formats.png" alt="Selector de calidad de vídeo y audio en paralelo" width="48%">
  &nbsp;
  <img src="screenshots/02-settings.png" alt="Ajustes" width="48%">
</p>

#### 🔄 Conversor multimedia &nbsp;·&nbsp; 💖 Donaciones
<p align="center">
  <img src="screenshots/03-converter.png" alt="Conversor de medios integrado" width="48%">
  &nbsp;
  <img src="screenshots/05-donations.png" alt="Ventana de donaciones" width="30%">
</p>

---

## ✨ Características

### 🔍 Búsqueda y descubrimiento
- **Búsqueda de YouTube integrada** - Busca sin abrir un navegador, con filtro de tipo: **Videos**, **Canales** o **Listas de reproducción**
- **Búsqueda nativa en YouTube Music** - Solo música (sin pódcasts), mediante la propia API de YouTube Music, filtrada por **Canciones**, **Álbumes**, **Artistas** o **Listas de reproducción**; las canciones entran como audio y los videos musicales como video
- **Enlaces directos** - Compatibilidad con más de 400 sitios mediante URL
- **Abrir contenedores** - Abre un canal, álbum, artista o lista de reproducción en una ventana modal con todos sus videos/pistas, con **Reproducir todo**, **Descargar todo** y un modo de selección para descargar solo los marcados
- **Listas de reproducción por enlace** - Pega un enlace de una lista de reproducción de YouTube (`playlist?list=` o `watch?v=...&list=`) y la búsqueda mostrará todos sus videos
- **Sugerencias de búsqueda** - Historial local más autocompletado en línea mientras escribes, con navegación completa por teclado (↑/↓ para moverte, Enter para elegir, Esc para cerrar)

### ⬇️ Descargas avanzadas
| Característica | Descripción |
|---------|-------------|
| **Calidad de video** | 4K (2160p), 2K (1440p), 1080p, 720p, 480p, 360p, 240p, 144p |
| **Formatos de audio** | MP3, M4A, Opus, FLAC, WAV, AAC con extracción de alta calidad |
| **Metadatos** | Incrustación automática de etiquetas, álbum y artista |
| **Subtítulos** | Incrusta o guarda como archivos sidecar, manuales + autogenerados, selección por idioma |
| **Programación** | Pon descargas en cola para ejecutarlas más tarde, una sola vez o de forma recurrente |
| **SponsorBlock** | Omite segmentos de patrocinio dentro del video — márcalos como capítulos o elimínalos del archivo (usa la base de datos de [SponsorBlock](https://sponsor.ajay.app/)) |
| **Concurrencia** | Múltiples descargas simultáneas con fragmentos paralelos configurables |
| **Reanudar** | Continúa descargas interrumpidas |

### 🔄 Convertidor multimedia
- Conversión de video a video (MP4, MKV, WebM)
- Extracción y conversión de audio (MP3, M4A, Opus, FLAC, WAV, AAC)
- Combinación de subtítulos (incrustar o sidecar)
- Cola de conversión por lotes
- Progreso en tiempo real con tiempo estimado (ETA)

### 📺 Reproductor integrado
- Motor de reproducción **GStreamer** (nativo, integrado con GTK4)
- Vista previa del video antes de descargar, con calidad de vista previa configurable (144p–720p)
- Navegación por la lista de reproducción (Anterior / Reproducir-Pausar / **Detener** / Siguiente), barra de búsqueda (seek) y un control de volumen que ajusta el propio flujo de la app en el mezclador del sistema (PulseAudio/PipeWire)
- Ventana de video desacoplable, con sus propios controles sobre el video, incluido el volumen

### 🎨 Personalización de la apariencia
| Modo | Descripción |
|------|-------------|
| **Tema** | Claro / Oscuro / Seguir al sistema |
| **Colores** | 16 esquemas de color (Default Blue, Modern Violet, Emerald Green, Sunburst Orange, Vibrant Rose, Nordic Cyan, Nordic Snow, Gruvbox Retro, Catppuccin Mocha, Dracula Dark, Tokyo Night, Rosé Pine, Solarized Dark, Monokai Pro, Cyberpunk Neon, BigTube Brand) |
| **Estilo** | Interfaz moderna con efecto glassmorphism |

### 📊 Gestión
- Historial de descargas
- Historial de conversiones
- Historial de búsquedas
- Descargas programadas
- Opción para borrar los datos automáticamente al salir

---

## 🛠️ Tecnologías

| Tecnología | Función |
|------------|------|
| **Rust 2021** | Núcleo de la aplicación (binario nativo) |
| **GTK4 + Libadwaita** | Interfaz nativa de GNOME |
| **GStreamer** | Motor de reproducción |
| **yt-dlp** | Motor de descargas |
| **FFmpeg** | Conversión multimedia |
| **Cargo** | Compilación y gestión de dependencias |

> El proyecto es un espacio de trabajo (workspace) de Cargo con tres crates: **`bigtube-core`** (lógica/motor), **`bigtube-cli`** (binario `bigtube` sin interfaz) y **`bigtube-gui`** (interfaz gráfica `bigtube-gui`).

---

## 🚀 Instalación

### Arch Linux (AUR) — recomendado
Paquete binario precompilado (`bigtube-bin`): se instala rápido, **sin compilar nada** en tu equipo.
```bash
yay -S bigtube-bin
# or
paru -S bigtube-bin
```

### Debian / Ubuntu (.deb)
Descarga el `.deb` de la [última versión](https://github.com/eltonfabricio10/bigtube/releases/latest) e instálalo (resuelve las dependencias automáticamente):
```bash
sudo apt install ./bigtube_*_amd64.deb
```
> Compilado en Ubuntu 24.04, por lo que requiere **Ubuntu 24.04+** o **Debian 13+** (GTK ≥ 4.12, libadwaita ≥ 1.5).

### Fedora (.rpm)
Descarga el `.rpm` de la [última versión](https://github.com/eltonfabricio10/bigtube/releases/latest) e instálalo:
```bash
sudo dnf install ./bigtube-*.x86_64.rpm
```
> Compilado en Fedora 40 (requiere **Fedora 40+**). `ffmpeg` (extracción de audio/conversión) está en [RPM Fusion](https://rpmfusion.org/) — actívalo y ejecuta `sudo dnf install ffmpeg` para esas funciones.

### AppImage (portátil, cualquier distro)
Descarga `BigTube-*-x86_64.AppImage` de la [última versión](https://github.com/eltonfabricio10/bigtube/releases/latest), hazlo ejecutable y ejecútalo:
```bash
chmod +x BigTube-*-x86_64.AppImage
./BigTube-*-x86_64.AppImage
```
> Incluye GTK4/libadwaita y los plugins de GStreamer (incluido el sink gtk4 del reproductor), así que funciona en cualquier sistema x86_64 sin importar la versión de GTK de la distro. `ffmpeg` y `yt-dlp` se usan en tiempo de ejecución si están presentes; la app descarga `yt-dlp` en su propia carpeta de datos en el primer uso.
>
> **Nota:** el AppImage necesita **glibc ≥ 2.41** (Debian 13+, Ubuntu 25.10+, Fedora 42+, o una distro rolling como Arch/openSUSE Tumbleweed). En sistemas más antiguos usa los paquetes `.deb`/`.rpm`/AUR.

### Compilar desde el código fuente (Cargo)
Requiere el conjunto de herramientas de Rust (`rustup`) y las dependencias del sistema que se indican a continuación.
```bash
# Clone the repository
git clone https://github.com/eltonfabricio10/bigtube.git
cd bigtube/rust

# Build in release mode
cargo build --release --locked

# The binaries end up in rust/target/release/
./target/release/bigtube-gui      # graphical interface
./target/release/bigtube --help   # headless mode (CLI)
```

Para instalarlo en todo el sistema a partir de una compilación local:
```bash
sudo install -Dm755 target/release/bigtube-gui /usr/bin/bigtube-gui
sudo install -Dm755 target/release/bigtube     /usr/bin/bigtube
sudo install -Dm644 ../assets/bigtube.svg /usr/share/icons/hicolor/scalable/apps/bigtube.svg
sudo install -Dm644 ../assets/bigtube.png /usr/share/icons/hicolor/512x512/apps/bigtube.png
sudo install -Dm644 packaging/io.github.eltonfabricio10.bigtube.desktop /usr/share/applications/io.github.eltonfabricio10.bigtube.desktop
```

---

## ⌨️ Línea de comandos

BigTube ofrece **dos binarios**:

| Binario | Función |
|--------|------|
| `bigtube-gui` | Abre la interfaz gráfica |
| `bigtube` | Modo sin interfaz (descarga directamente desde la terminal, sin GUI) |

### Interfaz gráfica
```bash
bigtube-gui      # opens the BigTube window
```

### Modo sin interfaz (`bigtube`)
```bash
bigtube -d <URL> [options]
```

| Opción | Descripción |
|--------|-------------|
| `-d, --download URL` | Descarga la URL directamente desde la terminal, sin abrir la ventana |
| `-o, --output DIR` | Carpeta de destino para `--download` (predeterminado: carpeta configurada) |
| `--audio-only` | Con `--download`, extrae el audio como MP3 |
| `--format FMT` | Con `--download`, selector de formato personalizado para `yt-dlp -f` |
| `--yt-dlp-version` | Muestra la versión de `yt-dlp` incluida |
| `--version` | Muestra la versión de BigTube |
| `--help` | Muestra la ayuda |

### Ejemplos
```bash
bigtube-gui                                      # opens the GUI
bigtube -d https://youtube.com/watch?v=...       # headless download
bigtube -d <url> -o ~/Music --audio-only         # headless MP3 audio
bigtube -d <url> --format "bestvideo+bestaudio"  # custom format
```

---

## 📁 Estructura de directorios

| Ubicación | Contenido |
|----------|----------|
| `~/.config/bigtube/` | Configuración e historiales |
| `~/.config/bigtube/config.json` | Configuración de la aplicación |
| `~/.config/bigtube/history.json` | Historial de descargas |
| `~/.config/bigtube/search_history.json` | Historial de búsquedas |
| `~/.config/bigtube/converter_history.json` | Historial de conversiones |
| `~/.config/bigtube/scheduled_downloads.json` | Descargas programadas |
| `~/.local/share/bigtube/bin/` | Binarios incluidos (`yt-dlp`, `deno`) |
| `~/.cache/bigtube/thumbnails/` | Caché de miniaturas |
| `~/Downloads/BigTube/` | Carpeta de descargas predeterminada |
| `~/Downloads/BigTube/Converted/` | Carpeta de salida predeterminada del conversor |

---

## ⚙️ Ajustes disponibles

Las preferencias se guardan en `~/.config/bigtube/config.json`. Cuando el archivo no existe o está dañado, BigTube vuelve a crear la configuración con los valores predeterminados. Las rutas vacías o las opciones deshabilitadas simplemente hacen que la aplicación recurra al comportamiento predeterminado.

> La página de ajustes está organizada en grupos en este orden: **Apariencia**, **Búsqueda**, **Reproducción**, **Descargas**, **Rendimiento**, **Posprocesamiento**, **Subtítulos**, **Convertidor multimedia**, **Red y avanzado**, **Sistema** y **Almacenamiento**.

### Apariencia
| Ajuste | Predeterminado | Explicación |
|---------|---------|-------------|
| **Tema de la interfaz** | Seguir al sistema | Define si la interfaz usa el tema del sistema, fuerza un tema claro o fuerza un tema oscuro. |
| **Esquema de color** | Azul predeterminado | Cambia la paleta/color de acento de la interfaz. Opciones: Azul predeterminado, Violeta moderno, Verde esmeralda, Naranja Sunburst, Rosa vibrante, Cian nórdico, Nieve nórdica, Gruvbox Retro, Catppuccin Mocha, Dracula Dark, Tokyo Night, Rosé Pine, Solarized Dark, Monokai Pro, Cyberpunk Neon y Marca BigTube. |

### Búsqueda
| Ajuste | Predeterminado | Explicación |
|---------|---------|-------------|
| **Guardar historial de búsquedas** | Habilitado | Almacena tus búsquedas localmente en `search_history.json`, lo que te permite reutilizar consultas anteriores. |
| **Habilitar sugerencias de búsqueda** | Habilitado | Muestra sugerencias mientras escribes, usando el historial de búsquedas local. |
| **Máximo de sugerencias** | 10 | Define cuántas sugerencias pueden aparecer a la vez. Acepta valores de 1 a 50. |
| **Borrar historial de búsquedas** | Acción manual | Elimina todas las entradas guardadas del historial de búsquedas. No borra los archivos descargados. |
| **Máximo de resultados de búsqueda** | 15 | Define cuántos resultados solicita BigTube a `yt-dlp` para las búsquedas de texto. Acepta valores de 5 a 100. |

### Reproducción
| Ajuste | Predeterminado | Explicación |
|---------|---------|-------------|
| **Calidad de vista previa** | 360p | Calidad usada por el reproductor de la aplicación al previsualizar antes de descargar: `144p`, `240p`, `360p` (progresivo), `480p` o `720p` (streaming HLS). |

### Descargas
| Ajuste | Predeterminado | Explicación |
|---------|---------|-------------|
| **Carpeta de descargas** | `~/Downloads/BigTube/` | Define dónde se guardan los archivos descargados. La aplicación crea la carpeta cuando es necesario. |
| **Calidad preferida** | Preguntar siempre | Define el formato predeterminado para las nuevas descargas. Puede preguntar en cada descarga, descargar el mejor video, elegir 4K, 2K, 1080p, 720p, 480p, 360p, 240p, 144p, o descargar solo el audio como MP3, M4A, Opus, FLAC, WAV o AAC. |
| **Guardar historial de descargas** | Habilitado | Mantiene un registro local de las descargas en `history.json`, usado por la vista de historial/lista. |
| **Máximo de entradas del historial** | 100 | Cuántas entradas de descargas se conservan en la lista. Acepta valores de 10 a 1000. |
| **Eliminar al completar** | Deshabilitado | Elimina automáticamente las descargas finalizadas de la lista. |
| **Eliminar al cancelar** | Deshabilitado | Elimina automáticamente las descargas canceladas de la lista. |

#### Opciones de calidad
| Opción | Explicación |
|--------|-------------|
| **Preguntar siempre** | Muestra la elección de calidad/formato en el momento de la descarga. |
| **Mejor (MKV)** | Descarga la mejor combinación disponible de video y audio y combina el resultado. |
| **4K, 2K, 1080p, 720p, 480p, 360p, 240p, 144p** | Prioriza el video MP4/AVC en la resolución elegida con audio M4A; si ese formato exacto no existe, `yt-dlp` usa la mejor alternativa compatible definida en el preajuste. |
| **Audio (MP3)** | Extrae solo el audio, lo convierte a MP3 de alta calidad e intenta incrustar la miniatura. |
| **Audio (M4A)** | Descarga solo el audio, priorizando el códec/contenedor M4A. |
| **Audio (Opus / FLAC / WAV / AAC)** | Extrae solo el audio y lo convierte al formato elegido con la máxima calidad. |

### Rendimiento
| Ajuste | Predeterminado | Explicación |
|---------|---------|-------------|
| **Descargas simultáneas máximas** | 3 | Controla cuántos videos se pueden descargar al mismo tiempo. Acepta valores de 1 a 10. |
| **Fragmentos concurrentes** | 16 | Define cuántos fragmentos paralelos usa `yt-dlp` por descarga. Acepta valores de 1 a 16. Los valores más altos pueden acelerar las descargas segmentadas, pero también aumentan el uso de la red. |
| **Límite de velocidad de descarga** | 0 KB/s | Limita la velocidad de descarga en KB/s. `0` significa sin límite. Acepta valores de 0 a 100000. |

### Posprocesamiento
| Ajuste | Predeterminado | Explicación |
|---------|---------|-------------|
| **Agregar metadatos** | Deshabilitado | Intenta incrustar el artista, el álbum, la portada y otros metadatos en los archivos descargados. Requiere `ffmpeg`; si no está instalado, la aplicación omite este paso. |
| **SponsorBlock** | Deshabilitado | Omite segmentos de patrocinio dentro del video usando la base de datos de SponsorBlock. "Marcar capítulos" añade marcadores (no destructivo); "Eliminar segmentos" los corta del archivo. Requiere `ffmpeg`. |
| **Comando de posprocesamiento** | Vacío | Ejecuta un comando después de la descarga usando `yt-dlp --exec`. Usa `{}` en el comando para representar el archivo descargado. |

### Subtítulos
| Ajuste | Predeterminado | Explicación |
|---------|---------|-------------|
| **Subtítulos** | Desactivado | Manejo de subtítulos para las descargas: `Off`, `Embed` (incrustar) en el archivo, guardar como `File` (sidecar) separado, o `Both` (ambos). La incrustación requiere `ffmpeg`. |
| **Idiomas** | `en,pt,es` | Lista separada por comas de códigos de idioma de subtítulos a obtener (p. ej. `en,pt,es`). |
| **Incluir autogenerados** | Habilitado | Obtiene también los subtítulos generados automáticamente (por máquina), no solo los manuales. |

### Convertidor multimedia
| Ajuste | Predeterminado | Explicación |
|---------|---------|-------------|
| **Guardar en la carpeta de origen** | Deshabilitado | Cuando está habilitado, el archivo convertido se guarda junto al archivo original. |
| **Carpeta de salida predeterminada** | `~/Downloads/BigTube/Converted/` | Define la carpeta usada por el convertidor cuando "guardar en la carpeta de origen" está deshabilitado. |
| **Guardar historial de conversiones** | Habilitado | Mantiene un registro local de las conversiones en `converter_history.json`. |
| **Eliminar al completar** | Deshabilitado | Elimina automáticamente las conversiones finalizadas de la lista. |
| **Eliminar al cancelar** | Deshabilitado | Elimina automáticamente las conversiones canceladas de la lista. |
| **Máximo de entradas del historial** | 50 | Cuántas entradas de conversiones se conservan en la lista. Acepta valores de 10 a 500. |

### Red y avanzado
| Ajuste | Predeterminado | Explicación |
|---------|---------|-------------|
| **Archivo de cookies** | Vacío | Usa un archivo `cookies.txt` en formato Netscape con `yt-dlp --cookies`, útil para contenido que requiere una sesión autenticada. |
| **Cookies del navegador** | Ninguno | Importa cookies directamente de un navegador detectado, como Firefox, Chrome, Chromium, Brave, Microsoft Edge, Vivaldi u Opera, usando `yt-dlp --cookies-from-browser`. |
| **User-Agent** | Predeterminado de BigTube | Reemplaza el User-Agent enviado a `yt-dlp`. Si se deja vacío, la aplicación usa un User-Agent seguro basado en Chrome. Incluye preajustes para los navegadores detectados. |
| **Proxy** | Vacío | Enruta las búsquedas, los metadatos, el reproductor y las descargas a través del proxy indicado. Acepta URLs `http`, `https`, `socks4`, `socks4a`, `socks5` y `socks5h`, p. ej. `socks5://127.0.0.1:1080`. |

### Sistema
| Ajuste | Predeterminado | Explicación |
|---------|---------|-------------|
| **Versión actual / actualizar componentes** | Automático | Muestra la versión local de `yt-dlp` y permite actualizar los componentes descargados por la aplicación, como `yt-dlp` y `deno`, en `~/.local/share/bigtube/bin/`. |
| **Buscar actualizaciones al iniciar** | Habilitado | Busca componentes `yt-dlp`/`deno` más recientes al iniciar la aplicación. |
| **Monitor del portapapeles** | Deshabilitado | Detecta automáticamente los enlaces de video copiados al portapapeles mientras la aplicación está abierta. |
| **Notificaciones del sistema** | Habilitado | Controla las notificaciones del sistema para los eventos y errores de descarga. |

### Almacenamiento y privacidad
| Ajuste | Predeterminado | Explicación |
|---------|---------|-------------|
| **Borrar datos al salir** | Deshabilitado | Al cerrar la aplicación, borra los historiales de descargas, búsquedas y conversiones. La configuración de la aplicación se conserva. Cuando está habilitado, las opciones de "guardar historial" se deshabilitan en la interfaz. |
| **Exportar copia de seguridad** | Acción manual | Guarda una copia de seguridad completa — la configuración más los historiales de descargas, búsquedas y conversiones, las descargas programadas, la caché de listas de reproducción y los favoritos — en un único archivo JSON. |
| **Importar copia de seguridad** | Acción manual | Restaura toda la configuración y los datos desde un archivo de copia de seguridad válido. |
| **Borrar todos los datos de la aplicación** | Acción manual | Elimina de forma permanente `config.json`, `history.json`, `search_history.json` y `converter_history.json`, vuelve a crear la configuración predeterminada y cierra la aplicación. |

### Claves de `config.json`
| Clave | Valor predeterminado | Usado por |
|-----|---------------|---------|
| `download_path` | `~/Downloads/BigTube/` | Carpeta de descargas |
| `theme_mode` | `system` | Tema de la interfaz |
| `theme_color` | `default` | Esquema de color |
| `default_quality` | `ask` | Calidad preferida |
| `max_concurrent_downloads` | `3` | Descargas simultáneas |
| `max_download_history` | `100` | Máx. de elementos en la lista de descargas |
| `max_converter_history` | `50` | Máx. de elementos en la lista del conversor |
| `add_metadata` | `false` | Metadatos en las descargas |
| `embed_subtitles` | `false` | Bandera de subtítulos heredada (migrada a `subtitle_mode`) |
| `subtitle_mode` | `off` | Manejo de subtítulos: `off`, `embed`, `file`, `both` |
| `subtitle_langs` | `en,pt,es` | Idiomas de subtítulos a obtener |
| `subtitle_auto` | `true` | Incluir subtítulos autogenerados |
| `save_history` | `true` | Historial de descargas |
| `save_search_history` | `true` | Historial de búsquedas |
| `enable_suggestions` | `true` | Sugerencias de búsqueda |
| `max_suggestions` | `10` | Número de sugerencias |
| `search_limit` | `15` | Número de resultados de búsqueda |
| `save_converter_history` | `true` | Historial del convertidor |
| `auto_clear_finished` | `false` | Borrar historiales al salir |
| `converter_path` | `~/Downloads/BigTube/Converted/` | Carpeta de salida del convertidor |
| `use_source_folder` | `false` | El convertidor guarda en el origen |
| `monitor_clipboard` | `false` | Monitor del portapapeles |
| `concurrent_fragments` | `16` | Fragmentos paralelos por descarga |
| `rate_limit` | `0` | Límite de velocidad en KB/s |
| `system_notifications` | `true` | Notificaciones del sistema |
| `post_process_cmd` | `""` | Comando posterior a la descarga |
| `cookies_file` | `""` | Archivo de cookies |
| `cookies_browser` | `""` | Cookies del navegador |
| `user_agent` | `""` | User-Agent personalizado |
| `proxy` | `""` | Proxy |
| `sponsorblock_mode` | `off` | SponsorBlock: `off`, `mark`, `remove` |
| `sponsorblock_cats` | `sponsor,selfpromo,interaction` | Categorías de SponsorBlock a aplicar |
| `preview_quality` | `360p` | Calidad de vista previa del reproductor de la aplicación |
| `remove_on_complete` | `false` | Eliminar las descargas finalizadas de la lista |
| `remove_on_cancel` | `false` | Eliminar las descargas canceladas de la lista |
| `converter_remove_on_complete` | `false` | Eliminar las conversiones finalizadas de la lista |
| `converter_remove_on_cancel` | `false` | Eliminar las conversiones canceladas de la lista |
| `check_updates_on_startup` | `true` | Buscar actualizaciones de `yt-dlp`/`deno` al iniciar |

> Compatibilidad: las configuraciones más antiguas con la clave `download_subtitles` se migran automáticamente a `embed_subtitles`.

### Variables de entorno
| Variable | Efecto |
|----------|--------|
| `BIGTUBE_NO_FULL_REDRAW=1` | Omite el workaround de redibujado completo de GSK. BigTube fuerza redibujados completos para evitar "fantasmas" al desplazar (texto/miniaturas que quedan pegados) en ciertas combinaciones GTK4/Mesa/KWin. Úsalo si tu sistema no está afectado, para ahorrar CPU/batería. |
| `GSK_RENDERER` | Variable estándar de GTK para elegir el renderizador (`gl`, `vulkan`, `cairo`, …); se respeta tal cual. |

---

## 📋 Dependencias del sistema

Entorno de ejecución (requerido para ejecutar el binario):

```bash
# Arch Linux
sudo pacman -S gtk4 libadwaita gstreamer gst-plugins-base gst-plugins-good \
               gst-plugins-bad gst-plugin-gtk4 yt-dlp
# optional: ffmpeg (audio extraction and media conversion)
sudo pacman -S ffmpeg

# Ubuntu/Debian (22.04+)
sudo apt install libgtk-4-1 libadwaita-1-0 \
                 gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
                 gstreamer1.0-plugins-bad gstreamer1.0-gtk4 yt-dlp ffmpeg

# Fedora
sudo dnf install gtk4 libadwaita gstreamer1-plugins-base \
                 gstreamer1-plugins-good gstreamer1-plugins-bad-free \
                 yt-dlp ffmpeg
```

Para **compilar desde el código fuente**, añade el conjunto de herramientas de Rust y los encabezados de desarrollo:

```bash
# Arch Linux
sudo pacman -S rustup gtk4 libadwaita gstreamer base-devel
rustup default stable
```

---

## 🤝 Contribuir

¡Las contribuciones son bienvenidas! No dudes en:

1. Abrir un **Issue** para reportar errores o sugerir funciones
2. Enviar un **Pull Request** con mejoras
3. Ayudar con las traducciones

---

## 💖 Apoya el proyecto

Si **BigTube** te resulta útil, considera apoyar su desarrollo. ¡Toda ayuda es muy bienvenida! ❤️

[![GitHub Sponsors](https://img.shields.io/badge/GitHub-Sponsors-EA4AAA?logo=githubsponsors&logoColor=white)](https://github.com/sponsors/eltonfabricio10)

**PIX** (clave aleatoria, para donaciones desde Brasil):

```
a30c24f3-490f-424b-93d3-f1181380bc30
```

> Consejo: también puedes encontrar estas opciones dentro de la aplicación, en **Menú → Donaciones** (con un código QR de PIX y "Copiar y pegar").

---

## 📄 Licencia

Este proyecto está bajo la licencia **MIT**. Consulta el archivo [LICENSE](LICENSE) para más detalles.

---

<p align="center">
  Hecho con ❤️ por <a href="https://github.com/eltonfabricio10">eltonff</a>
</p>
