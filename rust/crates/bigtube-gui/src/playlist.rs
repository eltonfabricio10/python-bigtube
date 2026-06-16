//! Playlist dialog, mirroring `playlist_dialog.py`. Expands a playlist URL via
//! the core search engine and lists its videos (reusing `SearchResultRow`), with
//! Play-All, Download-All / Download-Selected, and a selection mode.
//!
//! Playing any item seeds the player queue with the whole playlist, so playback
//! continues cyclically through the list.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};

use bigtube_core::search::SearchEngine;

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
) {
    let win = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .default_width(560)
        .default_height(480)
        .title(&title)
        .build();
    crate::app::apply_theme_classes(&win);

    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    let play_all = gtk::Button::from_icon_name("media-playback-start-symbolic");
    play_all.set_focus_on_click(false);
    play_all.set_tooltip_text(Some(&tr("Play all")));
    let dl_all = gtk::Button::from_icon_name("folder-download-symbolic");
    dl_all.set_focus_on_click(false);
    dl_all.set_tooltip_text(Some(&tr("Download all")));
    let select_btn = gtk::ToggleButton::new();
    select_btn.set_icon_name("selection-mode-symbolic");
    select_btn.set_focus_on_click(false);
    select_btn.set_tooltip_text(Some(&tr("Select videos")));
    header.pack_start(&play_all);
    header.pack_start(&dl_all);
    header.pack_end(&select_btn);
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
    let factory = gtk::SignalListItemFactory::new();
    let on_download_row = on_download.clone();
    let now_playing = player.now_playing();
    factory.connect_setup(move |_, list_item| {
        let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
        let row = SearchResultRow::new();
        row.set_handlers(
            on_play.clone(),
            on_download_row.clone(),
            Rc::new(|_| {}),
            on_copy.clone(),
        );
        row.set_now_playing(now_playing.clone());
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
    let selection = gtk::SingleSelection::new(Some(store.clone()));
    let list = gtk::ListView::new(Some(selection), Some(factory));
    list.set_vexpand(true);
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_child(Some(&list));
    stack.add_named(&scrolled, Some("results"));

    // Empty / error.
    let status = adw::StatusPage::builder()
        .icon_name("dialog-information-symbolic")
        .build();
    stack.add_named(&status, Some("empty"));

    stack.set_visible_child_name("loading");
    toolbar.set_content(Some(&stack));
    win.set_content(Some(&toolbar));
    win.present();

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

    // Fetch the playlist contents off the main thread.
    let (tx, rx) =
        async_channel::bounded::<Result<Vec<bigtube_core::search::SearchResult>, String>>(1);
    std::thread::spawn(move || {
        let result = SearchEngine::new()
            .map_err(|e| e.to_string())
            .and_then(|eng| eng.expand_playlist(&url).map_err(|e| e.to_string()));
        let _ = tx.send_blocking(result);
    });

    glib::spawn_future_local(async move {
        match rx.recv().await {
            Ok(Ok(list)) => {
                for r in &list {
                    if r.is_playlist {
                        continue; // keep the list flat
                    }
                    store.append(&VideoObject::from_result(r));
                }
                if store.n_items() == 0 {
                    status.set_title(&tr("No results found!"));
                    stack.set_visible_child_name("empty");
                } else {
                    stack.set_visible_child_name("results");
                }
            }
            Ok(Err(e)) => {
                status.set_title(&e);
                stack.set_visible_child_name("empty");
            }
            Err(_) => {}
        }
    });
}
