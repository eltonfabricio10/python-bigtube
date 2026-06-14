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
use crate::objects::VideoObject;

thread_local! {
    /// Main-thread texture cache so scrolling/rebinding doesn't re-download.
    static THUMB_CACHE: RefCell<HashMap<String, gtk::gdk::Texture>> = RefCell::new(HashMap::new());
}

/// Shared callback type for the row's action buttons.
pub type RowAction = Rc<dyn Fn(VideoObject)>;

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
        pub item: RefCell<Option<VideoObject>>,
        pub on_play: RefCell<Option<RowAction>>,
        pub on_download: RefCell<Option<RowAction>>,
        pub on_open: RefCell<Option<RowAction>>,
        pub on_copy: RefCell<Option<RowAction>>,
        pub thumb_gen: Cell<u64>,
        // Property bindings for the current item, cleared on rebind.
        pub bindings: RefCell<Vec<glib::Binding>>,
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

            // Selection checkbox (hidden unless selection mode is active).
            let checkbox = gtk::CheckButton::new();
            checkbox.set_valign(gtk::Align::Center);
            checkbox.set_visible(false);

            let thumb = gtk::Image::from_icon_name("video-x-generic-symbolic");
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

            let btn_play = gtk::Button::from_icon_name("media-playback-start-symbolic");
            btn_play.add_css_class("flat");
            btn_play.set_focus_on_click(false);
            btn_play.set_valign(gtk::Align::Center);
            btn_play.set_tooltip_text(Some(&tr("Play Video")));
            let btn_download = gtk::Button::from_icon_name("folder-download-symbolic");
            btn_download.add_css_class("flat");
            btn_download.set_focus_on_click(false);
            btn_download.set_valign(gtk::Align::Center);
            btn_download.set_tooltip_text(Some(&tr("Download")));
            let btn_open = gtk::Button::from_icon_name("folder-open-symbolic");
            btn_open.add_css_class("flat");
            btn_open.set_focus_on_click(false);
            btn_open.set_valign(gtk::Align::Center);
            btn_open.set_tooltip_text(Some(&tr("Open playlist")));
            btn_open.set_visible(false);
            let btn_copy = gtk::Button::from_icon_name("edit-copy-symbolic");
            btn_copy.add_css_class("flat");
            btn_copy.set_focus_on_click(false);
            btn_copy.set_valign(gtk::Align::Center);
            btn_copy.set_tooltip_text(Some(&tr("Copy Link")));

            obj.append(&checkbox);
            obj.append(&thumb);
            obj.append(&vbox);
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

            let _ = self.checkbox.set(checkbox);
            let _ = self.thumb.set(thumb);
            let _ = self.title.set(title);
            let _ = self.channel.set(channel);
            let _ = self.btn_play.set(btn_play);
            let _ = self.btn_download.set(btn_download);
            let _ = self.btn_open.set(btn_open);
            let _ = self.btn_copy.set(btn_copy);
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

    /// Bind a model item to this row (called on factory bind).
    pub fn set_item(&self, item: &VideoObject) {
        let imp = self.imp();
        imp.item.replace(Some(item.clone()));

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
            thumb.set_icon_name(Some("view-list-symbolic"));
        } else {
            thumb.set_icon_name(Some("video-x-generic-symbolic"));
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

        if let Some(tex) = THUMB_CACHE.with(|c| c.borrow().get(url).cloned()) {
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
