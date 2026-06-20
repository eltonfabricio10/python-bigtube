<p align="center">
  <img src="https://raw.githubusercontent.com/eltonfabricio10/bigtube/main/assets/banner.png" alt="BigTube Banner" width="100%">
</p>

<p align="center">
  <b>English</b> · <a href="docs/README.pt-BR.md">Português (BR)</a> · <a href="docs/README.es.md">Español</a> · <a href="docs/README.fr.md">Français</a>
</p>

# 🎬 BigTube

> **The Ultimate Multimedia Downloader for Linux**

**BigTube** is a modern, fast, and elegant desktop application built in **Rust** with **GTK4**, **Libadwaita**, and **GStreamer**. Designed for those who accept nothing less than perfection when downloading content from the internet, BigTube turns the complexity of `yt-dlp` into an intuitive and powerful tool — now as a native binary, with no Python runtime dependencies.

> ℹ️ As of version **2.0**, BigTube has been rewritten in Rust. The recommended AUR package is now **`bigtube-bin`** (precompiled binary). The old `bigtube` (Python) package has been discontinued.

---

## 📸 Screenshots

<p align="center">
  <img src="docs/screenshots/01-main.png" alt="BigTube — Search Manager" width="80%">
</p>

<p align="center">
  <img src="docs/screenshots/04-formats.png" alt="Side-by-side video and audio quality picker" width="48%">
  &nbsp;
  <img src="docs/screenshots/02-settings.png" alt="Settings" width="48%">
</p>

<p align="center">
  <img src="docs/screenshots/03-converter.png" alt="Built-in media converter" width="48%">
  &nbsp;
  <img src="docs/screenshots/05-donations.png" alt="Donations dialog" width="30%">
</p>

---

## ✨ Features

### 🔍 Search & Discovery
- **Built-in YouTube search** - Search for videos without opening a browser
- **YouTube Music search** - Find songs, music videos, and podcasts
- **Direct Links** - Support for 400+ sites via URL
- **Playlists in results** - YouTube searches return playlists alongside videos; click **Open playlist** to open a modal with all videos, with buttons for **Play all**, **Download all**, and a selection mode to download only the checked ones
- **Playlists by link** - Paste a YouTube playlist link (`playlist?list=` or `watch?v=...&list=`) and the search lists all its videos

### ⬇️ Advanced Downloads
| Feature | Description |
|---------|-------------|
| **Video Quality** | 4K (2160p), 2K (1440p), 1080p, 720p, 480p, 360p, 240p, 144p |
| **Audio Formats** | MP3, M4A with high-quality extraction |
| **Metadata** | Automatic embedding of tags, album, and artist |
| **Subtitles** | Download and embed subtitles (automatic + manual) |
| **Resume** | Continue interrupted downloads |

### 🔄 Media Converter
- Video-to-video conversion (MKV, MP4, WebM)
- Audio extraction and conversion
- Subtitle merging
- Batch conversion queue
- Real-time progress with ETA

### 📺 Built-in Player
- **GStreamer** playback engine (native, integrated with GTK4)
- Video preview before downloading
- Playlist navigation (Prev / Play-Pause / **Stop** / Next)
- Detachable video window

### 🎨 Appearance Customization
| Mode | Description |
|------|-------------|
| **Theme** | Light / Dark / Follow System |
| **Colors** | 10+ color schemes (Default, Violet, Emerald, Nordic, Gruvbox, Catppuccin, Dracula, Tokyo Night, Rosé Pine, Solarized, Monokai, Cyberpunk, BigTube Brand) |
| **Style** | Modern glassmorphism interface |

### 📊 Management
- Download history
- Conversion history
- Search history
- Option to automatically clear data on exit

---

## 🛠️ Technologies

| Technology | Role |
|------------|------|
| **Rust 2021** | Application core (native binary) |
| **GTK4 + Libadwaita** | Native GNOME interface |
| **GStreamer** | Playback engine |
| **yt-dlp** | Download engine |
| **FFmpeg** | Media conversion |
| **Cargo** | Build and dependency management |

> The project is a Cargo workspace with three crates: **`bigtube-core`** (logic/engine), **`bigtube-cli`** (headless `bigtube` binary), and **`bigtube-gui`** (graphical interface `bigtube-gui`).

