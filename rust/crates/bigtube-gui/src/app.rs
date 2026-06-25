//! Window construction, page wiring, and the search→download→progress loop.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use gtk::{gio, glib};

use bigtube_core::config;
use bigtube_core::downloader::VideoDownloader;
use bigtube_core::progress::{ProgressFn, StatusCode};

use crate::i18n::tr;
use crate::objects::VideoObject;

mod converter;
mod donations;
mod downloads;
mod search;
mod settings;
mod widgets;

use converter::{build_converter_page, load_converter_history, load_pending_conv};
use downloads::{
    build_downloads_page, codec_pretty, download_all, enqueue_common, load_download_history,
    next_occurrence, on_download_clicked, restore_scheduled_downloads, schedule_all,
};
use search::build_search_page;
use settings::build_settings_page;
use widgets::{add_page, human_size, parse_percent};

/// Translate, then escape Pango markup. Widgets like `AdwPreferencesGroup` and
/// `AdwActionRow` render their title with markup enabled, so a raw `&` (valid in
/// English source strings such as "Network & Advanced") breaks rendering. This
/// keeps those titles correct in every locale.
pub(crate) fn tr_markup(s: &str) -> String {
    glib::markup_escape_text(&tr(s)).to_string()
}

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
        // Tag the card so the downloads filter can match it by title/artist/path.
        set_row_filter_key(&container, &format!("{title} {artist} {file_path}"));
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
        let pause = gtk::Button::from_icon_name("bigtube-media-playback-pause-symbolic");
        pause.add_css_class("flat");
        pause.set_tooltip_text(Some(&tr("Pause")));
        a11y_label(&pause, &tr("Pause"));
        let cancel = gtk::Button::from_icon_name("bigtube-process-stop-symbolic");
        cancel.add_css_class("flat");
        cancel.add_css_class("destructive-action");
        cancel.set_tooltip_text(Some(&tr("Cancel")));
        a11y_label(&cancel, &tr("Cancel"));
        // Edit pencil: shown only while this row is a pending scheduled download.
        let edit = gtk::Button::from_icon_name("bigtube-document-edit-symbolic");
        edit.add_css_class("flat");
        edit.set_tooltip_text(Some(&tr("Edit")));
        a11y_label(&edit, &tr("Edit"));
        edit.set_visible(false);
        // Per-row delete (asks history-only vs file too); wired in wire_row_footer.
        let btn_delete = gtk::Button::from_icon_name("bigtube-user-trash-symbolic");
        btn_delete.add_css_class("flat");
        btn_delete.set_tooltip_text(Some(&tr("Remove from list")));
        a11y_label(&btn_delete, &tr("Remove from list"));
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
        let btn_folder = gtk::Button::from_icon_name("bigtube-folder-open-symbolic");
        btn_folder.add_css_class("flat");
        btn_folder.set_tooltip_text(Some(&tr("Open Folder")));
        a11y_label(&btn_folder, &tr("Open Folder"));
        let btn_play = gtk::Button::from_icon_name("bigtube-media-playback-start-symbolic");
        btn_play.add_css_class("flat");
        btn_play.set_tooltip_text(Some(&tr("Play Video")));
        a11y_label(&btn_play, &tr("Play Video"));
        let btn_convert = gtk::Button::from_icon_name("bigtube-emblem-synchronizing-symbolic");
        btn_convert.add_css_class("flat");
        btn_convert.set_tooltip_text(Some(&tr("Add to Converter")));
        a11y_label(&btn_convert, &tr("Add to Converter"));
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
                pause_btn.set_icon_name("bigtube-media-playback-pause-symbolic");
                pause_btn.set_tooltip_text(Some(&tr("Pause")));
                status_c.set_text(&tr("Queued"));
                for c in ["success", "warning", "error"] {
                    progress_c.remove_css_class(c);
                }
                cancel_c.set_visible(true);
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
                pause_btn.set_icon_name("bigtube-media-playback-pause-symbolic");
                if let Some(cb) = pf.borrow().as_ref().cloned() {
                    std::thread::spawn(move || {
                        d.resume(&cb);
                    });
                }
            } else {
                paused.set(true);
                pause_btn.set_icon_name("bigtube-media-playback-start-symbolic");
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
        // The Cancel button only makes sense while a transfer is actually
        // running — hide it in the idle "Queued" state. (A pending *scheduled*
        // row keeps its own Cancel: it never reaches update() until it starts.)
        let in_progress = matches!(
            status,
            StatusCode::Starting
                | StatusCode::Downloading
                | StatusCode::Processing
                | StatusCode::Merging
                | StatusCode::Extracting
                | StatusCode::Resuming
        );
        if in_progress {
            self.cancel.set_visible(true);
            self.cancel.set_sensitive(true);
        } else if status == StatusCode::Queued {
            self.cancel.set_visible(false);
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
            self.pause.set_icon_name("bigtube-view-refresh-symbolic");
            self.pause.set_tooltip_text(Some(&tr("Retry")));
            self.cancel.set_visible(true);
            self.cancel.set_sensitive(true);
        } else if status == StatusCode::Cancelled {
            // A real cancel (not a pause): don't leave a dead "Cancelled" row —
            // reset it to the initial, restartable look.
            self.reset_to_initial();
        }
    }

    /// Return a cancelled row to its initial "Queued" appearance: empty bar, no
    /// status colour, and the pause button turned into a Retry that re-runs the
    /// download from scratch (the core clears its cancelled flag on resume).
    fn reset_to_initial(&self) {
        self.is_error.set(true); // routes the pause button to the retry path
        self.is_paused.set(false);
        self.progress.set_fraction(0.0);
        self.set_progress_class("");
        self.detail.set_visible(false);
        self.actions.set_visible(false);
        self.status.set_text(&tr("Queued"));
        self.pause.set_visible(true);
        self.pause.set_sensitive(true);
        self.pause.set_icon_name("bigtube-view-refresh-symbolic");
        self.pause.set_tooltip_text(Some(&tr("Retry")));
        // Idle initial state — nothing to cancel; the X reappears once Retry
        // restarts the transfer (update() shows it on the next progress tick).
        self.cancel.set_visible(false);
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
    // The search box, so the Ctrl+L shortcut can jump focus to it.
    search_entry: RefCell<Option<gtk::SearchEntry>>,
    select_mode: Cell<bool>,
    select_revealer: gtk::Revealer,
    // Header toggle that enters selection mode (disabled when there are no
    // search results to select).
    btn_select: gtk::ToggleButton,
    select_btn: gtk::Button,
    sched_selected_btn: gtk::Button,
    downloads_box: gtk::ListBox,
    downloads_stack: gtk::Stack,
    // "Clear history" header button (disabled when the list is empty).
    downloads_clear: gtk::Button,
    download_rows: RefCell<HashMap<String, DownloadRow>>,
    converter_box: gtk::ListBox,
    converter_stack: gtk::Stack,
    // "Clear history" header button (disabled when the list is empty).
    converter_clear: gtk::Button,
    // Conversions run one at a time (mirrors `converter_controller.py`): a click
    // enqueues, and each finish pumps the next. Without this they'd all run in
    // parallel threads and thrash the CPU.
    conv_active: Cell<bool>,
    conv_queue: RefCell<std::collections::VecDeque<converter::PendingConv>>,
    player: RefCell<Option<Rc<crate::player::Player>>>,
    busy_spinner: gtk::Spinner,
    // Centered "busy" card (spinner + message) shown over the whole window
    // during a background fetch, instead of a header spinner + bottom toast.
    busy_overlay: gtk::Box,
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
        self.busy_spinner.start();
        self.busy_overlay.set_visible(true);
    }

    fn busy_end(&self) {
        let n = (self.busy_count.get() - 1).max(0);
        self.busy_count.set(n);
        if n == 0 {
            self.busy_spinner.stop();
            self.busy_overlay.set_visible(false);
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
        // Nothing to clear when the list is empty.
        self.downloads_clear.set_sensitive(has);
    }

    fn update_converter_empty(&self) {
        let has = self.converter_box.first_child().is_some();
        self.converter_stack
            .set_visible_child_name(if has { "list" } else { "empty" });
        self.converter_clear.set_sensitive(has);
    }

    fn update_search_empty(&self) {
        let has = self.search_store.n_items() > 0;
        self.search_stack
            .set_visible_child_name(if has { "list" } else { "empty" });
        // No results → can't enter selection mode; leave/cancel it if active.
        if !has {
            self.btn_select.set_active(false);
        }
        self.btn_select.set_sensitive(has);
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
        search_entry: RefCell::new(None),
        select_mode: Cell::new(false),
        select_revealer: gtk::Revealer::new(),
        btn_select: gtk::ToggleButton::new(),
        select_btn: gtk::Button::new(),
        sched_selected_btn: gtk::Button::new(),
        downloads_box: downloads_box.clone(),
        downloads_stack: gtk::Stack::new(),
        downloads_clear: gtk::Button::new(),
        download_rows: RefCell::new(HashMap::new()),
        converter_box: converter_box.clone(),
        converter_stack: gtk::Stack::new(),
        converter_clear: gtk::Button::new(),
        conv_active: Cell::new(false),
        conv_queue: RefCell::new(std::collections::VecDeque::new()),
        player: RefCell::new(None),
        busy_spinner: gtk::Spinner::new(),
        busy_overlay: gtk::Box::new(gtk::Orientation::Vertical, 14),
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
        "bigtube-system-search-symbolic",
    );
    add_page(
        &stack,
        &downloads_page,
        "downloads",
        &tr("Downloads"),
        "bigtube-download-symbolic",
    );
    add_page(
        &stack,
        &converter_page,
        "converter",
        &tr("Converter"),
        "bigtube-view-refresh-symbolic",
    );
    add_page(
        &stack,
        &settings_page,
        "settings",
        &tr("Settings"),
        "bigtube-emblem-system-symbolic",
    );

    let switcher = adw::ViewSwitcher::builder()
        .stack(&stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .build();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&switcher));

    // Primary (hamburger) menu: About / Quit.
    let menu = gio::Menu::new();
    menu.append(Some(&tr("About")), Some("app.about"));
    menu.append(Some(&tr("Donations")), Some("app.donate"));
    menu.append(Some(&tr("Quit")), Some("app.quit"));
    let menu_btn = gtk::MenuButton::new();
    menu_btn.set_icon_name("bigtube-open-menu-symbolic");
    menu_btn.set_menu_model(Some(&menu));
    header.pack_end(&menu_btn);
    setup_app_actions(app);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&stack));
    // Narrow-window navigation: a bottom view-switcher bar (auto-reveals).
    let switcher_bar = adw::ViewSwitcherBar::builder().stack(&stack).build();
    toolbar.add_bottom_bar(&switcher_bar);

    // Busy overlay: a dimmed scrim covering the whole window during a background
    // fetch, with a spinner + message card floating centered on top (replaces
    // the old header spinner and "Processing…" toast). Shown/hidden by
    // busy_begin/busy_end. The scrim is modal — it intercepts clicks while the
    // fetch runs (which always ends, via success or error).
    {
        let scrim = &state.busy_overlay;
        scrim.set_halign(gtk::Align::Fill);
        scrim.set_valign(gtk::Align::Fill);
        scrim.set_hexpand(true);
        scrim.set_vexpand(true);
        scrim.add_css_class("busy-dim");
        scrim.set_visible(false);

        let card = gtk::Box::new(gtk::Orientation::Vertical, 18);
        card.set_halign(gtk::Align::Center);
        card.set_valign(gtk::Align::Center);
        // In a vertical box a single child packs to the TOP; vexpand gives the
        // card the full height so valign=Center actually centers it vertically.
        card.set_vexpand(true);
        card.add_css_class("busy-card");
        state.busy_spinner.set_size_request(54, 54);
        state.busy_spinner.set_margin_top(34);
        state.busy_spinner.set_margin_start(64);
        state.busy_spinner.set_margin_end(64);
        let label = gtk::Label::new(Some(&tr("Processing...")));
        label.add_css_class("title-2");
        label.set_margin_bottom(34);
        card.append(&state.busy_spinner);
        card.append(&label);
        scrim.append(&card);
    }
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&toolbar));
    overlay.add_overlay(&state.busy_overlay);
    toasts.set_child(Some(&overlay));

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

    // Flush any debounced config write before the window closes, so a setting
    // changed right before quitting isn't lost.
    window.connect_close_request(|_| {
        config_saver().flush();
        // "Clear All Data on Exit": wipe the history/finished-item stores (never
        // the config itself) so the next launch starts empty.
        if config::global()
            .read()
            .map(|c| c.get_bool("auto_clear_finished"))
            .unwrap_or(false)
        {
            wipe_finished_data();
        }
        glib::Propagation::Proceed
    });

    // Window-level keyboard shortcuts: Ctrl+L focuses search; Ctrl+1..4 switch
    // tabs. (Dialogs keep handling Esc natively.)
    {
        let controller = gtk::ShortcutController::new();
        controller.set_scope(gtk::ShortcutScope::Global);
        fn add(c: &gtk::ShortcutController, accel: &str, cb: impl Fn() + 'static) {
            if let Some(trigger) = gtk::ShortcutTrigger::parse_string(accel) {
                let action = gtk::CallbackAction::new(move |_, _| {
                    cb();
                    glib::Propagation::Stop
                });
                c.add_shortcut(gtk::Shortcut::new(Some(trigger), Some(action)));
            }
        }
        let tab = |name: &'static str| {
            let state = state.clone();
            move || state.stack.set_visible_child_name(name)
        };
        add(&controller, "<Control>1", tab("search"));
        add(&controller, "<Control>2", tab("downloads"));
        add(&controller, "<Control>3", tab("converter"));
        add(&controller, "<Control>4", tab("settings"));
        {
            let state = state.clone();
            add(&controller, "<Control>l", move || {
                state.stack.set_visible_child_name("search");
                if let Some(e) = state.search_entry.borrow().as_ref() {
                    e.grab_focus();
                }
            });
        }
        window.add_controller(controller);
    }

    // Player + bottom transport bar. Flat bottom area so the player's rounded
    // card visibly floats instead of sitting in a styled toolbar strip. The
    // player is optional: if its GStreamer video stack is missing we skip the
    // transport bar and leave `state.player` None (playback attempts no-op with
    // a toast), so the app still runs for downloads/conversion.
    toolbar.set_bottom_bar_style(adw::ToolbarStyle::Flat);
    if let Some((player, player_bar)) = crate::player::build(&window) {
        toolbar.add_bottom_bar(&player_bar);
        state.player.replace(Some(player));
    }

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
                    // On completion: either auto-remove the finished row (opt-in,
                    // "remove when complete") or probe the real file (codecs +
                    // on-disk size) off-thread and show it as the row's status.
                    if status == StatusCode::Completed {
                        // Opt-in desktop notification (the file name as the body).
                        if config::global()
                            .read()
                            .map(|c| c.get_bool("system_notifications"))
                            .unwrap_or(false)
                        {
                            if let Some(gapp) = gio::Application::default() {
                                let note = gio::Notification::new(&tr("Download complete"));
                                if let Some((path, _)) = &info {
                                    if let Some(name) = std::path::Path::new(path).file_name() {
                                        note.set_body(Some(&name.to_string_lossy()));
                                    }
                                }
                                gapp.send_notification(None, &note);
                            }
                        }
                        let remove = config::global()
                            .read()
                            .map(|c| c.get_bool("remove_on_complete"))
                            .unwrap_or(false);
                        if remove {
                            if let Some((path, _)) = &info {
                                if !path.is_empty() {
                                    bigtube_core::history::HistoryManager::new(history_path())
                                        .remove_entry(path);
                                }
                            }
                            if let Some(row) =
                                state_for_loop.download_rows.borrow_mut().remove(&key)
                            {
                                remove_list_card(&state_for_loop.downloads_box, &row.container);
                            }
                            state_for_loop.update_downloads_empty();
                        } else if let Some(path) = info
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
                    // partial files. Drop the row + history entry only when
                    // "remove when cancelled" is on; otherwise the row was just
                    // reset to its initial, restartable state by `update`.
                    if status == StatusCode::Cancelled {
                        let remove = config::global()
                            .read()
                            .map(|c| c.get_bool("remove_on_cancel"))
                            .unwrap_or(false);
                        if remove {
                            if let Some((path, paused)) = &info {
                                if !paused {
                                    if !path.is_empty() {
                                        bigtube_core::history::HistoryManager::new(history_path())
                                            .remove_entry(path);
                                    }
                                    if let Some(row) =
                                        state_for_loop.download_rows.borrow_mut().remove(&key)
                                    {
                                        remove_list_card(
                                            &state_for_loop.downloads_box,
                                            &row.container,
                                        );
                                    }
                                    state_for_loop.update_downloads_empty();
                                }
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
    // Re-add converter items that were queued but never converted.
    load_pending_conv(&state);
    // Recreate persisted scheduled downloads (re-arming their timers, or running
    // immediately any whose time passed while the app was closed).
    restore_scheduled_downloads(&state);
    // Trim the on-disk thumbnail cache off the main thread so it can't grow
    // without bound over many browsing sessions.
    std::thread::spawn(crate::row::prune_thumbnail_cache);

    // Always run the monitor; it honours the live `monitor_clipboard` setting.
    start_clipboard_monitor(&state);

    // Optional background check: install missing components and flag a yt-dlp
    // update. Off the UI thread so it never delays the window appearing.
    start_update_check(&state);

    // Keyboard focus follows the active tab: land on the search field when the
    // Search page is shown (including at startup), and clear focus elsewhere so
    // the compact header filter never grabs it (e.g. on the Downloads tab).
    {
        let state_cb = state.clone();
        let window_cb = window.clone();
        state.stack.connect_visible_child_name_notify(move |stack| {
            if stack.visible_child_name().as_deref() == Some("search") {
                if let Some(e) = state_cb.search_entry.borrow().as_ref() {
                    e.grab_focus();
                }
            } else {
                gtk::prelude::GtkWindowExt::set_focus(&window_cb, gtk::Widget::NONE);
            }
        });
    }

    window.present();
    // Initial tab is Search → focus the search field, not the header filter.
    let initial_entry = state.search_entry.borrow().clone();
    if let Some(e) = initial_entry {
        e.grab_focus();
    }
}

/// On launch (when `check_updates_on_startup` is on), make sure yt-dlp/deno are
/// present and notify if a newer yt-dlp exists. Fully background — startup is
/// never blocked, and any network failure is silent.
fn start_update_check(state: &Rc<AppState>) {
    if !config::global()
        .read()
        .unwrap()
        .get_bool("check_updates_on_startup")
    {
        return;
    }
    let (yt_dlp, deno) = {
        let c = config::global().read().unwrap();
        (c.yt_dlp_path.clone(), c.deno_path.clone())
    };

    struct UpdateMsg {
        installed: bool,
        check: bigtube_core::updater::UpdateCheck,
    }
    let (tx, rx) = async_channel::bounded::<UpdateMsg>(1);
    std::thread::spawn(move || {
        // Download whichever binary is missing first (fresh install), then
        // compare the installed yt-dlp against the latest release.
        let installed = !yt_dlp.exists() || !deno.exists();
        bigtube_core::updater::ensure_exists(&yt_dlp, &deno);
        let check = bigtube_core::updater::check_yt_dlp_update(&yt_dlp);
        let _ = tx.send_blocking(UpdateMsg { installed, check });
    });

    let state = state.clone();
    glib::spawn_future_local(async move {
        if let Ok(msg) = rx.recv().await {
            if msg.installed {
                state.toast(&tr("Components installed successfully! ✅"));
            } else if msg.check.update_available() {
                let latest = msg.check.latest.unwrap_or_default();
                state.toast(&format!("{} ({latest})", tr("yt-dlp update available")));
            }
        }
    });
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
                .website("https://github.com/eltonfabricio10/bigtube")
                .issue_url("https://github.com/eltonfabricio10/bigtube/issues")
                .build();
            dialog.present(app.active_window().as_ref());
        });
    }
    app.add_action(&about);

    let donate = gio::SimpleAction::new("donate", None);
    {
        let app = app.clone();
        donate.connect_activate(move |_, _| {
            if let Some(win) = app.active_window() {
                donations::show_donations_dialog(&win);
            }
        });
    }
    app.add_action(&donate);
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

/// Debounced config writer: a slider drag fires dozens of value-changed events,
/// and a synchronous save per tick means dozens of atomic disk writes. Coalesce
/// them into one write ~0.8s after the last change. Flushed on close/restart.
fn config_saver() -> &'static bigtube_core::debounce::Debouncer {
    static SAVER: std::sync::OnceLock<bigtube_core::debounce::Debouncer> =
        std::sync::OnceLock::new();
    SAVER.get_or_init(|| {
        bigtube_core::debounce::Debouncer::new(std::time::Duration::from_millis(800), || {
            if let Ok(c) = config::global().read() {
                c.save();
            }
        })
    })
}

