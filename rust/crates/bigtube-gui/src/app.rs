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
    footer: gtk::Box,
    btn_folder: gtk::Button,
    btn_play: gtk::Button,
    btn_convert: gtk::Button,
    file_path: Rc<RefCell<String>>,
    artist: Rc<RefCell<String>>,
    // Shared across clones so buttons and the Started handler see the same state.
    downloader: Rc<RefCell<Option<Arc<VideoDownloader>>>>,
    progress_fn: Rc<RefCell<Option<ProgressFn>>>,
    is_paused: Rc<Cell<bool>>,
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
        cancel.set_tooltip_text(Some(&tr("Cancel")));
        header.append(&title_lbl);
        header.append(&status);
        header.append(&pause);
        header.append(&cancel);

        // Destination path shown under the title.
        let path_lbl = gtk::Label::new(Some(file_path));
        path_lbl.set_xalign(0.0);
        path_lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        path_lbl.set_tooltip_text(Some(file_path));
        path_lbl.add_css_class("dim-label");
        path_lbl.add_css_class("caption");

        let progress = gtk::ProgressBar::new();
        progress.set_fraction(0.0);

        // Live transfer detail: "12.3MiB / 45.6MiB · 2.1MiB/s · ETA 00:15".
        let detail = gtk::Label::new(None);
        detail.set_xalign(0.0);
        detail.set_ellipsize(gtk::pango::EllipsizeMode::End);
        detail.add_css_class("dim-label");
        detail.add_css_class("caption");
        detail.set_visible(false);

        // Footer actions revealed on completion (open folder / play / convert).
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        footer.set_halign(gtk::Align::End);
        footer.set_visible(false);
        let btn_folder = gtk::Button::from_icon_name("folder-open-symbolic");
        btn_folder.add_css_class("flat");
        btn_folder.set_tooltip_text(Some(&tr("Open Folder")));
        let btn_play = gtk::Button::from_icon_name("media-playback-start-symbolic");
        btn_play.add_css_class("flat");
        btn_play.set_tooltip_text(Some(&tr("Play Video")));
        let btn_convert = gtk::Button::from_icon_name("emblem-synchronizing-symbolic");
        btn_convert.add_css_class("flat");
        btn_convert.set_tooltip_text(Some(&tr("Add to Converter")));
        footer.append(&btn_folder);
        footer.append(&btn_play);
        footer.append(&btn_convert);

        pad.append(&header);
        pad.append(&path_lbl);
        pad.append(&progress);
        pad.append(&detail);
        pad.append(&footer);
        container.append(&pad);

        let downloader: Rc<RefCell<Option<Arc<VideoDownloader>>>> = Rc::new(RefCell::new(None));
        let progress_fn: Rc<RefCell<Option<ProgressFn>>> = Rc::new(RefCell::new(None));
        let is_paused = Rc::new(Cell::new(false));

        let slot = downloader.clone();
        cancel.connect_clicked(move |_| {
            if let Some(d) = slot.borrow().as_ref() {
                d.cancel();
            }
        });

        // Pause / resume. Resume re-runs the (blocking) downloader on a thread.
        let dl = downloader.clone();
        let pf = progress_fn.clone();
        let paused = is_paused.clone();
        let pause_btn = pause.clone();
        pause.connect_clicked(move |_| {
            let Some(d) = dl.borrow().as_ref().cloned() else {
                return;
            };
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
            footer,
            btn_folder,
            btn_play,
            btn_convert,
            file_path: Rc::new(RefCell::new(file_path.to_string())),
            artist: Rc::new(RefCell::new(artist.to_string())),
            downloader,
            progress_fn,
            is_paused,
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
            self.set_progress_class("error");
            self.pause.set_sensitive(false);
            self.cancel.set_sensitive(false);
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
        self.footer.set_visible(exists);
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
    downloads_box: gtk::ListBox,
    downloads_stack: gtk::Stack,
    download_rows: RefCell<HashMap<String, DownloadRow>>,
    converter_box: gtk::ListBox,
    converter_stack: gtk::Stack,
    player: RefCell<Option<Rc<crate::player::Player>>>,
    busy_spinner: gtk::Spinner,
    busy_count: Cell<i32>,
    // Show the "enable cookies" guidance dialog only once per session.
    bot_block_hinted: Cell<bool>,
    ui_tx: async_channel::Sender<UiMsg>,
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
        downloads_box: downloads_box.clone(),
        downloads_stack: gtk::Stack::new(),
        download_rows: RefCell::new(HashMap::new()),
        converter_box: converter_box.clone(),
        converter_stack: gtk::Stack::new(),
        player: RefCell::new(None),
        busy_spinner: gtk::Spinner::new(),
        busy_count: Cell::new(0),
        bot_block_hinted: Cell::new(false),
        ui_tx,
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
                    if let Some(row) = state_for_loop.download_rows.borrow().get(&key) {
                        row.update(percent.as_deref(), status, detail.as_deref());
                    }
                    // Bot block — guide the user to enable cookies once.
                    if status == StatusCode::BotBlocked {
                        state_for_loop.notify_bot_block();
                    }
                }
                UiMsg::Started { key, downloader } => {
                    if let Some(row) = state_for_loop.download_rows.borrow().get(&key) {
                        row.downloader.replace(Some(downloader));
                    }
                }
            }
        }
    });

    // Restore persisted download / converter history into their lists.
    load_download_history(&state);
    load_converter_history(&state);

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
            crate::playlist::show(
                &win,
                item.url(),
                item.title(),
                player,
                on_download.clone(),
                on_download_all,
            );
        })
    };
    factory.connect_setup(move |_, list_item| {
        let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
        let row = SearchResultRow::new();
        row.set_handlers(
            on_play.clone(),
            on_download.clone(),
            on_open.clone(),
            on_copy.clone(),
        );
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

    let selection = gtk::SingleSelection::new(Some(state.search_store.clone()));
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
    batch.append(&select_all);
    batch.append(&state.select_btn);
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

    // Suggestion popover (search-history matches while typing).
    let popover = gtk::Popover::new();
    popover.set_parent(&entry);
    popover.set_autohide(false);
    popover.set_has_arrow(false);
    popover.set_position(gtk::PositionType::Bottom);
    popover.add_css_class("menu");
    let sugg_list = gtk::ListBox::new();
    sugg_list.set_selection_mode(gtk::SelectionMode::None);
    let sugg_scroll = gtk::ScrolledWindow::new();
    sugg_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    sugg_scroll.set_max_content_height(190);
    sugg_scroll.set_propagate_natural_height(true);
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
            *last_query.borrow_mut() = query.trim().to_string();
            let src = match source.selected() {
                1 => "youtube_music",
                2 => "url",
                _ => "youtube",
            }
            .to_string();
            run_search(&state, query, src);
        })
    };

    // Rebuild the suggestion list for the current text.
    let rebuild: Rc<dyn Fn(&str)> = {
        let sugg_list = sugg_list.clone();
        let popover = popover.clone();
        let entry = entry.clone();
        let trigger = trigger.clone();
        Rc::new(move |text: &str| {
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
                let pick = gtk::Button::new();
                pick.add_css_class("flat");
                pick.set_hexpand(true);
                // Don't steal focus from the entry (keeps the popover open on click).
                pick.set_can_focus(false);
                pick.set_focus_on_click(false);
                let inner = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                let icon = gtk::Image::from_icon_name("document-open-recent-symbolic");
                icon.add_css_class("dim-label");
                let lbl = gtk::Label::new(Some(&m));
                lbl.set_xalign(0.0);
                lbl.set_hexpand(true);
                lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
                inner.append(&icon);
                inner.append(&lbl);
                pick.set_child(Some(&inner));
                let del = gtk::Button::from_icon_name("window-close-symbolic");
                del.add_css_class("flat");
                del.add_css_class("circular");
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
                    let sugg_list = sugg_list.clone();
                    let popover = popover.clone();
                    let rowbox = rowbox.clone();
                    let q = m.clone();
                    del.connect_clicked(move |_| {
                        bigtube_core::search_history::SearchHistory::new(search_history_path())
                            .remove_item(&q);
                        sugg_list.remove(&rowbox);
                        if sugg_list.first_child().is_none() {
                            popover.popdown();
                        }
                    });
                }
            }
            // Match the popover width to the search entry so labels aren't clipped.
            let w = entry.width().max(320);
            popover.set_size_request(w, -1);
            popover.popup();
        })
    };

    // Typing clears stale results and refreshes suggestions.
    {
        let state = state.clone();
        let rebuild = rebuild.clone();
        let last_query = last_query.clone();
        entry.connect_search_changed(move |e| {
            let text = e.text().to_string();
            if text.trim() == *last_query.borrow() {
                return; // results we just loaded for this query — keep them
            }
            state.search_store.remove_all();
            state.update_search_empty();
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

const QUALITY_OPTIONS: [(&str, bigtube_core::enums::VideoQuality); 12] = {
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
fn detect_browsers() -> Vec<(&'static str, String)> {
    let candidates: [(&str, &str, &[&str]); 7] = [
        ("firefox", "Firefox", &["firefox"]),
        (
            "chrome",
            "Chrome",
            &["google-chrome", "google-chrome-stable"],
        ),
        ("chromium", "Chromium", &["chromium", "chromium-browser"]),
        ("brave", "Brave", &["brave", "brave-browser"]),
        (
            "edge",
            "Microsoft Edge",
            &["microsoft-edge", "microsoft-edge-stable"],
        ),
        ("vivaldi", "Vivaldi", &["vivaldi", "vivaldi-stable"]),
        ("opera", "Opera", &["opera"]),
    ];
    let mut out: Vec<(&str, String)> = vec![("", tr("None"))];
    for (val, label, cmds) in candidates {
        if cmds.iter().any(|c| in_path(c)) {
            out.push((val, label.to_string()));
        }
    }
    out
}

fn in_path(cmd: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|p| p.join(cmd).exists()))
        .unwrap_or(false)
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
            embed_subtitles: cfg.get_bool("embed_subtitles"),
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
    page.add(&build_storage_group(state, &c));
    page.add(&build_converter_group(state, &c));
    page.add(&build_search_group(state, &c));

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
    embed_subtitles: bool,
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

    // In-app preview/player quality. 360p is progressive (rock-solid); 480p/720p
    // stream via HLS. Takes effect on the next item played.
    let preview_row = combo_row(&tr("Preview Quality"), PREVIEW_QUALITIES);
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

    group.add(&spin_row(
        &tr("Max Simultaneous Downloads"),
        1.0,
        10.0,
        c.max_concurrent as f64,
        |v| set_cfg("max_concurrent_downloads", serde_json::json!(v as i64)),
    ));
    group.add(&spin_row(
        &tr("Concurrent Fragments"),
        1.0,
        16.0,
        c.concurrent_fragments as f64,
        |v| set_cfg("concurrent_fragments", serde_json::json!(v as i64)),
    ));
    group.add(&spin_row_step(
        &tr("Download Speed Limit (KB/s)"),
        0.0,
        100_000.0,
        100.0,
        c.rate_limit as f64,
        |v| set_cfg("rate_limit", serde_json::json!(v as i64)),
    ));
    group.add(&switch_row(
        &tr("Add Metadata to Files"),
        c.add_metadata,
        |v| set_cfg("add_metadata", serde_json::json!(v)),
    ));
    group.add(&switch_row(
        &tr("Embed Subtitles"),
        c.embed_subtitles,
        |v| set_cfg("embed_subtitles", serde_json::json!(v)),
    ));
    group.add(&switch_row(
        &tr("System Notifications"),
        c.system_notifications,
        |v| set_cfg("system_notifications", serde_json::json!(v)),
    ));
    group.add(&switch_row(
        &tr("Enable ClipBoard Monitor"),
        c.monitor_clipboard,
        |v| set_cfg("monitor_clipboard", serde_json::json!(v)),
    ));
    group.add(&entry_row(
        &tr("Post-Processing Command"),
        &c.post_process_cmd,
        |v| set_cfg("post_process_cmd", serde_json::json!(v)),
    ));

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

    group.add(&entry_row(&tr("User Agent"), &c.user_agent, |v| {
        set_cfg("user_agent", serde_json::json!(v))
    }));
    group.add(&entry_row(&tr("Proxy"), &c.proxy, |v| {
        set_cfg("proxy", serde_json::json!(v.trim()))
    }));

    group
}

fn build_storage_group(state: &Rc<AppState>, c: &Cfg) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Storage"))
        .build();

    group.add(&switch_row(
        &tr("Save Download History"),
        c.save_history,
        |v| set_cfg("save_history", serde_json::json!(v)),
    ));
    group.add(&switch_row(
        &tr("Clear All Data on Exit"),
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
        c.use_source_folder,
        |v| set_cfg("use_source_folder", serde_json::json!(v)),
    ));
    group.add(&switch_row(
        &tr("Save Conversion History"),
        c.save_converter_history,
        |v| set_cfg("save_converter_history", serde_json::json!(v)),
    ));

    group
}

fn build_search_group(state: &Rc<AppState>, c: &Cfg) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Search Settings"))
        .build();

    group.add(&switch_row(
        &tr("Save Search History"),
        c.save_search_history,
        |v| set_cfg("save_search_history", serde_json::json!(v)),
    ));
    group.add(&spin_row(
        &tr("Maximum Search Results"),
        5.0,
        100.0,
        c.search_limit as f64,
        |v| set_cfg("search_limit", serde_json::json!(v as i64)),
    ));
    group.add(&switch_row(
        &tr("Enable Search Suggestions"),
        c.enable_suggestions,
        |v| set_cfg("enable_suggestions", serde_json::json!(v)),
    ));
    group.add(&spin_row(
        &tr("Maximum Suggestions"),
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

fn switch_row(title: &str, active: bool, on_change: impl Fn(bool) + 'static) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder()
        .title(title)
        .active(active)
        .build();
    row.connect_active_notify(move |r| on_change(r.is_active()));
    row
}

fn spin_row(
    title: &str,
    min: f64,
    max: f64,
    value: f64,
    on_change: impl Fn(f64) + 'static,
) -> adw::SpinRow {
    spin_row_step(title, min, max, 1.0, value, on_change)
}

fn spin_row_step(
    title: &str,
    min: f64,
    max: f64,
    step: f64,
    value: f64,
    on_change: impl Fn(f64) + 'static,
) -> adw::SpinRow {
    let row = adw::SpinRow::with_range(min, max, step);
    row.set_title(title);
    row.set_value(value);
    row.connect_value_notify(move |r| on_change(r.value()));
    row
}

fn entry_row(title: &str, value: &str, on_apply: impl Fn(String) + 'static) -> adw::EntryRow {
    let row = adw::EntryRow::builder()
        .title(title)
        .text(value)
        .show_apply_button(true)
        .build();
    row.connect_apply(move |r| on_apply(r.text().to_string()));
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
    bigtube_core::search_history::SearchHistory::new(search_history_path()).clear();
    state.toast(&tr("History cleared successfully!"));
}

fn reset_all_data(state: &Rc<AppState>) {
    config::global().write().unwrap().reset_all();
    state.toast(&tr(
        "All application data has been cleared. The app will now restart.",
    ));
}

fn build_downloads_page(state: &Rc<AppState>) -> gtk::Widget {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Header with an icon "clear history" button.
    let clear = gtk::Button::from_icon_name("edit-clear-history-symbolic");
    clear.add_css_class("flat");
    clear.set_tooltip_text(Some(&tr("Clear History")));
    {
        let state = state.clone();
        clear.connect_clicked(move |_| {
            // Remove every finished (terminal) row from the visible list. This
            // covers both in-session rows and history-loaded rows (all tracked
            // in download_rows now).
            let mut rows = state.download_rows.borrow_mut();
            rows.retain(|_, row| {
                if !row.pause.is_sensitive() && !row.cancel.is_sensitive() {
                    remove_list_card(&state.downloads_box, &row.container);
                    false
                } else {
                    true
                }
            });
            drop(rows);
            // Wipe the saved download history so it doesn't reload on restart.
            bigtube_core::history::HistoryManager::new(history_path()).clear_all();
            state.update_downloads_empty();
        });
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
// CONVERTER
// =============================================================================

const VIDEO_FORMATS: [&str; 6] = ["mp4", "mkv", "webm", "mp3", "m4a", "wav"];
const AUDIO_FORMATS: [&str; 4] = ["mp3", "m4a", "wav", "flac"];

/// Output formats offered for a given source file, by media type (`converter_row.py`).
fn convert_formats_for(path: &std::path::Path) -> &'static [&'static str] {
    let is_audio = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_lowercase().as_str(),
                "mp3" | "m4a" | "wav" | "flac" | "ogg" | "opus" | "aac" | "wma"
            )
        })
        .unwrap_or(false);
    if is_audio {
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

fn build_converter_page(state: &Rc<AppState>) -> gtk::Widget {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Header with an icon "add files" button.
    let add = gtk::Button::from_icon_name("list-add-symbolic");
    add.add_css_class("flat");
    add.set_tooltip_text(Some(&tr("Add Files")));
    {
        let state = state.clone();
        add.connect_clicked(move |_| pick_files(&state));
    }
    let header = page_header(&tr("Converter Manager"), &[add]);

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
    out_path: Rc<RefCell<String>>,
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
    header.append(&name_lbl);
    header.append(&format);
    header.append(&convert);
    header.append(&cancel);
    header.append(&folder);
    header.append(&play);
    header.append(&remove);

    let status = gtk::Label::new(Some(tr("Ready").as_str()));
    status.set_xalign(0.0);
    status.add_css_class("dim-label");
    status.add_css_class("caption");
    let progress = gtk::ProgressBar::new();
    progress.set_fraction(0.0);

    pad.append(&header);
    pad.append(&progress);
    pad.append(&status);
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
        out_path: Rc::new(RefCell::new(String::new())),
    };

    // Remove this row from the list.
    {
        let state = state.clone();
        let container = container.clone();
        let source = path.to_string_lossy().to_string();
        remove.connect_clicked(move |_| {
            // If this file was converted, drop its history entries too so the row
            // doesn't reappear on restart.
            bigtube_core::converter_history::ConverterHistoryManager::new(converter_history_path())
                .remove_entry(&source, None);
            remove_list_card(&state.converter_box, &container);
            state.update_converter_empty();
        });
    }
    // Open the converted file's folder.
    {
        let state = state.clone();
        let out_path = ui.out_path.clone();
        folder.connect_clicked(move |_| open_containing_folder(&state, &out_path.borrow()));
    }
    // Play the converted file.
    {
        let state = state.clone();
        let out_path = ui.out_path.clone();
        play.connect_clicked(move |_| {
            let p = out_path.borrow().clone();
            let title = std::path::Path::new(&p)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if let Some(pl) = state.player.borrow().clone() {
                pl.play_local(&p, &title, "");
            }
        });
    }

    // Convert (with a cancel flag the cancel button flips).
    {
        let ui = ui.clone();
        let format = format.clone();
        let cancel = cancel.clone();
        convert.connect_clicked(move |btn| {
            let fmt = formats
                .get(format.selected() as usize)
                .copied()
                .unwrap_or("mp4")
                .to_string();
            let flag = Arc::new(AtomicBool::new(false));
            {
                let flag = flag.clone();
                cancel.connect_clicked(move |_| flag.store(true, Ordering::SeqCst));
            }
            btn.set_visible(false);
            ui.cancel.set_visible(true);
            ui.folder.set_visible(false);
            ui.play.set_visible(false);
            ui.set_progress_class("");
            run_conversion(path.clone(), fmt, ui.clone(), flag);
        });
    }
}

fn run_conversion(path: std::path::PathBuf, fmt: String, ui: ConvUi, cancel_flag: Arc<AtomicBool>) {
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
        let result = convert_media(&input, &fmt, Some(&cb), true, true, Some(&flag))
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
                    ui.out_path.replace(out.clone());
                    ui.folder.set_visible(true);
                    ui.play.set_visible(true);
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
                    ui.cancel.set_visible(false);
                    ui.convert.set_visible(true);
                    if cancel_flag.load(Ordering::SeqCst) {
                        ui.set_progress_class("warning");
                        ui.status.set_text(&status_label(StatusCode::Cancelled));
                    } else {
                        ui.set_progress_class("error");
                        ui.status.set_text(&format!("{}: {e}", tr("Error:")));
                    }
                }
            }
        }
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
    let state = state.clone();
    let last = Rc::new(RefCell::new(String::new()));

    glib::timeout_add_seconds_local(1, move || {
        // Respect the live setting; skip polling while disabled.
        if !config::global()
            .read()
            .unwrap()
            .get_bool("monitor_clipboard")
        {
            return glib::ControlFlow::Continue;
        }
        let state = state.clone();
        let last = last.clone();
        clipboard.read_text_async(gtk::gio::Cancellable::NONE, move |res| {
            if let Ok(Some(text)) = res {
                let text = text.to_string();
                if text != *last.borrow() && is_valid_url(&text) {
                    last.replace(text);
                    state.toast(&tr("Link detected! Paste in search to download."));
                }
            }
        });
        glib::ControlFlow::Continue
    });
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
                    state.toast(&e);
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
                enqueue_download(&st, &url, &title, &thumb, &uploader, &format_id, &ext);
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
                    Rc::new(move |ts: f64| {
                        enqueue_scheduled(
                            &st, &url, &title, &thumb, &uploader, &format_id, &ext, ts,
                        );
                    }),
                );
            })
        };
        dialog::show(&window, &info, audio_only, on_pick, on_schedule);
    });
}

