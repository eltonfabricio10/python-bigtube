# BigTube → Rust: Análise Profunda e Plano de Port

> Documento de análise técnica do projeto **python-bigtube** (v2.0.63) com vistas a um
> reescrita em Rust. Cobre arquitetura, dependências, pontos de atrito e um plano de
> migração faseado. Gerado em 2026-06-13.

---

## 1. Visão geral do que estamos portando

| Métrica | Valor |
|---------|-------|
| Python (src) | ~11.300 LOC |
| UI XML (.ui) | ~1.245 linhas |
| CSS | 977 linhas |
| Idiomas (i18n) | 16 + en_US base |
| Binários externos | `yt-dlp`, `ffmpeg`/`ffprobe`, `mpv` (libmpv), `deno` (baixado, uso marginal) |
| Stack atual | Python 3.10+, PyGObject (GTK4 + libadwaita), GStreamer, python-mpv, requests |

**O que o app faz:** downloader multimídia desktop. Busca (YouTube / YouTube Music / link
direto) via `yt-dlp`, baixa com seleção de qualidade/agendamento, converte mídia via
`ffmpeg`, reproduz com player embutido (GStreamer com fallback para MPV), tudo numa UI
GTK4/libadwaita com temas, i18n e histórico persistente.

### Arquitetura em camadas (MVC)

```
main.py (Adw.Application)
  └─ ui/main_window.py (Adw.ApplicationWindow, Gtk.Template) ── 1187 LOC, o hub
       ├─ controllers/        (mediam UI ↔ core, sinais GObject, threading→idle_add)
       │    search / download / download_workflow / player / converter / settings / startup
       ├─ ui/                 (widgets, dialogs, rows, player widgets)
       └─ core/               (lógica pura + I/O + subprocessos — quase tudo GTK-free)
            config / json_store / *_history / search / downloader / download_manager /
            converter / updater / network_checker / validators / enums / helpers / logger
```

**Insight-chave para o port:** a camada `core/` é ~80% lógica pura (subprocesso + JSON +
threads), com pouquíssimo acoplamento a GLib. É portável quase 1:1. O custo real do port
está na camada **UI** (Gtk.Template, bindings de propriedade, factories de ListView) e no
**player** (embedding de MPV/GStreamer).

---

## 2. Mapa de dependências Python → crates Rust

| Python | Crate Rust | Observação |
|--------|-----------|------------|
| PyGObject (GTK4) | `gtk4` (gtk4-rs) | binding 1:1, maduro |
| PyGObject (libadwaita) | `libadwaita` (libadwaita-rs) | menos maduro, mas cobre Adw.* usados |
| GStreamer (gi) | `gstreamer` + `gstreamer-play`/`playbin` | `gst-plugin-gtk4` para o paintable sink |
| python-mpv | `libmpv2` / `libmpv-rs` ou FFI direto | ponto de atrito (ver §6) |
| requests | `reqwest` (blocking) ou `ureq` | downloads/HTTP simples |
| json (stdlib) | `serde` + `serde_json` | structs tipadas em vez de dicts |
| threading + GLib.idle_add | `std::thread` + `glib::MainContext::spawn_local`/channels | ver §5 |
| ThreadPoolExecutor | `rayon` ou `tokio::task::spawn_blocking` | buscas paralelas, image loader |
| fcntl flock | `fs2` ou `nix::flock` | locks de arquivo no json_store |
| subprocess.Popen | `std::process::Command` + `Stdio::piped` | core de download/converter |
| gettext | `gettextrs` | **reutiliza .po/.mo existentes sem mudança** |
| polib | `pofile` | só nos scripts de tradução |
| deep-translator | `reqwest` + `serde_json` | só no auto_translate |
| GdkPixbuf | `gdk-pixbuf` (gtk4-rs) ou `image` crate | thumbnails |
| dirs XDG (GLib.get_user_*) | `dirs` ou `glib::user_*_dir` | config/cache/data |
| logging + RotatingFileHandler | `tracing` + `tracing-appender` | rotação de logs |
| uuid | `uuid` | IDs de tarefa |
| heapq | `std::collections::BinaryHeap` | fila de prioridade |

---

## 3. Camada `core/` — port quase direto

Esta camada é a base. Recomenda-se portá-la **primeiro** como uma crate de biblioteca
(`bigtube-core`), testável sem UI.

### 3.1 Persistência

- **`json_store.rs`** — 100% portável. Escrita atômica = `tempfile` no mesmo dir +
  `fs::rename` + `fsync` (use `nix` para fsync do diretório pai). Locks: `LOCK_SH` na
  leitura, arquivo `.lock` separado com `LOCK_EX` na escrita. → `fs2::FileExt`.
