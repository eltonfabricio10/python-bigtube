//! Window construction, page wiring, and the search→download→progress loop.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use adw::prelude::*;
use gtk::{gio, glib};

use bigtube_core::config;
use bigtube_core::download_manager::{self, OnStartFn};
use bigtube_core::downloader::{DownloadParams, VideoDownloader};
use bigtube_core::progress::{Progress, ProgressFn, StatusCode};
use bigtube_core::search::SearchEngine;

use crate::dialog;
use crate::i18n::tr;
use crate::objects::VideoObject;
use crate::row::{RowAction, SearchResultRow};

/// One visible download (a row in the Downloads list).
#[derive(Clone)]
struct DownloadRow {
    container: gtk::Box,
    status: gtk::Label,
    detail: gtk::Label,
    progress: gtk::ProgressBar,
    pause: gtk::Button,
    cancel: gtk::Button,
    // Pencil shown only for a pending scheduled row: opens the schedule editor.
    edit: gtk::Button,
    btn_delete: gtk::Button,
    actions: gtk::Box,
    btn_folder: gtk::Button,
    btn_play: gtk::Button,
    btn_convert: gtk::Button,
    file_path: Rc<RefCell<String>>,
    artist: Rc<RefCell<String>>,
    // Shared across clones so buttons and the Started handler see the same state.
    downloader: Rc<RefCell<Option<Arc<VideoDownloader>>>>,
    progress_fn: Rc<RefCell<Option<ProgressFn>>>,
    is_paused: Rc<Cell<bool>>,
    // True once the download has errored: the pause button becomes a retry button.
    is_error: Rc<Cell<bool>>,
    // The persisted schedule id, while this row is a pending scheduled download
    // (lets the "Scheduled" management tab find and cancel/edit the live row).
    sched_id: Rc<RefCell<Option<String>>>,
}

impl DownloadRow {
    fn new(title: &str, file_path: &str, artist: &str) -> Self {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 4);
        container.add_css_class("card");
        container.set_margin_top(6);
        container.set_margin_bottom(6);
        container.set_margin_start(8);
        container.set_margin_end(8);
        // Inner padding so the card border doesn't hug the content.
        let pad = gtk::Box::new(gtk::Orientation::Vertical, 4);
        pad.set_margin_top(8);
        pad.set_margin_bottom(8);
        pad.set_margin_start(12);
        pad.set_margin_end(12);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title_lbl = gtk::Label::new(Some(title));
        title_lbl.set_xalign(0.0);
        title_lbl.set_hexpand(true);
        title_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title_lbl.add_css_class("heading");
        let status = gtk::Label::new(Some(&tr("Queued")));
        status.add_css_class("dim-label");
        status.add_css_class("caption");
        let pause = gtk::Button::from_icon_name("media-playback-pause-symbolic");
        pause.add_css_class("flat");
        pause.set_tooltip_text(Some(&tr("Pause")));
        let cancel = gtk::Button::from_icon_name("process-stop-symbolic");
        cancel.add_css_class("flat");
        cancel.add_css_class("destructive-action");
        cancel.set_tooltip_text(Some(&tr("Cancel")));
        // Edit pencil: shown only while this row is a pending scheduled download.
        let edit = gtk::Button::from_icon_name("document-edit-symbolic");
        edit.add_css_class("flat");
        edit.set_tooltip_text(Some(&tr("Edit")));
        edit.set_visible(false);
        // Per-row delete (asks history-only vs file too); wired in wire_row_footer.
        let btn_delete = gtk::Button::from_icon_name("user-trash-symbolic");
        btn_delete.add_css_class("flat");
        btn_delete.set_tooltip_text(Some(&tr("Remove from list")));
        header.append(&title_lbl);
        header.append(&status);
        header.append(&edit);
        header.append(&pause);
        header.append(&cancel);
        header.append(&btn_delete);

        // Destination path shown under the title.
        let path_lbl = gtk::Label::new(Some(file_path));
        path_lbl.set_xalign(0.0);
        path_lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        path_lbl.set_tooltip_text(Some(file_path));
        path_lbl.add_css_class("dim-label");
        path_lbl.add_css_class("caption");

        let progress = gtk::ProgressBar::new();
        progress.set_fraction(0.0);

        // Live transfer detail ("12.3MiB / 45.6MiB · 2.1MiB/s · ETA 00:15") while
        // running, and the media summary ("Video MP4 · h264 · 1920×1080 · …")
        // once done. Sits at the bottom-left, on the same row as the actions.
        let detail = gtk::Label::new(None);
        detail.set_xalign(0.0);
        detail.set_hexpand(true);
        detail.set_ellipsize(gtk::pango::EllipsizeMode::End);
        detail.add_css_class("dim-label");
        detail.add_css_class("caption");
        detail.set_visible(false);

        // Bottom row: status detail on the left, action buttons on the right.
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        actions.set_halign(gtk::Align::End);
        // Revealed on completion (open folder / play / convert).
        actions.set_visible(false);
        let btn_folder = gtk::Button::from_icon_name("folder-open-symbolic");
        btn_folder.add_css_class("flat");
        btn_folder.set_tooltip_text(Some(&tr("Open Folder")));
        let btn_play = gtk::Button::from_icon_name("media-playback-start-symbolic");
        btn_play.add_css_class("flat");
        btn_play.set_tooltip_text(Some(&tr("Play Video")));
        let btn_convert = gtk::Button::from_icon_name("emblem-synchronizing-symbolic");
        btn_convert.add_css_class("flat");
        btn_convert.set_tooltip_text(Some(&tr("Add to Converter")));
        actions.append(&btn_folder);
        actions.append(&btn_play);
        actions.append(&btn_convert);
        footer.append(&detail);
        footer.append(&actions);

        pad.append(&header);
        pad.append(&path_lbl);
        pad.append(&progress);
        pad.append(&footer);
        container.append(&pad);

        let downloader: Rc<RefCell<Option<Arc<VideoDownloader>>>> = Rc::new(RefCell::new(None));
        let progress_fn: Rc<RefCell<Option<ProgressFn>>> = Rc::new(RefCell::new(None));
        let is_paused = Rc::new(Cell::new(false));
        let is_error = Rc::new(Cell::new(false));

        let slot = downloader.clone();
        cancel.connect_clicked(move |_| {
            if let Some(d) = slot.borrow().as_ref() {
                d.cancel();
            }
        });

        // Pause / resume, or — after an error — retry. Both re-run the (blocking)
        // downloader on a thread via `resume`.
        let dl = downloader.clone();
        let pf = progress_fn.clone();
        let paused = is_paused.clone();
        let err = is_error.clone();
        let pause_btn = pause.clone();
        let status_c = status.clone();
        let progress_c = progress.clone();
        let cancel_c = cancel.clone();
        pause.connect_clicked(move |_| {
            let Some(d) = dl.borrow().as_ref().cloned() else {
                return;
            };
            if err.get() {
                // Retry a failed download: reset the row to a running look and
                // re-run from scratch.
                err.set(false);
                paused.set(false);
                pause_btn.set_icon_name("media-playback-pause-symbolic");
                pause_btn.set_tooltip_text(Some(&tr("Pause")));
                status_c.set_text(&tr("Queued"));
                for c in ["success", "warning", "error"] {
                    progress_c.remove_css_class(c);
                }
                cancel_c.set_sensitive(true);
                if let Some(cb) = pf.borrow().as_ref().cloned() {
                    std::thread::spawn(move || {
                        d.resume(&cb);
                    });
                }
                return;
            }
            if paused.get() {
                paused.set(false);
                pause_btn.set_icon_name("media-playback-pause-symbolic");
                if let Some(cb) = pf.borrow().as_ref().cloned() {
                    std::thread::spawn(move || {
                        d.resume(&cb);
                    });
                }
            } else {
                paused.set(true);
                pause_btn.set_icon_name("media-playback-start-symbolic");
                d.pause();
            }
        });

        Self {
            container,
            status,
            detail,
            progress,
            pause,
            cancel,
            edit,
            actions,
            btn_folder,
            btn_play,
            btn_convert,
            btn_delete,
            file_path: Rc::new(RefCell::new(file_path.to_string())),
            artist: Rc::new(RefCell::new(artist.to_string())),
            downloader,
            progress_fn,
            is_paused,
            is_error,
            sched_id: Rc::new(RefCell::new(None)),
        }
    }

    fn update(&self, percent: Option<&str>, status: StatusCode, detail: Option<&str>) {
        // A pause terminates the yt-dlp process, surfacing as "Cancelled"; keep
        // the row interactive while the user has it paused.
        if self.is_paused.get() && status == StatusCode::Cancelled {
            self.status.set_text(&tr("Paused"));
            self.set_progress_class("warning");
            return;
        }
        self.status.set_text(&status_label(status));
        if let Some(p) = percent {
            if let Some(f) = parse_percent(p) {
                self.progress.set_fraction(f);
            }
        }
        // Live size/speed/ETA line (shown only while it carries data).
        if let Some(d) = detail.filter(|d| !d.is_empty()) {
            self.detail.set_text(d);
            self.detail.set_visible(true);
        }
        if status == StatusCode::Completed {
            self.mark_completed();
        } else if status.is_error() {
            // Errored: keep the row interactive — Cancel stays, and Pause becomes
            // a Retry button (circular arrow).
            self.set_progress_class("error");
            self.is_error.set(true);
            self.pause.set_visible(true);
            self.pause.set_sensitive(true);
            self.pause.set_icon_name("view-refresh-symbolic");
            self.pause.set_tooltip_text(Some(&tr("Retry")));
            self.cancel.set_visible(true);
            self.cancel.set_sensitive(true);
        } else if status == StatusCode::Cancelled {
            self.set_progress_class("warning");
            self.pause.set_sensitive(false);
            self.cancel.set_sensitive(false);
        }
    }

    /// Apply exactly one of the success/warning/error progress styles.
    fn set_progress_class(&self, class: &str) {
        for c in ["success", "warning", "error"] {
            self.progress.remove_css_class(c);
        }
        if !class.is_empty() {
            self.progress.add_css_class(class);
        }
    }

    /// Switch the row to its completed look: full bar, no transport, footer shown.
    fn mark_completed(&self) {
        self.is_error.set(false);
        self.progress.set_fraction(1.0);
        self.set_progress_class("success");
        self.detail.set_visible(false);
        self.pause.set_visible(false);
        self.cancel.set_visible(false);
        // Mark the row terminal so "Clear" recognizes it as removable.
        self.pause.set_sensitive(false);
        self.cancel.set_sensitive(false);
        self.status.set_text(&status_label(StatusCode::Completed));
        // Only offer file actions if the output really exists.
        let exists = std::path::Path::new(&*self.file_path.borrow()).exists();
        self.actions.set_visible(exists);
    }
}

/// Message marshaled from worker threads back to the main loop.
enum UiMsg {
    Progress {
        key: String,
        percent: Option<String>,
        status: StatusCode,
        detail: Option<String>,
    },
    Started {
        key: String,
        downloader: Arc<VideoDownloader>,
    },
    /// Real codecs + on-disk size, probed after a download completes.
    MediaInfo { key: String, text: String },
    /// A recurring scheduled download just started: arm its next occurrence.
    Reschedule { info: RescheduleInfo, base_ts: f64 },
}

/// Everything needed to re-create the next occurrence of a recurring schedule.
#[derive(Clone)]
struct RescheduleInfo {
    url: String,
    title: String,
    thumbnail: String,
    uploader: String,
    format_id: String,
    ext: String,
    force_overwrite: bool,
    recurrence: String,
}

/// Human file size from raw bytes, e.g. "57.9 MiB" / "1.23 GiB".
fn human_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1024.0 * 1024.0 * 1024.0 {
        format!("{:.2} GiB", b / 1024.0 / 1024.0 / 1024.0)
    } else {
        format!("{:.1} MiB", b / 1024.0 / 1024.0)
    }
}

/// "H.264 · AAC · 57.9 MiB" from a probed file (omitting unknown parts).
/// "Video MP4" / "Audio MP3": the media kind (by presence of a video stream)
/// plus the upper-cased file extension, derived from `path`.
fn media_kind_label(s: &bigtube_core::converter::MediaSummary, path: &str) -> String {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_uppercase())
        .unwrap_or_default();
    let kind = if !s.vcodec.is_empty() {
        tr("Video")
    } else if !s.acodec.is_empty() {
        tr("Audio")
    } else {
        String::new()
    };
    match (kind.is_empty(), ext.is_empty()) {
        (false, false) => format!("{kind} {ext}"),
        (false, true) => kind,
        (true, false) => ext,
        (true, true) => String::new(),
    }
}

fn media_summary_text(s: &bigtube_core::converter::MediaSummary, path: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    // Lead with the media kind + container, e.g. "Video MP4" / "Audio MP3".
    let kind = media_kind_label(s, path);
    if !kind.is_empty() {
        parts.push(kind);
    }
    let (v, a) = (codec_pretty(&s.vcodec), codec_pretty(&s.acodec));
    if !v.is_empty() {
        parts.push(v);
        // Resolution next to the video codec.
        if s.width > 0 && s.height > 0 {
            parts.push(format!("{}×{}", s.width, s.height));
        }
    }
    if !a.is_empty() {
        parts.push(a);
        // Sample rate next to the audio codec (e.g. 48 kHz).
        if s.sample_rate > 0 {
            let khz = s.sample_rate as f64 / 1000.0;
            if (khz.fract()).abs() < 0.05 {
                parts.push(format!("{khz:.0} kHz"));
            } else {
                parts.push(format!("{khz:.1} kHz"));
            }
        }
    }
    if s.size_bytes > 0 {
        parts.push(human_size(s.size_bytes));
    }
    parts.join(" · ")
}

struct AppState {
    window: RefCell<Option<adw::ApplicationWindow>>,
    stack: adw::ViewStack,
    toasts: adw::ToastOverlay,
    search_store: gio::ListStore,
    search_stack: gtk::Stack,
    select_mode: Cell<bool>,
    select_revealer: gtk::Revealer,
    select_btn: gtk::Button,
    sched_selected_btn: gtk::Button,
    downloads_box: gtk::ListBox,
    downloads_stack: gtk::Stack,
    download_rows: RefCell<HashMap<String, DownloadRow>>,
    converter_box: gtk::ListBox,
    converter_stack: gtk::Stack,
    // Conversions run one at a time (mirrors `converter_controller.py`): a click
    // enqueues, and each finish pumps the next. Without this they'd all run in
    // parallel threads and thrash the CPU.
    conv_active: Cell<bool>,
    conv_queue: RefCell<std::collections::VecDeque<PendingConv>>,
    player: RefCell<Option<Rc<crate::player::Player>>>,
    busy_spinner: gtk::Spinner,
    busy_count: Cell<i32>,
    // Show the "enable cookies" guidance dialog only once per session.
    bot_block_hinted: Cell<bool>,
    ui_tx: async_channel::Sender<UiMsg>,
    // Paste a URL into the search entry and run the search (set by the search
    // page; used by the clipboard monitor's "paste detected link" prompt).
    #[allow(clippy::type_complexity)]
    paste_and_search: RefCell<Option<Rc<dyn Fn(String)>>>,
}

impl AppState {
    fn toast(&self, msg: &str) {
        self.toasts.add_toast(adw::Toast::new(msg));
    }

    /// Tell the user YouTube blocked the request and how to fix it (enable
    /// browser cookies in Settings). A full dialog the first time per session,
    /// then a lighter toast on subsequent hits.
    fn notify_bot_block(self: &Rc<Self>) {
        if self.bot_block_hinted.get() {
            self.toast(&tr("Blocked by YouTube — enable cookies in Settings"));
            return;
        }
        self.bot_block_hinted.set(true);
        let Some(window) = self.window.borrow().clone() else {
            self.toast(&tr("Blocked by YouTube — enable cookies in Settings"));
            return;
        };
        let dialog = adw::MessageDialog::new(
            Some(&window),
            Some(&tr("Blocked by YouTube")),
            Some(&tr(
                "YouTube asked to \"confirm you're not a bot\". To keep downloading, \
                 open Settings and set \"Cookies From Browser\" to the browser where \
                 you're signed in to YouTube (e.g. Firefox or Chrome).",
            )),
        );
        dialog.add_response("close", &tr("Close"));
        dialog.add_response("settings", &tr("Open Settings"));
        dialog.set_response_appearance("settings", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("settings"));
        crate::app::apply_theme_classes(&dialog);
        let state = self.clone();
        dialog.connect_response(None, move |dlg, resp| {
            if resp == "settings" {
                state.stack.set_visible_child_name("settings");
            }
            dlg.close();
        });
        dialog.present();
    }

    /// Show the header busy spinner (ref-counted across concurrent tasks).
    fn busy_begin(&self) {
        self.busy_count.set(self.busy_count.get() + 1);
        self.busy_spinner.set_visible(true);
        self.busy_spinner.start();
    }

    fn busy_end(&self) {
        let n = (self.busy_count.get() - 1).max(0);
        self.busy_count.set(n);
        if n == 0 {
            self.busy_spinner.stop();
            self.busy_spinner.set_visible(false);
        }
    }

    /// Recompute the "Download Selected (N)" label/sensitivity from the store.
    fn refresh_selection_count(&self) {
        let mut n = 0;
        for i in 0..self.search_store.n_items() {
            if let Some(o) = self
                .search_store
                .item(i)
                .and_then(|o| o.downcast::<VideoObject>().ok())
            {
                if o.is_selected() {
                    n += 1;
                }
            }
        }
        self.select_btn
            .set_label(&tr("Download Selected ({count})").replace("{count}", &n.to_string()));
        self.select_btn.set_sensitive(n > 0);
        self.sched_selected_btn.set_sensitive(n > 0);
    }

    fn update_downloads_empty(&self) {
        let has = self.downloads_box.first_child().is_some();
        self.downloads_stack
            .set_visible_child_name(if has { "list" } else { "empty" });
    }

    fn update_converter_empty(&self) {
        let has = self.converter_box.first_child().is_some();
        self.converter_stack
            .set_visible_child_name(if has { "list" } else { "empty" });
    }

    fn update_search_empty(&self) {
        let has = self.search_store.n_items() > 0;
        self.search_stack
            .set_visible_child_name(if has { "list" } else { "empty" });
    }
}

