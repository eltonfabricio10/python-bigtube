<p align="center">
  <img src="https://raw.githubusercontent.com/eltonfabricio10/bigtube/main/assets/banner.png" alt="BigTube Banner" width="100%">
</p>

<p align="center">
  <a href="../README.md">English</a> · <a href="README.pt-BR.md">Português (BR)</a> · <b>Español</b> · <a href="README.fr.md">Français</a>
</p>

# 🎬 BigTube

> **El descargador multimedia definitivo para Linux**

**BigTube** es una aplicación de escritorio moderna, rápida y elegante creada en **Rust** con **GTK4**, **Libadwaita** y **GStreamer**. Diseñada para quienes no aceptan nada menos que la perfección al descargar contenido de internet, BigTube convierte la complejidad de `yt-dlp` en una herramienta intuitiva y potente, ahora como binario nativo, sin dependencias del entorno de ejecución de Python.

> ℹ️ A partir de la versión **2.0**, BigTube se reescribió en Rust. El paquete de AUR recomendado ahora es **`bigtube-bin`** (binario precompilado). El antiguo paquete `bigtube` (Python) se ha descontinuado.

---

## 📸 Capturas de pantalla

<p align="center">
  <img src="screenshots/01-main.png" alt="BigTube — Administrador de búsqueda" width="80%">
</p>

<p align="center">
  <img src="screenshots/04-formats.png" alt="Selector de calidad de vídeo y audio en paralelo" width="48%">
  &nbsp;
  <img src="screenshots/02-settings.png" alt="Ajustes" width="48%">
</p>

<p align="center">
  <img src="screenshots/03-converter.png" alt="Conversor de medios integrado" width="48%">
  &nbsp;
  <img src="screenshots/05-donations.png" alt="Ventana de donaciones" width="30%">
</p>

---

## ✨ Características

### 🔍 Búsqueda y descubrimiento
- **Búsqueda de YouTube integrada** - Busca videos sin abrir un navegador
- **Búsqueda en YouTube Music** - Encuentra canciones, videos musicales y pódcasts
- **Enlaces directos** - Compatibilidad con más de 400 sitios mediante URL
- **Listas de reproducción en los resultados** - Las búsquedas de YouTube devuelven listas de reproducción junto con los videos; haz clic en **Abrir lista de reproducción** para abrir una ventana modal con todos los videos, con botones para **Reproducir todo**, **Descargar todo** y un modo de selección para descargar solo los marcados
- **Listas de reproducción por enlace** - Pega un enlace de una lista de reproducción de YouTube (`playlist?list=` o `watch?v=...&list=`) y la búsqueda mostrará todos sus videos

### ⬇️ Descargas avanzadas
| Característica | Descripción |
|---------|-------------|
| **Calidad de video** | 4K (2160p), 2K (1440p), 1080p, 720p, 480p, 360p, 240p, 144p |
| **Formatos de audio** | MP3, M4A con extracción de alta calidad |
| **Metadatos** | Incrustación automática de etiquetas, álbum y artista |
| **Subtítulos** | Descarga e incrusta subtítulos (automáticos + manuales) |
| **Reanudar** | Continúa descargas interrumpidas |

### 🔄 Convertidor multimedia
- Conversión de video a video (MKV, MP4, WebM)
- Extracción y conversión de audio
- Combinación de subtítulos
- Cola de conversión por lotes
- Progreso en tiempo real con tiempo estimado (ETA)

### 📺 Reproductor integrado
- Motor de reproducción **GStreamer** (nativo, integrado con GTK4)
- Vista previa del video antes de descargar
- Navegación por la lista de reproducción (Anterior / Reproducir-Pausar / **Detener** / Siguiente)
- Ventana de video desacoplable

### 🎨 Personalización de la apariencia
| Modo | Descripción |
|------|-------------|
| **Tema** | Claro / Oscuro / Seguir al sistema |
| **Colores** | Más de 10 esquemas de color (Predeterminado, Violeta, Esmeralda, Nórdico, Gruvbox, Catppuccin, Dracula, Tokyo Night, Rosé Pine, Solarized, Monokai, Cyberpunk, Marca BigTube) |
| **Estilo** | Interfaz moderna con efecto glassmorphism |

### 📊 Gestión
- Historial de descargas
- Historial de conversiones
- Historial de búsquedas
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
> El binario provee y reemplaza al antiguo paquete `bigtube` (`provides=bigtube`, `conflicts=bigtube`).

### Flatpak — cualquier distribución de Linux
El Flatpak incluye el GTK4/libadwaita correctos, GStreamer **y** una compilación
de `ffmpeg`, así que funciona igual en cualquier distribución (Ubuntu, Fedora,
Debian, openSUSE, Arch, …), sin importar las versiones de las bibliotecas del
sistema.