- **`config.rs` (ConfigManager)** — singleton com `RLock` → `once_cell::sync::Lazy<RwLock<Config>>`
  (`parking_lot`). 26 chaves JSON (ver README §config.json). Lógica relevante:
  - merge de defaults sobre dados carregados (releases novas ganham chaves);
  - recuperação de corrupção (se não é dict → reseta);
  - migração de alias `download_subtitles` → `embed_subtitles`;
  - construção de args do `yt-dlp` (`get_yt_dlp_common_args`, cookies, user-agent, proxy);
  - validação/teste de proxy (`TcpStream::connect_timeout`).
  - Paths XDG: `~/.config/bigtube/`, `~/.local/share/bigtube/bin/`, downloads.
- **`history_manager.rs` / `converter_history.rs`** — cache em memória + **save com debounce
  de 2s via `GLib.timeout_add`**. Em Rust: ou `glib::timeout_add_local` (se rodando no main
  loop) ou um worker com `tokio::time::sleep`/canal. `MAX_HISTORY_SIZE` 100 / 50. Cuidado
  com a distinção `cache is None` (não carregado) vs `[]` (vazio) para não sobrescrever disco.
- **`search_history.rs`** — `SearchHistory` (persistido, LRU 20) + `SearchCache` (em memória,
  `OrderedDict` com TTL 3600s, máx 50) → `lru` crate ou `IndexMap`.
- **`scheduled_downloads.rs`** — lista JSON ordenada por `scheduled_time`; `upsert`/`remove`/
  `clear_past`. Sem locks. Trivial.

### 3.2 Enums / helpers / validators (100% puros)

- **`enums.rs`** — `str`-enums → `#[derive(Serialize, Deserialize)]` com `#[serde(rename)]`.
  `VideoQuality` guarda **strings de formato do yt-dlp** (não rótulos!). `DownloadStatus`,
  `ThemeMode`, `ThemeColor` (16), `FileExt` (com `is_audio()`).
- **`validators.rs`** — 23 regex de URL, `is_playlist_url`, sanitização de query/filename
  (anti path-traversal), `retry_with_backoff` (decorator → função genérica/macro),
  `run_subprocess_with_timeout`, struct `Timeouts` (constantes). `regex` crate.
- **`helpers.rs`** — `get_status_label` (status→string i18n), `is_youtube_url`. Nota: `PAUSED`
  não tem mapping de label (comportamento atual a replicar).
- **`logger.rs`** — config de logging + exceptions de domínio. Em Rust: `tracing` +
  `tracing-appender` (rotação 5MB×3) e um enum `BigTubeError` com `thiserror`.

### 3.3 Engine (subprocessos)

Estes módulos definem o **contrato exato** com binários externos. Replicar comandos é crítico.

- **`search.rs` (SearchEngine)**
  - YouTube combinado: roda em paralelo (ThreadPool 2) vídeos + playlists.
    - vídeos: `yt-dlp --extractor-args youtube:player_client=web,android_vr --flat-playlist --dump-json ytsearch<N>:<query>`
    - playlists: mesmo, com `--playlist-end <3..5>` sobre `https://www.youtube.com/results?search_query=<q>&sp=EgIQAw%3D%3D`
  - YouTube Music: `--flat-playlist --dump-json https://music.youtube.com/search?q=<q>` + filtragem de entradas não-`/watch`.
  - Link direto: `--dump-json --skip-download` (+`--flat-playlist` se playlist, senão `--no-playlist`).
  - Parsing: JSON linha-a-linha; entradas podem ter `entries[]` aninhado. Normalização de
    thumbnail (maior candidato), uploader (prefere artista p/ música, remove " - Topic").
  - `expand_playlist(url)` reusa o link direto. → `std::process::Command`, `rayon`, `serde_json`.
