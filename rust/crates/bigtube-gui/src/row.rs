//! `SearchResultRow` — a composite widget for one search result, mirroring
//! `search_result_row.py`. Built in Rust (no .ui) as a `gtk::Box` subclass.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::i18n::tr;
use crate::objects::{FavoritesWatch, NowPlaying, VideoObject};

/// Cap the in-memory texture cache so long browsing sessions don't grow it
/// without bound (each decoded thumbnail holds GPU memory). Thumbnails are
/// cheap to re-decode from the on-disk cache, so a simple FIFO eviction is fine.
const THUMB_CACHE_CAP: usize = 256;

#[derive(Default)]
struct ThumbCache {
    map: HashMap<String, gtk::gdk::Texture>,
    order: std::collections::VecDeque<String>,
}

impl ThumbCache {
    fn get(&self, url: &str) -> Option<gtk::gdk::Texture> {
        self.map.get(url).cloned()
    }
    fn insert(&mut self, url: String, tex: gtk::gdk::Texture) {
        if self.map.insert(url.clone(), tex).is_none() {
            self.order.push_back(url);
            while self.order.len() > THUMB_CACHE_CAP {
                if let Some(old) = self.order.pop_front() {
                    self.map.remove(&old);
                }
            }
        }
    }
}

thread_local! {
    /// Main-thread texture cache so scrolling/rebinding doesn't re-download.
    static THUMB_CACHE: RefCell<ThumbCache> = RefCell::new(ThumbCache::default());
}

