//! `VideoObject` — a `glib::Object` model for ListView rows, mirroring the
//! Python `VideoDataObject` (search_result_row.py).

use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use bigtube_core::search::SearchResult;

mod imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::VideoObject)]
    pub struct VideoObject {
        #[property(get, set)]
        pub title: RefCell<String>,
        #[property(get, set)]
        pub url: RefCell<String>,
        #[property(get, set)]
        pub thumbnail: RefCell<String>,
        #[property(get, set)]
        pub uploader: RefCell<String>,
        #[property(get, set)]
        pub is_video: Cell<bool>,
        #[property(get, set)]
        pub is_playlist: Cell<bool>,
        #[property(get, set)]
        pub playlist_count: Cell<i32>,
        #[property(get, set)]
        pub is_selected: Cell<bool>,
        #[property(get, set)]
        pub selection_mode: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VideoObject {
        const NAME: &'static str = "BigTubeVideoObject";
        type Type = super::VideoObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for VideoObject {}
}

glib::wrapper! {
    pub struct VideoObject(ObjectSubclass<imp::VideoObject>);
}

mod now_playing_imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::NowPlaying)]
    pub struct NowPlaying {
        /// URL/path of the item the player is currently on (empty = nothing).
        #[property(get, set)]
        pub url: RefCell<String>,
        /// True while the active track is actually playing (false = paused/stopped),
        /// so result rows can mirror the bar's play/pause glyph.
        #[property(get, set)]
        pub playing: Cell<bool>,
        /// Player-installed callback to toggle play/pause, so a row's play button
        /// can drive the player without a direct handle to it.
        pub toggle_cb: RefCell<Option<std::rc::Rc<dyn Fn()>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NowPlaying {
        const NAME: &'static str = "BigTubeNowPlaying";
        type Type = super::NowPlaying;
    }

    #[glib::derived_properties]
    impl ObjectImpl for NowPlaying {}
}

glib::wrapper! {
    /// A shared, observable "what's playing now" handle. The player writes its
    /// current URL here; result rows watch it to highlight the active track.
    pub struct NowPlaying(ObjectSubclass<now_playing_imp::NowPlaying>);
}

impl NowPlaying {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Install the player's play/pause toggle (called once by the player).
    pub fn set_toggle_cb(&self, f: std::rc::Rc<dyn Fn()>) {
        self.imp().toggle_cb.replace(Some(f));
    }

    /// Ask the player to toggle play/pause (no-op if no player is wired).
    pub fn request_toggle(&self) {
        let cb = self.imp().toggle_cb.borrow().clone();
        if let Some(cb) = cb {
            cb();
        }
    }
}

impl Default for NowPlaying {
    fn default() -> Self {
        Self::new()
    }
}

mod fav_watch_imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::FavoritesWatch)]
    pub struct FavoritesWatch {
        /// Monotonic revision, bumped whenever the favorites list changes.
        #[property(get, set)]
        pub rev: Cell<u64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FavoritesWatch {
        const NAME: &'static str = "BigTubeFavoritesWatch";
        type Type = super::FavoritesWatch;
    }

    #[glib::derived_properties]
    impl ObjectImpl for FavoritesWatch {}
}

glib::wrapper! {
    /// A shared, observable "favorites changed" handle. Any mutation bumps `rev`;
    /// rows and the player-bar heart watch it to re-query their starred state.
    pub struct FavoritesWatch(ObjectSubclass<fav_watch_imp::FavoritesWatch>);
}

impl FavoritesWatch {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Signal that the favorites list changed.
    pub fn bump(&self) {
        self.set_rev(self.rev().wrapping_add(1));
    }
}

impl Default for FavoritesWatch {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoObject {
    /// Build from a core `SearchResult`.
    pub fn from_result(r: &SearchResult) -> Self {
        glib::Object::builder()
            .property("title", &r.title)
            .property("url", &r.url)
            .property("thumbnail", &r.thumbnail)
            .property("uploader", &r.uploader)
            .property("is-video", r.is_video)
            .property("is-playlist", r.is_playlist)
            .property("playlist-count", r.playlist_count as i32)
            .build()
    }
}