- **`downloader.rs` (VideoDownloader)**
  - `fetch_video_info`: `yt-dlp --dump-single-json --no-warnings --ignore-no-formats-error [...extractor-args...]`.
  - `start_download`: `Popen` com `--newline --no-playlist --ignore-config --concurrent-fragments <N> --progress-template "postprocess:[postprocess] %(progress._percent_str)s" -o <dir>/<safe_title>.<ext>` + flags condicionais (`-f`, `--merge-output-format`, `--extract-audio --audio-format --audio-quality 0`, `--rate-limit`, `--exec`, `--embed-metadata`, `--write-sub --write-auto-sub --sub-langs "en.*,pt.*,es.*" --embed-subs`, `--force-overwrites`).
  - Leitura **não-bloqueante** com `select.select(..., 1.0)` (idle-timeout 180s) e regex `(\d{1,3}(?:\.\d+)?)\s*%`. Estados via prefixos `[Merger]`, `[ExtractAudio]`, `[postprocess]`.
  - `start_new_session=True` para `killpg` em cascata (pause/cancel via SIGTERM→SIGKILL).
  - Checagem de disco (margem 10% + 10MB). Parsing de formatos do `--dump-single-json`
    (audio-only vs vídeo; estima filesize por `tbr*duration` quando ausente; injeta opções
    virtuais "Best MKV" e "Convert MP3"). → Rust: `Command` + `mio`/leitura com timeout +
    `nix::sys::signal` para process group.
- **`download_manager.rs` (singleton)** — fila de prioridade (`heapq` de `(-prio, seq, task)`),
  lista de agendados ordenada, thread daemon scheduler (acorda por `Event` ou ≤5s), N threads
  worker (1 por download), `max_concurrent` do config. → `BinaryHeap`, `std::thread`,
  `Condvar`/canal para o scheduler, `once_cell` p/ singleton.
- **`converter.rs` (MediaConverter)** — `ffprobe` para duração; `ffmpeg -i <in> [-i sub] -y
  [-map 0:v? -map 0:a? -map 1:s?] [-c:s mov_text|copy] [-map_metadata 0] -progress pipe:1
  -nostats <out>`. Parsing `out_time_us=` / `speed=`. Cancelamento via `threading.Event`
  (→ `Arc<AtomicBool>` ou `watch` channel). Busca subs `.srt/.vtt/.ass` ao lado do arquivo.
- **`updater.rs`** — baixa `yt-dlp_linux` e `deno...zip` de GitHub releases (`urllib` →
  `reqwest`), `chmod +x`, valida `--version`, descompacta (`zip` crate).
- **`network_checker.rs`** — `TcpStream` p/ google:80 / 1.1.1.1:53; GitHub API p/ versão do
  yt-dlp; compara versões `YYYY.MM.DD`.

### 3.4 Módulos `core` com acoplamento a GTK (re-arquitetar)

- **`clipboard_monitor.py`** — usa `Gdk` clipboard + `GLib.timeout_add(1000)`. Em Rust fica
  na camada UI (gdk4) ou via crate de clipboard; é só um poll + `is_valid_url`.
- **`image_loader.py`** — `GdkPixbuf` + `GLib.idle_add` + ThreadPool(8) + cache LRU
  (memória 100/scaled 200) + cache em disco (`~/.cache/bigtube/thumbnails/`, md5(url).jpg,
  máx 500 arquivos, limite 10MB/imagem). Mantém na UI; lógica de cache é portável.

---

## 4. Camada UI — o grosso do esforço

A UI é GTK4 + libadwaita com `Gtk.Template(filename=...)` carregando `.ui` XML em runtime.

### 4.1 Estrutura

- **`main.py`**: `Adw.Application` com `HANDLES_COMMAND_LINE` (aceita URL/arquivo/query na CLI),
  init de `Gst`, carga de CSS via `Gtk.CssProvider`. Sinais `activate`/`startup`/`close-request`.
- **`main_window.py`** (1187 LOC): `Adw.ApplicationWindow` com template `bigtube.ui`. 50+
  `Template.Child`. Layout: `AdwToastOverlay → Overlay → Box[HeaderBar, AdwNavigationSplitView
  com GtkStack de 4 páginas (Search/Downloads/Converter/Settings), player bar inferior]`.
  Instancia todos os controllers, faz embedding do player, aplica tema, restaura histórico.
- **Rows** (`search_result_row`, `download_row`, `converter_row`) e **dialogs**
  (`format_dialog`, `schedule_dialog`, `playlist_dialog`) — cada um `Gtk.Template` ou
  construído em código (dialogs).
- **`message_manager.py`** — toasts e `Adw.AlertDialog` centralizados.
- **`async_utils.py`** — `run_in_background(fn, on_success, on_error)` = thread daemon +
  `GLib.idle_add`. Padrão usado em toda a app.

### 4.2 Decisão central: como tratar Gtk.Template em Rust

gtk4-rs **suporta** templates de duas formas:
1. **`#[template(file = "x.ui")]`** com `CompositeTemplate` derive — equivalente direto ao
   `@Gtk.Template`. **Reutiliza os `.ui` existentes quase sem mudança** (ajustar `<template
   class>`/`parent` e os ids `Template.Child` → campos `#[template_child]`).