pub fn build_window(app: &adw::Application) {
    let stack = adw::ViewStack::new();
    let toasts = adw::ToastOverlay::new();
    let search_store = gio::ListStore::new::<VideoObject>();
    // Rows are individually carded (see DownloadRow/converter row), so the list
    // itself stays transparent and just provides spacing.
    let downloads_box = gtk::ListBox::new();
    downloads_box.set_selection_mode(gtk::SelectionMode::None);
    downloads_box.add_css_class("background");
    let converter_box = gtk::ListBox::new();
    converter_box.set_selection_mode(gtk::SelectionMode::None);
    converter_box.add_css_class("background");

    let (ui_tx, ui_rx) = async_channel::unbounded::<UiMsg>();

    let state = Rc::new(AppState {
        window: RefCell::new(None),
        stack: stack.clone(),
        toasts: toasts.clone(),
        search_store: search_store.clone(),
        search_stack: gtk::Stack::new(),
        select_mode: Cell::new(false),
        select_revealer: gtk::Revealer::new(),
        select_btn: gtk::Button::new(),
        sched_selected_btn: gtk::Button::new(),
        downloads_box: downloads_box.clone(),
        downloads_stack: gtk::Stack::new(),
        download_rows: RefCell::new(HashMap::new()),
        converter_box: converter_box.clone(),
        converter_stack: gtk::Stack::new(),
        conv_active: Cell::new(false),
        conv_queue: RefCell::new(std::collections::VecDeque::new()),
        player: RefCell::new(None),
        busy_spinner: gtk::Spinner::new(),
        busy_count: Cell::new(0),
        bot_block_hinted: Cell::new(false),
        ui_tx,
        paste_and_search: RefCell::new(None),
    });

    // Pages.
    let search_page = build_search_page(&state);
    let downloads_page = build_downloads_page(&state);
    let converter_page = build_converter_page(&state);
    let settings_page = build_settings_page(&state);

    add_page(
        &stack,
        &search_page,
        "search",
        &tr("Search"),
        "system-search-symbolic",
    );
    add_page(
        &stack,
        &downloads_page,
        "downloads",
        &tr("Downloads"),
        "folder-download-symbolic",
    );
    add_page(
        &stack,
        &converter_page,
        "converter",
        &tr("Converter"),
        "media-playback-start-symbolic",
    );
    add_page(
        &stack,
        &settings_page,
        "settings",
        &tr("Settings"),
        "emblem-system-symbolic",
    );

    let switcher = adw::ViewSwitcher::builder()
        .stack(&stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .build();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&switcher));
    // Global busy spinner for background yt-dlp tasks (hidden when idle).
    state.busy_spinner.set_visible(false);
    header.pack_start(&state.busy_spinner);

    // Primary (hamburger) menu: About / Quit.
    let menu = gio::Menu::new();
    menu.append(Some(&tr("About")), Some("app.about"));
    menu.append(Some(&tr("Quit")), Some("app.quit"));
    let menu_btn = gtk::MenuButton::new();
    menu_btn.set_icon_name("open-menu-symbolic");
    menu_btn.set_menu_model(Some(&menu));
    header.pack_end(&menu_btn);
    setup_app_actions(app);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&stack));
    // Narrow-window navigation: a bottom view-switcher bar (auto-reveals).
    let switcher_bar = adw::ViewSwitcherBar::builder().stack(&stack).build();
    toolbar.add_bottom_bar(&switcher_bar);
    toasts.set_child(Some(&toolbar));

    // Size the window to a comfortable fraction of the monitor.
    let (win_w, win_h) = comfortable_window_size();
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("BigTube")
        .default_width(win_w)
        .default_height(win_h)
        .width_request(360)
        .height_request(480)
        .content(&toasts)
        .build();

    apply_theme(&window);
    state.window.replace(Some(window.clone()));

    // Player + bottom transport bar. Flat bottom area so the player's rounded
    // card visibly floats instead of sitting in a styled toolbar strip.
    toolbar.set_bottom_bar_style(adw::ToolbarStyle::Flat);
    let (player, player_bar) = crate::player::build(&window);
    toolbar.add_bottom_bar(&player_bar);
    state.player.replace(Some(player));

    // Main-thread UI update loop.
    let state_for_loop = state.clone();
    glib::spawn_future_local(async move {
        while let Ok(msg) = ui_rx.recv().await {
            match msg {
                UiMsg::Progress {
                    key,
                    percent,
                    status,
                    detail,
                } => {
                    let info = state_for_loop.download_rows.borrow().get(&key).map(|row| {
                        row.update(percent.as_deref(), status, detail.as_deref());
                        (row.file_path.borrow().clone(), row.is_paused.get())
                    });
                    // On completion, probe the real file (codecs + on-disk size)
                    // off-thread and show it as the row's status.
                    if status == StatusCode::Completed {
                        if let Some(path) = info
                            .as_ref()
                            .map(|(p, _)| p.clone())
                            .filter(|p| !p.is_empty())
                        {
                            let tx = state_for_loop.ui_tx.clone();
                            let key = key.clone();
                            std::thread::spawn(move || {
                                let s = bigtube_core::converter::probe_media_summary(&path);
                                let text = media_summary_text(&s, &path);
                                if !text.is_empty() {
                                    let _ = tx.send_blocking(UiMsg::MediaInfo { key, text });
                                }
                            });
                        }
                    }
                    // A REAL cancel (not a pause): the core already deleted the
                    // partial files — here drop the row and its history entry.
                    if status == StatusCode::Cancelled {
                        if let Some((path, paused)) = &info {
                            if !paused {
                                if !path.is_empty() {
                                    bigtube_core::history::HistoryManager::new(history_path())
                                        .remove_entry(path);
                                }
                                if let Some(row) =
                                    state_for_loop.download_rows.borrow_mut().remove(&key)
                                {
                                    remove_list_card(&state_for_loop.downloads_box, &row.container);
                                }
                                state_for_loop.update_downloads_empty();
                            }
                        }
                    }
                    // Bot block — guide the user to enable cookies once.
                    if status == StatusCode::BotBlocked {
                        state_for_loop.notify_bot_block();
                    }
                }
                UiMsg::Started { key, downloader } => {
                    if let Some(row) = state_for_loop.download_rows.borrow().get(&key) {
                        row.downloader.replace(Some(downloader));
                        // Once it's actually downloading it's no longer editable.
                        row.edit.set_visible(false);
                        row.sched_id.replace(None);
                    }
                }
                UiMsg::MediaInfo { key, text } => {
                    if let Some(row) = state_for_loop.download_rows.borrow().get(&key) {
                        // Show the real codecs/resolution/size at the bottom-left
                        // (the `detail` line), leaving the header status as "Done".
                        row.detail.set_text(&text);
                        row.detail.set_visible(true);
                        // Persist it so the row shows the same summary after restart.
                        let fp = row.file_path.borrow().clone();
                        if !fp.is_empty() {
                            bigtube_core::history::HistoryManager::new(history_path())
                                .set_media_summary(&fp, &text);
                        }
                    }
                }
                UiMsg::Reschedule { info, base_ts } => {
                    // Compute the next instant after now and enqueue a fresh
                    // scheduled download (new id, persisted) that will itself
                    // arm the occurrence after it — the chain self-perpetuates.
                    let now = now_epoch_secs();
                    if let Some(next_ts) = next_occurrence(base_ts, &info.recurrence, now) {
                        enqueue_common(
                            &state_for_loop,
                            &info.url,
                            &info.title,
                            &info.thumbnail,
                            &info.uploader,
                            &info.format_id,
                            &info.ext,
                            Some(next_ts),
                            info.force_overwrite,
                            None,
                            None,
                            &info.recurrence,
                        );
                    }
                }
            }
        }
    });

    // Restore persisted download / converter history into their lists.
    load_download_history(&state);
    load_converter_history(&state);
    // Recreate persisted scheduled downloads (re-arming their timers, or running
    // immediately any whose time passed while the app was closed).
    restore_scheduled_downloads(&state);

    // Always run the monitor; it honours the live `monitor_clipboard` setting.
    start_clipboard_monitor(&state);

    window.present();
}

/// Register the app-scoped `about` / `quit` actions used by the primary menu.
fn setup_app_actions(app: &adw::Application) {
    let quit = gio::SimpleAction::new("quit", None);
    {
        let app = app.clone();
        quit.connect_activate(move |_, _| app.quit());
    }
    app.add_action(&quit);

    let about = gio::SimpleAction::new("about", None);
    {
        let app = app.clone();
        about.connect_activate(move |_, _| {
            let dialog = adw::AboutDialog::builder()
                .application_name("BigTube")
                .application_icon("bigtube")
                .developer_name("Elton Fabricio a.k.a eltonff")
                .version(env!("CARGO_PKG_VERSION"))
                .license_type(gtk::License::MitX11)
                .website("https://github.com/eltonfabricio10/python-bigtube")
                .issue_url("https://github.com/eltonfabricio10/python-bigtube/issues")
                .build();
            dialog.present(app.active_window().as_ref());
        });
    }
    app.add_action(&about);
}

fn build_search_page(state: &Rc<AppState>) -> gtk::Widget {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Search bar.
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    bar.set_margin_top(8);
    bar.set_margin_bottom(8);
    bar.set_margin_start(12);
    bar.set_margin_end(12);
    let source = gtk::DropDown::from_strings(&[
        tr("YouTube").as_str(),
        tr("YouTube Music").as_str(),
        tr("Direct Link").as_str(),
    ]);
    let entry = gtk::SearchEntry::new();
    entry.set_hexpand(true);
    entry.set_placeholder_text(Some(&tr("Paste URL or type keywords...")));
    let button = gtk::Button::with_label(&tr("Search"));
    button.add_css_class("suggested-action");
    let btn_select = gtk::ToggleButton::new();
    btn_select.set_icon_name("selection-mode-symbolic");
    btn_select.add_css_class("flat");
    btn_select.set_tooltip_text(Some(&tr("Toggle Selection Mode")));
    bar.append(&source);
    bar.append(&entry);
    bar.append(&button);
    bar.append(&btn_select);

    // Results list.
    let factory = gtk::SignalListItemFactory::new();
    let on_download: RowAction = {
        let state = state.clone();
        Rc::new(move |item: VideoObject| on_download_clicked(&state, &item))
    };
    let on_play: RowAction = {
        let state = state.clone();
        Rc::new(move |item: VideoObject| {
            let store = state.search_store.clone();
            play_from_store(&state, &store, &item);
        })
    };
    let on_copy: RowAction = {
        let state = state.clone();
        Rc::new(move |item: VideoObject| {
            if let Some(win) = state.window.borrow().clone() {
                win.clipboard().set_text(&item.url());
                state.toast(&tr("Link Copied!"));
            }
        })
    };
    // Opening a playlist: the dialog plays from its own (cyclic) queue.
    let on_open: RowAction = {
        let state = state.clone();
        let on_download = on_download.clone();
        Rc::new(move |item: VideoObject| {
            let (Some(win), Some(player)) =
                (state.window.borrow().clone(), state.player.borrow().clone())
            else {
                return;
            };
            // Batch download (one quality dialog for the whole list).
            let on_download_all: crate::playlist::BatchAction = {
                let state = state.clone();
                Rc::new(move |items: Vec<VideoObject>| download_all(&state, items))
            };
            // Batch schedule (one quality + one time/recurrence for the list).
            let on_schedule_all: crate::playlist::BatchAction = {
                let state = state.clone();
                Rc::new(move |items: Vec<VideoObject>| schedule_all(&state, items))
            };
            crate::playlist::show(
                &win,
                item.url(),
                item.title(),
                player,
                on_download.clone(),
                on_download_all,
                on_schedule_all,
            );
        })
    };
    let setup_state = state.clone();
    factory.connect_setup(move |_, list_item| {
        let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
        let row = SearchResultRow::new();
        row.set_handlers(
            on_play.clone(),
            on_download.clone(),
            on_open.clone(),
            on_copy.clone(),
        );
        if let Some(player) = setup_state.player.borrow().clone() {
            row.set_now_playing(player.now_playing());
        }
        list_item.set_child(Some(&row));
    });
    factory.connect_bind(|_, list_item| {
        let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
        if let (Some(child), Some(item)) = (list_item.child(), list_item.item()) {
            let row = child.downcast::<SearchResultRow>().unwrap();
            let video = item.downcast::<VideoObject>().unwrap();
            row.set_item(&video);
        }
    });

    // NoSelection: rows act via their own buttons / selection-mode checkboxes, so
    // the ListView must not auto-select (and highlight) row 0 — that competes with
    // the now-playing highlight.
    let selection = gtk::NoSelection::new(Some(state.search_store.clone()));
    let list = gtk::ListView::new(Some(selection), Some(factory));
    list.set_vexpand(true);
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_child(Some(&list));
    scrolled.set_vexpand(true);

    // Empty-state / results stack.
    let empty = status_page(
        "system-search-symbolic",
        &tr("No Results"),
        &tr("Search for videos or paste a URL"),
    );
    state.search_stack.set_vexpand(true);
    state.search_stack.add_named(&empty, Some("empty"));
    state
        .search_stack
        .add_named(&loading_page(&tr("Searching")), Some("loading"));
    state.search_stack.add_named(&scrolled, Some("list"));
    state.search_stack.set_visible_child_name("empty");

    // Batch selection bar (revealed in selection mode).
    let batch = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    batch.set_halign(gtk::Align::Center);
    batch.set_margin_bottom(6);
    batch.add_css_class("linked");
    let select_all = gtk::Button::with_label(&tr("Select All / None"));
    state
        .select_btn
        .set_label(&tr("Download Selected ({count})").replace("{count}", "0"));
    state.select_btn.add_css_class("suggested-action");
    state.select_btn.set_sensitive(false);
    state.sched_selected_btn.set_label(&tr("Schedule Selected"));
    state.sched_selected_btn.set_sensitive(false);
    batch.append(&select_all);
    batch.append(&state.select_btn);
    batch.append(&state.sched_selected_btn);
    state.select_revealer.set_child(Some(&batch));
    state.select_revealer.set_reveal_child(false);

    page.append(&page_header(&tr("Search Manager"), &[]));
    page.append(&bar);
    page.append(&state.select_revealer);
    page.append(&state.search_stack);

    // Toggle selection mode: flips the flag on every item and reveals the bar.
    {
        let state = state.clone();
        btn_select.connect_toggled(move |b| {
            let on = b.is_active();
            state.select_mode.set(on);
            for i in 0..state.search_store.n_items() {
                if let Some(o) = state
                    .search_store
                    .item(i)
                    .and_then(|o| o.downcast::<VideoObject>().ok())
                {
                    o.set_selection_mode(on);
                    if !on {
                        o.set_is_selected(false);
                    }
                }
            }
            state.select_revealer.set_reveal_child(on);
            state.refresh_selection_count();
        });
    }
    // Select all / none toggles every item.
    {
        let state = state.clone();
        select_all.connect_clicked(move |_| {
            let store = &state.search_store;
            let mut any_unselected = false;
            for i in 0..store.n_items() {
                if let Some(o) = store.item(i).and_then(|o| o.downcast::<VideoObject>().ok()) {
                    if !o.is_playlist() && !o.is_selected() {
                        any_unselected = true;
                        break;
                    }
                }
            }
            for i in 0..store.n_items() {
                if let Some(o) = store.item(i).and_then(|o| o.downcast::<VideoObject>().ok()) {
                    if !o.is_playlist() {
                        o.set_is_selected(any_unselected);
                    }
                }
            }
            state.refresh_selection_count();
        });
    }
    // Download all selected items.
    {
        let state = state.clone();
        let btn = state.select_btn.clone();
        btn.connect_clicked(move |_| {
            let store = &state.search_store;
            let mut picked = Vec::new();
            for i in 0..store.n_items() {
                if let Some(o) = store.item(i).and_then(|o| o.downcast::<VideoObject>().ok()) {
                    if o.is_selected() {
                        picked.push(o);
                    }
                }
            }
            download_all(&state, picked);
        });
    }
    // Schedule all selected items (same picked set, routed to the schedule flow).
    {
        let state = state.clone();
        let btn = state.sched_selected_btn.clone();
        btn.connect_clicked(move |_| {
            let store = &state.search_store;
            let mut picked = Vec::new();
            for i in 0..store.n_items() {
                if let Some(o) = store.item(i).and_then(|o| o.downcast::<VideoObject>().ok()) {
                    if o.is_selected() {
                        picked.push(o);
                    }
                }
            }
            schedule_all(&state, picked);
        });
    }

    // Suggestion popover (search-history matches while typing).
    let popover = gtk::Popover::new();
    popover.set_parent(&entry);
    popover.set_autohide(false);
    popover.set_has_arrow(false);
    popover.set_position(gtk::PositionType::Bottom);
    popover.add_css_class("menu");
    popover.add_css_class("suggestions-popover");
    // A plain vertical Box (not a ListBox: exact natural height, no ListBoxRow
    // overhead/inflation) inside a ScrolledWindow that PROPAGATES the box's
    // natural height up to a cap. Few matches -> the scroll is exactly the box
    // height (no leftover); many -> it caps and scrolls. No manual min/max-content
    // height (that path hit a Gtk-CRITICAL); the close+reopen on each keystroke
    // gives a fresh surface so the popover never keeps a stale height.
    let sugg_list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sugg_list.set_valign(gtk::Align::Start);
    let sugg_scroll = gtk::ScrolledWindow::new();
    sugg_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    sugg_scroll.set_propagate_natural_height(true);
    sugg_scroll.set_max_content_height(240);
    sugg_scroll.set_min_content_width(320);
    sugg_scroll.set_child(Some(&sugg_list));
    popover.set_child(Some(&sugg_scroll));

    // Dismiss the popover when the entry loses focus (clicking away, minimizing,
    // switching windows) so it never gets stuck on screen.
    {
        let focus = gtk::EventControllerFocus::new();
        let popover = popover.clone();
        focus.connect_leave(move |_| popover.popdown());
        entry.add_controller(focus);
    }

    // The last query actually searched — guards against the delayed
    // `search-changed` wiping freshly loaded results after a suggestion pick.
    let last_query: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

    // Shared search trigger.
    let trigger: Rc<dyn Fn()> = {
        let state = state.clone();
        let entry = entry.clone();
        let source = source.clone();
        let popover = popover.clone();
        let last_query = last_query.clone();
        Rc::new(move || {
            popover.popdown();
            let query = entry.text().to_string();
            // Empty search: tell the user instead of silently doing nothing.
            if query.trim().is_empty() {
                state.toast(&tr("Type something to search."));
                return;
            }
            *last_query.borrow_mut() = query.trim().to_string();
            // Auto-pick the source from the input, only at search time:
            //  - a URL while on YouTube / YouTube Music → switch to Direct Link;
            //  - plain text while on YouTube / YouTube Music → leave it;
            //  - plain text while on Direct Link → back to YouTube.
            let is_url = bigtube_core::validators::is_valid_url(query.trim());
            match source.selected() {
                2 if !is_url => source.set_selected(0), // Direct Link → YouTube
                0 | 1 if is_url => source.set_selected(2), // YT/YTMusic → Direct Link
                _ => {}
            }
            let src = match source.selected() {
                1 => "youtube_music",
                2 => "url",
                _ => "youtube",
            }
            .to_string();
            run_search(&state, query, src);
        })
    };

    // Expose "paste a URL and search" so the clipboard monitor can drive it.
    {
        let entry = entry.clone();
        let trigger = trigger.clone();
        let stack = state.stack.clone();
        *state.paste_and_search.borrow_mut() = Some(Rc::new(move |url: String| {
            stack.set_visible_child_name("search");
            entry.set_text(&url);
            trigger();
        }));
    }

    // Self-reference so a suggestion's delete button can rebuild (close+reopen)
    // the popover to resize it to the remaining matches.
    #[allow(clippy::type_complexity)]
    let rebuild_slot: Rc<RefCell<Option<Rc<dyn Fn(&str)>>>> = Rc::new(RefCell::new(None));

    // Rebuild the suggestion list for the current text.
    let rebuild: Rc<dyn Fn(&str)> = {
        let sugg_list = sugg_list.clone();
        let popover = popover.clone();
        let entry = entry.clone();
        let trigger = trigger.clone();
        let rebuild_slot = rebuild_slot.clone();
        Rc::new(move |text: &str| {
            // Close on every keystroke; we re-open (popup) below only when there
            // are matches. Re-opening yields a fresh surface sized to the current
            // content, so the popover never keeps a stale, stretched height — and
            // clearing the text simply leaves it closed.
            popover.popdown();
            while let Some(c) = sugg_list.first_child() {
                sugg_list.remove(&c);
            }
            let (enabled, max) = {
                let c = config::global().read().unwrap();
                (
                    c.get_bool("enable_suggestions"),
                    c.get_i64("max_suggestions").max(1) as usize,
                )
            };
            if !enabled || text.trim().is_empty() {
                popover.popdown();
                return;
            }
            let matches = bigtube_core::search_history::SearchHistory::new(search_history_path())
                .get_matches(text, max);
            if matches.is_empty() {
                popover.popdown();
                return;
            }
            for m in matches {
                let rowbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                rowbox.add_css_class("suggestion-row");
                let pick = gtk::Button::new();
                pick.add_css_class("flat");
                pick.set_hexpand(true);
                // Don't steal focus from the entry (keeps the popover open on click).
                pick.set_can_focus(false);
                pick.set_focus_on_click(false);
                let inner = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                let icon = gtk::Image::from_icon_name("document-open-recent-symbolic");
                icon.add_css_class("dim-label");
                icon.set_pixel_size(14);
                let lbl = gtk::Label::new(Some(&m));
                lbl.set_xalign(0.0);
                lbl.set_hexpand(true);
                lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
                inner.append(&icon);
                inner.append(&lbl);
                pick.set_child(Some(&inner));
                let del = gtk::Button::from_icon_name("window-close-symbolic");
                del.add_css_class("flat");
                del.set_valign(gtk::Align::Center);
                del.set_can_focus(false);
                del.set_focus_on_click(false);
                del.set_tooltip_text(Some(&tr("Remove from list")));
                rowbox.append(&pick);
                rowbox.append(&del);
                sugg_list.append(&rowbox);

                {
                    let entry = entry.clone();
                    let trigger = trigger.clone();
                    let q = m.clone();
                    pick.connect_clicked(move |_| {
                        entry.set_text(&q);
                        trigger();
                    });
                }
                {
                    let entry = entry.clone();
                    let rebuild_slot = rebuild_slot.clone();
                    let q = m.clone();
                    del.connect_clicked(move |_| {
                        bigtube_core::search_history::SearchHistory::new(search_history_path())
                            .remove_item(&q);
                        // Close and reopen the popover, resized to the matches that
                        // remain (an empty result just leaves it closed).
                        if let Some(rebuild) = rebuild_slot.borrow().as_ref() {
                            rebuild(&entry.text());
                        }
                    });
                }
            }
            // Match the popover width to the search entry so labels aren't clipped;
            // the popover hugs the box height on its own (no scroll = no leftover).
            popover.set_size_request(entry.width().max(320), -1);
            popover.popup();
        })
    };
    *rebuild_slot.borrow_mut() = Some(rebuild.clone());

    // Typing refreshes suggestions only. Results are NOT cleared on every
    // keystroke — only when the field is fully emptied — so the previous results
    // stay visible until a new search replaces them. (The source is auto-picked
    // on Search, not while typing — see `trigger`.)
    {
        let state = state.clone();
        let rebuild = rebuild.clone();
        let last_query = last_query.clone();
        entry.connect_search_changed(move |e| {
            let text = e.text().to_string();
            // Clear results ONLY when all text is deleted (also closes the popover).
            if text.trim().is_empty() {
                state.search_store.remove_all();
                state.update_search_empty();
                rebuild(&text); // rebuild("") -> popdown
                return;
            }
            if text.trim() == *last_query.borrow() {
                return; // results we just loaded for this query — keep them
            }
            rebuild(&text);
        });
    }

    {
        let trigger = trigger.clone();
        button.connect_clicked(move |_| trigger());
    }
    entry.connect_activate(move |_| trigger());

    page.upcast()
}