/// File extension that pairs with a quality selector (audio/MKV/MP4).
fn quality_ext(q: bigtube_core::enums::VideoQuality) -> &'static str {
    use bigtube_core::enums::VideoQuality::*;
    match q {
        AudioMp3 => "mp3",
        AudioM4a => "m4a",
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
    let st = state.clone();
    show_quality_dialog(&window, move |q| {
        let sel = q.as_value().to_string();
        let ext = quality_ext(q);
        for o in &items {
            enqueue_download(
                &st,
                &o.url(),
                &o.title(),
                &o.thumbnail(),
                &o.uploader(),
                &sel,
                ext,
            );
        }
        st.toast(&tr("Added to downloads"));
    });
}

/// A single quality picker for batch downloads. Lists every quality (minus
/// "Ask Every Time"), defaulting to the configured preferred quality.
fn show_quality_dialog(
    parent: &adw::ApplicationWindow,
    on_pick: impl Fn(bigtube_core::enums::VideoQuality) + 'static,
) {
    use bigtube_core::enums::VideoQuality;
    let opts: Vec<(&str, VideoQuality)> = QUALITY_OPTIONS
        .iter()
        .copied()
        .filter(|(_, q)| !matches!(q, VideoQuality::Ask))
        .collect();
    let labels: Vec<String> = opts.iter().map(|(l, _)| tr(l)).collect();

    let win = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(tr("Select Quality"))
        .default_width(400)
        .build();
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    let dl_btn = gtk::Button::with_label(&tr("Download"));
    dl_btn.add_css_class("suggested-action");
    dl_btn.set_focus_on_click(false);
    header.pack_end(&dl_btn);
    toolbar.add_top_bar(&header);

    let group = adw::PreferencesGroup::new();
    group.set_margin_top(12);
    group.set_margin_bottom(12);
    group.set_margin_start(12);
    group.set_margin_end(12);
    let combo = combo_row(&tr("Preferred Quality"), &labels);
    let default_quality = config::global()
        .read()
        .unwrap()
        .get_string("default_quality");
    let sel = opts
        .iter()
        .position(|(_, q)| q.as_value() == default_quality)
        .unwrap_or(0);
    combo.set_selected(sel as u32);
    group.add(&combo);
    toolbar.set_content(Some(&group));
    win.set_content(Some(&toolbar));
    apply_theme_classes(&win);
    win.present();

    let on_pick = Rc::new(on_pick);
    dl_btn.connect_clicked(move |_| {
        if let Some((_, q)) = opts.get(combo.selected() as usize) {
            on_pick(*q);
        }
        win.close();
    });
}