/// Give an icon-only widget an accessible *name*. A tooltip alone is exposed as
/// a description, so screen readers otherwise announce just "button". Pair this
/// with `set_tooltip_text` on every icon-only control.
pub(crate) fn a11y_label(w: &impl IsA<gtk::Accessible>, label: &str) {
    w.update_property(&[gtk::accessible::Property::Label(label)]);
}

/// qdata key holding a row's lowercased searchable text for the list filter.
const FILTER_KEY: &str = "bigtube-filter-key";

/// A collapsible list filter for a page header: a funnel icon button that, when
/// clicked, swaps to a small text entry (focused) to type into. It collapses
/// back to the icon on Escape, or when the entry is emptied and loses focus.
/// Returns the control widget (place it in the header) and the inner entry (wire
/// its `connect_search_changed` to drive the actual filtering).
pub(crate) fn make_filter_control() -> (gtk::Widget, gtk::SearchEntry) {
    let entry = gtk::SearchEntry::new();
    entry.set_placeholder_text(Some(&tr("Filter…")));
    entry.set_max_width_chars(14);
    entry.set_width_chars(10);
    entry.set_valign(gtk::Align::Center);
    a11y_label(&entry, &tr("Filter…"));

    let button = gtk::Button::from_icon_name("bigtube-filter-symbolic");
    button.add_css_class("flat");
    button.set_tooltip_text(Some(&tr("Filter")));
    a11y_label(&button, &tr("Filter"));

    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_valign(gtk::Align::Center);
    stack.add_named(&button, Some("icon"));
    stack.add_named(&entry, Some("entry"));
    stack.set_visible_child_name("icon");

    // Click the funnel → reveal the entry and focus it.
    {
        let stack = stack.clone();
        let entry = entry.clone();
        button.connect_clicked(move |_| {
            stack.set_visible_child_name("entry");
            entry.grab_focus();
        });
    }
    // Escape clears the filter and collapses back to the icon.
    {
        let stack = stack.clone();
        entry.connect_stop_search(move |e| {
            e.set_text("");
            stack.set_visible_child_name("icon");
        });
    }
    // Losing focus while empty collapses back to the icon; a non-empty filter
    // stays open so the active filter remains visible.
    {
        let stack = stack.clone();
        let entry_w = entry.clone();
        let focus = gtk::EventControllerFocus::new();
        focus.connect_leave(move |_| {
            if entry_w.text().is_empty() {
                stack.set_visible_child_name("icon");
            }
        });
        entry.add_controller(focus);
    }

    (stack.upcast(), entry)
}