const QUALITY_OPTIONS: [(&str, bigtube_core::enums::VideoQuality); 16] = {
    use bigtube_core::enums::VideoQuality::*;
    [
        ("Ask Every Time", Ask),
        ("Best (MKV)", Best),
        ("4K (2160p)", P2160),
        ("2K (1440p)", P1440),
        ("Full HD (1080p)", P1080),
        ("HD (720p)", P720),
        ("SD (480p)", P480),
        ("Low Definition (360p)", P360),
        ("Very Low (240p)", P240),
        ("Lowest (144p)", P144),
        ("Audio (MP3)", AudioMp3),
        ("Audio (M4A)", AudioM4a),
        ("Audio (Opus)", AudioOpus),
        ("Audio (FLAC)", AudioFlac),
        ("Audio (WAV)", AudioWav),
        ("Audio (AAC)", AudioAac),
    ]
};

/// In-app preview/player quality options (config values double as labels).
const PREVIEW_QUALITIES: &[&str] = &["360p", "480p", "720p"];

/// Human-readable accent-colour name (matches `locales.py` so the catalogs resolve).
fn color_label(value: &str) -> &'static str {
    match value {
        "violet" => "Modern Violet",
        "emerald" => "Emerald Green",
        "sunburst" => "Sunburst Orange",
        "rose" => "Vibrant Rose",
        "cyan" => "Nordic Cyan",
        "nordic" => "Nordic Snow",
        "gruvbox" => "Gruvbox Retro",
        "catppuccin" => "Catppuccin Mocha",
        "dracula" => "Dracula Dark",
        "tokyo_night" => "Tokyo Night",
        "rose_pine" => "Rosé Pine",
        "solarized" => "Solarized Dark",
        "monokai" => "Monokai Pro",
        "cyberpunk" => "Cyberpunk Neon",
        "bigtube" => "BigTube Brand",
        _ => "Default Blue",
    }
}

/// Browsers offered for "Cookies From Browser", detected on PATH (`download_settings.py`).
/// Detect installed browsers by probing the *real* binary at canonical absolute
/// install paths (`/usr/bin`, `/usr/lib`, `/opt`) — deliberately NOT a `$PATH`
/// lookup. Tools like `auto-tweaks-browser` drop wrapper scripts into
/// `/usr/local/bin` (and `~/.local/bin`) for every browser, so a PATH/`which`
/// scan reports browsers that aren't actually installed. We only trust the real
/// package locations and reject anything that resolves into a wrapper.
fn detect_browsers() -> Vec<(&'static str, String)> {
    let candidates: [(&str, &str, &[&str]); 7] = [
        (
            "firefox",
            "Firefox",
            &[
                "/usr/bin/firefox",
                "/usr/lib/firefox/firefox",
                "/opt/firefox/firefox",
            ],
        ),
        (
            "chrome",
            "Chrome",
            &[
                "/usr/bin/google-chrome-stable",
                "/usr/bin/google-chrome",
                "/opt/google/chrome/google-chrome",
            ],
        ),
        (
            "chromium",
            "Chromium",
            &["/usr/bin/chromium", "/usr/lib/chromium/chromium"],
        ),
        (
            "brave",
            "Brave",
            &[
                "/usr/bin/brave",
                "/usr/bin/brave-browser",
                "/opt/brave-bin/brave",
                "/opt/brave.com/brave/brave",
            ],
        ),
        (
            "edge",
            "Microsoft Edge",
            &[
                "/usr/bin/microsoft-edge",
                "/usr/bin/microsoft-edge-stable",
                "/opt/microsoft/msedge/microsoft-edge",
            ],
        ),
        (
            "vivaldi",
            "Vivaldi",
            &[
                "/usr/bin/vivaldi",
                "/usr/bin/vivaldi-stable",
                "/opt/vivaldi/vivaldi",
            ],
        ),
        (
            "opera",
            "Opera",
            &["/usr/bin/opera", "/usr/lib/x86_64-linux-gnu/opera/opera"],
        ),
    ];
    let mut out: Vec<(&str, String)> = vec![("", tr("None"))];
    for (val, label, paths) in candidates {
        if paths.iter().any(|p| is_real_browser_binary(p)) {
            out.push((val, label.to_string()));
        }
    }
    out
}

/// True if `path` is a real browser binary — it exists and does not resolve into
/// a wrapper directory (`/usr/local/...`) or an `auto-tweaks-browser` shim.
fn is_real_browser_binary(path: &str) -> bool {
    let p = std::path::Path::new(path);
    if !p.exists() {
        return false;
    }
    match std::fs::canonicalize(p) {
        Ok(real) => {
            let s = real.to_string_lossy();
            !s.starts_with("/usr/local/") && !s.contains("browser-tweaks")
        }
        Err(_) => true,
    }
}

fn set_cfg(key: &str, value: serde_json::Value) {
    config::global().write().unwrap().set(key, value);
}

fn build_settings_page(state: &Rc<AppState>) -> gtk::Widget {
    let page = adw::PreferencesPage::new();

    // Snapshot every config value up front (drops the read lock before wiring).
    let c = {
        let cfg = config::global().read().unwrap();
        Cfg {
            theme_mode: cfg.get_string("theme_mode"),
            theme_color: cfg.get_string("theme_color"),
            default_quality: cfg.get_string("default_quality"),
            preview_quality: cfg.get_string("preview_quality"),
            download_path: cfg.get_string("download_path"),
            max_concurrent: cfg.get_i64("max_concurrent_downloads"),
            concurrent_fragments: cfg.get_i64("concurrent_fragments"),
            rate_limit: cfg.get_i64("rate_limit"),
            add_metadata: cfg.get_bool("add_metadata"),
            subtitle_mode: cfg.get_string("subtitle_mode"),
            subtitle_langs: cfg.get_string("subtitle_langs"),
            subtitle_auto: cfg.get_bool("subtitle_auto"),
            system_notifications: cfg.get_bool("system_notifications"),
            monitor_clipboard: cfg.get_bool("monitor_clipboard"),
            post_process_cmd: cfg.get_string("post_process_cmd"),
            cookies_file: cfg.get_string("cookies_file"),
            cookies_browser: cfg.get_string("cookies_browser"),
            user_agent: cfg.get_string("user_agent"),
            proxy: cfg.get_string("proxy"),
            save_history: cfg.get_bool("save_history"),
            auto_clear_finished: cfg.get_bool("auto_clear_finished"),
            converter_path: cfg.get_string("converter_path"),
            use_source_folder: cfg.get_bool("use_source_folder"),
            save_converter_history: cfg.get_bool("save_converter_history"),
            search_limit: cfg.get_i64("search_limit"),
            enable_suggestions: cfg.get_bool("enable_suggestions"),
            max_suggestions: cfg.get_i64("max_suggestions"),
            save_search_history: cfg.get_bool("save_search_history"),
        }
    };

    page.add(&build_appearance_group(state, &c));
    page.add(&build_downloads_group(state, &c));
    page.add(&build_subtitles_group(state, &c));
    page.add(&build_playback_group(state, &c));
    page.add(&build_converter_group(state, &c));
    page.add(&build_search_group(state, &c));
    page.add(&build_network_group(state, &c));
    page.add(&build_storage_group(state, &c));

    page.upcast()
}

/// Snapshot of every setting read once when the page is built.
struct Cfg {
    theme_mode: String,
    theme_color: String,
    default_quality: String,
    preview_quality: String,
    download_path: String,
    max_concurrent: i64,
    concurrent_fragments: i64,
    rate_limit: i64,
    add_metadata: bool,
    subtitle_mode: String,
    subtitle_langs: String,
    subtitle_auto: bool,
    system_notifications: bool,
    monitor_clipboard: bool,
    post_process_cmd: String,
    cookies_file: String,
    cookies_browser: String,
    user_agent: String,
    proxy: String,
    save_history: bool,
    auto_clear_finished: bool,
    converter_path: String,
    use_source_folder: bool,
    save_converter_history: bool,
    search_limit: i64,
    enable_suggestions: bool,
    max_suggestions: i64,
    save_search_history: bool,
}

fn build_appearance_group(state: &Rc<AppState>, c: &Cfg) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Appearance"))
        .build();

    // Interface theme.
    let theme_modes = ["system", "light", "dark"];
    let theme_row = combo_row(
        &tr("Interface Theme"),
        &[tr("System"), tr("Light"), tr("Dark")],
    );
    theme_row.set_subtitle(&tr("Follow the system or force light/dark"));
    theme_row.set_selected(
        theme_modes
            .iter()
            .position(|m| *m == c.theme_mode)
            .unwrap_or(0) as u32,
    );
    {
        let state = state.clone();
        theme_row.connect_selected_notify(move |row| {
            let val = theme_modes
                .get(row.selected() as usize)
                .copied()
                .unwrap_or("system");
            set_cfg("theme_mode", serde_json::json!(val));
            if let Some(w) = state.window.borrow().clone() {
                apply_theme(&w);
            }
        });
    }
    group.add(&theme_row);

    // Colour scheme (pretty, translated labels).
    let color_values: Vec<&str> = bigtube_core::enums::ThemeColor::ALL
        .iter()
        .map(|c| c.as_value())
        .collect();
    let color_labels: Vec<String> = color_values.iter().map(|v| tr(color_label(v))).collect();
    let color_row = combo_row(&tr("Color Scheme"), &color_labels);
    color_row.set_subtitle(&tr("Accent colour used across the app"));
    color_row.set_selected(
        color_values
            .iter()
            .position(|v| *v == c.theme_color)
            .unwrap_or(0) as u32,
    );
    {
        let state = state.clone();
        let color_values = color_values.clone();
        color_row.connect_selected_notify(move |row| {
            let val = color_values
                .get(row.selected() as usize)
                .copied()
                .unwrap_or("default");
            set_cfg("theme_color", serde_json::json!(val));
            if let Some(w) = state.window.borrow().clone() {
                apply_theme(&w);
            }
        });
    }
    group.add(&color_row);

    // Current version + yt-dlp update.
    let version_row = adw::ActionRow::builder()
        .title(tr("Current Version"))
        .subtitle("yt-dlp v?")
        .build();
    let update_btn = gtk::Button::from_icon_name("software-update-symbolic");
    update_btn.add_css_class("flat");
    update_btn.set_valign(gtk::Align::Center);
    version_row.add_suffix(&update_btn);
    group.add(&version_row);
    refresh_version_subtitle(&version_row);
    {
        let state = state.clone();
        let version_row = version_row.clone();
        update_btn.connect_clicked(move |btn| {
            btn.set_sensitive(false);
            run_update(&state, &version_row, btn.clone());
        });
    }

    group
}

fn build_downloads_group(state: &Rc<AppState>, c: &Cfg) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Downloads"))
        .build();

    // Download folder.
    let folder_row = adw::ActionRow::builder()
        .title(tr("Download Folder"))
        .subtitle(&c.download_path)
        .build();
    let folder_btn = gtk::Button::from_icon_name("folder-open-symbolic");
    folder_btn.add_css_class("flat");
    folder_btn.set_valign(gtk::Align::Center);
    {
        let state = state.clone();
        let folder_row = folder_row.clone();
        folder_btn.connect_clicked(move |_| pick_download_folder(&state, &folder_row));
    }
    folder_row.add_suffix(&folder_btn);
    group.add(&folder_row);

    // Preferred quality (translated labels).
    let quality_labels: Vec<String> = QUALITY_OPTIONS.iter().map(|(l, _)| tr(l)).collect();
    let quality_row = combo_row(&tr("Preferred Quality"), &quality_labels);
    quality_row.set_subtitle(&tr("Default quality for new downloads"));
    let qsel = QUALITY_OPTIONS
        .iter()
        .position(|(_, q)| q.as_value() == c.default_quality)
        .unwrap_or(0);
    quality_row.set_selected(qsel as u32);
    quality_row.connect_selected_notify(|row| {
        if let Some((_, q)) = QUALITY_OPTIONS.get(row.selected() as usize) {
            set_cfg("default_quality", serde_json::json!(q.as_value()));
        }
    });
    group.add(&quality_row);

    group.add(&spin_row(
        &tr("Max Simultaneous Downloads"),
        &tr("How many downloads run at the same time"),
        1.0,
        10.0,
        c.max_concurrent as f64,
        |v| set_cfg("max_concurrent_downloads", serde_json::json!(v as i64)),
    ));
    group.add(&spin_row(
        &tr("Concurrent Fragments"),
        &tr("Parallel fragments per download (faster, uses more bandwidth)"),
        1.0,
        16.0,
        c.concurrent_fragments as f64,
        |v| set_cfg("concurrent_fragments", serde_json::json!(v as i64)),
    ));
    group.add(&spin_row_step(
        &tr("Download Speed Limit (KB/s)"),
        &tr("Cap the download rate (0 = unlimited)"),
        0.0,
        100_000.0,
        100.0,
        c.rate_limit as f64,
        |v| set_cfg("rate_limit", serde_json::json!(v as i64)),
    ));
    group.add(&switch_row(
        &tr("Add Metadata to Files"),
        &tr("Embed title, artist and other tags in the file"),
        c.add_metadata,
        |v| set_cfg("add_metadata", serde_json::json!(v)),
    ));
    group.add(&switch_row(
        &tr("System Notifications"),
        &tr("Notify when a download finishes"),
        c.system_notifications,
        |v| set_cfg("system_notifications", serde_json::json!(v)),
    ));
    group.add(&switch_row(
        &tr("Enable ClipBoard Monitor"),
        &tr("Detect copied links and offer to download them"),
        c.monitor_clipboard,
        |v| set_cfg("monitor_clipboard", serde_json::json!(v)),
    ));

    group
}

/// Subtitle download settings: mode, languages, and auto-generated captions.
fn build_subtitles_group(_state: &Rc<AppState>, c: &Cfg) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Subtitles"))
        .build();

    // Mode: off / embed in the video / separate file / both.
    let modes = ["off", "embed", "file", "both"];
    let mode_row = combo_row(
        &tr("Subtitles"),
        &[
            tr("Off"),
            tr("Embed in video"),
            tr("Separate file"),
            tr("Embed + file"),
        ],
    );
    mode_row.set_subtitle(&tr("Download subtitles and how to store them"));
    mode_row.set_selected(
        modes
            .iter()
            .position(|m| *m == c.subtitle_mode)
            .unwrap_or(0) as u32,
    );
    mode_row.connect_selected_notify(move |row| {
        let val = modes.get(row.selected() as usize).copied().unwrap_or("off");
        set_cfg("subtitle_mode", serde_json::json!(val));
    });
    group.add(&mode_row);

    // Languages (comma-separated, validated lightly).
    let lang_row = adw::EntryRow::builder()
        .title(tr("Languages"))
        .text(&c.subtitle_langs)
        .show_apply_button(true)
        .build();
    lang_row.set_tooltip_text(Some(&tr("Comma-separated language codes, e.g. pt, en, es")));
    lang_row.connect_apply(|r| {
        let txt = r.text().trim().to_string();
        set_cfg("subtitle_langs", serde_json::json!(txt));
    });
    group.add(&lang_row);

    group.add(&switch_row(
        &tr("Include Auto-generated"),
        &tr("Also fetch automatic (machine) captions"),
        c.subtitle_auto,
        |v| set_cfg("subtitle_auto", serde_json::json!(v)),
    ));

    group
}

/// In-app player / preview settings.
fn build_playback_group(_state: &Rc<AppState>, c: &Cfg) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Playback"))
        .build();

    // In-app preview/player quality. 360p is progressive (rock-solid); 480p/720p
    // stream via HLS. Takes effect on the next item played.
    let preview_row = combo_row(&tr("Preview Quality"), PREVIEW_QUALITIES);
    preview_row.set_subtitle(&tr("Quality used by the in-app player"));
    let psel = PREVIEW_QUALITIES
        .iter()
        .position(|q| *q == c.preview_quality)
        .unwrap_or(0);
    preview_row.set_selected(psel as u32);
    preview_row.connect_selected_notify(|row| {
        if let Some(q) = PREVIEW_QUALITIES.get(row.selected() as usize) {
            set_cfg("preview_quality", serde_json::json!(q));
        }
    });
    group.add(&preview_row);
    group
}

/// Network, authentication and advanced (cookies, proxy, UA, post-processing).
fn build_network_group(state: &Rc<AppState>, c: &Cfg) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Network & Advanced"))
        .build();

    // Cookies file.
    let cookies_row = adw::ActionRow::builder()
        .title(tr("Cookies File"))
        .subtitle(&c.cookies_file)
        .build();
    let cookies_btn = gtk::Button::from_icon_name("document-open-symbolic");
    cookies_btn.add_css_class("flat");
    cookies_btn.set_valign(gtk::Align::Center);
    {
        let state = state.clone();
        let cookies_row = cookies_row.clone();
        cookies_btn.connect_clicked(move |_| pick_cookies_file(&state, &cookies_row));
    }
    cookies_row.add_suffix(&cookies_btn);
    group.add(&cookies_row);

    // Cookies from browser (detected on PATH).
    let browsers = detect_browsers();
    let browser_labels: Vec<String> = browsers.iter().map(|(_, l)| l.clone()).collect();
    let browser_row = combo_row(&tr("Cookies From Browser"), &browser_labels);
    browser_row.set_subtitle(&tr("Use this browser's cookies for restricted videos"));
    let bsel = browsers
        .iter()
        .position(|(v, _)| *v == c.cookies_browser)
        .unwrap_or(0);
    browser_row.set_selected(bsel as u32);
    {
        let browsers: Vec<&str> = browsers.iter().map(|(v, _)| *v).collect();
        browser_row.connect_selected_notify(move |row| {
            let val = browsers.get(row.selected() as usize).copied().unwrap_or("");
            set_cfg("cookies_browser", serde_json::json!(val));
        });
    }
    group.add(&browser_row);

    group.add(&entry_row_with_presets(
        &tr("User Agent"),
        &c.user_agent,
        &tr("Installed browsers"),
        user_agent_presets(),
        "user_agent",
        state,
        validate_user_agent,
    ));
    group.add(&entry_row_with_presets(
        &tr("Proxy"),
        &c.proxy,
        &tr("Known proxies"),
        to_presets(&PROXY_PRESETS),
        "proxy",
        state,
        validate_proxy,
    ));
    group.add(&entry_row_with_presets(
        &tr("Post-Processing Command"),
        &c.post_process_cmd,
        &tr("Common commands"),
        to_presets(&POST_PROCESS_PRESETS),
        "post_process_cmd",
        state,
        validate_post_process,
    ));

    group
}