2. Construção manual em código (mais verboso).

**Recomendação:** manter os arquivos `.ui` e usar `CompositeTemplate`. Isso preserva o
layout e o CSS sem reescrever a UI. Cada widget custom (rows, window) vira um
`glib::Object` subclasse com `ObjectSubclass` + `#[template_child]`.

### 4.3 Sinais e property binding

- **`__gsignals__`** custom (ex.: `play-requested`, `time-changed`, `playlist-activated`) →
  definir signals via `ObjectSubclass`/`glib::subclass::Signal` (boilerplate) **ou** trocar
  por canais/closures onde for interno. Para os sinais entre widget↔controller, closures
  Rust (`connect_local` ou callbacks `Box<dyn Fn>`) costumam ser mais simples.
- **`bind_property` bidirecional** (`is_selected ↔ checkbox.active`, `selection_mode →
  visible`) → `glib::Object::bind_property(...).bidirectional().sync_create().build()`.
  Existe 1:1 em gtk4-rs.
- **`Gio.ListStore` + `Gtk.SignalListItemFactory`** → `gio::ListStore::new::<T>()` +
  `SignalListItemFactory` com `connect_setup`/`connect_bind`. `VideoDataObject` vira um
  `glib::Object` com propriedades. Praticamente 1:1.

### 4.4 Demais pontos de UI

- **Temas**: classes CSS (`light`/`dark`/`accent-*`) + `Adw.StyleManager.set_color_scheme`.
  **O `style.css` pode ser reutilizado como está.** 16 cores/temas.
- **Drag & Drop** (converter): `Gtk.DropTarget`/`Gtk.DragSource` + `Gdk.ContentProvider`
  (formato `row::<path>` p/ reordenação, `Gdk.FileList`/string p/ externo). → gtk4-rs tem tudo,
  mais verboso.
- **Dialogs modais** com callback async (`AlertDialog.choose`, `FileDialog.open/select_folder`)
  → libadwaita-rs/gtk4-rs com `Future`s (`.choose_future().await`) — na verdade mais ergonômico.
- **Token pattern** no `player_controller` (`_play_token` monotônico p/ invalidar stream
  resolvido fora de ordem) → `Arc<AtomicU64>`.

---

## 5. Modelo de concorrência (o ajuste mental do port)

Padrão onipresente no Python:
```python
threading.Thread(target=work, daemon=True).start()   # trabalho bloqueante
GLib.idle_add(update_ui, result)                      # volta ao main loop
```
Equivalentes em Rust/gtk4-rs:
```rust
// opção A: thread + canal + main context
let (tx, rx) = async_channel::bounded(1);
std::thread::spawn(move || { let r = work(); let _ = tx.send_blocking(r); });
glib::spawn_future_local(async move { while let Ok(r) = rx.recv().await { update_ui(r); } });

// opção B: spawn_blocking dentro de runtime async
```
- `GLib.idle_add` → `glib::idle_add_local_once` / `glib::MainContext::spawn_local`.
- `GLib.timeout_add` (debounce/poll) → `glib::timeout_add_local`.
- `RLock`/`Lock` → `parking_lot::{RwLock, Mutex}`; estado UI compartilhado em single-thread
  → `Rc<RefCell<_>>` com `glib::clone!`.
- `ProgressUpdateThrottle` (mín. 0.25s entre updates) → struct com `Instant`.

---

## 6. Riscos e pontos de atrito (ordenados por risco)

| # | Tema | Risco | Mitigação |
|---|------|-------|-----------|
| 1 | **Embedding do MPV** | ~~Alto~~ **Descartado** | **DECISÃO:** o port usará **apenas GStreamer** (`gtk4paintablesink`), que já é o player primário. O fallback MPV (`mpv_widget.py`, embedding X11/Wayland) **não será portado**. Elimina o maior risco de FFI do projeto. |
| 2 | **GStreamer paintable sink** | Médio | `gtk4paintablesink` precisa do plugin `gst-plugin-gtk4` instalado; em Rust usa-se o mesmo elemento via `gstreamer-rs`. Bus messages → mesmo modelo. |
| 3 | **Maturidade do libadwaita-rs** | Médio | Cobrir Adw.* usados (Application, ApplicationWindow, ToastOverlay, AlertDialog, PreferencesPage/Group, ActionRow, Banner, NavigationSplitView, WindowTitle, StatusPage, ToolbarView). Todos existem; testar versões. |
| 4 | **Sinais GObject custom** | Médio | Boilerplate de `glib::subclass::Signal`. Onde possível, preferir closures/canais. |
| 5 | **`setlocale(LC_NUMERIC,"C")` p/ MPV** | Baixo | MPV exige `LC_NUMERIC=C`. Em Rust: `libc::setlocale` (unsafe) no startup ou formatar manualmente. |
| 6 | **Paridade exata de flags do yt-dlp/ffmpeg** | Médio | Os comandos são contrato. Portar com **testes de snapshot do comando montado** (já existe `test_start_download_builds_expected_command`). |
| 7 | **Daemon threads** | Baixo | Rust não tem; usar threads normais e encerrar limpo no `close-request` (flush de histórico). |