/// Tag a `ListBox` child with the text the filter entry matches against. Stored
/// lowercased so the filter can do a case-insensitive substring test cheaply.
pub(crate) fn set_row_filter_key(w: &impl IsA<gtk::Widget>, text: &str) {
    unsafe {
        w.upcast_ref::<gtk::Widget>()
            .set_data::<String>(FILTER_KEY, text.to_lowercase());
    }
}

/// Read the filter key tagged onto a `ListBox` row's child (empty if none).
fn row_filter_key(row: &gtk::ListBoxRow) -> String {
    let Some(child) = row.child() else {
        return String::new();
    };
    unsafe {
        child
            .data::<String>(FILTER_KEY)
            .map(|p| p.as_ref().clone())
            .unwrap_or_default()
    }
}

/// Wire a filter entry to a `ListBox`: typing narrows the visible rows to those
/// whose tagged key contains the (lowercased) needle. Rows without a key always
/// show (e.g. nothing to hide before any text is entered).
pub(crate) fn wire_listbox_filter(entry: &gtk::SearchEntry, listbox: &gtk::ListBox) {
    let needle = Rc::new(RefCell::new(String::new()));
    let f_needle = needle.clone();
    listbox.set_filter_func(move |row| {
        let n = f_needle.borrow();
        n.is_empty() || row_filter_key(row).contains(n.as_str())
    });
    let listbox = listbox.clone();
    entry.connect_search_changed(move |e| {
        needle.replace(e.text().to_lowercase());
        listbox.invalidate_filter();
    });
}