/// Common `yt-dlp --exec` post-processing commands (`{}` = the output file).
const POST_PROCESS_PRESETS: [(&str, &str); 5] = [
    ("Choose a preset…", ""),
    ("Desktop notification", "notify-send 'BigTube' 'Done: {}'"),
    ("Open output folder", "xdg-open \"$(dirname \"{}\")\""),
    ("Make read-only", "chmod 444 {}"),
    ("Update timestamp", "touch {}"),
];

/// Well-known *local* proxy endpoints. Public free-proxy IPs are ephemeral and
/// untrustworthy, so we offer reliable local setups (Tor/Privoxy) instead — the
/// field stays free-text for any custom proxy.
const PROXY_PRESETS: [(&str, &str); 4] = [
    ("Choose a preset…", ""),
    ("Tor (SOCKS5)", "socks5://127.0.0.1:9050"),
    ("Local HTTP proxy", "http://127.0.0.1:8080"),
    ("Privoxy (HTTP)", "http://127.0.0.1:8118"),
];

/// A current Linux User-Agent for a detected browser key (`detect_browsers`).
fn browser_ua(key: &str) -> Option<&'static str> {
    const CHROME: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
    Some(match key {
        "firefox" => "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0",
        "chrome" | "chromium" | "brave" | "vivaldi" => CHROME,
        "edge" => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0",
        "opera" => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 OPR/111.0.0.0",
        _ => return None,
    })
}

/// User-Agent presets for browsers actually installed on this machine — one
/// entry per installed browser ([`detect_browsers`] already filters to real
/// installs, not `$PATH` wrappers).
fn user_agent_presets() -> Vec<(String, String)> {
    let mut out = vec![(tr("Choose a preset…"), String::new())];
    for (key, label) in detect_browsers() {
        if key.is_empty() {
            continue; // the "None" sentinel
        }
        if let Some(ua) = browser_ua(key) {
            out.push((label, ua.to_string()));
        }
    }
    out
}

/// Translate the labels of a static preset table into an owned preset list.
fn to_presets(arr: &[(&str, &str)]) -> Vec<(String, String)> {
    arr.iter().map(|(l, v)| (tr(l), v.to_string())).collect()
}

/// Validator: empty (no proxy) or a `scheme://host:port` with a known scheme.
fn validate_proxy(s: &str) -> Option<String> {
    let (ok, _, _) = bigtube_core::config::ConfigManager::validate_proxy_url(s);
    if ok {
        None
    } else {
        Some(tr("Invalid proxy address — use scheme://host:port."))
    }
}

/// Validator: a User-Agent must be a single printable line (no control chars).
fn validate_user_agent(s: &str) -> Option<String> {
    if s.chars().any(char::is_control) {
        Some(tr("Invalid user agent."))
    } else {
        None
    }
}

/// Validator: the post-processing command's program must exist on `$PATH`.
fn validate_post_process(s: &str) -> Option<String> {
    let prog = s.split_whitespace().next().unwrap_or("");
    if prog.is_empty() || bigtube_core::util::which(prog).is_some() {
        None
    } else {
        Some(format!("{} {}", tr("Command not found on PATH:"), prog))
    }
}

/// An entry row with a suffix dropdown of presets and an apply-time validator;
/// choosing a preset fills the entry and persists `cfg_key`. The first preset is
/// a no-op placeholder. An invalid entry is rejected with a toast and reverted.
fn entry_row_with_presets(
    title: &str,
    value: &str,
    tooltip: &str,
    presets: Vec<(String, String)>,
    cfg_key: &'static str,
    state: &Rc<AppState>,
    validate: fn(&str) -> Option<String>,
) -> adw::EntryRow {
    let row = adw::EntryRow::builder()
        .title(title)
        .text(value)
        .show_apply_button(true)
        .build();
    {
        let state = state.clone();
        row.connect_apply(move |r| {
            let txt = r.text().trim().to_string();
            if !txt.is_empty() {
                if let Some(err) = validate(&txt) {
                    state.toast(&err);
                    // Revert to the last saved value so the bad input doesn't stick.
                    let saved = config::global().read().unwrap().get_string(cfg_key);
                    r.set_text(&saved);
                    return;
                }
            }
            set_cfg(cfg_key, serde_json::json!(txt));
        });
    }

    let dd = {
        let labels: Vec<&str> = presets.iter().map(|(l, _)| l.as_str()).collect();
        gtk::DropDown::from_strings(&labels)
    };
    dd.set_valign(gtk::Align::Center);
    dd.set_tooltip_text(Some(tooltip));
    {
        let row = row.clone();
        dd.connect_selected_notify(move |d| {
            if let Some((_, val)) = presets.get(d.selected() as usize) {
                if !val.is_empty() {
                    row.set_text(val);
                    set_cfg(cfg_key, serde_json::json!(val));
                }
            }
        });
    }
    row.add_suffix(&dd);
    row
}

fn build_storage_group(state: &Rc<AppState>, c: &Cfg) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Storage"))
        .build();

    group.add(&switch_row(
        &tr("Save Download History"),
        &tr("Keep a record of completed downloads"),
        c.save_history,
        |v| set_cfg("save_history", serde_json::json!(v)),
    ));
    group.add(&switch_row(
        &tr("Clear All Data on Exit"),
        &tr("Wipe history and finished items when the app closes"),
        c.auto_clear_finished,
        |v| set_cfg("auto_clear_finished", serde_json::json!(v)),
    ));

    // Export history.
    {
        let state = state.clone();
        group.add(&button_row(
            &tr("Export History"),
            &tr("Save your download history to a file"),
            "document-export-symbolic",
            false,
            move || export_history(&state),
        ));
    }
    // Import history.
    {
        let state = state.clone();
        group.add(&button_row(
            &tr("Import History"),
            &tr("Restore history from a backup file"),
            "document-import-symbolic",
            false,
            move || import_history(&state),
        ));
    }
    // Reset all data (destructive).
    {
        let state = state.clone();
        group.add(&button_row(
            &tr("Clear All App Data (Reset)"),
            &tr("Permanently delete all settings and history"),
            "user-trash-symbolic",
            true,
            move || reset_all_data(&state),
        ));
    }

    group
}

fn build_converter_group(state: &Rc<AppState>, c: &Cfg) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Media Converter"))
        .build();

    let folder_row = adw::ActionRow::builder()
        .title(tr("Default Output Folder"))
        .subtitle(&c.converter_path)
        .build();
    let folder_btn = gtk::Button::from_icon_name("folder-open-symbolic");
    folder_btn.add_css_class("flat");
    folder_btn.set_valign(gtk::Align::Center);
    {
        let state = state.clone();
        let folder_row = folder_row.clone();
        folder_btn.connect_clicked(move |_| pick_converter_folder(&state, &folder_row));
    }
    folder_row.add_suffix(&folder_btn);
    group.add(&folder_row);

    group.add(&switch_row(
        &tr("Save in Source Directory"),
        &tr("Write the converted file next to the original"),
        c.use_source_folder,
        |v| set_cfg("use_source_folder", serde_json::json!(v)),
    ));
    group.add(&switch_row(
        &tr("Save Conversion History"),
        &tr("Keep a record of converted files"),
        c.save_converter_history,
        |v| set_cfg("save_converter_history", serde_json::json!(v)),
    ));
    {
        let state = state.clone();
        group.add(&button_row(
            &tr("Clear Conversion History"),
            &tr("Delete all previous conversion entries"),
            "user-trash-symbolic",
            false,
            move || clear_converter_history(&state),
        ));
    }

    group
}

fn build_search_group(state: &Rc<AppState>, c: &Cfg) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Search Settings"))
        .build();

    group.add(&switch_row(
        &tr("Save Search History"),
        &tr("Remember past searches for suggestions"),
        c.save_search_history,
        |v| set_cfg("save_search_history", serde_json::json!(v)),
    ));
    group.add(&spin_row(
        &tr("Maximum Search Results"),
        &tr("How many results to fetch per search"),
        5.0,
        100.0,
        c.search_limit as f64,
        |v| set_cfg("search_limit", serde_json::json!(v as i64)),
    ));
    group.add(&switch_row(
        &tr("Enable Search Suggestions"),
        &tr("Show matches from your history while typing"),
        c.enable_suggestions,
        |v| set_cfg("enable_suggestions", serde_json::json!(v)),
    ));
    group.add(&spin_row(
        &tr("Maximum Suggestions"),
        &tr("How many suggestions to show"),
        1.0,
        50.0,
        c.max_suggestions as f64,
        |v| set_cfg("max_suggestions", serde_json::json!(v as i64)),
    ));
    {
        let state = state.clone();
        group.add(&button_row(
            &tr("Clear Search History"),
            &tr("Delete all previous search entries"),
            "user-trash-symbolic",
            false,
            move || clear_search_history(&state),
        ));
    }

    group
}

/// A page banner (large title strip) shown at the top of each page.
/// A page title strip (full-width highlighted bar, matching the header bar
/// colour) with optional icon action buttons at the end.
fn page_header(title: &str, buttons: &[gtk::Button]) -> gtk::Widget {
    let cb = gtk::CenterBox::new();
    cb.add_css_class("page-title-bar");
    let lbl = gtk::Label::new(Some(title));
    lbl.add_css_class("title-1");
    cb.set_center_widget(Some(&lbl));
    if !buttons.is_empty() {
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        bx.set_halign(gtk::Align::End);
        for b in buttons {
            bx.append(b);
        }
        cb.set_end_widget(Some(&bx));
    }
    cb.upcast()
}

/// A centered spinner + label, used as a "loading" stack page.
fn loading_page(label: &str) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 12);
    b.set_valign(gtk::Align::Center);
    b.set_halign(gtk::Align::Center);
    b.set_vexpand(true);
    let spinner = gtk::Spinner::new();
    spinner.set_size_request(48, 48);
    spinner.start();
    b.append(&spinner);
    let lbl = gtk::Label::new(Some(label));
    lbl.add_css_class("dim-label");
    b.append(&lbl);
    b
}

/// A centered empty-state placeholder.
fn status_page(icon: &str, title: &str, desc: &str) -> adw::StatusPage {
    adw::StatusPage::builder()
        .icon_name(icon)
        .title(title)
        .description(desc)
        .vexpand(true)
        .build()
}

fn combo_row(title: &str, options: &[impl AsRef<str>]) -> adw::ComboRow {
    let strs: Vec<&str> = options.iter().map(|s| s.as_ref()).collect();
    let model = gtk::StringList::new(&strs);
    adw::ComboRow::builder().title(title).model(&model).build()
}

fn switch_row(
    title: &str,
    subtitle: &str,
    active: bool,
    on_change: impl Fn(bool) + 'static,
) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder()
        .title(title)
        .subtitle(subtitle)
        .active(active)
        .build();
    row.connect_active_notify(move |r| on_change(r.is_active()));
    row
}

fn spin_row(
    title: &str,
    subtitle: &str,
    min: f64,
    max: f64,
    value: f64,
    on_change: impl Fn(f64) + 'static,
) -> adw::SpinRow {
    spin_row_step(title, subtitle, min, max, 1.0, value, on_change)
}

fn spin_row_step(
    title: &str,
    subtitle: &str,
    min: f64,
    max: f64,
    step: f64,
    value: f64,
    on_change: impl Fn(f64) + 'static,
) -> adw::SpinRow {
    let row = adw::SpinRow::with_range(min, max, step);
    row.set_title(title);
    row.set_subtitle(subtitle);
    row.set_value(value);
    row.connect_value_notify(move |r| on_change(r.value()));
    row
}

/// An action row whose suffix is a single icon button.
fn button_row(
    title: &str,
    subtitle: &str,
    icon: &str,
    destructive: bool,
    on_click: impl Fn() + 'static,
) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .build();
    let btn = gtk::Button::from_icon_name(icon);
    btn.add_css_class("flat");
    if destructive {
        btn.add_css_class("destructive-action");
    }
    btn.set_valign(gtk::Align::Center);
    btn.connect_clicked(move |_| on_click());
    row.add_suffix(&btn);
    row
}

fn pick_download_folder(state: &Rc<AppState>, folder_row: &adw::ActionRow) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = gtk::FileDialog::builder().title(tr("Pick Folder")).build();
    let folder_row = folder_row.clone();
    dialog.select_folder(Some(&window), gtk::gio::Cancellable::NONE, move |res| {
        if let Ok(file) = res {
            if let Some(path) = file.path() {
                let p = path.to_string_lossy().to_string();
                set_cfg("download_path", serde_json::json!(p));
                folder_row.set_subtitle(&p);
            }
        }
    });
}

fn pick_converter_folder(state: &Rc<AppState>, folder_row: &adw::ActionRow) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = gtk::FileDialog::builder()
        .title(tr("Default Output Folder"))
        .build();
    let folder_row = folder_row.clone();
    dialog.select_folder(Some(&window), gtk::gio::Cancellable::NONE, move |res| {
        if let Ok(file) = res {
            if let Some(path) = file.path() {
                let p = path.to_string_lossy().to_string();
                set_cfg("converter_path", serde_json::json!(p));
                folder_row.set_subtitle(&p);
            }
        }
    });
}

fn pick_cookies_file(state: &Rc<AppState>, cookies_row: &adw::ActionRow) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = gtk::FileDialog::builder().title(tr("Cookies File")).build();
    let cookies_row = cookies_row.clone();
    dialog.open(Some(&window), gtk::gio::Cancellable::NONE, move |res| {
        if let Ok(file) = res {
            if let Some(path) = file.path() {
                let p = path.to_string_lossy().to_string();
                set_cfg("cookies_file", serde_json::json!(p));
                cookies_row.set_subtitle(&p);
            }
        }
    });
}

/// History file path under the app config dir (mirrors `reset_all`'s targets).
fn history_path() -> std::path::PathBuf {
    bigtube_core::paths::config_dir().join("history.json")
}

/// Path to the persisted scheduled-downloads file (`scheduled_downloads.py`).
fn scheduled_downloads_path() -> std::path::PathBuf {
    bigtube_core::paths::config_dir().join("scheduled_downloads.json")
}

/// Current Unix time in seconds (for comparing against `scheduled_time`).
fn now_epoch_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn refresh_version_subtitle(row: &adw::ActionRow) {
    let yt_dlp = config::global().read().unwrap().yt_dlp_path.clone();
    let (tx, rx) = async_channel::bounded::<String>(1);
    std::thread::spawn(move || {
        let v =
            bigtube_core::updater::get_local_version(&yt_dlp).unwrap_or_else(|| "?".to_string());
        let _ = tx.send_blocking(v);
    });
    let row = row.clone();
    glib::spawn_future_local(async move {
        if let Ok(v) = rx.recv().await {
            row.set_subtitle(&format!("yt-dlp v{v}"));
        }
    });
}

fn run_update(state: &Rc<AppState>, row: &adw::ActionRow, btn: gtk::Button) {
    let (yt_dlp, deno) = {
        let cfg = config::global().read().unwrap();
        (cfg.yt_dlp_path.clone(), cfg.deno_path.clone())
    };
    let (tx, rx) = async_channel::bounded::<(bool, bool, String)>(1);
    std::thread::spawn(move || {
        let (yt_ok, ver) = bigtube_core::updater::update_yt_dlp(&yt_dlp);
        let deno_ok = bigtube_core::updater::update_deno(&deno);
        let _ = tx.send_blocking((yt_ok, deno_ok, ver));
    });
    let state = state.clone();
    let row = row.clone();
    glib::spawn_future_local(async move {
        if let Ok((yt_ok, deno_ok, ver)) = rx.recv().await {
            if yt_ok {
                row.set_subtitle(&format!("yt-dlp v{ver}"));
                state.toast(&tr("Components updated successfully! ✅"));
            } else if deno_ok {
                state.toast(&tr("Deno updated, but yt-dlp failed."));
            } else {
                state.toast(&tr("Update check failed."));
            }
        }
        btn.set_sensitive(true);
    });
}

fn export_history(state: &Rc<AppState>) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = gtk::FileDialog::builder()
        .title(tr("Export History"))
        .initial_name("bigtube_history.json")
        .build();
    let state = state.clone();
    dialog.save(Some(&window), gtk::gio::Cancellable::NONE, move |res| {
        if let Ok(file) = res {
            if let Some(path) = file.path() {
                let items = bigtube_core::history::HistoryManager::new(history_path()).load();
                let ok = serde_json::to_string_pretty(&items)
                    .ok()
                    .and_then(|s| std::fs::write(&path, s).ok())
                    .is_some();
                state.toast(&tr(if ok {
                    "History exported successfully!"
                } else {
                    "Failed to select folder"
                }));
            }
        }
    });
}

fn import_history(state: &Rc<AppState>) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = gtk::FileDialog::builder()
        .title(tr("Import History"))
        .build();
    let state = state.clone();
    dialog.open(Some(&window), gtk::gio::Cancellable::NONE, move |res| {
        if let Ok(file) = res {
            if let Some(path) = file.path() {
                let msg = match std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                {
                    Some(serde_json::Value::Array(items)) => {
                        bigtube_core::history::HistoryManager::new(history_path())
                            .save_immediate(items);
                        "History imported successfully!"
                    }
                    Some(_) => "Invalid history file format",
                    None => "Error importing history file",
                };
                state.toast(&tr(msg));
            }
        }
    });
}

fn clear_search_history(state: &Rc<AppState>) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = adw::MessageDialog::new(
        Some(&window),
        Some(&tr("Clear search history?")),
        Some(&tr("Delete all previous search entries")),
    );
    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("clear", &tr("Clear"));
    dialog.set_response_appearance("clear", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    apply_theme_classes(&dialog);

    let state = state.clone();
    dialog.connect_response(None, move |dlg, resp| {
        dlg.close();
        if resp == "clear" {
            bigtube_core::search_history::SearchHistory::new(search_history_path()).clear();
            state.toast(&tr("History cleared successfully!"));
        }
    });
    dialog.present();
}

fn clear_converter_history(state: &Rc<AppState>) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = adw::MessageDialog::new(
        Some(&window),
        Some(&tr("Clear conversion history?")),
        Some(&tr("Delete all previous conversion entries")),
    );
    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("clear", &tr("Clear"));
    dialog.set_response_appearance("clear", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    apply_theme_classes(&dialog);

    let state = state.clone();
    dialog.connect_response(None, move |dlg, resp| {
        dlg.close();
        if resp == "clear" {
            bigtube_core::converter_history::ConverterHistoryManager::new(converter_history_path())
                .clear_all();
            state.toast(&tr("History cleared successfully!"));
        }
    });
    dialog.present();
}

fn reset_all_data(state: &Rc<AppState>) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let confirm = adw::MessageDialog::new(
        Some(&window),
        Some(&tr("Reset all app data?")),
        Some(&tr(
            "This permanently deletes all settings, history and scheduled downloads. The app will restart.",
        )),
    );
    confirm.add_response("cancel", &tr("Cancel"));
    confirm.add_response("reset", &tr("Reset & Restart"));
    confirm.set_response_appearance("reset", adw::ResponseAppearance::Destructive);
    confirm.set_default_response(Some("cancel"));
    confirm.set_close_response("cancel");
    apply_theme_classes(&confirm);

    let window_for_info = window.clone();
    confirm.connect_response(None, move |dlg, resp| {
        dlg.close();
        if resp != "reset" {
            return;
        }
        // Wipe config + every on-disk store (history, search, converter,
        // scheduled). reset_all() recreates the (now-default) config dir.
        config::global().write().unwrap().reset_all();

        // Confirm to the user, then restart on close so the fresh process loads
        // the default state (matches the dialog's promise).
        let info = adw::MessageDialog::new(
            Some(&window_for_info),
            Some(&tr("Done")),
            Some(&tr(
                "All application data has been cleared. The app will now restart.",
            )),
        );
        info.add_response("ok", &tr("Restart Now"));
        info.set_default_response(Some("ok"));
        info.set_close_response("ok");
        apply_theme_classes(&info);
        info.connect_response(None, |dlg, _| {
            dlg.close();
            restart_app();
        });
        info.present();
    });
    confirm.present();
}