```bash
# Compilar e instalar localmente desde el manifiesto
flatpak install flathub org.gnome.Platform//47 org.gnome.Sdk//47 \
    org.freedesktop.Sdk.Extension.rust-stable//24.08
flatpak-builder --user --install --force-clean build-dir flatpak/org.big.bigtube.yaml
flatpak run org.big.bigtube
```

> El CI genera un paquete `bigtube.flatpak` en cada cambio (workflow **Flatpak**).
> Está prevista la publicación en Flathub para una instalación con un solo comando.

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
sudo install -Dm644 ../src/bigtube/data/bigtube.svg /usr/share/icons/hicolor/scalable/apps/bigtube.svg
sudo install -Dm644 ../src/bigtube/data/bigtube.png /usr/share/icons/hicolor/512x512/apps/bigtube.png
sudo install -Dm644 packaging/org.big.bigtube.desktop /usr/share/applications/org.big.bigtube.desktop
```

---

## ⌨️ Línea de comandos

La adaptación a Rust expone **dos binarios**:

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
| `~/.local/share/bigtube/bin/` | Binarios (yt-dlp) |
| `~/.cache/bigtube/thumbnails/` | Caché de miniaturas |
| `~/Downloads/BigTube/` | Carpeta de descargas predeterminada |

---

## ⚙️ Ajustes disponibles

Las preferencias se guardan en `~/.config/bigtube/config.json`. Cuando el archivo no existe o está dañado, BigTube vuelve a crear la configuración con los valores predeterminados. Las rutas vacías o las opciones deshabilitadas simplemente hacen que la aplicación recurra al comportamiento predeterminado.

### Apariencia y componentes
| Ajuste | Predeterminado | Explicación |
|---------|---------|-------------|
| **Tema de la interfaz** | Seguir al sistema | Define si la interfaz usa el tema del sistema, fuerza un tema claro o fuerza un tema oscuro. |
| **Esquema de color** | Azul predeterminado | Cambia la paleta/color de acento de la interfaz. Opciones: Azul predeterminado, Violeta moderno, Verde esmeralda, Naranja Sunburst, Rosa vibrante, Cian nórdico, Nieve nórdica, Gruvbox Retro, Catppuccin Mocha, Dracula Dark, Tokyo Night, Rosé Pine, Solarized Dark, Monokai Pro, Cyberpunk Neon y Marca BigTube. |
| **Versión actual / actualizar componentes** | Automático | Muestra la versión local de `yt-dlp` y permite actualizar los componentes descargados por la aplicación, como `yt-dlp` y `deno`, en `~/.local/share/bigtube/bin/`. |

### Búsqueda
| Ajuste | Predeterminado | Explicación |
|---------|---------|-------------|
| **Guardar historial de búsquedas** | Habilitado | Almacena tus búsquedas localmente en `search_history.json`, lo que te permite reutilizar consultas anteriores. |
| **Habilitar sugerencias de búsqueda** | Habilitado | Muestra sugerencias mientras escribes, usando el historial de búsquedas local. |
| **Máximo de sugerencias** | 10 | Define cuántas sugerencias pueden aparecer a la vez. Acepta valores de 1 a 50. |
| **Borrar historial de búsquedas** | Acción manual | Elimina todas las entradas guardadas del historial de búsquedas. No borra los archivos descargados. |
| **Máximo de resultados de búsqueda** | 15 | Define cuántos resultados solicita BigTube a `yt-dlp` para las búsquedas de texto. Acepta valores de 5 a 100. |

### Descargas
| Ajuste | Predeterminado | Explicación |
|---------|---------|-------------|
| **Descargas simultáneas** | 3 | Controla cuántos videos se pueden descargar al mismo tiempo. Acepta valores de 1 a 10. |
| **Carpeta de descargas** | `~/Downloads/BigTube/` | Define dónde se guardan los archivos descargados. La aplicación crea la carpeta cuando es necesario. |
| **Monitor del portapapeles** | Deshabilitado | Detecta automáticamente los enlaces de video copiados al portapapeles mientras la aplicación está abierta. |
| **Notificaciones del sistema** | Habilitado | Controla las notificaciones del sistema para los eventos y errores de descarga. |
| **Calidad preferida** | Preguntar siempre | Define el formato predeterminado para las nuevas descargas. Puede preguntar en cada descarga, descargar el mejor video o elegir 4K, 2K, 1080p, 720p, 480p, 360p, 240p, 144p, o descargar solo el audio como MP3/M4A. |
| **Agregar metadatos** | Deshabilitado | Intenta incrustar el artista, el álbum, la portada y otros metadatos en los archivos descargados. Requiere `ffmpeg`; si no está instalado, la aplicación omite este paso. |
| **Incrustar subtítulos** | Deshabilitado | Intenta descargar subtítulos manuales y automáticos e incrustarlos en el archivo final. Actualmente busca los idiomas `en.*`, `pt.*` y `es.*`. Requiere `ffmpeg`. |
| **Fragmentos concurrentes** | 16 | Define cuántos fragmentos paralelos usa `yt-dlp` por descarga. Acepta valores de 1 a 16. Los valores más altos pueden acelerar las descargas segmentadas, pero también aumentan el uso de la red. |
| **Límite de velocidad** | 0 KB/s | Limita la velocidad de descarga en KB/s. `0` significa sin límite. |
| **Comando de posprocesamiento** | Vacío | Ejecuta un comando después de la descarga usando `yt-dlp --exec`. Usa `{}` en el comando para representar el archivo descargado. |
| **Archivo de cookies** | Vacío | Usa un archivo `cookies.txt` en formato Netscape con `yt-dlp --cookies`, útil para contenido que requiere una sesión autenticada. |
| **Cookies del navegador** | Ninguno | Importa cookies directamente de un navegador detectado, como Firefox, Chrome, Chromium, Brave, Microsoft Edge, Vivaldi u Opera, usando `yt-dlp --cookies-from-browser`. |
| **User-Agent** | Predeterminado de BigTube | Reemplaza el User-Agent enviado a `yt-dlp`. Si se deja vacío, la aplicación usa un User-Agent seguro basado en Chrome. |
| **Proxy** | Vacío | Enruta las búsquedas, los metadatos, el reproductor y las descargas a través del proxy indicado. Acepta URLs `http`, `https`, `socks4`, `socks4a`, `socks5` y `socks5h`, p. ej. `socks5://127.0.0.1:1080`. |
| **Guardar historial de descargas** | Habilitado | Mantiene un registro local de las descargas en `history.json`, usado por la vista de historial/lista. |