/// Shared callback type for the row's action buttons.
pub type RowAction = Rc<dyn Fn(VideoObject)>;
/// Toggle a favorite; returns the new state (true = now favorited).
pub type FavToggle = Rc<dyn Fn(VideoObject) -> bool>;
/// Query whether a URL is currently favorited.
pub type FavQuery = Rc<dyn Fn(&str) -> bool>;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct SearchResultRow {
        pub checkbox: OnceCell<gtk::CheckButton>,
        pub thumb: OnceCell<gtk::Image>,
        pub title: OnceCell<gtk::Label>,
        pub channel: OnceCell<gtk::Label>,
        pub btn_play: OnceCell<gtk::Button>,
        pub btn_download: OnceCell<gtk::Button>,
        pub btn_open: OnceCell<gtk::Button>,
        pub btn_copy: OnceCell<gtk::Button>,
        pub btn_favorite: OnceCell<gtk::Button>,
        pub item: RefCell<Option<VideoObject>>,
        pub on_play: RefCell<Option<RowAction>>,
        pub on_download: RefCell<Option<RowAction>>,
        pub on_open: RefCell<Option<RowAction>>,
        pub on_copy: RefCell<Option<RowAction>>,
        pub on_favorite: RefCell<Option<FavToggle>>,
        pub is_favorite: RefCell<Option<FavQuery>>,
        pub fav_watch: OnceCell<FavoritesWatch>,
        pub thumb_gen: Cell<u64>,
        // Property bindings for the current item, cleared on rebind.
        pub bindings: RefCell<Vec<glib::Binding>>,
        // Shared "current track" handle; the row highlights itself when its item
        // matches the one playing.
        pub now_playing: OnceCell<NowPlaying>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SearchResultRow {
        const NAME: &'static str = "BigTubeSearchResultRow";
        type Type = super::SearchResultRow;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for SearchResultRow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_orientation(gtk::Orientation::Horizontal);
            obj.set_spacing(12);
            obj.set_margin_top(6);
            obj.set_margin_bottom(6);
            obj.set_margin_start(6);
            obj.set_margin_end(6);
            // Inner padding so the now-playing highlight floats around the content
            // instead of being glued to the checkbox/thumbnail edge.
            obj.add_css_class("result-row");

            // Selection checkbox (hidden unless selection mode is active).
            let checkbox = gtk::CheckButton::new();
            checkbox.set_valign(gtk::Align::Center);
            checkbox.set_visible(false);

            let thumb = gtk::Image::from_icon_name("bigtube-video-x-generic-symbolic");
            thumb.set_pixel_size(48);
            thumb.set_size_request(80, 45);

            let vbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
            vbox.set_hexpand(true);
            vbox.set_valign(gtk::Align::Center);
            let title = gtk::Label::new(None);
            title.set_xalign(0.0);
            title.set_wrap(true);
            title.set_lines(2);
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            title.add_css_class("heading");
            let channel = gtk::Label::new(None);
            channel.set_xalign(0.0);
            channel.add_css_class("dim-label");
            channel.add_css_class("caption");
            vbox.append(&title);
            vbox.append(&channel);

            let btn_play = gtk::Button::from_icon_name("bigtube-media-playback-start-symbolic");
            btn_play.add_css_class("flat");
            btn_play.set_focus_on_click(false);
            btn_play.set_valign(gtk::Align::Center);
            btn_play.set_tooltip_text(Some(&tr("Play Video")));
            crate::app::a11y_label(&btn_play, &tr("Play Video"));
            let btn_download = gtk::Button::from_icon_name("bigtube-download-symbolic");
            btn_download.add_css_class("flat");
            btn_download.set_focus_on_click(false);
            btn_download.set_valign(gtk::Align::Center);
            btn_download.set_tooltip_text(Some(&tr("Download")));
            crate::app::a11y_label(&btn_download, &tr("Download"));
            let btn_open = gtk::Button::from_icon_name("bigtube-folder-open-symbolic");
            btn_open.add_css_class("flat");
            btn_open.set_focus_on_click(false);
            btn_open.set_valign(gtk::Align::Center);
            btn_open.set_tooltip_text(Some(&tr("Open playlist")));
            crate::app::a11y_label(&btn_open, &tr("Open playlist"));
            btn_open.set_visible(false);
            let btn_copy = gtk::Button::from_icon_name("bigtube-edit-copy-symbolic");
            btn_copy.add_css_class("flat");
            btn_copy.set_focus_on_click(false);
            btn_copy.set_valign(gtk::Align::Center);
            btn_copy.set_tooltip_text(Some(&tr("Copy Link")));
            crate::app::a11y_label(&btn_copy, &tr("Copy Link"));
            // Favorite toggle (hidden until favorite handlers are installed; the
            // icon flips between an outline and a filled heart per state).
            let btn_favorite = gtk::Button::from_icon_name("bigtube-emblem-favorite-symbolic");
            btn_favorite.add_css_class("flat");
            btn_favorite.set_focus_on_click(false);
            btn_favorite.set_valign(gtk::Align::Center);
            btn_favorite.set_tooltip_text(Some(&tr("Add to Favorites")));
            crate::app::a11y_label(&btn_favorite, &tr("Add to Favorites"));
            btn_favorite.set_visible(false);

            obj.append(&checkbox);
            obj.append(&thumb);
            obj.append(&vbox);
            obj.append(&btn_favorite);
            obj.append(&btn_play);
            obj.append(&btn_download);
            obj.append(&btn_open);
            obj.append(&btn_copy);

            // Wire buttons to the stored handlers + current item.
            let weak = obj.downgrade();
            btn_play.connect_clicked(move |_| {
                if let Some(row) = weak.upgrade() {
                    let imp = row.imp();
                    if let (Some(item), Some(cb)) =
                        (imp.item.borrow().clone(), imp.on_play.borrow().clone())
                    {
                        cb(item);
                    }
                }
            });
            let weak = obj.downgrade();
            btn_download.connect_clicked(move |_| {
                if let Some(row) = weak.upgrade() {
                    let imp = row.imp();
                    if let (Some(item), Some(cb)) =
                        (imp.item.borrow().clone(), imp.on_download.borrow().clone())
                    {
                        cb(item);
                    }
                }
            });
            let weak = obj.downgrade();
            btn_open.connect_clicked(move |_| {
                if let Some(row) = weak.upgrade() {
                    let imp = row.imp();
                    if let (Some(item), Some(cb)) =
                        (imp.item.borrow().clone(), imp.on_open.borrow().clone())
                    {
                        cb(item);
                    }
                }
            });
            let weak = obj.downgrade();
            btn_copy.connect_clicked(move |_| {
                if let Some(row) = weak.upgrade() {
                    let imp = row.imp();
                    if let (Some(item), Some(cb)) =
                        (imp.item.borrow().clone(), imp.on_copy.borrow().clone())
                    {
                        cb(item);
                    }
                }
            });
            let weak = obj.downgrade();
            btn_favorite.connect_clicked(move |_| {
                if let Some(row) = weak.upgrade() {
                    let imp = row.imp();
                    if let (Some(item), Some(cb)) =
                        (imp.item.borrow().clone(), imp.on_favorite.borrow().clone())
                    {
                        let now_fav = cb(item);
                        row.set_favorite_icon(now_fav);
                    }
                }
            });

            let _ = self.checkbox.set(checkbox);
            let _ = self.thumb.set(thumb);
            let _ = self.title.set(title);
            let _ = self.channel.set(channel);
            let _ = self.btn_play.set(btn_play);
            let _ = self.btn_download.set(btn_download);
            let _ = self.btn_open.set(btn_open);
            let _ = self.btn_copy.set(btn_copy);
            let _ = self.btn_favorite.set(btn_favorite);
        }
    }

    impl WidgetImpl for SearchResultRow {}
    impl BoxImpl for SearchResultRow {}
}