/// Re-launch the application from scratch (after a full data reset). Uses
/// `exec()` to replace the current process image: the single-instance D-Bus
/// socket is close-on-exec, so its name is released and the fresh process takes
/// over instead of just forwarding `activate` to the dying one.
fn restart_app() {
    use std::os::unix::process::CommandExt;
    let Ok(exe) = std::env::current_exe() else {
        std::process::exit(0);
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    // exec() only returns if it failed; otherwise it never comes back.
    let err = std::process::Command::new(exe).args(args).exec();
    tracing::error!("restart exec failed: {err}");
    std::process::exit(0);
}

fn build_downloads_page(state: &Rc<AppState>) -> gtk::Widget {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Header with an icon "clear history" button.
    let clear = gtk::Button::from_icon_name("edit-clear-history-symbolic");
    clear.add_css_class("flat");
    clear.set_tooltip_text(Some(&tr("Clear History")));
    {
        let state = state.clone();
        clear.connect_clicked(move |_| confirm_clear_all_downloads(&state));
    }
    let header = page_header(&tr("Downloads Manager"), &[clear]);

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&state.downloads_box));

    let empty = status_page(
        "folder-download-symbolic",
        &tr("No Downloads"),
        &tr("Your downloads will appear here"),
    );
    state.downloads_stack.set_vexpand(true);
    state.downloads_stack.add_named(&empty, Some("empty"));
    state.downloads_stack.add_named(&scrolled, Some("list"));
    state.downloads_stack.set_visible_child_name("empty");

    page.append(&header);
    page.append(&state.downloads_stack);
    page.upcast()
}

// =============================================================================
// SCHEDULED (per-row editing)
// =============================================================================

/// Cancel a scheduled download by its persisted id: drop the live (pending) row
/// and its history entry, kill the scheduler task, and remove the store entry.
fn cancel_scheduled_by_id(state: &Rc<AppState>, id: &str) {
    let key = state
        .download_rows
        .borrow()
        .iter()
        .find(|(_, r)| r.sched_id.borrow().as_deref() == Some(id))
        .map(|(k, _)| k.clone());
    if let Some(k) = key {
        if let Some(row) = state.download_rows.borrow_mut().remove(&k) {
            if let Some(d) = row.downloader.borrow().as_ref() {
                d.cancel();
            }
            let fp = row.file_path.borrow().clone();
            if !fp.is_empty() {
                bigtube_core::history::HistoryManager::new(history_path()).remove_entry(&fp);
            }
            remove_list_card(&state.downloads_box, &row.container);
        }
        state.update_downloads_empty();
    }
    download_manager::global().cancel_task(id);
    bigtube_core::scheduled_downloads::ScheduledDownloadStore::new(scheduled_downloads_path())
        .remove(id);
}

/// Reopen the schedule dialog pre-filled for a pending scheduled row's pencil
/// button, then re-arm with the new time/recurrence (cancelling the old first).
#[allow(clippy::too_many_arguments)]
fn open_schedule_editor(
    state: &Rc<AppState>,
    sched_id: &str,
    url: &str,
    title: &str,
    thumbnail: &str,
    uploader: &str,
    format_id: &str,
    ext: &str,
    ts: f64,
    recurrence: &str,
) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let st = state.clone();
    let (sid, url, title, thumb, uploader, fmt, ext) = (
        sched_id.to_string(),
        url.to_string(),
        title.to_string(),
        thumbnail.to_string(),
        uploader.to_string(),
        format_id.to_string(),
        ext.to_string(),
    );
    crate::schedule::show(
        &window,
        Some(ts),
        recurrence,
        Rc::new(move |new_ts: f64, new_rec: String| {
            // Drop the old occurrence, then create the edited one afresh.
            cancel_scheduled_by_id(&st, &sid);
            enqueue_scheduled(
                &st, &url, &title, &thumb, &uploader, &fmt, &ext, new_ts, &new_rec,
            );
        }),
    );
}

// =============================================================================
// CONVERTER
// =============================================================================

const VIDEO_FORMATS: [&str; 6] = ["mp4", "mkv", "webm", "mp3", "m4a", "wav"];
const AUDIO_FORMATS: [&str; 4] = ["mp3", "m4a", "wav", "flac"];

/// True when the source file is audio-only (by extension). Audio inputs never
/// carry subtitles, so the converter hides that toggle for them (`converter_row.py`).
fn is_audio_input(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_lowercase().as_str(),
                "mp3" | "m4a" | "wav" | "flac" | "ogg" | "opus" | "aac" | "wma"
            )
        })
        .unwrap_or(false)
}

/// Output formats offered for a given source file, by media type (`converter_row.py`).
fn convert_formats_for(path: &std::path::Path) -> &'static [&'static str] {
    if is_audio_input(path) {
        &AUDIO_FORMATS
    } else {
        &VIDEO_FORMATS
    }
}

enum ConvMsg {
    /// `(fraction 0..1, speed_x, eta_seconds)`.
    Progress(f64, Option<f64>, Option<f64>),
    Done(Result<String, String>),
}

/// Remove a card from a `ListBox`. A `ListBox` wraps non-row children in an
/// auto-created `ListBoxRow`, so removing the inner card directly can fail —
/// remove the wrapper row when present.
fn remove_list_card(list: &gtk::ListBox, card: &gtk::Box) {
    if let Some(row) = card.ancestor(gtk::ListBoxRow::static_type()) {
        list.remove(&row);
    } else {
        list.remove(card);
    }
}

/// Delete a produced output file, defensively: only an existing **regular file**
/// (never a directory, never an empty path) is removed, and errors are ignored.
/// Centralizes every "delete the file too" action so none can touch a directory.
fn delete_output_file(path: &str) {
    let p = std::path::Path::new(path);
    if !path.is_empty() && p.is_file() {
        let _ = std::fs::remove_file(p);
    }
}

fn build_converter_page(state: &Rc<AppState>) -> gtk::Widget {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Header with "add files" + "clear all" buttons.
    let add = gtk::Button::from_icon_name("list-add-symbolic");
    add.add_css_class("flat");
    add.set_tooltip_text(Some(&tr("Add Files")));
    {
        let state = state.clone();
        add.connect_clicked(move |_| pick_files(&state));
    }
    let clear = gtk::Button::from_icon_name("edit-clear-history-symbolic");
    clear.add_css_class("flat");
    clear.set_tooltip_text(Some(&tr("Clear History")));
    {
        let state = state.clone();
        clear.connect_clicked(move |_| confirm_clear_all_converter(&state));
    }
    let header = page_header(&tr("Converter Manager"), &[add, clear]);

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&state.converter_box));

    // Empty-state acts as the drop zone hint.
    let empty = status_page(
        "view-refresh-symbolic",
        &tr("Media Converter"),
        &tr("Drag and drop files here to convert"),
    );
    state.converter_stack.set_vexpand(true);
    state.converter_stack.add_named(&empty, Some("empty"));
    state.converter_stack.add_named(&scrolled, Some("list"));
    state.converter_stack.set_visible_child_name("empty");

    page.append(&header);
    page.append(&state.converter_stack);

    // Drag & drop of files onto the page.
    let drop = gtk::DropTarget::new(
        gtk::gdk::FileList::static_type(),
        gtk::gdk::DragAction::COPY,
    );
    {
        let state = state.clone();
        drop.connect_drop(move |_, value, _, _| {
            if let Ok(list) = value.get::<gtk::gdk::FileList>() {
                for file in list.files() {
                    if let Some(path) = file.path() {
                        add_converter_file(&state, path);
                    }
                }
                return true;
            }
            false
        });
    }
    page.add_controller(drop);

    page.upcast()
}

fn pick_files(state: &Rc<AppState>) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = gtk::FileDialog::builder()
        .title(tr("Select Media Files"))
        .build();
    let state = state.clone();
    dialog.open_multiple(Some(&window), gtk::gio::Cancellable::NONE, move |res| {
        if let Ok(files) = res {
            for i in 0..files.n_items() {
                if let Some(obj) = files.item(i) {
                    if let Ok(file) = obj.downcast::<gtk::gio::File>() {
                        if let Some(path) = file.path() {
                            add_converter_file(&state, path);
                        }
                    }
                }
            }
        }
    });
}

/// The per-row widgets a conversion updates (bundled to keep arg lists sane).
#[derive(Clone)]
struct ConvUi {
    progress: gtk::ProgressBar,
    status: gtk::Label,
    convert: gtk::Button,
    cancel: gtk::Button,
    folder: gtk::Button,
    play: gtk::Button,
    format: gtk::DropDown,
    meta_chk: gtk::CheckButton,
    subs_chk: gtk::CheckButton,
    out_path: Rc<RefCell<String>>,
}

impl ConvUi {
    /// Lock/unlock the format dropdown and option toggles while a conversion
    /// runs (mirrors `converter_row.py` disabling `combo_format`).
    fn set_inputs_sensitive(&self, on: bool) {
        self.format.set_sensitive(on);
        self.meta_chk.set_sensitive(on);
        self.subs_chk.set_sensitive(on);
    }
}

/// A queued conversion job, awaiting its turn in the single-slot pipeline.
struct PendingConv {
    path: std::path::PathBuf,
    fmt: String,
    add_metadata: bool,
    add_subtitles: bool,
    ui: ConvUi,
    cancel_flag: Arc<AtomicBool>,
}

/// Queue a conversion and try to start it. The row shows "Queued" until its
/// turn comes (`converter_controller.py::_on_row_request_start`).
fn enqueue_conversion(state: &Rc<AppState>, job: PendingConv) {
    job.ui.convert.set_visible(false);
    job.ui.cancel.set_visible(true);
    job.ui.folder.set_visible(false);
    job.ui.play.set_visible(false);
    job.ui.set_inputs_sensitive(false);
    job.ui.set_progress_class("");
    job.ui.status.set_text(&tr("Queued"));
    state.conv_queue.borrow_mut().push_back(job);
    pump_conversion(state);
}

/// Start the next queued conversion if none is running. Jobs cancelled while
/// still queued are dropped (their row removed) and the next is tried.
fn pump_conversion(state: &Rc<AppState>) {
    if state.conv_active.get() {
        return;
    }
    let job = match state.conv_queue.borrow_mut().pop_front() {
        Some(j) => j,
        None => return,
    };
    if job.cancel_flag.load(Ordering::SeqCst) {
        // Cancelled while still queued: reset the row (keep it for re-convert).
        job.ui.reset_ready();
        pump_conversion(state);
        return;
    }
    state.conv_active.set(true);
    run_conversion(
        job.path,
        job.fmt,
        job.add_metadata,
        job.add_subtitles,
        job.ui,
        job.cancel_flag,
        state.clone(),
    );
}

impl ConvUi {
    fn set_progress_class(&self, class: &str) {
        for c in ["success", "warning", "error"] {
            self.progress.remove_css_class(c);
        }
        if !class.is_empty() {
            self.progress.add_css_class(class);
        }
    }

    /// Reset the row to its initial idle look (after a cancel), so another
    /// format can be chosen and converted again — the row is NOT removed.
    fn reset_ready(&self) {
        self.cancel.set_visible(false);
        self.convert.set_visible(true);
        self.set_inputs_sensitive(true);
        self.set_progress_class("");
        self.progress.set_fraction(0.0);
        self.status.set_text(&tr("Ready"));
    }
}