/// Output file path the downloader will use (`{download}/{safe_title}.{ext}`).
fn output_path(title: &str, format_id: &str, ext: &str) -> String {
    let dir = config::global().read().unwrap().get_string("download_path");
    let mut safe = bigtube_core::validators::sanitize_filename(title, 200);
    if safe.is_empty() {
        safe = format!("video_{format_id}");
    }
    format!("{dir}/{safe}.{ext}")
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
    enqueue_common(state, url, title, thumbnail, uploader, format_id, ext, None);
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

/// Schedule a download for the Unix timestamp `ts` (seconds).
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
    );
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
) {
    let key = next_key();
    let file_path = output_path(title, format_id, ext);

    // Record a pending history entry up front (so it survives a crash mid-download).
    let save_history = config::global().read().unwrap().get_bool("save_history");
    if save_history {
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
    // as a toast.
    if let Some(ts) = schedule_ts {
        let msg = format!("{} {}", tr("Scheduled for:"), format_schedule_ts(ts));
        row.status.set_text(&msg);
        row.set_progress_class("warning");
        state.toast(&msg);
    }
    state.downloads_box.append(&row.container);
    state.download_rows.borrow_mut().insert(key.clone(), row);
    state.update_downloads_empty();
    state.stack.set_visible_child_name("downloads");

    // Capture the VideoDownloader when the task starts (for cancel/pause).
    let tx_started = state.ui_tx.clone();
    let k2 = key.clone();
    let on_start: OnStartFn = Arc::new(move |dl: Arc<VideoDownloader>| {
        let _ = tx_started.send_blocking(UiMsg::Started {
            key: k2.clone(),
            downloader: dl,
        });
    });

    let params = DownloadParams {
        url: url.to_string(),
        format_id: format_id.to_string(),
        title: title.to_string(),
        ext: ext.to_string(),
        force_overwrite: false,
        estimated_size_mb: None,
    };
    let mgr = download_manager::global();
    match schedule_ts {
        Some(ts) => {
            mgr.schedule_download(ts, params, cb, Some(on_start), 0, None);
        }
        None => {
            mgr.add_download(params, cb, Some(on_start), 0);
        }
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
        let fp = row.file_path.clone();
        let artist = row.artist.clone();
        row.btn_play.connect_clicked(move |_| {
            let path = fp.borrow().clone();
            let title = std::path::Path::new(&path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if let Some(p) = state.player.borrow().clone() {
                p.play_local(&path, &title, &artist.borrow());
            }
        });
    }
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

/// Load persisted download history into the Downloads list on startup.
fn load_download_history(state: &Rc<AppState>) {
    // Pure read (see load_converter_history): avoid the manager's drop-flush.
    let items: Vec<serde_json::Value> =
        bigtube_core::json_store::load_json(history_path(), Vec::new());
    for it in &items {
        let title = it
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Title");
        let fp = it.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
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
        row.footer.set_visible(exists);
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
        play.connect_clicked(move |_| {
            let title = std::path::Path::new(&out)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if let Some(pl) = state.player.borrow().clone() {
                pl.play_local(&out, &title, "");
            }
        });
    }
    {
        let state = state.clone();
        let container = container.clone();
        let source = source.to_string();
        let format = format.to_string();
        remove.connect_clicked(move |_| {
            // Also drop it from the saved history so it doesn't reload on restart.
            bigtube_core::converter_history::ConverterHistoryManager::new(converter_history_path())
                .remove_entry(&source, Some(&format));
            remove_list_card(&state.converter_box, &container);
            state.update_converter_empty();
        });
    }
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
