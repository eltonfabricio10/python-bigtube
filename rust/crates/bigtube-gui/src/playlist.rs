//! Playlist dialog, mirroring `playlist_dialog.py`. Expands a playlist URL via
//! the core search engine and lists its videos (reusing `SearchResultRow`), with
//! Play-All, Download-All / Download-Selected, and a selection mode.
//!
//! Playing any item seeds the player queue with the whole playlist, so playback
//! continues cyclically through the list.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};

use bigtube_core::search::{SearchEngine, SearchResult};

use crate::i18n::tr;
use crate::objects::VideoObject;
use crate::player::{Player, QueueItem};
use crate::row::{RowAction, SearchResultRow};

/// Callback to download a whole batch of items at once (one quality dialog).
pub type BatchAction = Rc<dyn Fn(Vec<VideoObject>)>;

/// Build a playback queue from every video in `store`.
fn build_queue(store: &gio::ListStore) -> Vec<QueueItem> {
    let mut items = Vec::new();
    for i in 0..store.n_items() {
        if let Some(o) = store.item(i).and_then(|o| o.downcast::<VideoObject>().ok()) {
            if o.is_playlist() {
                continue;
            }
            items.push(QueueItem {
                url: o.url(),
                title: o.title(),
                artist: o.uploader(),
                thumbnail: o.thumbnail(),
                is_local: false,
                is_video: o.is_video(),
            });
        }
    }
    items
}

