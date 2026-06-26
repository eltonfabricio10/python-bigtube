<p align="center">
  <img src="https://raw.githubusercontent.com/eltonfabricio10/bigtube/main/assets/banner.png" alt="BigTube Banner" width="100%">
</p>

<p align="center">
  <b>English</b> · <a href="docs/README.pt-BR.md">Português (BR)</a> · <a href="docs/README.es.md">Español</a> · <a href="docs/README.fr.md">Français</a>
</p>

# 🎬 BigTube

> **The Ultimate Multimedia Downloader for Linux**

**BigTube** is a modern, fast, and elegant desktop application built in **Rust** with **GTK4**, **Libadwaita**, and **GStreamer**. Designed for those who accept nothing less than perfection when downloading content from the internet, BigTube turns the complexity of `yt-dlp` into an intuitive and powerful tool — a fast, native binary.

---

## 📸 Screenshots

#### 🔍 Search Manager
<p align="center">
  <img src="docs/screenshots/01-main.png" alt="BigTube — Search Manager" width="80%">
</p>

#### 🎚️ Format Picker &nbsp;·&nbsp; ⚙️ Settings
<p align="center">
  <img src="docs/screenshots/04-formats.png" alt="Side-by-side video and audio quality picker" width="48%">
  &nbsp;
  <img src="docs/screenshots/02-settings.png" alt="Settings" width="48%">
</p>