fn add_converter_file(state: &Rc<AppState>, path: std::path::PathBuf) {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());

    let container = gtk::Box::new(gtk::Orientation::Vertical, 4);
    container.add_css_class("card");
    container.set_margin_top(6);
    container.set_margin_bottom(6);
    container.set_margin_start(8);
    container.set_margin_end(8);
    let pad = gtk::Box::new(gtk::Orientation::Vertical, 4);
    pad.set_margin_top(8);
    pad.set_margin_bottom(8);
    pad.set_margin_start(12);
    pad.set_margin_end(12);

    let formats = convert_formats_for(&path);
    let is_video = !is_audio_input(&path);
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name_lbl = gtk::Label::new(Some(&name));
    name_lbl.set_xalign(0.0);
    name_lbl.set_hexpand(true);
    name_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
    name_lbl.add_css_class("heading");
    let format = gtk::DropDown::from_strings(formats);
    let convert = gtk::Button::from_icon_name("system-run-symbolic");
    convert.add_css_class("flat");
    convert.set_tooltip_text(Some(&tr("Convert")));
    let cancel = gtk::Button::from_icon_name("process-stop-symbolic");
    cancel.add_css_class("flat");
    cancel.add_css_class("destructive-action");
    cancel.set_tooltip_text(Some(&tr("Cancel")));
    cancel.set_visible(false);
    let folder = gtk::Button::from_icon_name("folder-open-symbolic");
    folder.add_css_class("flat");
    folder.set_tooltip_text(Some(&tr("Open Folder")));
    folder.set_visible(false);
    let play = gtk::Button::from_icon_name("media-playback-start-symbolic");
    play.add_css_class("flat");
    play.set_tooltip_text(Some(&tr("Play Video")));
    play.set_visible(false);
    let remove = gtk::Button::from_icon_name("user-trash-symbolic");
    remove.add_css_class("flat");
    remove.set_tooltip_text(Some(&tr("Remove from list")));
    // Top row: name + format input + convert/cancel (next to the dropdown) +
    // delete. Only play/folder live in the bottom row next to the status.
    header.append(&name_lbl);
    header.append(&format);
    header.append(&convert);
    header.append(&cancel);
    header.append(&remove);

    // Conversion options (mirrors `converter_row.py`): both default on; the
    // subtitle toggle only applies to video inputs.
    let opts = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let meta_chk = gtk::CheckButton::with_label(&tr("Add Metadata"));
    meta_chk.set_active(true);
    let subs_chk = gtk::CheckButton::with_label(&tr("Add Subtitles"));
    subs_chk.set_active(true);
    subs_chk.set_visible(is_video);
    opts.append(&meta_chk);
    opts.append(&subs_chk);

    let status = gtk::Label::new(Some(tr("Ready").as_str()));
    status.set_xalign(0.0);
    status.set_hexpand(true);
    status.set_ellipsize(gtk::pango::EllipsizeMode::End);
    status.add_css_class("dim-label");
    status.add_css_class("caption");
    let progress = gtk::ProgressBar::new();
    progress.set_fraction(0.0);

    // Bottom row: status on the left, play/folder (post-conversion) on the right.
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    actions.set_halign(gtk::Align::End);
    actions.append(&folder);
    actions.append(&play);
    footer.append(&status);
    footer.append(&actions);

    pad.append(&header);
    pad.append(&opts);
    pad.append(&progress);
    pad.append(&footer);
    container.append(&pad);
    state.converter_box.append(&container);
    state.update_converter_empty();
    state.stack.set_visible_child_name("converter");

    let ui = ConvUi {
        progress,
        status,
        convert: convert.clone(),
        cancel: cancel.clone(),
        folder: folder.clone(),
        play: play.clone(),
        format: format.clone(),
        meta_chk: meta_chk.clone(),
        subs_chk: subs_chk.clone(),
        out_path: Rc::new(RefCell::new(String::new())),
    };

    // Remove this row from the list.
    {
        let state = state.clone();
        let container = container.clone();
        let source = path.to_string_lossy().to_string();
        let out_path = ui.out_path.clone();
        remove.connect_clicked(move |_| {
            confirm_delete_converter(&state, &container, &source, None, &out_path.borrow());
        });
    }
    // Open the converted file's folder.
    {
        let state = state.clone();
        let out_path = ui.out_path.clone();
        folder.connect_clicked(move |_| open_containing_folder(&state, &out_path.borrow()));
    }
    // Play the converted file, seeding a cyclic queue of all converted files.
    {
        let state = state.clone();
        let out_path = ui.out_path.clone();
        play.connect_clicked(move |_| play_converter_at(&state, &out_path.borrow()));
    }
    // Highlight this row while its output is the one playing.
    wire_play_highlight(state, &container, ui.out_path.clone());

    // Convert (with a cancel flag the cancel button flips).
    {
        let ui = ui.clone();
        let format = format.clone();
        let cancel = cancel.clone();
        let state = state.clone();
        convert.connect_clicked(move |btn| {
            let fmt = formats
                .get(format.selected() as usize)
                .copied()
                .unwrap_or("mp4")
                .to_string();
            // Read the per-row option toggles; subtitles never apply to audio.
            let add_metadata = ui.meta_chk.is_active();
            let add_subtitles = is_video && ui.subs_chk.is_active();
            let flag = Arc::new(AtomicBool::new(false));
            {
                let flag = flag.clone();
                cancel.connect_clicked(move |_| flag.store(true, Ordering::SeqCst));
            }
            let _ = btn;
            enqueue_conversion(
                &state,
                PendingConv {
                    path: path.clone(),
                    fmt,
                    add_metadata,
                    add_subtitles,
                    ui: ui.clone(),
                    cancel_flag: flag,
                },
            );
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn run_conversion(
    path: std::path::PathBuf,
    fmt: String,
    add_metadata: bool,
    add_subtitles: bool,
    ui: ConvUi,
    cancel_flag: Arc<AtomicBool>,
    state: Rc<AppState>,
) {
    use bigtube_core::converter::{convert_media, ConvertProgressFn};

    let (tx, rx) = async_channel::unbounded::<ConvMsg>();
    let tx_progress = tx.clone();
    let cb: ConvertProgressFn = Arc::new(move |p, speed, eta| {
        let _ = tx_progress.send_blocking(ConvMsg::Progress(p, speed, eta));
    });

    let input = path.to_string_lossy().to_string();
    let source = input.clone();
    let fmt_hist = fmt.clone();
    let flag = cancel_flag.clone();
    std::thread::spawn(move || {
        let result = convert_media(
            &input,
            &fmt,
            Some(&cb),
            add_metadata,
            add_subtitles,
            Some(&flag),
        )
        .map_err(|e| e.to_string());
        let _ = tx.send_blocking(ConvMsg::Done(result));
    });

    glib::spawn_future_local(async move {
        while let Ok(msg) = rx.recv().await {
            match msg {
                ConvMsg::Progress(p, speed, eta) => {
                    ui.progress.set_fraction(p);
                    let mut parts: Vec<String> = vec![format!("{:.0}%", p * 100.0)];
                    if let Some(s) = speed.filter(|s| *s > 0.0) {
                        parts.push(format!("{s:.1}x"));
                    }
                    if let Some(e) = eta.filter(|e| *e > 0.0) {
                        parts.push(format!("ETA {}", fmt_eta(e)));
                    }
                    ui.status
                        .set_text(&format!("{}: {}", tr("Converting"), parts.join(" · ")));
                }
                ConvMsg::Done(Ok(out)) => {
                    ui.progress.set_fraction(1.0);
                    ui.set_progress_class("success");
                    ui.status.set_text(&tr("Success!"));
                    ui.cancel.set_visible(false);
                    ui.convert.set_visible(true);
                    ui.set_inputs_sensitive(true);
                    ui.out_path.replace(out.clone());
                    ui.folder.set_visible(true);
                    ui.play.set_visible(true);
                    // Probe the converted file (codecs + real size) and show it as
                    // the status, replacing the generic "Success!".
                    {
                        let (itx, irx) = async_channel::bounded::<String>(1);
                        let outp = out.clone();
                        std::thread::spawn(move || {
                            let s = bigtube_core::converter::probe_media_summary(&outp);
                            let _ = itx.send_blocking(media_summary_text(&s, &outp));
                        });
                        let status_lbl = ui.status.clone();
                        glib::spawn_future_local(async move {
                            if let Ok(text) = irx.recv().await {
                                if !text.is_empty() {
                                    status_lbl.set_text(&text);
                                }
                            }
                        });
                    }
                    if config::global()
                        .read()
                        .unwrap()
                        .get_bool("save_converter_history")
                    {
                        bigtube_core::converter_history::ConverterHistoryManager::new(
                            bigtube_core::paths::config_dir().join("converter_history.json"),
                        )
                        .add_entry(&source, &out, &fmt_hist);
                    }
                }
                ConvMsg::Done(Err(e)) => {
                    if cancel_flag.load(Ordering::SeqCst) {
                        // Cancelled by the user: the core removed the partial
                        // output; keep the row and reset it so another format can
                        // be picked and converted again.
                        ui.reset_ready();
                    } else {
                        ui.cancel.set_visible(false);
                        ui.convert.set_visible(true);
                        ui.set_inputs_sensitive(true);
                        ui.set_progress_class("error");
                        ui.status.set_text(&format!("{}: {e}", tr("Error:")));
                    }
                }
            }
        }
        // This conversion finished (ok, error, or cancel): free the slot and
        // let the next queued job start.
        state.conv_active.set(false);
        pump_conversion(&state);
    });
}

// =============================================================================
// CLIPBOARD MONITOR
// =============================================================================

fn start_clipboard_monitor(state: &Rc<AppState>) {
    use bigtube_core::validators::is_valid_url;

    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let clipboard = window.clipboard();
    let win = window.clone();
    let state = state.clone();
    let last = Rc::new(RefCell::new(String::new()));
    // True while the prompt is open, so we don't stack dialogs each tick.
    let prompting = Rc::new(Cell::new(false));

    glib::timeout_add_seconds_local(1, move || {
        // Respect the live setting; skip polling while disabled.
        if !config::global()
            .read()
            .unwrap()
            .get_bool("monitor_clipboard")
        {
            return glib::ControlFlow::Continue;
        }
        if prompting.get() {
            return glib::ControlFlow::Continue;
        }
        let state = state.clone();
        let last = last.clone();
        let win = win.clone();
        let prompting = prompting.clone();
        clipboard.read_text_async(gtk::gio::Cancellable::NONE, move |res| {
            if let Ok(Some(text)) = res {
                let text = text.to_string();
                if text != *last.borrow() && is_valid_url(&text) {
                    last.replace(text.clone());
                    prompt_paste_link(&state, &win, text, prompting);
                }
            }
        });
        glib::ControlFlow::Continue
    });
}

/// Ask whether to paste a clipboard link into the search and run it.
fn prompt_paste_link(
    state: &Rc<AppState>,
    window: &adw::ApplicationWindow,
    url: String,
    prompting: Rc<Cell<bool>>,
) {
    prompting.set(true);
    let dialog = adw::MessageDialog::new(
        Some(window),
        Some(&tr("Link detected")),
        Some(&format!(
            "{}\n\n{url}",
            tr("Paste this link in the search and download it?")
        )),
    );
    dialog.add_response("no", &tr("Not Now"));
    dialog.add_response("yes", &tr("Paste & Search"));
    dialog.set_response_appearance("yes", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("yes"));
    dialog.set_close_response("no");
    apply_theme_classes(&dialog);

    let state = state.clone();
    dialog.connect_response(None, move |dlg, resp| {
        dlg.close();
        prompting.set(false);
        if resp == "yes" {
            if let Some(f) = state.paste_and_search.borrow().as_ref() {
                f(url.clone());
            }
        }
    });
    dialog.present();
}

/// Path to the persisted search-history file.
fn search_history_path() -> std::path::PathBuf {
    bigtube_core::paths::config_dir().join("search_history.json")
}

fn run_search(state: &Rc<AppState>, query: String, source: String) {
    let query = query.trim().to_string();
    if query.is_empty() {
        return;
    }
    state.search_store.remove_all();

    // Persist the query to search history (honouring the setting).
    let save = config::global()
        .read()
        .unwrap()
        .get_bool("save_search_history");
    bigtube_core::search_history::SearchHistory::new(search_history_path()).add(&query, save);

    // Show the spinner page while the search runs.
    state.search_stack.set_visible_child_name("loading");

    // A direct link (incl. a playlist URL) is expanded into its videos by the
    // core, so suppress any playlist wrapper rows — a pasted playlist lists its
    // videos inline, never a "playlist menu" row. Mirror the core's own
    // direct-link routing (source "url" OR a pasted http/www string).
    let is_url_search = source == "url" || query.starts_with("http") || query.starts_with("www");
    let (tx, rx) =
        async_channel::bounded::<Result<Vec<bigtube_core::search::SearchResult>, String>>(1);
    std::thread::spawn(move || {
        let result = SearchEngine::new()
            .map_err(|e| e.to_string())
            .and_then(|eng| eng.search(&query, &source).map_err(|e| e.to_string()));
        let _ = tx.send_blocking(result);
    });

    let state = state.clone();
    glib::spawn_future_local(async move {
        if let Ok(result) = rx.recv().await {
            match result {
                Ok(list) => {
                    if list.is_empty() {
                        state.toast(&tr("No results found!"));
                    }
                    let mode = state.select_mode.get();
                    for r in &list {
                        // Direct-link results are already expanded videos; drop any
                        // stray playlist wrapper so no "open playlist" row appears.
                        if is_url_search && r.is_playlist {
                            continue;
                        }
                        let obj = VideoObject::from_result(r);
                        obj.set_selection_mode(mode);
                        let st = state.clone();
                        obj.connect_is_selected_notify(move |_| st.refresh_selection_count());
                        state.search_store.append(&obj);
                    }
                    state.update_search_empty();
                    state.refresh_selection_count();
                }
                Err(e) => {
                    state.update_search_empty();
                    // The core returns a known English message; translate it via
                    // the catalog (tr() returns the input unchanged if unknown).
                    state.toast(&tr(&e));
                }
            }
        }
    });
}

/// Play `clicked`, seeding the player queue from the playable items of `store`
/// (so prev/next walk the list). Falls back to a one-item queue if `clicked`
/// isn't in the store (e.g. invoked from the playlist dialog).
fn play_from_store(state: &Rc<AppState>, store: &gio::ListStore, clicked: &VideoObject) {
    let Some(player) = state.player.borrow().clone() else {
        return;
    };
    let mut items = Vec::new();
    let mut start = None;
    for i in 0..store.n_items() {
        let Some(obj) = store.item(i).and_then(|o| o.downcast::<VideoObject>().ok()) else {
            continue;
        };
        if obj.is_playlist() {
            continue;
        }
        if obj.url() == clicked.url() {
            start = Some(items.len());
        }
        items.push(crate::player::QueueItem {
            url: obj.url(),
            title: obj.title(),
            artist: obj.uploader(),
            thumbnail: obj.thumbnail(),
            is_local: false,
            is_video: obj.is_video(),
        });
    }
    match start {
        Some(s) => player.play_queue(items, s),
        None => player.play(
            &clicked.url(),
            &clicked.title(),
            &clicked.uploader(),
            &clicked.thumbnail(),
        ),
    }
}

/// Fetch metadata for `item`, then present the format-selection dialog.
fn on_download_clicked(state: &Rc<AppState>, item: &VideoObject) {
    let url = item.url();
    if url.is_empty() {
        state.toast(&tr("Invalid URL format"));
        return;
    }
    let title = item.title();
    let thumb = item.thumbnail();
    let uploader = item.uploader();
    let audio_only = !item.is_video();
    state.toast(&tr("Processing..."));
    state.busy_begin();

    let (tx, rx) = async_channel::bounded::<
        std::result::Result<bigtube_core::downloader::ParsedInfo, StatusCode>,
    >(1);
    let url_thread = url.clone();
    std::thread::spawn(move || {
        let info = match VideoDownloader::new() {
            Ok(d) => d.fetch_video_info_checked(&url_thread),
            Err(_) => Err(StatusCode::UnknownError),
        };
        let _ = tx.send_blocking(info);
    });

    let state = state.clone();
    glib::spawn_future_local(async move {
        let received = rx.recv().await;
        state.busy_end();
        let info = match received {
            Ok(Ok(info)) => info,
            Ok(Err(StatusCode::BotBlocked)) => {
                state.notify_bot_block();
                return;
            }
            _ => {
                state.toast(&tr("No formats found"));
                return;
            }
        };
        run_download_flow(&state, info, url, title, thumb, uploader, audio_only);
    });
}

/// Drive the post-fetch flow: a single format dialog. YouTube Music
/// (`audio_only`) shows just the Audio column; a normal source shows Video and
/// Audio side by side (two columns) in one screen — no Video/Audio prompt.
fn run_download_flow(
    state: &Rc<AppState>,
    info: bigtube_core::downloader::ParsedInfo,
    url: String,
    title: String,
    thumb: String,
    uploader: String,
    audio_only: bool,
) {
    let info = Rc::new(info);
    show_format_dialog(state, info, url, title, thumb, uploader, audio_only);
}

/// Present the format-selection dialog for already-fetched `info`, wiring its
/// Download and Schedule buttons. Closing without a pick just closes.
#[allow(clippy::too_many_arguments)]
fn show_format_dialog(
    state: &Rc<AppState>,
    info: Rc<bigtube_core::downloader::ParsedInfo>,
    url: String,
    title: String,
    thumb: String,
    uploader: String,
    audio_only: bool,
) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let on_pick: dialog::PickFn = {
        let st = state.clone();
        let url = url.clone();
        let title = title.clone();
        let thumb = thumb.clone();
        let uploader = uploader.clone();
        Rc::new(move |format_id: String, ext: String| {
            enqueue_download_checked(&st, &url, &title, &thumb, &uploader, &format_id, &ext);
        })
    };
    let on_schedule: dialog::ScheduleFn = {
        let st = state.clone();
        let url = url.clone();
        let title = title.clone();
        let thumb = thumb.clone();
        let uploader = uploader.clone();
        Rc::new(move |format_id: String, ext: String| {
            let Some(win) = st.window.borrow().clone() else {
                return;
            };
            let st = st.clone();
            let url = url.clone();
            let title = title.clone();
            let thumb = thumb.clone();
            let uploader = uploader.clone();
            crate::schedule::show(
                &win,
                None,
                "once",
                Rc::new(move |ts: f64, recurrence: String| {
                    enqueue_scheduled(
                        &st,
                        &url,
                        &title,
                        &thumb,
                        &uploader,
                        &format_id,
                        &ext,
                        ts,
                        &recurrence,
                    );
                }),
            );
        })
    };
    let on_close: dialog::CloseFn = Rc::new(|| {});
    dialog::show(&window, &info, audio_only, on_pick, on_schedule, on_close);
}

/// File extension that pairs with a quality selector (audio/MKV/MP4).
fn quality_ext(q: bigtube_core::enums::VideoQuality) -> &'static str {
    use bigtube_core::enums::VideoQuality::*;
    match q {
        AudioMp3 => "mp3",
        AudioM4a => "m4a",
        AudioOpus => "opus",
        AudioFlac => "flac",
        AudioWav => "wav",
        AudioAac => "aac",
        Best => "mkv",
        _ => "mp4",
    }
}

/// Batch-download every item with a SINGLE quality dialog (instead of one
/// per-item format dialog). The chosen quality's yt-dlp selector is applied to
/// all items.
fn download_all(state: &Rc<AppState>, items: Vec<VideoObject>) {
    let items: Vec<VideoObject> = items
        .into_iter()
        .filter(|o| !o.is_playlist() && !o.url().is_empty())
        .collect();
    if items.is_empty() {
        state.toast(&tr("No results found!"));
        return;
    }
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    // Group a playlist / multi-selection under a folder named after the first
    // item's artist (uploader), so batches don't scatter across the download dir.
    let artist = bigtube_core::validators::sanitize_filename(&items[0].uploader(), 100);
    let subfolder = if artist.is_empty() {
        None
    } else {
        Some(artist)
    };

    let st = state.clone();
    show_quality_dialog(&window, move |q| {
        let sel = q.as_value().to_string();
        let ext = quality_ext(q);
        for o in &items {
            enqueue_common(
                &st,
                &o.url(),
                &o.title(),
                &o.thumbnail(),
                &o.uploader(),
                &sel,
                ext,
                None,
                false,
                subfolder.as_deref(),
                None,
                "once",
            );
        }
        let msg = match &subfolder {
            Some(s) => format!("{} → {s}/", tr("Added to downloads")),
            None => tr("Added to downloads"),
        };
        st.toast(&msg);
    });
}

/// Like [`download_all`] but routes the batch through the schedule dialog: ONE
/// quality pick + ONE time/recurrence for every item (playlist or selection).
fn schedule_all(state: &Rc<AppState>, items: Vec<VideoObject>) {
    let items: Vec<VideoObject> = items
        .into_iter()
        .filter(|o| !o.is_playlist() && !o.url().is_empty())
        .collect();
    if items.is_empty() {
        state.toast(&tr("No results found!"));
        return;
    }
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let artist = bigtube_core::validators::sanitize_filename(&items[0].uploader(), 100);
    let subfolder = if artist.is_empty() {
        None
    } else {
        Some(artist)
    };

    let st = state.clone();
    show_quality_dialog(&window, move |q| {
        let sel = q.as_value().to_string();
        let ext = quality_ext(q);
        let Some(win) = st.window.borrow().clone() else {
            return;
        };
        // One time + recurrence applied to the whole batch.
        let st2 = st.clone();
        let items2 = items.clone();
        let subfolder2 = subfolder.clone();
        crate::schedule::show(
            &win,
            None,
            "once",
            Rc::new(move |ts: f64, recurrence: String| {
                for o in &items2 {
                    enqueue_common(
                        &st2,
                        &o.url(),
                        &o.title(),
                        &o.thumbnail(),
                        &o.uploader(),
                        &sel,
                        ext,
                        Some(ts),
                        false,
                        subfolder2.as_deref(),
                        None,
                        &recurrence,
                    );
                }
                st2.toast(&tr("Scheduled!"));
            }),
        );
    });
}

/// True for the audio-only batch qualities (MP3 / M4A).
fn is_audio_quality(q: bigtube_core::enums::VideoQuality) -> bool {
    use bigtube_core::enums::VideoQuality::*;
    matches!(
        q,
        AudioMp3 | AudioM4a | AudioOpus | AudioFlac | AudioWav | AudioAac
    )
}

/// A single quality picker for batch downloads: a Video/Audio radio chooses the
/// kind, and the dropdown lists the qualities for that kind. Defaults to the
/// configured preferred quality.
fn show_quality_dialog(
    parent: &adw::ApplicationWindow,
    on_pick: impl Fn(bigtube_core::enums::VideoQuality) + 'static,
) {
    use bigtube_core::enums::VideoQuality;
    // Split the (non-"Ask") options into video and audio sets.
    let video: Vec<(&str, VideoQuality)> = QUALITY_OPTIONS
        .iter()
        .copied()
        .filter(|(_, q)| !matches!(q, VideoQuality::Ask) && !is_audio_quality(*q))
        .collect();
    let audio: Vec<(&str, VideoQuality)> = QUALITY_OPTIONS
        .iter()
        .copied()
        .filter(|(_, q)| is_audio_quality(*q))
        .collect();
    let default_quality = config::global()
        .read()
        .unwrap()
        .get_string("default_quality");
    let start_audio = audio.iter().any(|(_, q)| q.as_value() == default_quality);

    let win = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(tr("Select Quality"))
        .default_width(400)
        // Size to content so there's no dead space below the dropdown.
        .resizable(false)
        .build();
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    let dl_btn = gtk::Button::with_label(&tr("Download"));
    dl_btn.add_css_class("suggested-action");
    dl_btn.set_focus_on_click(false);
    header.pack_end(&dl_btn);
    toolbar.add_top_bar(&header);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);

    // Video/Audio radio.
    let kind = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    kind.set_halign(gtk::Align::Center);
    let video_radio = gtk::CheckButton::with_label(&tr("Video"));
    let audio_radio = gtk::CheckButton::with_label(&tr("Audio"));
    audio_radio.set_group(Some(&video_radio));
    video_radio.set_active(!start_audio);
    audio_radio.set_active(start_audio);
    kind.append(&video_radio);
    kind.append(&audio_radio);
    content.append(&kind);

    let group = adw::PreferencesGroup::new();
    let combo = combo_row(&tr("Preferred Quality"), &[] as &[&str]);
    group.add(&combo);
    content.append(&group);

    // The qualities currently shown in the dropdown (kept in sync with the radio).
    let current: Rc<RefCell<Vec<VideoQuality>>> = Rc::new(RefCell::new(Vec::new()));

    // Repopulate the dropdown for the selected kind, preselecting the default.
    let populate: Rc<dyn Fn(bool)> = {
        let combo = combo.clone();
        let current = current.clone();
        let video = video.clone();
        let audio = audio.clone();
        let default_quality = default_quality.clone();
        Rc::new(move |as_audio: bool| {
            let list = if as_audio { &audio } else { &video };
            let labels: Vec<String> = list.iter().map(|(l, _)| tr(l)).collect();
            let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
            combo.set_model(Some(&gtk::StringList::new(&refs)));
            let sel = list
                .iter()
                .position(|(_, q)| q.as_value() == default_quality)
                .unwrap_or(0);
            combo.set_selected(sel as u32);
            *current.borrow_mut() = list.iter().map(|(_, q)| *q).collect();
        })
    };
    populate(start_audio);
    {
        let populate = populate.clone();
        audio_radio.connect_toggled(move |b| populate(b.is_active()));
    }

    toolbar.set_content(Some(&content));
    win.set_content(Some(&toolbar));
    apply_theme_classes(&win);
    win.present();

    let on_pick = Rc::new(on_pick);
    dl_btn.connect_clicked(move |_| {
        if let Some(q) = current.borrow().get(combo.selected() as usize) {
            on_pick(*q);
        }
        win.close();
    });
}

/// Output file path the downloader will use
/// (`{download}/[subfolder/]{safe_title}.{ext}`).
fn output_path(title: &str, format_id: &str, ext: &str, subfolder: Option<&str>) -> String {
    let dir = config::global().read().unwrap().get_string("download_path");
    let mut safe = bigtube_core::validators::sanitize_filename(title, 200);
    if safe.is_empty() {
        safe = format!("video_{format_id}");
    }
    match subfolder {
        Some(sub) if !sub.is_empty() => format!("{dir}/{sub}/{safe}.{ext}"),
        _ => format!("{dir}/{safe}.{ext}"),
    }
}

#[allow(clippy::too_many_arguments)]
fn enqueue_download(
    state: &Rc<AppState>,
    url: &str,
    title: &str,
    thumbnail: &str,
    uploader: &str,
    format_id: &str,
    ext: &str,
) {
    enqueue_common(
        state, url, title, thumbnail, uploader, format_id, ext, None, false, None, None, "once",
    );
}

/// Like [`enqueue_download`] but first checks whether the target file already
/// exists and, if so, asks the user to Overwrite / Keep both / Cancel.
#[allow(clippy::too_many_arguments)]
fn enqueue_download_checked(
    state: &Rc<AppState>,
    url: &str,
    title: &str,
    thumbnail: &str,
    uploader: &str,
    format_id: &str,
    ext: &str,
) {
    let path = output_path(title, format_id, ext, None);
    if !std::path::Path::new(&path).exists() {
        enqueue_download(state, url, title, thumbnail, uploader, format_id, ext);
        return;
    }
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = adw::MessageDialog::new(
        Some(&window),
        Some(&tr("File already exists")),
        Some(&format!(
            "{}\n\n{}",
            tr("A file with this name is already in the download folder."),
            path
        )),
    );
    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("keep", &tr("Keep Both"));
    dialog.add_response("overwrite", &tr("Overwrite"));
    dialog.set_response_appearance("overwrite", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("keep"));
    dialog.set_close_response("cancel");
    apply_theme_classes(&dialog);

    // Own the strings for the response closure.
    let (state, url, title, thumbnail, uploader, format_id, ext) = (
        state.clone(),
        url.to_string(),
        title.to_string(),
        thumbnail.to_string(),
        uploader.to_string(),
        format_id.to_string(),
        ext.to_string(),
    );
    dialog.connect_response(None, move |dlg, resp| {
        match resp {
            "overwrite" => enqueue_common(
                &state, &url, &title, &thumbnail, &uploader, &format_id, &ext, None, true, None,
                None, "once",
            ),
            "keep" => {
                let t = unique_title(&title, &format_id, &ext);
                enqueue_download(&state, &url, &t, &thumbnail, &uploader, &format_id, &ext);
            }
            _ => {}
        }
        dlg.close();
    });
    dialog.present();
}