---

## 🚀 Installation

### Arch Linux (AUR) — recommended
Precompiled binary package (`bigtube-bin`): installs fast, **without compiling anything** on your machine.
```bash
yay -S bigtube-bin
# or
paru -S bigtube-bin
```
> The binary provides and replaces the old `bigtube` package (`provides=bigtube`, `conflicts=bigtube`).

### Debian / Ubuntu (.deb)
Download the `.deb` from the [latest release](https://github.com/eltonfabricio10/bigtube/releases/latest) and install it (pulls in dependencies automatically):
```bash
sudo apt install ./bigtube_*_amd64.deb
```
> Built on Ubuntu 24.04, so it needs **Ubuntu 24.04+** or **Debian 13+** (GTK ≥ 4.12, libadwaita ≥ 1.5).

### Fedora (.rpm)
Download the `.rpm` from the [latest release](https://github.com/eltonfabricio10/bigtube/releases/latest) and install it:
```bash
sudo dnf install ./bigtube-*.x86_64.rpm
```
> Built on Fedora 40 (needs **Fedora 40+**). `ffmpeg` (audio extraction/conversion) lives in [RPM Fusion](https://rpmfusion.org/) — enable it and `sudo dnf install ffmpeg` for those features.

### Building from source (Cargo)
Requires the Rust toolchain (`rustup`) and the system dependencies listed below.
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

To install system-wide from a local build:
```bash
sudo install -Dm755 target/release/bigtube-gui /usr/bin/bigtube-gui
sudo install -Dm755 target/release/bigtube     /usr/bin/bigtube
sudo install -Dm644 ../src/bigtube/data/bigtube.svg /usr/share/icons/hicolor/scalable/apps/bigtube.svg
sudo install -Dm644 ../src/bigtube/data/bigtube.png /usr/share/icons/hicolor/512x512/apps/bigtube.png
sudo install -Dm644 packaging/io.github.eltonfabricio10.bigtube.desktop /usr/share/applications/io.github.eltonfabricio10.bigtube.desktop
```

---

## ⌨️ Command Line

The Rust port exposes **two binaries**:

| Binary | Role |
|--------|------|
| `bigtube-gui` | Opens the graphical interface |
| `bigtube` | Headless mode (download directly from the terminal, no GUI) |

### Graphical interface
```bash
bigtube-gui      # opens the BigTube window
```

### Headless mode (`bigtube`)
```bash
bigtube -d <URL> [options]
```

| Option | Description |
|--------|-------------|
| `-d, --download URL` | Downloads the URL directly from the terminal, without opening the window |
| `-o, --output DIR` | Destination folder for `--download` (default: configured folder) |
| `--audio-only` | With `--download`, extracts audio as MP3 |
| `--format FMT` | With `--download`, custom format selector for `yt-dlp -f` |
| `--yt-dlp-version` | Shows the bundled `yt-dlp` version |
| `--version` | Shows the BigTube version |
| `--help` | Shows help |

### Examples
```bash
bigtube-gui                                      # opens the GUI
bigtube -d https://youtube.com/watch?v=...       # headless download
bigtube -d <url> -o ~/Music --audio-only         # headless MP3 audio
bigtube -d <url> --format "bestvideo+bestaudio"  # custom format
```

---

## 📁 Directory Structure

| Location | Contents |
|----------|----------|
| `~/.config/bigtube/` | Settings and histories |
| `~/.config/bigtube/config.json` | Application settings |
| `~/.config/bigtube/history.json` | Download history |
| `~/.local/share/bigtube/bin/` | Binaries (yt-dlp) |
| `~/.cache/bigtube/thumbnails/` | Thumbnail cache |
| `~/Downloads/BigTube/` | Default downloads folder |

---

## ⚙️ Available Settings

Preferences are saved in `~/.config/bigtube/config.json`. When the file doesn't exist or is corrupted, BigTube recreates the configuration with default values. Empty paths or disabled options simply make the app fall back to default behavior.

### Appearance and components
| Setting | Default | Explanation |
|---------|---------|-------------|
| **Interface theme** | Follow system | Defines whether the interface uses the system theme, forces a light theme, or forces a dark theme. |
| **Color scheme** | Default Blue | Changes the visual palette/accent of the interface. Options: Default Blue, Modern Violet, Emerald Green, Sunburst Orange, Vibrant Rose, Nordic Cyan, Nordic Snow, Gruvbox Retro, Catppuccin Mocha, Dracula Dark, Tokyo Night, Rosé Pine, Solarized Dark, Monokai Pro, Cyberpunk Neon, and BigTube Brand. |
| **Current version / update components** | Automatic | Shows the local `yt-dlp` version and lets you update the components downloaded by the app, such as `yt-dlp` and `deno`, in `~/.local/share/bigtube/bin/`. |

### Search
| Setting | Default | Explanation |
|---------|---------|-------------|
| **Save search history** | Enabled | Stores your searches locally in `search_history.json`, allowing you to reuse previous queries. |
| **Enable search suggestions** | Enabled | Shows suggestions as you type, using the local search history. |
| **Maximum suggestions** | 10 | Defines how many suggestions can appear at once. Accepts values from 1 to 50. |
| **Clear search history** | Manual action | Removes all saved search history entries. Does not delete downloaded files. |
| **Maximum search results** | 15 | Defines how many results BigTube requests from `yt-dlp` for text searches. Accepts values from 5 to 100. |

### Downloads
| Setting | Default | Explanation |
|---------|---------|-------------|
| **Simultaneous downloads** | 3 | Controls how many videos can download at the same time. Accepts values from 1 to 10. |
| **Download folder** | `~/Downloads/BigTube/` | Defines where downloaded files are saved. The app creates the folder when needed. |
| **Clipboard monitor** | Disabled | Automatically detects video links copied to the clipboard while the app is open. |
| **System notifications** | Enabled | Controls system notifications for download events and errors. |
| **Preferred quality** | Ask every time | Defines the default format for new downloads. It can ask on each download, download the best video, or choose 4K, 2K, 1080p, 720p, 480p, 360p, 240p, 144p, or download audio only as MP3/M4A. |
| **Add metadata** | Disabled | Tries to embed artist, album, cover, and other metadata into downloaded files. Requires `ffmpeg`; if it isn't installed, the app skips this step. |
| **Embed subtitles** | Disabled | Tries to download manual and automatic subtitles and embed them into the final file. Currently looks for `en.*`, `pt.*`, and `es.*` languages. Requires `ffmpeg`. |
| **Concurrent fragments** | 16 | Defines how many parallel fragments `yt-dlp` uses per download. Accepts values from 1 to 16. Higher values can speed up segmented downloads but also increase network usage. |
| **Speed limit** | 0 KB/s | Limits download speed in KB/s. `0` means no limit. |
| **Post-processing command** | Empty | Runs a command after the download using `yt-dlp --exec`. Use `{}` in the command to represent the downloaded file. |
| **Cookies file** | Empty | Uses a Netscape-format `cookies.txt` file with `yt-dlp --cookies`, useful for content that requires an authenticated session. |
| **Browser cookies** | None | Imports cookies directly from a detected browser, such as Firefox, Chrome, Chromium, Brave, Microsoft Edge, Vivaldi, or Opera, using `yt-dlp --cookies-from-browser`. |
| **User-Agent** | BigTube default | Overrides the User-Agent sent to `yt-dlp`. If left empty, the app uses a safe Chrome-based User-Agent. |
| **Proxy** | Empty | Routes searches, metadata, player, and downloads through the given proxy. Accepts `http`, `https`, `socks4`, `socks4a`, `socks5`, and `socks5h` URLs, e.g. `socks5://127.0.0.1:1080`. |
| **Save download history** | Enabled | Keeps a local record of downloads in `history.json`, used by the history/list view. |

#### Quality options
| Option | Explanation |
|--------|-------------|
| **Ask every time** | Shows the quality/format choice at download time. |
| **Best (MKV)** | Downloads the best available video and audio combination and merges the result. |
| **4K, 2K, 1080p, 720p, 480p, 360p, 240p, 144p** | Prioritizes MP4/AVC video at the chosen resolution with M4A audio; if that exact format doesn't exist, `yt-dlp` uses the best compatible alternative defined in the preset. |
| **Audio (MP3)** | Extracts audio only, converts to high-quality MP3, and tries to embed the thumbnail. |
| **Audio (M4A)** | Downloads audio only, prioritizing the M4A codec/container. |

### Media converter
| Setting | Default | Explanation |
|---------|---------|-------------|
| **Save to source folder** | Disabled | When enabled, the converted file is saved next to the original file. |
| **Default output folder** | `~/Downloads/BigTube/Converted/` | Defines the folder used by the converter when "save to source folder" is disabled. |
| **Save conversion history** | Enabled | Keeps a local record of conversions in `converter_history.json`. |

### Storage and privacy
| Setting | Default | Explanation |
|---------|---------|-------------|
| **Clear data on exit** | Disabled | When closing the app, clears the download, search, and conversion histories. App settings are preserved. When enabled, the "save history" options are disabled in the interface. |
| **Export history** | Manual action | Saves the download history to a JSON file, by default `bigtube_history.json`. |
| **Import history** | Manual action | Restores a download history from a valid JSON file. |
| **Clear all app data** | Manual action | Permanently deletes `config.json`, `history.json`, `search_history.json`, and `converter_history.json`, recreates the default configuration, and quits the application. |

### `config.json` keys
| Key | Default value | Used by |
|-----|---------------|---------|
| `download_path` | `~/Downloads/BigTube/` | Download folder |
| `theme_mode` | `system` | Interface theme |
| `theme_color` | `default` | Color scheme |
| `default_quality` | `ask` | Preferred quality |
| `max_concurrent_downloads` | `3` | Simultaneous downloads |
| `add_metadata` | `false` | Metadata on downloads |
| `embed_subtitles` | `false` | Subtitles on downloads |
| `save_history` | `true` | Download history |
| `save_search_history` | `true` | Search history |
| `enable_suggestions` | `true` | Search suggestions |
| `max_suggestions` | `10` | Number of suggestions |
| `search_limit` | `15` | Number of search results |
| `save_converter_history` | `true` | Converter history |
| `auto_clear_finished` | `false` | Clear histories on exit |
| `converter_path` | `~/Downloads/BigTube/Converted/` | Converter output folder |
| `use_source_folder` | `false` | Converter saves to source |
| `monitor_clipboard` | `false` | Clipboard monitor |
| `concurrent_fragments` | `16` | Parallel fragments per download |
| `rate_limit` | `0` | Speed limit in KB/s |
| `system_notifications` | `true` | System notifications |
| `post_process_cmd` | `""` | Post-download command |
| `cookies_file` | `""` | Cookies file |
| `cookies_browser` | `""` | Browser cookies |
| `user_agent` | `""` | Custom User-Agent |
| `proxy` | `""` | Proxy |

> Compatibility: older configurations with the `download_subtitles` key are automatically migrated to `embed_subtitles`.

---

## 📋 System Dependencies

Runtime (required to run the binary):

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

To **build from source**, add the Rust toolchain and development headers:

```bash
# Arch Linux
sudo pacman -S rustup gtk4 libadwaita gstreamer base-devel
rustup default stable
```

---

## 🤝 Contributing

Contributions are welcome! Feel free to:

1. Open an **Issue** to report bugs or suggest features
2. Submit a **Pull Request** with improvements
3. Help with translations

---

## 💖 Support the Project

If **BigTube** is useful to you, consider supporting its development. Any help is very welcome! ❤️

[![GitHub Sponsors](https://img.shields.io/badge/GitHub-Sponsors-EA4AAA?logo=githubsponsors&logoColor=white)](https://github.com/sponsors/eltonfabricio10)

**PIX** (random key, for donations from Brazil):

```
a30c24f3-490f-424b-93d3-f1181380bc30
```

> Tip: you can also find these options inside the app, under **Menu → Donations** (with a PIX QR code and "Copy & Paste").

---

## 📄 License

This project is licensed under the **MIT** license. See the [LICENSE](LICENSE) file for more details.

---

<p align="center">
  Made with ❤️ by <a href="https://github.com/eltonfabricio10">eltonff</a>
</p>
