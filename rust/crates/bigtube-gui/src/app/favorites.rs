//! Favorites: a single persisted list of starred tracks, surfaced through a
//! heart toggle on result/playlist rows and a modal opened from the player bar.
//! The modal lists the favorites and lets the user play, remove, or clear them.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use bigtube_core::favorites::{FavoriteItem, Favorites};

use crate::i18n::tr;
use crate::objects::{FavoritesWatch, VideoObject};
use crate::player::{Player, QueueItem};
use crate::row::{FavQuery, FavToggle};

thread_local! {
    /// Process-wide "favorites changed" observable (main thread only).
    static WATCH: FavoritesWatch = FavoritesWatch::new();
}

/// Clone of the shared favorites-changed observable.
pub(crate) fn watch() -> FavoritesWatch {
    WATCH.with(|w| w.clone())
}

/// Bump the observable so every heart re-queries its state.
fn notify_changed() {
    WATCH.with(|w| w.bump());
}

/// On-disk favorites file (`~/.config/bigtube/favorites.json`).
pub(crate) fn favorites_path() -> std::path::PathBuf {
    bigtube_core::paths::config_dir().join("favorites.json")
}

/// A fresh handle to the favorites store (cheap — it only holds a path and reads
/// disk per operation, so multiple handles stay consistent).
pub(crate) fn favorites() -> Favorites {
    Favorites::new(favorites_path())
}

/// Build a `FavoriteItem` from a search/playlist row's model object.
fn item_from(obj: &VideoObject) -> FavoriteItem {
    FavoriteItem {
        url: obj.url(),
        title: obj.title(),
        uploader: obj.uploader(),
        thumbnail: obj.thumbnail(),
        is_video: obj.is_video(),
        is_local: false,
        added: 0,
    }
}

/// Build a `FavoriteItem` for a downloaded local file (title from the file
/// stem, video-ness from its extension).
pub(crate) fn local_item(path: &str, artist: &str) -> FavoriteItem {
    let p = std::path::Path::new(path);
    let title = p
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let is_video = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            !matches!(
                e.to_lowercase().as_str(),
                "mp3" | "m4a" | "wav" | "flac" | "ogg" | "opus" | "aac" | "wma"
            )
        })
        .unwrap_or(true);
    FavoriteItem {
        url: path.to_string(),
        title,
        uploader: artist.to_string(),
        thumbnail: String::new(),
        is_video,
        is_local: true,
        added: 0,
    }
}

/// Set a heart button's glyph + tooltip to reflect favorited state.
pub(crate) fn set_heart_icon(btn: &gtk::Button, favorited: bool) {
    btn.set_icon_name(if favorited {
        "bigtube-emblem-favorite-filled-symbolic"
    } else {
        "bigtube-emblem-favorite-symbolic"
    });
    btn.set_tooltip_text(Some(&if favorited {
        tr("Remove from Favorites")
    } else {
        tr("Add to Favorites")
    }));
}

/// Toggle + query handlers to wire into a `SearchResultRow`'s heart button.
pub(crate) fn make_handlers() -> (FavToggle, FavQuery) {
    let toggle: FavToggle = Rc::new(|obj: VideoObject| {
        let now = favorites().toggle(item_from(&obj));
        notify_changed();
        now
    });
    let query: FavQuery = Rc::new(|url: &str| favorites().contains(url));
    (toggle, query)
}

/// Toggle a downloaded local file's favorite state; returns the new state.
pub(crate) fn toggle_local(path: &str, artist: &str) -> bool {
    let now = favorites().toggle(local_item(path, artist));
    notify_changed();
    now
}

/// Favorite every (non-playlist) item in `objs`; returns how many were newly
/// added. Used by the playlist "favorite all" header button.
pub(crate) fn add_all(objs: &[VideoObject]) -> usize {
    let favs = favorites();
    let mut added = 0;
    for obj in objs {
        if obj.is_playlist() {
            continue;
        }
        if favs.add(item_from(obj)) {
            added += 1;
        }
    }
    if added > 0 {
        notify_changed();
    }
    added
}

/// Remove every (non-playlist) item in `objs` from favorites.
pub(crate) fn remove_all(objs: &[VideoObject]) {
    let favs = favorites();
    let mut removed = false;
    for obj in objs {
        if obj.is_playlist() {
            continue;
        }
        if favs.contains(&obj.url()) {
            favs.remove(&obj.url());
            removed = true;
        }
    }
    if removed {
        notify_changed();
    }
}

/// Whether every (non-playlist) item in `objs` is already favorited. False when
/// there are no video items at all.
pub(crate) fn videos_all_favorited(objs: &[VideoObject]) -> bool {
    let favs = favorites();
    let mut any = false;
    for obj in objs {
        if obj.is_playlist() {
            continue;
        }
        any = true;
        if !favs.contains(&obj.url()) {
            return false;
        }
    }
    any
}