/// A title whose `output_path` doesn't collide, appending " (n)" as needed.
fn unique_title(title: &str, format_id: &str, ext: &str) -> String {
    if !std::path::Path::new(&output_path(title, format_id, ext, None)).exists() {
        return title.to_string();
    }
    for n in 1..1000 {
        let candidate = format!("{title} ({n})");
        if !std::path::Path::new(&output_path(&candidate, format_id, ext, None)).exists() {
            return candidate;
        }
    }
    format!("{title} ({})", glib::real_time())
}

/// Format a unix timestamp as a local "DD/MM/YYYY HH:MM" string for the
/// "Scheduled for:" message.
fn format_schedule_ts(ts: f64) -> String {
    glib::DateTime::from_unix_local(ts as i64)
        .ok()
        .and_then(|dt| dt.format("%d/%m/%Y %H:%M").ok())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// Schedule a download for the Unix timestamp `ts` (seconds), with an optional
/// recurrence ("once" / "daily" / "weekly" / "monthly").
#[allow(clippy::too_many_arguments)]
fn enqueue_scheduled(
    state: &Rc<AppState>,
    url: &str,
    title: &str,
    thumbnail: &str,
    uploader: &str,
    format_id: &str,
    ext: &str,
    ts: f64,
    recurrence: &str,
) {
    enqueue_common(
        state,
        url,
        title,
        thumbnail,
        uploader,
        format_id,
        ext,
        Some(ts),
        false,
        None,
        None,
        recurrence,
    );
}

/// Human label for a recurrence key (for the scheduled row's status line).
fn recurrence_label(recurrence: &str) -> Option<String> {
    match recurrence {
        "daily" => Some(tr("Daily")),
        "weekly" => Some(tr("Weekly")),
        "monthly" => Some(tr("Monthly")),
        _ => None,
    }
}

/// Next future occurrence of a recurring schedule: advance `base_ts` by the
/// recurrence interval (calendar-aware, so DST and month lengths are honoured)
/// until it lands strictly after `now`. None for "once"/unknown keys.
fn next_occurrence(base_ts: f64, recurrence: &str, now: f64) -> Option<f64> {
    let mut dt = glib::DateTime::from_unix_local(base_ts as i64).ok()?;
    loop {
        dt = match recurrence {
            "daily" => dt.add_days(1).ok()?,
            "weekly" => dt.add_weeks(1).ok()?,
            "monthly" => dt.add_months(1).ok()?,
            _ => return None,
        };
        if dt.to_unix() as f64 > now {
            return Some(dt.to_unix() as f64);
        }
    }
}

/// Pretty codec name for the resolved-plan summary line.
fn codec_pretty(c: &str) -> String {
    let l = c.to_lowercase();
    if l.contains("avc") || l.contains("h264") {
        "H.264".into()
    } else if l.contains("av01") || l.contains("av1") {
        "AV1".into()
    } else if l.contains("vp9") || l.contains("vp09") {
        "VP9".into()
    } else if l.contains("vp8") {
        "VP8".into()
    } else if l.contains("mp4a") || l.contains("aac") {
        "AAC".into()
    } else if l.contains("opus") {
        "Opus".into()
    } else if l.contains("mp3") {
        "MP3".into()
    } else if l.contains("ac-3") || l.contains("ac3") {
        "AC3".into()
    } else if l.is_empty() {
        String::new()
    } else {
        c.split('.').next().unwrap_or(c).to_uppercase()
    }
}

/// One-line summary of a resolved plan, e.g. "H.264 + AAC · 1080p60 · 275.0 MiB".
fn plan_summary(p: &bigtube_core::downloader::ResolvedPlan) -> String {
    let mut parts: Vec<String> = Vec::new();
    let (v, a) = (codec_pretty(&p.vcodec), codec_pretty(&p.acodec));
    let codecs = match (v.is_empty(), a.is_empty()) {
        (false, false) => format!("{v} + {a}"),
        (false, true) => v,
        (true, false) => a,
        _ => String::new(),
    };
    if !codecs.is_empty() {
        parts.push(codecs);
    }
    if p.height > 0 {
        parts.push(if p.fps > 30 {
            format!("{}p{}", p.height, p.fps)
        } else {
            format!("{}p", p.height)
        });
    }
    if p.size_mb > 0.0 {
        // Pre-download size from yt-dlp is an estimate (often overshoots); the
        // live progress line shows the real bytes once the transfer starts.
        parts.push(if p.size_mb >= 1024.0 {
            format!("~{:.2} GiB", p.size_mb / 1024.0)
        } else {
            format!("~{:.0} MiB", p.size_mb)
        });
    }
    parts.join(" · ")
}

/// Should we probe the real plan before downloading? Only for immediate video
/// downloads — scheduled tasks may run hours later (concrete ids could go
/// stale), and audio-convert picks re-encode so a probed size wouldn't match.
fn should_probe_plan(ext: &str, schedule_ts: Option<f64>) -> bool {
    schedule_ts.is_none() && matches!(ext, "mp4" | "mkv" | "webm")
}

#[allow(clippy::too_many_arguments)]
fn enqueue_common(
    state: &Rc<AppState>,
    url: &str,
    title: &str,
    thumbnail: &str,
    uploader: &str,
    format_id: &str,
    ext: &str,
    schedule_ts: Option<f64>,
    force_overwrite: bool,
    subfolder: Option<&str>,
    restore_id: Option<String>,
    recurrence: &str,
) {
    let key = next_key();
    let file_path = output_path(title, format_id, ext, subfolder);

    // Restoring a persisted schedule: the entry is already in history and in the
    // scheduled store, so don't re-add either.
    let restoring = restore_id.is_some();
    // Stable id shared by the scheduler task and the persisted store entry, so we
    // can remove the right one when the download starts.
    let sched_id = restore_id.unwrap_or_else(|| key.clone());

    // Record a pending history entry up front (so it survives a crash mid-download).
    let save_history = config::global().read().unwrap().get_bool("save_history");
    if save_history && !restoring {
        let video_info = serde_json::json!({
            "title": title, "url": url, "webpage_url": url,
            "thumbnail": thumbnail, "uploader": uploader,
            "scheduled_time": schedule_ts,
        });
        let format_data = serde_json::json!({ "id": format_id, "ext": ext });
        bigtube_core::history::HistoryManager::new(history_path()).add_entry(
            &video_info,
            &format_data,
            &file_path,
        );
    }

    let tx = state.ui_tx.clone();
    let k = key.clone();
    let fp_cb = file_path.clone();
    let cb: ProgressFn = Arc::new(move |p: Progress| {
        // Persist terminal states to history.
        if save_history && (p.status == StatusCode::Completed || p.status.is_error()) {
            use bigtube_core::enums::DownloadStatus;
            let st = if p.status == StatusCode::Completed {
                DownloadStatus::Completed
            } else {
                DownloadStatus::Error
            };
            bigtube_core::history::HistoryManager::new(history_path()).update_status(
                &fp_cb,
                st,
                Some(if p.status == StatusCode::Completed {
                    1.0
                } else {
                    0.0
                }),
            );
        }
        let _ = tx.send_blocking(UiMsg::Progress {
            key: k.clone(),
            percent: p.percent.clone(),
            status: p.status,
            detail: p.detail.clone(),
        });
    });

    let row = DownloadRow::new(title, &file_path, uploader);
    wire_row_footer(state, &row);
    // Store the progress callback so the row's pause/resume can re-run it.
    row.progress_fn.replace(Some(cb.clone()));
    // For a scheduled download, show "Scheduled for: <date time>" on the row and
    // as a toast, and make Cancel drop the still-pending timer (the core emits no
    // progress for a not-yet-started task, so we clean up the row here directly).
    if let Some(ts) = schedule_ts {
        let mut msg = format!("{} {}", tr("Scheduled for:"), format_schedule_ts(ts));
        if let Some(rep) = recurrence_label(recurrence) {
            msg = format!("{msg} · {rep}");
        }
        row.status.set_text(&msg);
        row.set_progress_class("warning");
        // Tag the row with its schedule id and reveal the edit pencil.
        row.sched_id.replace(Some(sched_id.clone()));
        row.edit.set_visible(true);
        state.toast(&msg);

        // Pencil → reopen the schedule dialog pre-filled and re-arm on confirm.
        {
            let st = state.clone();
            let (sid, url, title, thumb, uploader, fmt, ext2, rec) = (
                sched_id.clone(),
                url.to_string(),
                title.to_string(),
                thumbnail.to_string(),
                uploader.to_string(),
                format_id.to_string(),
                ext.to_string(),
                recurrence.to_string(),
            );
            row.edit.connect_clicked(move |_| {
                open_schedule_editor(
                    &st, &sid, &url, &title, &thumb, &uploader, &fmt, &ext2, ts, &rec,
                );
            });
        }

        let st = state.clone();
        let key_c = key.clone();
        let sid = sched_id.clone();
        let dl_slot = row.downloader.clone();
        row.cancel.connect_clicked(move |_| {
            // Already running: the active-downloader handler cancels it and the
            // core's Cancelled progress cleans up — nothing to do here.
            if dl_slot.borrow().is_some() {
                return;
            }
            download_manager::global().cancel_task(&sid);
            bigtube_core::scheduled_downloads::ScheduledDownloadStore::new(
                scheduled_downloads_path(),
            )
            .remove(&sid);
            if let Some(row) = st.download_rows.borrow_mut().remove(&key_c) {
                let fp = row.file_path.borrow().clone();
                if !fp.is_empty() {
                    bigtube_core::history::HistoryManager::new(history_path()).remove_entry(&fp);
                }
                remove_list_card(&st.downloads_box, &row.container);
            }
            st.update_downloads_empty();
        });
    }
    // Labels to update once the real plan is resolved (clones share the widget).
    let status_lbl = row.status.clone();
    let detail_lbl = row.detail.clone();
    state.downloads_box.append(&row.container);
    state.download_rows.borrow_mut().insert(key.clone(), row);
    state.update_downloads_empty();
    state.stack.set_visible_child_name("downloads");

    // Capture the VideoDownloader when the task starts (for cancel/pause). A
    // scheduled task also drops its persisted store entry here — once it's
    // running it must not be restored again on the next launch.
    let tx_started = state.ui_tx.clone();
    let k2 = key.clone();
    let was_scheduled = schedule_ts.is_some();
    let start_sched_id = sched_id.clone();
    // Owned bundle so a recurring task can spawn its next occurrence on fire.
    let rec_start = recurrence.to_string();
    let base_ts = schedule_ts;
    let reschedule = RescheduleInfo {
        url: url.to_string(),
        title: title.to_string(),
        thumbnail: thumbnail.to_string(),
        uploader: uploader.to_string(),
        format_id: format_id.to_string(),
        ext: ext.to_string(),
        force_overwrite,
        recurrence: rec_start,
    };
    let on_start: OnStartFn = Arc::new(move |dl: Arc<VideoDownloader>| {
        if was_scheduled {
            bigtube_core::scheduled_downloads::ScheduledDownloadStore::new(
                scheduled_downloads_path(),
            )
            .remove(&start_sched_id);
            // A recurring task arms its next occurrence the moment this one
            // starts (the main loop computes the next instant and re-enqueues).
            if reschedule.recurrence != "once" {
                if let Some(ts) = base_ts {
                    let _ = tx_started.send_blocking(UiMsg::Reschedule {
                        info: reschedule.clone(),
                        base_ts: ts,
                    });
                }
            }
        }
        let _ = tx_started.send_blocking(UiMsg::Started {
            key: k2.clone(),
            downloader: dl,
        });
    });

    // Build params + enqueue, given the (possibly probe-resolved) format and size.
    let (url_o, title_o, ext_o) = (url.to_string(), title.to_string(), ext.to_string());
    let sub_o = subfolder.map(str::to_string);
    let mgr = download_manager::global();
    // Owned copies for persisting the schedule (so the entry can recreate the
    // download after a restart). The original selector is stored, not a probed
    // concrete id (scheduled tasks run later, when those ids may be stale).
    let persist = schedule_ts.is_some() && !restoring;
    let rec_persist = recurrence.to_string();
    let (s_url, s_title, s_thumb, s_uploader, s_fmt, s_ext, s_path) = (
        url.to_string(),
        title.to_string(),
        thumbnail.to_string(),
        uploader.to_string(),
        format_id.to_string(),
        ext.to_string(),
        file_path.clone(),
    );
    let finalize: Box<dyn FnOnce(String, Option<f64>)> = Box::new(move |fmt, size| {
        let params = DownloadParams {
            url: url_o,
            format_id: fmt,
            title: title_o,
            ext: ext_o,
            force_overwrite,
            estimated_size_mb: size,
            subfolder: sub_o,
        };
        match schedule_ts {
            Some(ts) => {
                if persist {
                    let item = serde_json::json!({
                        "id": sched_id,
                        "scheduled_time": ts,
                        "recurrence": rec_persist,
                        "video_info": {
                            "url": s_url, "title": s_title,
                            "thumbnail": s_thumb, "uploader": s_uploader,
                        },
                        "format_data": { "id": s_fmt, "ext": s_ext },
                        "full_path": s_path,
                        "force_overwrite": force_overwrite,
                        "estimated_size_mb": size,
                    });
                    bigtube_core::scheduled_downloads::ScheduledDownloadStore::new(
                        scheduled_downloads_path(),
                    )
                    .upsert(&item);
                }
                mgr.schedule_download(ts, params, cb, Some(on_start), 0, Some(sched_id));
            }
            None => {
                mgr.add_download(params, cb, Some(on_start), 0);
            }
        }
    });

    if should_probe_plan(ext, schedule_ts) {
        // Exact "sondagem": ask yt-dlp what `format_id` actually resolves to, pin
        // those concrete ids (so the file matches what we show), and display the
        // real codecs/size. The original selector stays as a safety fallback.
        status_lbl.set_text(&tr("Resolving format…"));
        let orig_sel = format_id.to_string();
        let (ptx, prx) =
            async_channel::bounded::<Option<bigtube_core::downloader::ResolvedPlan>>(1);
        let url_p = url.to_string();
        let sel_p = format_id.to_string();
        std::thread::spawn(move || {
            let plan = VideoDownloader::new()
                .ok()
                .and_then(|d| d.resolve_plan(&url_p, &sel_p));
            let _ = ptx.send_blocking(plan);
        });
        glib::spawn_future_local(async move {
            let plan = prx.recv().await.ok().flatten();
            let (fmt, size) = match &plan {
                Some(p) if !p.format_id.is_empty() => {
                    detail_lbl.set_text(&plan_summary(p));
                    detail_lbl.set_visible(true);
                    // Pin concrete ids; keep the codec-aware chain as a fallback.
                    let f = format!("{}/{}", p.format_id, orig_sel);
                    (f, Some(p.size_mb).filter(|s| *s > 0.0))
                }
                _ => (orig_sel, None),
            };
            finalize(fmt, size);
        });
    } else {
        finalize(format_id.to_string(), None);
    }
}

/// Wire a completed download row's footer actions (open folder / play / convert).
fn wire_row_footer(state: &Rc<AppState>, row: &DownloadRow) {
    {
        let state = state.clone();
        let fp = row.file_path.clone();
        row.btn_folder
            .connect_clicked(move |_| open_containing_folder(&state, &fp.borrow()));
    }
    {
        let state = state.clone();
        let container = row.container.clone();
        row.btn_play
            .connect_clicked(move |_| play_download_at(&state, &container));
    }
    // Highlight this row while its file is the one playing.
    wire_play_highlight(state, &row.container, row.file_path.clone());
    {
        let state = state.clone();
        let fp = row.file_path.clone();
        row.btn_convert.connect_clicked(move |_| {
            let path = std::path::PathBuf::from(&*fp.borrow());
            if path.exists() {
                add_converter_file(&state, path);
            }
        });
    }
    // Per-row delete: ask whether to drop just the history entry or the file too.
    {
        let state = state.clone();
        let container = row.container.clone();
        let fp = row.file_path.clone();
        let downloader = row.downloader.clone();
        row.btn_delete.connect_clicked(move |_| {
            confirm_delete_download(&state, &container, &fp.borrow(), &downloader);
        });
    }
}

/// The card box backing a `ListBox` child (the child may be wrapped in an
/// auto-created `ListBoxRow`).
fn card_of(child: &gtk::Widget) -> Option<gtk::Box> {
    if let Ok(row) = child.clone().downcast::<gtk::ListBoxRow>() {
        row.child().and_then(|w| w.downcast::<gtk::Box>().ok())
    } else {
        child.clone().downcast::<gtk::Box>().ok()
    }
}

/// Play the clicked completed download, seeding the player queue from every
/// playable file in the list (in visual order) so prev/next/EOS cycle through
/// them. Highlights follow via the shared NowPlaying handle.
fn play_download_at(state: &Rc<AppState>, clicked: &gtk::Box) {
    let Some(player) = state.player.borrow().clone() else {
        return;
    };
    let rows = state.download_rows.borrow();
    let mut items = Vec::new();
    let mut start = 0usize;
    let mut child = state.downloads_box.first_child();
    while let Some(c) = child {
        let next = c.next_sibling();
        if let Some(card) = card_of(&c) {
            if let Some((_, row)) = rows.iter().find(|(_, r)| r.container == card) {
                let path = row.file_path.borrow().clone();
                if !path.is_empty() && std::path::Path::new(&path).exists() {
                    if card == *clicked {
                        start = items.len();
                    }
                    let title = std::path::Path::new(&path)
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    items.push(crate::player::QueueItem {
                        url: path.clone(),
                        title,
                        artist: row.artist.borrow().clone(),
                        thumbnail: String::new(),
                        is_local: true,
                        is_video: !is_audio_input(std::path::Path::new(&path)),
                    });
                }
            }
        }
        child = next;
    }
    drop(rows);
    if !items.is_empty() {
        player.play_queue(items, start);
    }
}

/// Play a converted file, seeding the queue from every converted output (from
/// history) so prev/next/EOS cycle through them. Falls back to a single play if
/// the clicked output isn't in history (e.g. converter history disabled).
fn play_converter_at(state: &Rc<AppState>, clicked: &str) {
    let Some(player) = state.player.borrow().clone() else {
        return;
    };
    if clicked.is_empty() {
        return;
    }
    let history: Vec<serde_json::Value> =
        bigtube_core::json_store::load_json(converter_history_path(), Vec::new());
    let mut items = Vec::new();
    let mut start = 0usize;
    let mut found = false;
    for it in &history {
        let out = it.get("output").and_then(|v| v.as_str()).unwrap_or("");
        if out.is_empty() || !std::path::Path::new(out).exists() {
            continue;
        }
        if out == clicked {
            start = items.len();
            found = true;
        }
        let title = std::path::Path::new(out)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        items.push(crate::player::QueueItem {
            url: out.to_string(),
            title,
            artist: String::new(),
            thumbnail: String::new(),
            is_local: true,
            is_video: !is_audio_input(std::path::Path::new(out)),
        });
    }
    if found && !items.is_empty() {
        player.play_queue(items, start);
    } else {
        // Not in history: just play the one file.
        let title = std::path::Path::new(clicked)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        player.play_local(clicked, &title, "");
    }
}

/// Add/remove the `.playing` highlight on `container` as the player's current
/// track (its url/path) matches `path`. Uses a weak widget ref so a removed row
/// isn't kept alive.
fn wire_play_highlight(state: &Rc<AppState>, container: &gtk::Box, path: Rc<RefCell<String>>) {
    let Some(player) = state.player.borrow().clone() else {
        return;
    };
    let weak = container.downgrade();
    player.now_playing().connect_url_notify(move |n| {
        let Some(cont) = weak.upgrade() else {
            return;
        };
        let cur = n.url();
        let p = path.borrow();
        if !cur.is_empty() && !p.is_empty() && *p == cur {
            cont.add_css_class("playing");
        } else {
            cont.remove_css_class("playing");
        }
    });
}

/// "Clear all" downloads: ask history-only vs file-too, then wipe every row.
fn confirm_clear_all_downloads(state: &Rc<AppState>) {
    if state.download_rows.borrow().is_empty() {
        return;
    }
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = adw::MessageDialog::new(
        Some(&window),
        Some(&tr("Clear all downloads?")),
        Some(&tr(
            "Remove only the history entries, or delete the files too?",
        )),
    );
    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("history", &tr("Remove from history"));
    dialog.add_response("file", &tr("Delete files too"));
    dialog.set_response_appearance("file", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("history"));
    dialog.set_close_response("cancel");
    apply_theme_classes(&dialog);

    let state = state.clone();
    dialog.connect_response(None, move |dlg, resp| {
        if resp == "history" || resp == "file" {
            let delete_files = resp == "file";
            let mut rows = state.download_rows.borrow_mut();
            for (_, row) in rows.drain() {
                if let Some(d) = row.downloader.borrow().as_ref() {
                    d.cancel();
                }
                if delete_files {
                    delete_output_file(&row.file_path.borrow());
                }
                remove_list_card(&state.downloads_box, &row.container);
            }
            drop(rows);
            // Wipe the saved history so nothing reloads on restart.
            bigtube_core::history::HistoryManager::new(history_path()).clear_all();
            state.update_downloads_empty();
        }
        dlg.close();
    });
    dialog.present();
}

/// Remove a download row (by widget identity) from the list and the row map.
fn remove_download_row(state: &Rc<AppState>, container: &gtk::Box) {
    let mut rows = state.download_rows.borrow_mut();
    let key = rows
        .iter()
        .find(|(_, r)| &r.container == container)
        .map(|(k, _)| k.clone());
    if let Some(k) = key {
        if let Some(r) = rows.remove(&k) {
            remove_list_card(&state.downloads_box, &r.container);
        }
    }
    drop(rows);
    state.update_downloads_empty();
}

/// Ask "remove from history" vs "delete file too" for one download, then apply.
fn confirm_delete_download(
    state: &Rc<AppState>,
    container: &gtk::Box,
    file_path: &str,
    downloader: &Rc<RefCell<Option<Arc<VideoDownloader>>>>,
) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = adw::MessageDialog::new(
        Some(&window),
        Some(&tr("Remove download?")),
        Some(&tr(
            "Remove only the history entry, or delete the file too?",
        )),
    );
    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("history", &tr("Remove from history"));
    dialog.add_response("file", &tr("Delete file too"));
    dialog.set_response_appearance("file", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("history"));
    dialog.set_close_response("cancel");
    apply_theme_classes(&dialog);

    let state = state.clone();
    let container = container.clone();
    let downloader = downloader.clone();
    let file_path = file_path.to_string();
    dialog.connect_response(None, move |dlg, resp| {
        if resp == "history" || resp == "file" {
            // Stop the download first if it's still running.
            if let Some(d) = downloader.borrow().as_ref() {
                d.cancel();
            }
            if resp == "file" {
                delete_output_file(&file_path);
            }
            if !file_path.is_empty() {
                bigtube_core::history::HistoryManager::new(history_path()).remove_entry(&file_path);
            }
            remove_download_row(&state, &container);
        }
        dlg.close();
    });
    dialog.present();
}