glib::wrapper! {
    pub struct SearchResultRow(ObjectSubclass<imp::SearchResultRow>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Orientable;
}

impl Default for SearchResultRow {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}

impl SearchResultRow {
    pub fn new() -> Self {
        Self::default()
    }

    /// Give the row the shared "now playing" handle and start watching it, so it
    /// highlights itself whenever its bound item becomes the active track.
    pub fn set_now_playing(&self, now: NowPlaying) {
        let weak = self.downgrade();
        now.connect_url_notify(move |_| {
            if let Some(row) = weak.upgrade() {
                row.refresh_playing();
            }
        });
        let _ = self.imp().now_playing.set(now);
    }

    /// Add/remove the `.playing` highlight based on whether this row's item is
    /// the track the player is currently on.
    fn refresh_playing(&self) {
        let imp = self.imp();
        let active = match (imp.item.borrow().as_ref(), imp.now_playing.get()) {
            (Some(item), Some(now)) => {
                let cur = now.url();
                !cur.is_empty() && item.url() == cur
            }
            _ => false,
        };
        if active {
            self.add_css_class("playing");
        } else {
            self.remove_css_class("playing");
        }
    }

    /// Install the (shared) play/download/open/copy handlers once per row.
    pub fn set_handlers(
        &self,
        on_play: RowAction,
        on_download: RowAction,
        on_open: RowAction,
        on_copy: RowAction,
    ) {
        self.imp().on_play.replace(Some(on_play));
        self.imp().on_download.replace(Some(on_download));
        self.imp().on_open.replace(Some(on_open));
        self.imp().on_copy.replace(Some(on_copy));
    }

    /// Install the favorite toggle + state-query handlers and reveal the heart
    /// button. `toggle` flips membership (returning the new state); `is_fav`
    /// reports the current state so the icon can be set on (re)bind. `watch`
    /// notifies when the list changes elsewhere (e.g. "favorite all") so the
    /// heart restays in sync without a rebind.
    pub fn set_favorite_handlers(
        &self,
        toggle: FavToggle,
        is_fav: FavQuery,
        watch: FavoritesWatch,
    ) {
        let imp = self.imp();
        imp.on_favorite.replace(Some(toggle));
        imp.is_favorite.replace(Some(is_fav));
        if let Some(btn) = imp.btn_favorite.get() {
            btn.set_visible(true);
        }
        if imp.fav_watch.get().is_none() {
            let weak = self.downgrade();
            watch.connect_rev_notify(move |_| {
                if let Some(row) = weak.upgrade() {
                    row.refresh_favorite();
                }
            });
            let _ = imp.fav_watch.set(watch);
        }
        self.refresh_favorite();
    }

    /// Flip the heart between filled (favorited) and outline, updating tooltip.
    fn set_favorite_icon(&self, favorited: bool) {
        if let Some(btn) = self.imp().btn_favorite.get() {
            btn.set_icon_name(if favorited {
                "bigtube-emblem-favorite-filled-symbolic"
            } else {
                "bigtube-emblem-favorite-symbolic"
            });
            let tip = if favorited {
                tr("Remove from Favorites")
            } else {
                tr("Add to Favorites")
            };
            btn.set_tooltip_text(Some(&tip));
        }
    }

    /// Recompute the heart state for the currently-bound item.
    fn refresh_favorite(&self) {
        let imp = self.imp();
        let favorited = match (
            imp.item.borrow().as_ref(),
            imp.is_favorite.borrow().as_ref(),
        ) {
            (Some(item), Some(q)) => q(&item.url()),
            _ => false,
        };
        self.set_favorite_icon(favorited);
    }

    /// Bind a model item to this row (called on factory bind).
    pub fn set_item(&self, item: &VideoObject) {
        let imp = self.imp();
        imp.item.replace(Some(item.clone()));
        // Recompute the highlight + favorite state for the newly-bound item
        // (rows are recycled).
        self.refresh_playing();
        self.refresh_favorite();

        // Rebind the selection checkbox to this item (drop the previous item's
        // bindings first, since rows are recycled).
        for b in imp.bindings.borrow_mut().drain(..) {
            b.unbind();
        }
        let checkbox = imp.checkbox.get().unwrap();
        let b1 = item
            .bind_property("is-selected", checkbox, "active")
            .sync_create()
            .bidirectional()
            .build();
        // Only offer selection for real videos, even in selection mode.
        let playable = !item.is_playlist();
        let b2 = item
            .bind_property("selection-mode", checkbox, "visible")
            .sync_create()
            .transform_to(move |_, mode: bool| Some(mode && playable))
            .build();
        imp.bindings.borrow_mut().extend([b1, b2]);

        let title = item.title();
        imp.title.get().unwrap().set_text(&title);

        let is_playlist = item.is_playlist();
        let subtitle = if is_playlist {
            let count = item.playlist_count();
            let uploader = item.uploader();
            let label = if count > 0 {
                tr("{count} videos").replace("{count}", &count.to_string())
            } else {
                tr("Playlist")
            };
            if uploader.is_empty() || uploader == "Unknown" {
                label
            } else {
                format!("{uploader} • {label}")
            }
        } else {
            let u = item.uploader();
            if u.is_empty() {
                tr("Unknown Artist")
            } else {
                u
            }
        };
        imp.channel.get().unwrap().set_text(&subtitle);

        // Playlist rows aren't directly playable: show "open" instead.
        imp.btn_play.get().unwrap().set_visible(!is_playlist);
        imp.btn_download.get().unwrap().set_visible(!is_playlist);
        imp.btn_open.get().unwrap().set_visible(is_playlist);

        // Invalidate any in-flight thumbnail load for the previous item.
        imp.thumb_gen.set(imp.thumb_gen.get().wrapping_add(1));

        let thumb = imp.thumb.get().unwrap();
        if is_playlist {
            thumb.set_icon_name(Some("bigtube-view-list-symbolic"));
        } else {
            thumb.set_icon_name(Some("bigtube-video-x-generic-symbolic"));
            let url = item.thumbnail();
            if !url.is_empty() {
                self.load_thumbnail(&url);
            }
        }
    }

    /// Load `url` into the thumbnail asynchronously (memory-cached, recycle-safe).
    fn load_thumbnail(&self, url: &str) {
        let imp = self.imp();
        let gen = imp.thumb_gen.get();
        let thumb = imp.thumb.get().unwrap().clone();

        if let Some(tex) = THUMB_CACHE.with(|c| c.borrow().get(url)) {
            thumb.set_paintable(Some(&tex));
            return;
        }

        let (tx, rx) = async_channel::bounded::<Option<Vec<u8>>>(1);
        let url_thread = url.to_string();
        std::thread::spawn(move || {
            let _ = tx.send_blocking(fetch_bytes(&url_thread));
        });

        let weak = self.downgrade();
        let url_key = url.to_string();
        glib::spawn_future_local(async move {
            let Ok(Some(bytes)) = rx.recv().await else {
                return;
            };
            let Some(row) = weak.upgrade() else { return };
            if row.imp().thumb_gen.get() != gen {
                return; // row was rebound to another item
            }
            if let Some(tex) = decode_texture(&bytes) {
                THUMB_CACHE.with(|c| c.borrow_mut().insert(url_key, tex.clone()));
                row.imp().thumb.get().unwrap().set_paintable(Some(&tex));
            }
        });
    }
}

const MAX_THUMB_BYTES: usize = 10 * 1024 * 1024;

/// On-disk cache path for a thumbnail URL (`~/.cache/bigtube/thumbnails/<hash>`),
/// mirroring `image_loader.py`'s disk cache.
fn cache_file(url: &str) -> std::path::PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut h);
    bigtube_core::paths::user_cache_dir()
        .join("bigtube")
        .join("thumbnails")
        .join(format!("{:016x}.img", h.finish()))
}