/// Configured cap for the downloads history list (>= 1).
fn max_download_history() -> usize {
    config::global()
        .read()
        .map(|c| c.get_i64("max_download_history"))
        .unwrap_or(100)
        .max(1) as usize
}

/// Configured cap for the converter history list (>= 1).
fn max_converter_history() -> usize {
    config::global()
        .read()
        .map(|c| c.get_i64("max_converter_history"))
        .unwrap_or(50)
        .max(1) as usize
}

fn set_cfg(key: &str, value: serde_json::Value) {
    let changed = config::global()
        .write()
        .map(|mut c| c.set_mem(key, value))
        .unwrap_or(false);
    if changed {
        config_saver().touch();
    }
}

/// Delete the on-disk history / finished-item stores (NOT the config), used by
/// the "Clear All Data on Exit" setting. Mirrors `reset_all`'s data targets.
fn wipe_finished_data() {
    let dir = bigtube_core::paths::config_dir();
    for name in [
        "history.json",
        "search_history.json",
        "converter_history.json",
        "scheduled_downloads.json",
        "converter_pending.json",
        "playlist_cache.json",
    ] {
        let f = dir.join(name);
        if f.exists() {
            let _ = std::fs::remove_file(&f);
        }
    }
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
    // Persist any debounced config write before replacing the process image.
    config_saver().flush();
    let Ok(exe) = std::env::current_exe() else {
        std::process::exit(0);
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    // exec() only returns if it failed; otherwise it never comes back.
    let err = std::process::Command::new(exe).args(args).exec();
    tracing::error!("restart exec failed: {err}");
    std::process::exit(0);
}

// =============================================================================
// SCHEDULED (per-row editing)
// =============================================================================

// =============================================================================
// SHARED LIST / FILE HELPERS (used by both the downloads and converter pages)
// =============================================================================

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

fn open_containing_folder(state: &Rc<AppState>, path: &str) {
    if path.is_empty() {
        return;
    }
    let file = gtk::gio::File::for_path(path);
    let launcher = gtk::FileLauncher::new(Some(&file));
    let window = state.window.borrow().clone();
    launcher.open_containing_folder(window.as_ref(), gtk::gio::Cancellable::NONE, |_| {});
}

/// Path to the persisted converter-history file.
fn converter_history_path() -> std::path::PathBuf {
    bigtube_core::paths::config_dir().join("converter_history.json")
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

/// Translated label for a status code. The msgids match `locales.py`'s
/// `StringKey` values so the existing `.mo` catalogs resolve them.
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