fn open_containing_folder(state: &Rc<AppState>, path: &str) {
    if path.is_empty() {
        return;
    }
    let file = gtk::gio::File::for_path(path);
    let launcher = gtk::FileLauncher::new(Some(&file));
    let window = state.window.borrow().clone();
    launcher.open_containing_folder(window.as_ref(), gtk::gio::Cancellable::NONE, |_| {});
}

/// Recreate persisted scheduled downloads after startup, mirroring
/// `download_workflow.restore_scheduled_downloads`. Re-arms each future timer;
/// any whose time already passed (app was closed) downloads immediately.
fn restore_scheduled_downloads(state: &Rc<AppState>) {
    let store =
        bigtube_core::scheduled_downloads::ScheduledDownloadStore::new(scheduled_downloads_path());
    let now = now_epoch_secs();
    for item in store.load() {
        let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let video_info = item.get("video_info").and_then(|v| v.as_object());
        let format_data = item.get("format_data").and_then(|v| v.as_object());
        let full_path = item.get("full_path").and_then(|v| v.as_str()).unwrap_or("");

        // Drop corrupt entries we can't recreate.
        let (Some(video_info), Some(format_data)) = (video_info, format_data) else {
            if !id.is_empty() {
                store.remove(id);
            }
            continue;
        };
        if full_path.is_empty() || id.is_empty() {
            if !id.is_empty() {
                store.remove(id);
            }
            continue;
        }

        let get = |m: &serde_json::Map<String, serde_json::Value>, k: &str| {
            m.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
        };
        let url = get(video_info, "url");
        let title = get(video_info, "title");
        let thumbnail = get(video_info, "thumbnail");
        let uploader = get(video_info, "uploader");
        let format_id = get(format_data, "id");
        let ext = get(format_data, "ext");
        let force_overwrite = item
            .get("force_overwrite")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let recurrence = item
            .get("recurrence")
            .and_then(|v| v.as_str())
            .unwrap_or("once")
            .to_string();

        // Past its time while the app was closed. For a one-shot, download right
        // away (schedule_ts = None). For a recurring one, skip the missed runs
        // (cron-style) and re-arm the next future occurrence in place.
        let mut schedule_ts = item.get("scheduled_time").and_then(|v| v.as_f64());
        if let Some(ts) = schedule_ts {
            if ts <= now {
                if recurrence != "once" {
                    schedule_ts = next_occurrence(ts, &recurrence, now);
                    match schedule_ts {
                        Some(next) => {
                            // Persist the advanced time on the same entry so it
                            // isn't restored as past-due again next launch.
                            let mut updated = item.clone();
                            updated["scheduled_time"] = serde_json::json!(next);
                            store.upsert(&updated);
                        }
                        None => store.remove(id),
                    }
                } else {
                    store.remove(id);
                    schedule_ts = None;
                }
            }
        }

        enqueue_common(
            state,
            &url,
            &title,
            &thumbnail,
            &uploader,
            &format_id,
            &ext,
            schedule_ts,
            force_overwrite,
            None,
            Some(id.to_string()),
            &recurrence,
        );
    }
}

/// Load persisted download history into the Downloads list on startup.
fn load_download_history(state: &Rc<AppState>) {
    // Pure read (see load_converter_history): avoid the manager's drop-flush.
    let items: Vec<serde_json::Value> =
        bigtube_core::json_store::load_json(history_path(), Vec::new());

    // Items that are still scheduled are restored as live scheduled rows by
    // `restore_scheduled_downloads`; skip them here so they don't show twice
    // (once as a stale "Scheduled" history row, once as the live one).
    let scheduled_paths: std::collections::HashSet<String> =
        bigtube_core::scheduled_downloads::ScheduledDownloadStore::new(scheduled_downloads_path())
            .load()
            .iter()
            .filter_map(|e| {
                e.get("full_path")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .collect();

    for it in &items {
        let title = it
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Title");
        let fp = it.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
        if !fp.is_empty() && scheduled_paths.contains(fp) {
            continue;
        }
        let uploader = it.get("uploader").and_then(|v| v.as_str()).unwrap_or("");
        let status = it
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("completed");

        let row = DownloadRow::new(title, fp, uploader);
        row.pause.set_visible(false);
        row.cancel.set_visible(false);
        row.progress.set_visible(false);
        // Past-session rows are terminal, so "Clear" can remove them.
        row.pause.set_sensitive(false);
        row.cancel.set_sensitive(false);
        row.status.set_text(&history_status_label(status));
        let exists = !fp.is_empty() && std::path::Path::new(fp).exists();
        row.actions.set_visible(exists);

        // Restore the saved media summary (codecs/resolution/size) bottom-left.
        let media = it
            .get("media_summary")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !media.is_empty() {
            row.detail.set_text(media);
            row.detail.set_visible(true);
        }

        // A failed download comes back retryable: red bar + a Retry button that
        // re-enqueues from the stored url/format and drops the stale entry.
        if status == "error" {
            row.progress.set_visible(true);
            row.progress.set_fraction(0.0);
            row.set_progress_class("error");
            row.pause.set_visible(true);
            row.pause.set_sensitive(true);
            row.pause.set_icon_name("view-refresh-symbolic");
            row.pause.set_tooltip_text(Some(&tr("Retry")));

            let state2 = state.clone();
            let container = row.container.clone();
            let u = it
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let t = title.to_string();
            let th = it
                .get("thumbnail")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let up = uploader.to_string();
            let f = it
                .get("format_id")
                .and_then(|v| v.as_str())
                .unwrap_or("best")
                .to_string();
            let e = it
                .get("ext")
                .and_then(|v| v.as_str())
                .unwrap_or("mp4")
                .to_string();
            let fp_owned = fp.to_string();
            row.pause.connect_clicked(move |_| {
                if u.is_empty() {
                    return;
                }
                remove_download_row(&state2, &container);
                if !fp_owned.is_empty() {
                    bigtube_core::history::HistoryManager::new(history_path())
                        .remove_entry(&fp_owned);
                }
                enqueue_download(&state2, &u, &t, &th, &up, &f, &e);
            });
        }

        wire_row_footer(state, &row);
        state.downloads_box.append(&row.container);
        // Track in the row map so "Clear" can find and remove them too.
        state.download_rows.borrow_mut().insert(next_key(), row);
    }
    state.update_downloads_empty();
}

/// Path to the persisted converter-history file.
fn converter_history_path() -> std::path::PathBuf {
    bigtube_core::paths::config_dir().join("converter_history.json")
}

/// Restore past conversions into the Converter list as completed rows.
fn load_converter_history(state: &Rc<AppState>) {
    // Pure read: do NOT construct a ConverterHistoryManager here — its debouncer
    // flushes on drop, which would turn this load into a write and could clobber
    // the file with an empty list on a transient read race.
    let items: Vec<serde_json::Value> =
        bigtube_core::json_store::load_json(converter_history_path(), Vec::new());
    for it in &items {
        let source = it.get("source").and_then(|v| v.as_str()).unwrap_or("");
        let output = it.get("output").and_then(|v| v.as_str()).unwrap_or("");
        let format = it.get("format").and_then(|v| v.as_str()).unwrap_or("");
        if output.is_empty() {
            continue;
        }
        add_converted_history_row(state, source, output, format);
    }
    state.update_converter_empty();
}

/// A finished converter row restored from history: shows the output name, a
/// completed bar, and open-folder / play / remove actions (no convert flow).
fn add_converted_history_row(state: &Rc<AppState>, source: &str, output: &str, format: &str) {
    let name = std::path::Path::new(output)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| output.to_string());

    let container = gtk::Box::new(gtk::Orientation::Vertical, 4);
    container.add_css_class("card");
    container.set_margin_top(6);
    container.set_margin_bottom(6);
    container.set_margin_start(8);
    container.set_margin_end(8);
    let pad = gtk::Box::new(gtk::Orientation::Vertical, 4);
    pad.set_margin_top(8);
    pad.set_margin_bottom(8);
    pad.set_margin_start(12);
    pad.set_margin_end(12);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name_lbl = gtk::Label::new(Some(&name));
    name_lbl.set_xalign(0.0);
    name_lbl.set_hexpand(true);
    name_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
    name_lbl.add_css_class("heading");
    let fmt_lbl = gtk::Label::new(Some(&format.to_uppercase()));
    fmt_lbl.add_css_class("dim-label");
    fmt_lbl.add_css_class("caption");
    let folder = gtk::Button::from_icon_name("folder-open-symbolic");
    folder.add_css_class("flat");
    folder.set_tooltip_text(Some(&tr("Open Folder")));
    let play = gtk::Button::from_icon_name("media-playback-start-symbolic");
    play.add_css_class("flat");
    play.set_tooltip_text(Some(&tr("Play Video")));
    let remove = gtk::Button::from_icon_name("user-trash-symbolic");
    remove.add_css_class("flat");
    remove.set_tooltip_text(Some(&tr("Remove from list")));
    header.append(&name_lbl);
    header.append(&fmt_lbl);
    header.append(&folder);
    header.append(&play);
    header.append(&remove);

    // Destination path under the name.
    let path_lbl = gtk::Label::new(Some(output));
    path_lbl.set_xalign(0.0);
    path_lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    path_lbl.set_tooltip_text(Some(output));
    path_lbl.add_css_class("dim-label");
    path_lbl.add_css_class("caption");

    let status = gtk::Label::new(Some(tr("Success!").as_str()));
    status.set_xalign(0.0);
    status.add_css_class("dim-label");
    status.add_css_class("caption");

    pad.append(&header);
    pad.append(&path_lbl);
    pad.append(&status);
    container.append(&pad);
    state.converter_box.append(&container);

    let exists = std::path::Path::new(output).exists();
    folder.set_visible(exists);
    play.set_visible(exists);

    let out = output.to_string();
    {
        let state = state.clone();
        let out = out.clone();
        folder.connect_clicked(move |_| open_containing_folder(&state, &out));
    }
    {
        let state = state.clone();
        let out = out.clone();
        play.connect_clicked(move |_| play_converter_at(&state, &out));
    }
    // Highlight this row while its output is the one playing.
    wire_play_highlight(state, &container, Rc::new(RefCell::new(output.to_string())));
    {
        let state = state.clone();
        let container = container.clone();
        let source = source.to_string();
        let format = format.to_string();
        let out = out.clone();
        remove.connect_clicked(move |_| {
            confirm_delete_converter(&state, &container, &source, Some(&format), &out);
        });
    }
}

/// "Clear all" conversions: ask history vs files, then wipe every row.
fn confirm_clear_all_converter(state: &Rc<AppState>) {
    if state.converter_box.first_child().is_none() {
        return;
    }
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = adw::MessageDialog::new(
        Some(&window),
        Some(&tr("Clear all conversions?")),
        Some(&tr(
            "Remove only the history entries, or delete the files too?",
        )),
    );
    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("history", &tr("Remove from history"));
    dialog.add_response("file", &tr("Delete files too"));
    dialog.set_response_appearance("file", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("history"));
    dialog.set_close_response("cancel");
    apply_theme_classes(&dialog);

    let state = state.clone();
    dialog.connect_response(None, move |dlg, resp| {
        if resp == "history" || resp == "file" {
            // Stop anything still queued (a running one finishes on its own).
            state.conv_queue.borrow_mut().clear();
            let mgr = bigtube_core::converter_history::ConverterHistoryManager::new(
                converter_history_path(),
            );
            if resp == "file" {
                for it in mgr.load() {
                    if let Some(out) = it.get("output").and_then(|v| v.as_str()) {
                        delete_output_file(out);
                    }
                }
            }
            mgr.clear_all();
            while let Some(c) = state.converter_box.first_child() {
                state.converter_box.remove(&c);
            }
            state.update_converter_empty();
        }
        dlg.close();
    });
    dialog.present();
}

/// Ask "remove from history" vs "delete output file too" for one conversion.
fn confirm_delete_converter(
    state: &Rc<AppState>,
    container: &gtk::Box,
    source: &str,
    format: Option<&str>,
    out_path: &str,
) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = adw::MessageDialog::new(
        Some(&window),
        Some(&tr("Remove conversion?")),
        Some(&tr(
            "Remove only the history entry, or delete the file too?",
        )),
    );
    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("history", &tr("Remove from history"));
    dialog.add_response("file", &tr("Delete file too"));
    dialog.set_response_appearance("file", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("history"));
    dialog.set_close_response("cancel");
    apply_theme_classes(&dialog);

    let state = state.clone();
    let container = container.clone();
    let source = source.to_string();
    let format = format.map(str::to_string);
    let out_path = out_path.to_string();
    dialog.connect_response(None, move |dlg, resp| {
        if resp == "history" || resp == "file" {
            if resp == "file" {
                delete_output_file(&out_path);
            }
            bigtube_core::converter_history::ConverterHistoryManager::new(converter_history_path())
                .remove_entry(&source, format.as_deref());
            remove_list_card(&state.converter_box, &container);
            state.update_converter_empty();
        }
        dlg.close();
    });
    dialog.present();
}

fn history_status_label(status: &str) -> String {
    use StatusCode::*;
    let code = match status {
        "completed" => Completed,
        "cancelled" | "interrupted" | "paused" => Cancelled,
        "error" => UnknownError,
        _ => Queued,
    };
    status_label(code)
}

fn next_key() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    format!("dl-{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

fn parse_percent(s: &str) -> Option<f64> {
    s.trim()
        .trim_end_matches('%')
        .parse::<f64>()
        .ok()
        .map(|v| (v / 100.0).clamp(0.0, 1.0))
}

fn add_page(stack: &adw::ViewStack, child: &gtk::Widget, name: &str, title: &str, icon: &str) {
    let page = stack.add_titled(child, Some(name), title);
    page.set_icon_name(Some(icon));
}

/// Translated label for a status code. The msgids match `locales.py`'s
/// `StringKey` values so the existing `.mo` catalogs resolve them.
/// Format a seconds count as `M:SS` (or `H:MM:SS` past an hour).
fn fmt_eta(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}

fn status_label(s: StatusCode) -> String {
    use StatusCode::*;
    let msgid = match s {
        Starting => "Starting download...",
        Downloading => "Downloading...",
        Processing => "Processing...",
        Merging => "Merging...",
        Extracting => "Extracting Audio...",
        Completed => "Completed",
        Cancelled => "Cancelled",
        Resuming => "Resuming download...",
        Scheduled => "Scheduled",
        Queued => "Queued",
        FfmpegMissingMetadata => "FFmpeg not found. Skipping metadata.",
        FfmpegMissingSubtitles => "FFmpeg not found. Skipping subtitles.",
        DiskSpaceError => "Not enough disk space!",
        Timeout => "Timeout",
        NetworkError => "Network Error!",
        DrmError => "Content is DRM Protected!",
        PrivateError => "Video is Private!",
        FfmpegError => "FFmpeg Error - Missing or incompatible!",
        BotBlocked => "Blocked by YouTube — enable cookies in Settings",
        UnknownError => "Unknown Error!",
    };
    tr(msgid)
}

/// Apply theme mode + accent color from config (`_apply_theme_to_window`).
/// Idempotent: clears previously-set classes so it can run on live changes.
/// A comfortable default window size derived from the primary monitor: ~75% of
/// its width and ~82% of its height, clamped to sane bounds. Falls back to a
/// fixed size when the monitor geometry isn't available.
fn comfortable_window_size() -> (i32, i32) {
    let geo = gtk::gdk::Display::default()
        .and_then(|d| d.monitors().item(0))
        .and_then(|o| o.downcast::<gtk::gdk::Monitor>().ok())
        .map(|m| m.geometry());
    match geo {
        Some(g) if g.width() > 0 && g.height() > 0 => {
            let w = ((g.width() as f64 * 0.75) as i32).clamp(900, 1600);
            let h = ((g.height() as f64 * 0.82) as i32).clamp(600, 1040);
            (w, h)
        }
        _ => (1000, 700),
    }
}

fn apply_theme(window: &adw::ApplicationWindow) {
    let mode = config::global().read().unwrap().get_string("theme_mode");
    let sm = adw::StyleManager::default();
    match mode.as_str() {
        "dark" => sm.set_color_scheme(adw::ColorScheme::ForceDark),
        "light" => sm.set_color_scheme(adw::ColorScheme::ForceLight),
        _ => sm.set_color_scheme(adw::ColorScheme::Default),
    }

    apply_theme_classes(window);
    // Accent CSS classes only style the widget subtree they're set on, so every
    // separate top-level window (player, playlist, dialogs, about) needs them
    // too — update all currently-open toplevels.
    let toplevels = gtk::Window::toplevels();
    for i in 0..toplevels.n_items() {
        if let Some(w) = toplevels
            .item(i)
            .and_then(|o| o.downcast::<gtk::Window>().ok())
        {
            apply_theme_classes(&w);
        }
    }
}

/// Apply the configured light/dark + accent CSS classes to a single widget
/// (any top-level window). Call this when creating a secondary window so it
/// matches the selected theme.
pub(crate) fn apply_theme_classes(widget: &impl IsA<gtk::Widget>) {
    let (mode, color) = {
        let cfg = config::global().read().unwrap();
        (cfg.get_string("theme_mode"), cfg.get_string("theme_color"))
    };
    let w = widget.as_ref();
    w.remove_css_class("light");
    w.remove_css_class("dark");
    for c in bigtube_core::enums::ThemeColor::ALL {
        w.remove_css_class(&format!("accent-{}", c.as_value()));
    }
    if mode == "dark" {
        w.add_css_class("dark");
    } else if mode == "light" {
        w.add_css_class("light");
    }
    if !color.is_empty() {
        w.add_css_class(&format!("accent-{color}"));
    }
}