/// Keep the on-disk thumbnail cache from growing forever: drop the oldest files
/// (by mtime) once it exceeds the cap. Cheap to lose — they re-download on
/// demand. Meant to be called once on startup from a background thread.
pub(crate) fn prune_thumbnail_cache() {
    const MAX_FILES: usize = 600;
    let dir = bigtube_core::paths::user_cache_dir()
        .join("bigtube")
        .join("thumbnails");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, std::path::PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let mtime = e.metadata().and_then(|m| m.modified()).ok()?;
            p.is_file().then_some((mtime, p))
        })
        .collect();
    if files.len() <= MAX_FILES {
        return;
    }
    files.sort_by_key(|(mtime, _)| *mtime); // oldest first
    for (_, path) in files.iter().take(files.len() - MAX_FILES) {
        let _ = std::fs::remove_file(path);
    }
}

/// Fetch thumbnail bytes: disk cache first, then network (and persist to disk).
pub(crate) fn fetch_bytes(url: &str) -> Option<Vec<u8>> {
    use std::io::Read;

    let path = cache_file(url);
    if let Ok(bytes) = std::fs::read(&path) {
        if !bytes.is_empty() {
            return Some(bytes);
        }
    }

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .build();
    let resp = agent.get(url).call().ok()?;
    let mut buf = Vec::new();
    resp.into_reader()
        .take(MAX_THUMB_BYTES as u64)
        .read_to_end(&mut buf)
        .ok()?;
    if buf.is_empty() {
        return None;
    }

    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(&path, &buf);
    }
    Some(buf)
}

pub(crate) fn decode_texture(bytes: &[u8]) -> Option<gtk::gdk::Texture> {
    decode_texture_sized(bytes, 80, 45)
}

pub(crate) fn decode_texture_sized(bytes: &[u8], w: i32, h: i32) -> Option<gtk::gdk::Texture> {
    let loader = gdk_pixbuf::PixbufLoader::new();
    loader.write(bytes).ok()?;
    loader.close().ok()?;
    let pixbuf = loader.pixbuf()?;
    let scaled = pixbuf
        .scale_simple(w, h, gdk_pixbuf::InterpType::Bilinear)
        .unwrap_or(pixbuf);
    Some(gtk::gdk::Texture::for_pixbuf(&scaled))
}