/// Map favorites to a playback queue.
fn to_queue(items: &[FavoriteItem]) -> Vec<QueueItem> {
    items
        .iter()
        .map(|f| QueueItem {
            url: f.url.clone(),
            title: f.title.clone(),
            artist: f.uploader.clone(),
            thumbnail: f.thumbnail.clone(),
            is_local: f.is_local,
            is_video: f.is_video,
        })
        .collect()
}

/// Open the favorites list as a popover (balloon) anchored to `anchor` (the
/// player-bar heart button), playing through `player`.
pub(crate) fn open_popover(anchor: &impl IsA<gtk::Widget>, player: &Rc<Player>) {
    let pop = gtk::Popover::new();
    pop.set_parent(anchor);
    pop.set_autohide(true);
    pop.add_css_class("menu");
    // Pop up above the bar button and bias toward the right edge instead of
    // centering over the (right-side) anchor.
    pop.set_position(gtk::PositionType::Top);
    pop.set_halign(gtk::Align::End);
    pop.set_offset(40, 0);
    crate::app::apply_theme_classes(&pop);
    // Free the popover (and its parent link) once dismissed, so each open starts
    // fresh and we don't leak hidden popovers under the button.
    pop.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    let now_playing = player.now_playing();
    let root_win = anchor
        .as_ref()
        .root()
        .and_then(|r| r.downcast::<gtk::Window>().ok());

    // Compact header: title + play-all + clear-all.
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    header.set_margin_start(6);
    header.set_margin_end(6);
    header.set_margin_top(4);
    header.set_margin_bottom(2);
    let title = gtk::Label::new(Some(&tr("Favorites")));
    title.add_css_class("heading");
    title.set_hexpand(true);
    title.set_xalign(0.0);
    let play_all = gtk::Button::from_icon_name("bigtube-media-playback-start-symbolic");
    play_all.add_css_class("flat");
    play_all.set_focus_on_click(false);
    play_all.set_tooltip_text(Some(&tr("Play all")));
    crate::app::a11y_label(&play_all, &tr("Play all"));
    let clear_all = gtk::Button::from_icon_name("bigtube-user-trash-symbolic");
    clear_all.add_css_class("flat");
    clear_all.set_focus_on_click(false);
    clear_all.set_tooltip_text(Some(&tr("Clear favorites")));
    crate::app::a11y_label(&clear_all, &tr("Clear favorites"));
    header.append(&title);
    header.append(&play_all);
    header.append(&clear_all);

    // A stack toggles between the list and an empty state.
    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("boxed-list");
    list.set_valign(gtk::Align::Start);
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_propagate_natural_height(true);
    scrolled.set_min_content_width(400);
    scrolled.set_max_content_height(380);
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_child(Some(&list));
    stack.add_named(&scrolled, Some("list"));

    let empty = gtk::Label::new(Some(&tr("No favorites yet")));
    empty.add_css_class("dim-label");
    empty.set_margin_top(18);
    empty.set_margin_bottom(18);
    empty.set_margin_start(24);
    empty.set_margin_end(24);
    stack.add_named(&empty, Some("empty"));

    let root_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    root_box.append(&header);
    root_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    root_box.append(&stack);
    pop.set_child(Some(&root_box));

    // Switch to the empty state when the last row is removed.
    let show_empty_if_needed: Rc<dyn Fn()> = {
        let list = list.clone();
        let stack = stack.clone();
        Rc::new(move || {
            if list.first_child().is_none() {
                stack.set_visible_child_name("empty");
            }
        })
    };

    // Track the now-playing signal handlers so they can be disconnected when the
    // popover closes (it's recreated on each open — don't pile up handlers on the
    // long-lived NowPlaying object).
    let np_handlers: Rc<std::cell::RefCell<Vec<glib::SignalHandlerId>>> =
        Rc::new(std::cell::RefCell::new(Vec::new()));

    // Populate from disk.
    let items = favorites().list();
    if items.is_empty() {
        stack.set_visible_child_name("empty");
    } else {
        stack.set_visible_child_name("list");
        let current = now_playing.url();
        for fav in &items {
            let lbr = build_fav_row(fav);
            // Highlight the row that's playing now, and keep it in sync as the
            // track changes.
            if !current.is_empty() && current == fav.url {
                lbr.row.add_css_class("playing");
            }
            {
                let row = lbr.row.clone();
                let url = fav.url.clone();
                let id = now_playing.connect_url_notify(move |np| {
                    let cur = np.url();
                    if !cur.is_empty() && cur == url {
                        row.add_css_class("playing");
                    } else {
                        row.remove_css_class("playing");
                    }
                });
                np_handlers.borrow_mut().push(id);
            }
            // Play from this item (resolve its index in the live list at click
            // time, since removals shift positions), then dismiss the popover.
            {
                let player = player.clone();
                let url = fav.url.clone();
                let pop = pop.clone();
                lbr.on_play.connect_clicked(move |_| {
                    let items = favorites().list();
                    let start = items.iter().position(|f| f.url == url).unwrap_or(0);
                    player.play_queue(to_queue(&items), start);
                    pop.popdown();
                });
            }
            // Remove just this row, in place.
            {
                let url = fav.url.clone();
                let list = list.clone();
                let row = lbr.row.clone();
                let show_empty = show_empty_if_needed.clone();
                lbr.on_remove.connect_clicked(move |_| {
                    favorites().remove(&url);
                    notify_changed();
                    list.remove(&row);
                    show_empty();
                });
            }
            list.append(&lbr.row);
        }
    }

    // Play all, then dismiss.
    {
        let player = player.clone();
        let pop = pop.clone();
        play_all.connect_clicked(move |_| {
            let items = favorites().list();
            if !items.is_empty() {
                player.play_queue(to_queue(&items), 0);
                pop.popdown();
            }
        });
    }
    // Clear all (with confirmation). The confirm dialog is parented to the main
    // window; opening it dismisses the autohiding popover.
    {
        let pop = pop.clone();
        clear_all.connect_clicked(move |_| {
            if favorites().list().is_empty() {
                return;
            }
            pop.popdown();
            let dialog = adw::MessageDialog::new(
                root_win.as_ref(),
                Some(&tr("Clear favorites?")),
                Some(&tr("This removes every item from your favorites.")),
            );
            dialog.add_response("cancel", &tr("Cancel"));
            dialog.add_response("clear", &tr("Clear"));
            dialog.set_response_appearance("clear", adw::ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");
            crate::app::apply_theme_classes(&dialog);
            dialog.connect_response(None, move |dlg, resp| {
                if resp == "clear" {
                    favorites().clear();
                    notify_changed();
                }
                dlg.close();
            });
            dialog.present();
        });
    }

    // Disconnect the now-playing watchers when the popover is dismissed.
    {
        let np = now_playing.clone();
        let ids = np_handlers.clone();
        pop.connect_closed(move |_| {
            for id in ids.borrow_mut().drain(..) {
                np.disconnect(id);
            }
        });
    }

    pop.popup();
}

/// One favorites row (a `ListBoxRow`) with play + remove buttons exposed.
struct FavRow {
    row: gtk::ListBoxRow,
    on_play: gtk::Button,
    on_remove: gtk::Button,
}

fn build_fav_row(fav: &FavoriteItem) -> FavRow {
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    container.set_margin_top(6);
    container.set_margin_bottom(6);
    container.set_margin_start(6);
    container.set_margin_end(6);

    let thumb = gtk::Image::from_icon_name(if fav.is_video {
        "bigtube-video-x-generic-symbolic"
    } else {
        "bigtube-audio-x-generic-symbolic"
    });
    thumb.set_pixel_size(40);
    thumb.set_size_request(72, 40);
    if !fav.thumbnail.is_empty() {
        load_thumb(&thumb, &fav.thumbnail);
    }

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
    vbox.set_hexpand(true);
    vbox.set_valign(gtk::Align::Center);
    let title = gtk::Label::new(Some(&if fav.title.is_empty() {
        tr("Unknown Title")
    } else {
        fav.title.clone()
    }));
    title.set_xalign(0.0);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.add_css_class("heading");
    let uploader = gtk::Label::new(Some(&if fav.uploader.is_empty() {
        tr("Unknown Artist")
    } else {
        fav.uploader.clone()
    }));
    uploader.set_xalign(0.0);
    uploader.set_ellipsize(gtk::pango::EllipsizeMode::End);
    uploader.add_css_class("dim-label");
    uploader.add_css_class("caption");
    vbox.append(&title);
    vbox.append(&uploader);

    let on_play = gtk::Button::from_icon_name("bigtube-media-playback-start-symbolic");
    on_play.add_css_class("flat");
    on_play.set_valign(gtk::Align::Center);
    on_play.set_focus_on_click(false);
    on_play.set_tooltip_text(Some(&tr("Play Video")));
    let on_remove = gtk::Button::from_icon_name("bigtube-emblem-favorite-filled-symbolic");
    on_remove.add_css_class("flat");
    on_remove.set_valign(gtk::Align::Center);
    on_remove.set_focus_on_click(false);
    on_remove.set_tooltip_text(Some(&tr("Remove from Favorites")));

    container.append(&thumb);
    container.append(&vbox);
    container.append(&on_play);
    container.append(&on_remove);

    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_child(Some(&container));

    FavRow {
        row,
        on_play,
        on_remove,
    }
}

/// Asynchronously load a thumbnail into `img` (memory/disk cached via row::fetch).
fn load_thumb(img: &gtk::Image, url: &str) {
    let (tx, rx) = async_channel::bounded::<Option<Vec<u8>>>(1);
    let url = url.to_string();
    std::thread::spawn(move || {
        let _ = tx.send_blocking(crate::row::fetch_bytes(&url));
    });
    let img = img.clone();
    glib::spawn_future_local(async move {
        let Ok(Some(bytes)) = rx.recv().await else {
            return;
        };
        if let Some(tex) = crate::row::decode_texture_sized(&bytes, 72, 40) {
            img.set_paintable(Some(&tex));
        }
    });
}