---

## 7. Infraestrutura (build, i18n, packaging, testes)

- **Versão a partir do git** (`scripts/sync_version_from_git.py`): `git describe --tags --long`,
  patch++ por commit pós-tag, propaga p/ `pyproject.toml`, `PKGBUILD`, headers `.po/.pot` e
  User-Agents. → reescrever como `build.rs` + um pequeno binário/script; atualizar `Cargo.toml`.
- **i18n** (`gettext` + `xgettext`/`msgmerge`/`msgfmt`, `deep-translator`): **reaproveitar
  100% dos `.po/.mo`** via `gettextrs`. `xgettext` precisa extrair de Rust (`--language=C` com
  macro `t!`/comentários, ou `cargo-i18n`/`xtr`). `auto_translate.py` → script Rust (`pofile` +
  `reqwest`) ou manter o Python como ferramenta de build.
- **CI/CD** (GitHub Actions: lint+test → build → release → Arch → AUR): trocar `ruff`/`pytest`
  por `cargo fmt`/`clippy`/`cargo test`; `cargo build --release`; manter geração de `.mo`,
  release com `gh`, build Arch e deploy AUR. `.desktop` e ícone reaproveitados (ajustar `Exec`).
- **Testes** (~1180 LOC, 14 módulos): cobertura sólida e **portável como spec** — config,
  downloader (comando esperado, parsing, disco, redação de credenciais), search engine
  (YouTube Music, thumbnails, uploader), workflow (throttle, agendamento, erro), scheduler
  (prioridade, sem deadlock), startup (restauração), settings (detecção de browser), validators,
  converter, history. Reescrever como `#[test]`/`mockito` mantendo os mesmos casos.

---

## 8. Plano de migração faseado

**Fase 0 — Andaime.** `cargo` workspace: `bigtube-core` (lib), `bigtube` (bin/UI). CI mínimo
(fmt/clippy/test). `build.rs` para versão e compilação de `.mo`.

**Fase 1 — `bigtube-core` (sem UI, ~maior parte do valor, baixo risco).**
Portar e testar: `enums`, `validators`, `helpers`, `logger`, `json_store`, `config`,
`*_history`, `scheduled_downloads`, `search_history`, `network_checker`, `updater`,
`search`, `downloader`, `download_manager`, `converter`. Reusar a suíte de testes como spec.
Marco: CLI headless (`bigtube -d <url>`) funcionando 100% em Rust.

**Fase 2 — Shell de UI.** `Adw.Application` + `main_window` via `CompositeTemplate` reusando
`bigtube.ui` + `style.css`. Navegação entre as 4 páginas, tema, toasts. Sem lógica ainda.

**Fase 3 — Busca + downloads na UI.** `VideoDataObject` (glib::Object), ListView factory,
`search_controller`, rows, dialog de formato, fluxo de download com progresso/throttle,
histórico persistente, agendamento. Image loader com cache.

**Fase 4 — Converter + clipboard + settings.** Drag&drop, fila de conversão, todas as
preferências, import/export, detecção de browsers de cookies.

**Fase 5 — Player.** Começar pelo GStreamer (`gtk4paintablesink`) que já é o primário.
Avaliar necessidade do fallback MPV e, se preciso, embutir via X11 XID / GL render.

**Fase 6 — Packaging.** PKGBUILD para binário Rust, AUR, release, `.desktop`/ícone, todos
os `.mo`.

---

## 9. Recomendação de arranque

1. Confirmar a **biblioteca de UI** (gtk4-rs + libadwaita-rs é a escolha óbvia para preservar
   `.ui`/`.css`/tema/i18n).
2. Decidir o **destino do player**: só GStreamer (mais simples, já é default) vs. manter o
   fallback MPV (mais trabalho de FFI/embedding).
3. Começar pela **Fase 1** — é onde está a maior parte da lógica, tem testes prontos como
   especificação e não depende de nenhuma decisão de UI.
```
```

---

Pontos a decidir antes do código estão na §9.