pub fn show(
    parent: &adw::ApplicationWindow,
    url: String,
    title: String,
    player: Rc<Player>,
    on_download: RowAction,
    on_download_all: BatchAction,
    on_schedule_all: BatchAction,
) {
    // Size proportional to the main window (clamped to sane bounds).
    let (win_w, win_h) = {
        let pw = parent.width();
        let ph = parent.height();
        if pw > 0 && ph > 0 {
            (
                ((pw as f64 * 0.62) as i32).clamp(480, 1000),
                ((ph as f64 * 0.82) as i32).clamp(420, 920),
            )
        } else {
            (560, 480)
        }
    };
    // Non-modal so the player bar in the main window stays usable while a
    // playlist is open.
    let win = adw::Window::builder()
        .transient_for(parent)
        .modal(false)
        .default_width(win_w)
        .default_height(win_h)
        .title(&title)
        .build();
    crate::app::apply_theme_classes(&win);

    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    let play_all = gtk::Button::from_icon_name("bigtube-media-playback-start-symbolic");
    play_all.set_focus_on_click(false);
    play_all.set_tooltip_text(Some(&tr("Play all")));
    crate::app::a11y_label(&play_all, &tr("Play all"));
    let dl_all = gtk::Button::from_icon_name("bigtube-download-symbolic");
    dl_all.set_focus_on_click(false);
    dl_all.set_tooltip_text(Some(&tr("Download all")));
    crate::app::a11y_label(&dl_all, &tr("Download all"));
    let sched_all = gtk::Button::from_icon_name("bigtube-alarm-symbolic");
    sched_all.set_focus_on_click(false);
    sched_all.set_tooltip_text(Some(&tr("Schedule all")));
    crate::app::a11y_label(&sched_all, &tr("Schedule all"));
    let fav_all = gtk::Button::from_icon_name("bigtube-emblem-favorite-symbolic");
    fav_all.set_focus_on_click(false);
    fav_all.set_tooltip_text(Some(&tr("Favorite all")));
    crate::app::a11y_label(&fav_all, &tr("Favorite all"));
    let select_btn = gtk::ToggleButton::new();
    select_btn.set_icon_name("bigtube-selection-mode-symbolic");
    select_btn.set_focus_on_click(false);
    select_btn.set_tooltip_text(Some(&tr("Select videos")));
    crate::app::a11y_label(&select_btn, &tr("Select videos"));
    header.pack_start(&play_all);
    header.pack_start(&dl_all);
    header.pack_start(&sched_all);
    header.pack_start(&fav_all);
    // select_btn + the filter control are packed at the end later, so the filter
    // can sit at the far-right corner (after the select button).
    toolbar.add_top_bar(&header);

    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);

    // Loading.
    let spinner_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
    spinner_box.set_valign(gtk::Align::Center);
    spinner_box.set_halign(gtk::Align::Center);
    spinner_box.set_vexpand(true);
    let spinner = gtk::Spinner::new();
    spinner.set_size_request(48, 48);
    spinner.start();
    spinner_box.append(&spinner);
    let loading = gtk::Label::new(Some(&tr("Loading playlist…")));
    loading.add_css_class("dim-label");
    spinner_box.append(&loading);
    stack.add_named(&spinner_box, Some("loading"));

    // Results.
    let store = gio::ListStore::new::<VideoObject>();
    let select_mode = Rc::new(Cell::new(false));

    // Play a clicked item, seeding the queue from the whole playlist.
    let on_play: RowAction = {
        let store = store.clone();
        let player = player.clone();
        Rc::new(move |item: VideoObject| {
            let items = build_queue(&store);
            let start = items.iter().position(|q| q.url == item.url()).unwrap_or(0);
            player.play_queue(items, start);
        })
    };

    let on_copy: RowAction = {
        let win = win.clone();
        Rc::new(move |item: VideoObject| {
            win.clipboard().set_text(&item.url());
        })
    };
    let on_download_row = on_download.clone();
    let now_playing = player.now_playing();
    // Favorite-all toggle: adds every video, or removes them all if they're
    // already favorited. The heart fills while the whole list is favorited and
    // stays in sync with per-row toggles (via the shared favorites watch) and as
    // the playlist finishes loading.
    {
        let collect = {
            let store = store.clone();
            move || -> Vec<VideoObject> {
                (0..store.n_items())
                    .filter_map(|i| store.item(i).and_then(|o| o.downcast::<VideoObject>().ok()))
                    .collect()
            }
        };
        let update_icon = {
            let btn = fav_all.clone();
            let collect = collect.clone();
            Rc::new(move || {
                let all = crate::app::favorites::videos_all_favorited(&collect());
                btn.set_icon_name(if all {
                    "bigtube-emblem-favorite-filled-symbolic"
                } else {
                    "bigtube-emblem-favorite-symbolic"
                });
            })
        };
        {
            let collect = collect.clone();
            let update_icon = update_icon.clone();
            fav_all.connect_clicked(move |_| {
                let objs = collect();
                if crate::app::favorites::videos_all_favorited(&objs) {
                    crate::app::favorites::remove_all(&objs);
                } else {
                    crate::app::favorites::add_all(&objs);
                }
                update_icon();
            });
        }
        // Refresh the heart when favorites change anywhere, and as rows load in.
        {
            let update_icon = update_icon.clone();
            let watch = crate::app::favorites::watch();
            let id = watch.connect_rev_notify(move |_| update_icon());
            // Disconnect from the long-lived watch when this window is gone, so
            // handlers don't pile up across playlist opens.
            let watch2 = watch.clone();
            let id = std::cell::RefCell::new(Some(id));
            win.connect_destroy(move |_| {
                if let Some(id) = id.borrow_mut().take() {
                    watch2.disconnect(id);
                }
            });
        }
        {
            let update_icon = update_icon.clone();
            store.connect_items_changed(move |_, _, _, _| update_icon());
        }
        update_icon();
    }
    // A local title filter narrows the playlist as you type (wraps the store;
    // selection wraps the filter so only matching rows show).
    let needle = Rc::new(RefCell::new(String::new()));
    let f_needle = needle.clone();
    let filter = gtk::CustomFilter::new(move |obj| {
        let n = f_needle.borrow();
        n.is_empty()
            || obj
                .downcast_ref::<VideoObject>()
                .map(|v| v.title().to_lowercase().contains(n.as_str()))
                .unwrap_or(true)
    });
    let filter_model = gtk::FilterListModel::new(Some(store.clone()), Some(filter.clone()));
    // A boxed-list ListBox gives the carded look of the favorites popover while
    // keeping the same SearchResultRow content (thumbnail, title, all actions,
    // selection). Rows act via their own buttons, so no selection highlight.
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("boxed-list");
    list.set_valign(gtk::Align::Start);
    list.set_margin_top(12);
    list.set_margin_bottom(12);
    list.set_margin_start(12);
    list.set_margin_end(12);
    {
        let on_play = on_play.clone();
        let on_download_row = on_download_row.clone();
        let on_copy = on_copy.clone();
        let now_playing = now_playing.clone();
        list.bind_model(Some(&filter_model), move |obj| {
            let video = obj.downcast_ref::<VideoObject>().unwrap();
            let row = SearchResultRow::new();
            row.set_handlers(
                on_play.clone(),
                on_download_row.clone(),
                Rc::new(|_| {}),
                on_copy.clone(),
            );
            let (fav_toggle, fav_query) = crate::app::favorites::make_handlers();
            row.set_favorite_handlers(fav_toggle, fav_query, crate::app::favorites::watch());
            row.set_now_playing(now_playing.clone());
            row.set_item(video);
            row.upcast::<gtk::Widget>()
        });
    }
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_child(Some(&list));
    // Collapsible filter pinned to the far-right of the header (after select).
    // Disabled until the playlist has loaded some videos to filter.
    let (filter_ctrl, filter_entry) = crate::app::make_filter_control();
    filter_ctrl.set_sensitive(false);
    filter_entry.connect_search_changed(move |e| {
        needle.replace(e.text().to_lowercase());
        filter.changed(gtk::FilterChange::Different);
    });
    header.pack_end(&filter_ctrl);
    header.pack_end(&select_btn);

    // Maximize toggle (icon flips to "restore" while maximized).
    let max_btn = gtk::Button::from_icon_name("bigtube-window-maximize-symbolic");
    max_btn.set_focus_on_click(false);
    max_btn.set_tooltip_text(Some(&tr("Maximize")));
    crate::app::a11y_label(&max_btn, &tr("Maximize"));
    {
        let w = win.clone();
        max_btn.connect_clicked(move |_| {
            if w.is_maximized() {
                w.unmaximize();
            } else {
                w.maximize();
            }
        });
    }
    {
        let b = max_btn.clone();
        win.connect_maximized_notify(move |w| {
            b.set_icon_name(if w.is_maximized() {
                "bigtube-window-restore-symbolic"
            } else {
                "bigtube-window-maximize-symbolic"
            });
        });
    }
    header.pack_end(&max_btn);
    stack.add_named(&scrolled, Some("results"));

    // Empty / error.
    let status = adw::StatusPage::builder()
        .icon_name("bigtube-dialog-information-symbolic")
        .build();
    stack.add_named(&status, Some("empty"));

    stack.set_visible_child_name("loading");
    toolbar.set_content(Some(&stack));
    win.set_content(Some(&toolbar));
    win.present();

    // Escape closes the playlist window (matches the player video window).
    {
        let w = win.clone();
        let key = gtk::EventControllerKey::new();
        key.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == gtk::gdk::Key::Escape {
                w.close();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        win.add_controller(key);
    }

    // Play All seeds the queue from the start.
    {
        let store = store.clone();
        let player = player.clone();
        play_all.connect_clicked(move |_| {
            let items = build_queue(&store);
            player.play_queue(items, 0);
        });
    }

    // Selection mode toggles checkboxes on every item.
    {
        let store = store.clone();
        let select_mode = select_mode.clone();
        select_btn.connect_toggled(move |b| {
            let on = b.is_active();
            select_mode.set(on);
            for i in 0..store.n_items() {
                if let Some(o) = store.item(i).and_then(|o| o.downcast::<VideoObject>().ok()) {
                    o.set_selection_mode(on);
                    if !on {
                        o.set_is_selected(false);
                    }
                }
            }
        });
    }

    // Download: selected items in selection mode, else everything — collected
    // into ONE batch so a single quality dialog covers the whole list.
    {
        let store = store.clone();
        let select_mode = select_mode.clone();
        dl_all.connect_clicked(move |_| {
            let only_selected = select_mode.get()
                && (0..store.n_items()).any(|i| {
                    store
                        .item(i)
                        .and_then(|o| o.downcast::<VideoObject>().ok())
                        .map(|o| o.is_selected())
                        .unwrap_or(false)
                });
            let mut picked = Vec::new();
            for i in 0..store.n_items() {
                if let Some(o) = store.item(i).and_then(|o| o.downcast::<VideoObject>().ok()) {
                    if !only_selected || o.is_selected() {
                        picked.push(o);
                    }
                }
            }
            on_download_all(picked);
        });
    }

    // Schedule: same collection as download, routed to the schedule dialog.
    {
        let store = store.clone();
        let select_mode = select_mode.clone();
        sched_all.connect_clicked(move |_| {
            let only_selected = select_mode.get()
                && (0..store.n_items()).any(|i| {
                    store
                        .item(i)
                        .and_then(|o| o.downcast::<VideoObject>().ok())
                        .map(|o| o.is_selected())
                        .unwrap_or(false)
                });
            let mut picked = Vec::new();
            for i in 0..store.n_items() {
                if let Some(o) = store.item(i).and_then(|o| o.downcast::<VideoObject>().ok()) {
                    if !only_selected || o.is_selected() {
                        picked.push(o);
                    }
                }
            }
            on_schedule_all(picked);
        });
    }

    // Show any cached contents instantly so reopening a playlist is immediate;
    // the live fetch below still runs and refreshes the list when it returns.
    if let Some(cached) = cache_get(&url) {
        populate(&store, &cached);
        if store.n_items() > 0 {
            stack.set_visible_child_name("results");
            filter_ctrl.set_sensitive(true);
        }
    }

    // Fetch the playlist contents off the main thread.
    let url_cache = url.clone();
    let filter_ctrl = filter_ctrl.clone();
    let (tx, rx) = async_channel::bounded::<Result<Vec<SearchResult>, String>>(1);
    std::thread::spawn(move || {
        let result = SearchEngine::new()
            .map_err(|e| e.to_string())
            .and_then(|eng| eng.expand_playlist(&url).map_err(|e| e.to_string()));
        let _ = tx.send_blocking(result);
    });

    glib::spawn_future_local(async move {
        match rx.recv().await {
            Ok(Ok(list)) => {
                // Replace whatever is shown (cache or empty) with the fresh data,
                // then update the cache for next time.
                populate(&store, &list);
                if store.n_items() == 0 {
                    status.set_title(&tr("No results found!"));
                    stack.set_visible_child_name("empty");
                } else {
                    stack.set_visible_child_name("results");
                }
                filter_ctrl.set_sensitive(store.n_items() > 0);
                cache_put(&url_cache, &list);
            }
            Ok(Err(e)) => {
                // If cached contents are already on screen, keep them rather than
                // wiping a usable list because the refresh failed.
                if store.n_items() > 0 {
                    tracing::warn!("playlist refresh failed, keeping cached list: {e}");
                    return;
                }
                // Friendly title; show the raw error as the (smaller) description
                // and log it, instead of using a cryptic error as the heading.
                tracing::error!("playlist load failed: {e}");
                status.set_title(&tr("Couldn't load this playlist"));
                status.set_description(Some(&e));
                stack.set_visible_child_name("empty");
            }
            Err(_) => {}
        }
    });
}