#### 🔄 Media Converter &nbsp;·&nbsp; 💖 Donations
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
| **Audio Formats** | MP3, M4A, Opus, FLAC, WAV, AAC with high-quality extraction |
| **Metadata** | Automatic embedding of tags, album, and artist |
| **Subtitles** | Embed and/or save as sidecar files, manual + auto-generated, per-language selection |
| **Scheduling** | Queue downloads to run later, one-off or on a recurring schedule |
| **SponsorBlock** | Skip in-video sponsor segments — mark them as chapters or cut them out (uses the [SponsorBlock](https://sponsor.ajay.app/) database) |
| **Concurrency** | Multiple simultaneous downloads with configurable parallel fragments |
| **Resume** | Continue interrupted downloads |

### 🔄 Media Converter
- Video-to-video conversion (MP4, MKV, WebM)
- Audio extraction and conversion (MP3, M4A, Opus, FLAC, WAV, AAC)
- Subtitle merging (embed and/or sidecar)
- Batch conversion queue
- Real-time progress with ETA

### 📺 Built-in Player
- **GStreamer** playback engine (native, integrated with GTK4)
- Video preview before downloading, with configurable preview quality (144p–720p)
- Playlist navigation (Prev / Play-Pause / **Stop** / Next), seek bar and volume
- Detachable video window

### 🎨 Appearance Customization
| Mode | Description |
|------|-------------|
| **Theme** | Light / Dark / Follow System |
| **Colors** | 16 color schemes (Default Blue, Modern Violet, Emerald Green, Sunburst Orange, Vibrant Rose, Nordic Cyan, Nordic Snow, Gruvbox Retro, Catppuccin Mocha, Dracula Dark, Tokyo Night, Rosé Pine, Solarized Dark, Monokai Pro, Cyberpunk Neon, BigTube Brand) |
| **Style** | Modern glassmorphism interface |

### 📊 Management
- Download history
- Conversion history
- Search history
- Scheduled downloads
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

### AppImage (portable, any distro)
Download `BigTube-*-x86_64.AppImage` from the [latest release](https://github.com/eltonfabricio10/bigtube/releases/latest), make it executable, and run it:
```bash
chmod +x BigTube-*-x86_64.AppImage
./BigTube-*-x86_64.AppImage
```
> Bundles GTK4/libadwaita and the GStreamer plugins (including the player's gtk4 sink), so it runs on any x86_64 system regardless of the distro's GTK version. `ffmpeg` and `yt-dlp` are still used at runtime if present; the app fetches `yt-dlp` into its own data dir on first run.
>
> **Note:** the AppImage needs **glibc ≥ 2.41** (Debian 13+, Ubuntu 25.10+, Fedora 42+, or a rolling distro like Arch/openSUSE Tumbleweed). On older systems use the `.deb`/`.rpm`/AUR packages instead.

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
sudo install -Dm644 ../assets/bigtube.svg /usr/share/icons/hicolor/scalable/apps/bigtube.svg
sudo install -Dm644 ../assets/bigtube.png /usr/share/icons/hicolor/512x512/apps/bigtube.png
sudo install -Dm644 packaging/io.github.eltonfabricio10.bigtube.desktop /usr/share/applications/io.github.eltonfabricio10.bigtube.desktop
```

---

## ⌨️ Command Line

BigTube ships **two binaries**:

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
| `~/.config/bigtube/search_history.json` | Search history |
| `~/.config/bigtube/converter_history.json` | Conversion history |
| `~/.config/bigtube/scheduled_downloads.json` | Scheduled downloads |
| `~/.local/share/bigtube/bin/` | Bundled binaries (`yt-dlp`, `deno`) |
| `~/.cache/bigtube/thumbnails/` | Thumbnail cache |
| `~/Downloads/BigTube/` | Default downloads folder |
| `~/Downloads/BigTube/Converted/` | Default converter output folder |

---

## ⚙️ Available Settings

Preferences are saved in `~/.config/bigtube/config.json`. When the file doesn't exist or is corrupted, BigTube recreates the configuration with default values. Empty paths or disabled options simply make the app fall back to default behavior.

> The settings page is organized into groups in this order: **Appearance**, **Search**, **Playback**, **Downloads**, **Performance**, **Post-Processing**, **Subtitles**, **Media converter**, **Network & advanced**, **System** and **Storage**.

### Appearance
| Setting | Default | Explanation |
|---------|---------|-------------|
| **Interface theme** | Follow system | Defines whether the interface uses the system theme, forces a light theme, or forces a dark theme. |
| **Color scheme** | Default Blue | Changes the visual palette/accent of the interface. Options: Default Blue, Modern Violet, Emerald Green, Sunburst Orange, Vibrant Rose, Nordic Cyan, Nordic Snow, Gruvbox Retro, Catppuccin Mocha, Dracula Dark, Tokyo Night, Rosé Pine, Solarized Dark, Monokai Pro, Cyberpunk Neon, and BigTube Brand. |

### Search
| Setting | Default | Explanation |
|---------|---------|-------------|
| **Save search history** | Enabled | Stores your searches locally in `search_history.json`, allowing you to reuse previous queries. |
| **Enable search suggestions** | Enabled | Shows suggestions as you type, using the local search history. |
| **Maximum suggestions** | 10 | Defines how many suggestions can appear at once. Accepts values from 1 to 50. |
| **Clear search history** | Manual action | Removes all saved search history entries. Does not delete downloaded files. |
| **Maximum search results** | 15 | Defines how many results BigTube requests from `yt-dlp` for text searches. Accepts values from 5 to 100. |

### Playback
| Setting | Default | Explanation |
|---------|---------|-------------|
| **Preview quality** | 360p | Quality used by the in-app player when previewing before download: `144p`, `240p`, `360p` (progressive), `480p`, or `720p` (HLS streaming). |

### Downloads
| Setting | Default | Explanation |
|---------|---------|-------------|
| **Download folder** | `~/Downloads/BigTube/` | Defines where downloaded files are saved. The app creates the folder when needed. |
| **Preferred quality** | Ask every time | Defines the default format for new downloads. It can ask on each download, download the best video, choose 4K, 2K, 1080p, 720p, 480p, 360p, 240p, 144p, or download audio only as MP3, M4A, Opus, FLAC, WAV, or AAC. |
| **Save download history** | Enabled | Keeps a local record of downloads in `history.json`, used by the history/list view. |
| **Maximum history entries** | 100 | How many download entries to keep in the list. Accepts values from 10 to 1000. |
| **Remove when complete** | Disabled | Automatically removes finished downloads from the list. |
| **Remove when cancelled** | Disabled | Automatically removes cancelled downloads from the list. |

#### Quality options
| Option | Explanation |
|--------|-------------|
| **Ask every time** | Shows the quality/format choice at download time. |
| **Best (MKV)** | Downloads the best available video and audio combination and merges the result. |
| **4K, 2K, 1080p, 720p, 480p, 360p, 240p, 144p** | Prioritizes MP4/AVC video at the chosen resolution with M4A audio; if that exact format doesn't exist, `yt-dlp` uses the best compatible alternative defined in the preset. |
| **Audio (MP3)** | Extracts audio only, converts to high-quality MP3, and tries to embed the thumbnail. |
| **Audio (M4A)** | Downloads audio only, prioritizing the M4A codec/container. |
| **Audio (Opus / FLAC / WAV / AAC)** | Extracts audio only and converts it to the chosen format at the highest quality. |

### Performance
| Setting | Default | Explanation |
|---------|---------|-------------|
| **Max simultaneous downloads** | 3 | Controls how many videos can download at the same time. Accepts values from 1 to 10. |
| **Concurrent fragments** | 16 | Defines how many parallel fragments `yt-dlp` uses per download. Accepts values from 1 to 16. Higher values can speed up segmented downloads but also increase network usage. |
| **Download speed limit** | 0 KB/s | Limits download speed in KB/s. `0` means no limit. Accepts values from 0 to 100000. |

### Post-Processing
| Setting | Default | Explanation |
|---------|---------|-------------|
| **Add metadata** | Disabled | Tries to embed artist, album, cover, and other metadata into downloaded files. Requires `ffmpeg`; if it isn't installed, the app skips this step. |
| **SponsorBlock** | Off | Skips in-video sponsor segments using the SponsorBlock database. `Mark chapters` adds chapter markers (non-destructive); `Remove segments` cuts them out. Requires `ffmpeg`. |
| **Post-processing command** | Empty | Runs a command after the download using `yt-dlp --exec`. Use `{}` in the command to represent the downloaded file. |

### Subtitles
| Setting | Default | Explanation |
|---------|---------|-------------|
| **Subtitles** | Off | Subtitle handling for downloads: `Off`, `Embed` into the file, save as a separate `File` (sidecar), or `Both`. Embedding requires `ffmpeg`. |
| **Languages** | `en,pt,es` | Comma-separated list of subtitle language codes to fetch (e.g. `en,pt,es`). |
| **Include auto-generated** | Enabled | Also fetches machine-generated (automatic) captions, not just manual ones. |

### Media converter
| Setting | Default | Explanation |
|---------|---------|-------------|
| **Save to source folder** | Disabled | When enabled, the converted file is saved next to the original file. |
| **Default output folder** | `~/Downloads/BigTube/Converted/` | Defines the folder used by the converter when "save to source folder" is disabled. |
| **Save conversion history** | Enabled | Keeps a local record of conversions in `converter_history.json`. |
| **Remove when complete** | Disabled | Automatically removes finished conversions from the list. |
| **Remove when cancelled** | Disabled | Automatically removes cancelled conversions from the list. |
| **Maximum history entries** | 50 | How many conversion entries to keep in the list. Accepts values from 10 to 500. |

### Network and advanced
| Setting | Default | Explanation |
|---------|---------|-------------|
| **Cookies file** | Empty | Uses a Netscape-format `cookies.txt` file with `yt-dlp --cookies`, useful for content that requires an authenticated session. |
| **Browser cookies** | None | Imports cookies directly from a detected browser, such as Firefox, Chrome, Chromium, Brave, Microsoft Edge, Vivaldi, or Opera, using `yt-dlp --cookies-from-browser`. |
| **User-Agent** | BigTube default | Overrides the User-Agent sent to `yt-dlp`. If left empty, the app uses a safe Chrome-based User-Agent. Includes presets for detected browsers. |
| **Proxy** | Empty | Routes searches, metadata, player, and downloads through the given proxy. Accepts `http`, `https`, `socks4`, `socks4a`, `socks5`, and `socks5h` URLs, e.g. `socks5://127.0.0.1:1080`. |

### System
| Setting | Default | Explanation |
|---------|---------|-------------|
| **Current version / update components** | Automatic | Shows the local `yt-dlp` version and lets you update the components downloaded by the app, such as `yt-dlp` and `deno`, in `~/.local/share/bigtube/bin/`. |
| **Check for updates on startup** | Enabled | Checks for newer `yt-dlp`/`deno` components when the app starts. |
| **Clipboard monitor** | Disabled | Automatically detects video links copied to the clipboard while the app is open. |
| **System notifications** | Enabled | Controls system notifications for download events and errors. |

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
| `max_download_history` | `100` | Max entries kept in the downloads list |
| `max_converter_history` | `50` | Max entries kept in the converter list |
| `add_metadata` | `false` | Metadata on downloads |
| `embed_subtitles` | `false` | Legacy subtitle flag (migrated to `subtitle_mode`) |
| `subtitle_mode` | `off` | Subtitle handling: `off`, `embed`, `file`, `both` |
| `subtitle_langs` | `en,pt,es` | Subtitle languages to fetch |
| `subtitle_auto` | `true` | Include auto-generated captions |
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
| `sponsorblock_mode` | `off` | SponsorBlock: `off`, `mark`, `remove` |
| `sponsorblock_cats` | `sponsor,selfpromo,interaction` | SponsorBlock categories to act on |
| `preview_quality` | `360p` | In-app player preview quality |
| `remove_on_complete` | `false` | Remove finished downloads from the list |
| `remove_on_cancel` | `false` | Remove cancelled downloads from the list |
| `converter_remove_on_complete` | `false` | Remove finished conversions from the list |
| `converter_remove_on_cancel` | `false` | Remove cancelled conversions from the list |
| `check_updates_on_startup` | `true` | Check for `yt-dlp`/`deno` updates on startup |

> Compatibility: older configurations with the `download_subtitles` key are automatically migrated to `embed_subtitles`.

### Environment variables
| Variable | Effect |
|----------|--------|
| `BIGTUBE_NO_FULL_REDRAW=1` | Skips the forced GSK full-redraw workaround. BigTube forces full redraws to avoid scroll "ghosting" (stale text/thumbnails) seen on some GTK4/Mesa/KWin stacks. Set this if your system is unaffected, to save CPU/battery. |
| `GSK_RENDERER` | Standard GTK variable to pick the renderer (`gl`, `vulkan`, `cairo`, …); honored as-is. |

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
