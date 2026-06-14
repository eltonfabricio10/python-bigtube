# BigTube — Rust port

Rust reimplementation of [python-bigtube](../README.md). GTK4 + libadwaita front-end
over a UI-free core library, with GStreamer playback. See [`../PORTING_RUST.md`](../PORTING_RUST.md)
for the full analysis and phased plan.

## Workspace layout

| Crate | What it is |
|-------|-----------|
| `crates/bigtube-core` | UI-free library: config, persistence/history, validators, yt-dlp search & downloader, ffmpeg converter, download queue/scheduler, updater. 59 unit tests. |
| `crates/bigtube-cli`  | `bigtube` — headless downloader (`-d/--download`, `-o`, `--audio-only`, `--format`, `--yt-dlp-version`). |
| `crates/bigtube-gui`  | `bigtube-gui` — GTK4/libadwaita app: search, downloads, converter, settings, GStreamer player. |

## Build & run

```bash
cd rust
cargo build --release
./target/release/bigtube-gui                 # GUI
./target/release/bigtube -d <url> --audio-only -o ~/Music   # headless
```

Runtime deps: `gtk4`, `libadwaita`, `gstreamer` + `gst-plugins-{base,good}` +
`gst-plugin-gtk4`, `yt-dlp`, `ffmpeg`. (yt-dlp auto-downloads to
`~/.local/share/bigtube/bin/` if absent.)

## Design notes

- **GStreamer-only** player (`gtk4paintablesink`); the Python MPV fallback is not ported.
- The core emits a `progress::StatusCode` enum (not localized strings); the GUI localizes
  via gettext, **reusing the existing `po/*.po` catalogs** (`bigtube` text domain).
- Threading: Python's `threading + GLib.idle_add` becomes `std::thread` + `async_channel`
  + `glib::spawn_future_local`; subprocess control uses process groups + `nix` signals.
- Config is a dynamic `serde_json::Map` behind a `once_cell` `RwLock` singleton, matching the
  Python dict semantics and on-disk format.

## CI / packaging

- `.github/workflows/rust-ci.yml` — fmt + clippy (`-D warnings`) + core tests + release build.
- `packaging/PKGBUILD` + `packaging/org.big.bigtube.desktop` — Arch package (installs both
  binaries, the icon, the desktop entry, and compiles `po/*.po` → `.mo`).
- `crates/bigtube-cli/build.rs` derives the version from `git describe`.