/// Fill `store` with the videos in `list` (replacing its contents and keeping
/// the list flat — nested playlist entries are skipped).
fn populate(store: &gio::ListStore, list: &[SearchResult]) {
    store.remove_all();
    for r in list {
        if r.is_playlist {
            continue;
        }
        store.append(&VideoObject::from_result(r));
    }
}

/// On-disk cache of expanded playlists: `{ url: [SearchResult, …] }`. Lets a
/// reopened playlist render instantly while the live fetch refreshes it.
fn cache_path() -> PathBuf {
    bigtube_core::paths::config_dir().join("playlist_cache.json")
}

/// Hard cap on how many expanded playlists we keep on disk so the cache can't
/// grow without bound (each entry can hold hundreds of items). The cache is a
/// pure optimization — a miss just re-fetches — so over-cap entries are simply
/// evicted on write.
const CACHE_MAX_ENTRIES: usize = 30;

fn load_cache() -> HashMap<String, Vec<SearchResult>> {
    bigtube_core::json_store::load_json(cache_path(), HashMap::new())
}

fn cache_get(url: &str) -> Option<Vec<SearchResult>> {
    load_cache().remove(url).filter(|v| !v.is_empty())
}

fn cache_put(url: &str, list: &[SearchResult]) {
    let mut map = load_cache();
    map.insert(url.to_string(), list.to_vec());
    // Keep the just-written entry; drop others until within the cap.
    if map.len() > CACHE_MAX_ENTRIES {
        let victims: Vec<String> = map
            .keys()
            .filter(|k| k.as_str() != url)
            .take(map.len() - CACHE_MAX_ENTRIES)
            .cloned()
            .collect();
        for k in victims {
            map.remove(&k);
        }
    }
    // Atomic + locked write through json_store, like every other data file.
    bigtube_core::json_store::save_json(cache_path(), &map, None);
}