#### Opciones de calidad
| Opción | Explicación |
|--------|-------------|
| **Preguntar siempre** | Muestra la elección de calidad/formato en el momento de la descarga. |
| **Mejor (MKV)** | Descarga la mejor combinación disponible de video y audio y combina el resultado. |
| **4K, 2K, 1080p, 720p, 480p, 360p, 240p, 144p** | Prioriza el video MP4/AVC en la resolución elegida con audio M4A; si ese formato exacto no existe, `yt-dlp` usa la mejor alternativa compatible definida en el preajuste. |
| **Audio (MP3)** | Extrae solo el audio, lo convierte a MP3 de alta calidad e intenta incrustar la miniatura. |
| **Audio (M4A)** | Descarga solo el audio, priorizando el códec/contenedor M4A. |

### Convertidor multimedia
| Ajuste | Predeterminado | Explicación |
|---------|---------|-------------|
| **Guardar en la carpeta de origen** | Deshabilitado | Cuando está habilitado, el archivo convertido se guarda junto al archivo original. |
| **Carpeta de salida predeterminada** | `~/Downloads/BigTube/Converted/` | Define la carpeta usada por el convertidor cuando "guardar en la carpeta de origen" está deshabilitado. |
| **Guardar historial de conversiones** | Habilitado | Mantiene un registro local de las conversiones en `converter_history.json`. |

### Almacenamiento y privacidad
| Ajuste | Predeterminado | Explicación |
|---------|---------|-------------|
| **Borrar datos al salir** | Deshabilitado | Al cerrar la aplicación, borra los historiales de descargas, búsquedas y conversiones. La configuración de la aplicación se conserva. Cuando está habilitado, las opciones de "guardar historial" se deshabilitan en la interfaz. |
| **Exportar historial** | Acción manual | Guarda el historial de descargas en un archivo JSON, de forma predeterminada `bigtube_history.json`. |
| **Importar historial** | Acción manual | Restaura un historial de descargas desde un archivo JSON válido. |
| **Borrar todos los datos de la aplicación** | Acción manual | Elimina de forma permanente `config.json`, `history.json`, `search_history.json` y `converter_history.json`, vuelve a crear la configuración predeterminada y cierra la aplicación. |

### Claves de `config.json`
| Clave | Valor predeterminado | Usado por |
|-----|---------------|---------|
| `download_path` | `~/Downloads/BigTube/` | Carpeta de descargas |
| `theme_mode` | `system` | Tema de la interfaz |
| `theme_color` | `default` | Esquema de color |
| `default_quality` | `ask` | Calidad preferida |
| `max_concurrent_downloads` | `3` | Descargas simultáneas |
| `add_metadata` | `false` | Metadatos en las descargas |
| `embed_subtitles` | `false` | Subtítulos en las descargas |
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

> Compatibilidad: las configuraciones más antiguas con la clave `download_subtitles` se migran automáticamente a `embed_subtitles`.

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
