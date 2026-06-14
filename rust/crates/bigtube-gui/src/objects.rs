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
