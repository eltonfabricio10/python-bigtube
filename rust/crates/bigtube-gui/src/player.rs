//! GStreamer player (port decision: GStreamer-only, no MPV fallback).
//!
//! A `playbin` renders into a `gtk4paintablesink`, whose `paintable` is shown in
//! a `Gtk.Picture` inside a detachable video window. The bottom player bar holds
//! the transport controls, mirroring the Python `control_box`: thumbnail, title +
//! artist, prev/play/stop/next, current time / seek / total time, volume, and a
//! video-window toggle. Stream URLs are resolved via `bigtube_core::player`.
//!
//! Playback is queue-based: `play_queue` seeds a list and an index; prev/next and
//! end-of-stream walk the queue.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use adw::prelude::*;
use gst::prelude::*;
use gstreamer as gst;
use gtk::glib;

use bigtube_core::player::extract_stream_url;

use crate::i18n::tr;

/// One entry in the playback queue.
#[derive(Clone, Default)]
pub struct QueueItem {
    pub url: String,
    pub title: String,
    pub artist: String,
    pub thumbnail: String,
    pub is_local: bool,
    pub is_video: bool,
}

pub struct Player {
    playbin: gst::Element,
    thumb: gtk::Image,
    title_lbl: gtk::Label,
    artist_lbl: gtk::Label,
    btn_play: gtk::Button,
    btn_prev: gtk::Button,
    btn_next: gtk::Button,
    btn_stop: gtk::Button,
    spinner: gtk::Spinner,
    scale: gtk::Scale,
    time_cur: gtk::Label,
    time_tot: gtk::Label,
    thumb_stack: gtk::Stack,
    video_available: Cell<bool>,
    // True once real video frames are flowing — until then we keep the
    // thumbnail visible instead of the black, frame-less video surface.
    showing_frames: Cell<bool>,
    volume: gtk::ScaleButton,
    video_window: adw::Window,
    seeking: Rc<Cell<bool>>,
    duration: Rc<Cell<f64>>,
    token: Arc<AtomicU64>,
    thumb_token: Arc<AtomicU64>,
    queue: RefCell<Vec<QueueItem>>,
    index: Cell<usize>,
    // True while the user has explicitly paused, so buffering doesn't auto-resume.
    paused_by_user: Cell<bool>,
}

/// Build the player and its bottom bar widget.
pub fn build(parent: &adw::ApplicationWindow) -> (Rc<Player>, gtk::Widget) {
    let playbin = gst::ElementFactory::make("playbin")
        .build()
        .expect("playbin element");
    let sink = gst::ElementFactory::make("gtk4paintablesink")
        .build()
        .expect("gtk4paintablesink element");
    let paintable: gtk::gdk::Paintable = sink.property("paintable");
    playbin.set_property("video-sink", &sink);

    // Hint a fast connection so adaptive (HLS) playback stays on the top
    // rendition instead of dropping to a pixelated lower one on brief dips, and
    // grow the buffer so a short bandwidth dip rebuffers rather than degrades.
    if playbin.has_property("connection-speed", None) {
        playbin.set_property("connection-speed", 100_000u64); // kbit/s
    }
    if playbin.has_property("buffer-duration", None) {
        playbin.set_property("buffer-duration", 15_000_000_000i64); // 15s in ns
    }

    // Video surface in a detachable window.
    let picture = gtk::Picture::new();
    picture.set_paintable(Some(&paintable));
    picture.set_size_request(640, 360);
    let video_view = adw::ToolbarView::new();
    video_view.add_top_bar(&adw::HeaderBar::new());
    video_view.set_content(Some(&picture));
    let video_window = adw::Window::builder()
        .transient_for(parent)
        .title(tr("BigTube Player"))
        .default_width(854)
        .default_height(480)
        .hide_on_close(true)
        .content(&video_view)
        .build();
    crate::app::apply_theme_classes(&video_window);

    // --- Bottom bar (control_box) — floating, rounded card with a shadow. ---
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bar.set_widget_name("control_box");
    bar.add_css_class("card");
    bar.add_css_class("rounded");
    bar.set_margin_top(6);
    bar.set_margin_bottom(10);
    bar.set_margin_start(10);
    bar.set_margin_end(10);

    // Thumbnail / inline video. A stack swaps between the static thumbnail
    // (audio or idle) and a small live video surface that shares the playbin
    // paintable; clicking the area opens the detachable video window. The area
    // is a fixed 16:9 box; the video is cropped to cover it (no stretching) and
    // overflow is clipped so it never resizes/deforms the bar.
    const THUMB_W: i32 = 96;
    const THUMB_H: i32 = 54;

    let thumb = gtk::Image::from_icon_name("image-x-generic-symbolic");
    thumb.set_pixel_size(40);
    thumb.set_size_request(THUMB_W, THUMB_H);

    let small_video = gtk::Picture::new();
    small_video.set_paintable(Some(&paintable));
    small_video.set_content_fit(gtk::ContentFit::Cover);
    small_video.set_can_shrink(true);
    small_video.set_hexpand(false);
    small_video.set_vexpand(false);
    small_video.set_size_request(THUMB_W, THUMB_H);
    // A Picture reports its natural size as the video's full resolution, and a
    // size_request only sets a *minimum* — so on its own it would grow the bar
    // to the video size. Wrapping it in a non-scrolling ScrolledWindow clamps
    // it: the viewport's natural size is its own fixed request, not the child's.
    let video_clamp = gtk::ScrolledWindow::new();
    video_clamp.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Never);
    video_clamp.set_hexpand(false);
    video_clamp.set_vexpand(false);
    video_clamp.set_size_request(THUMB_W, THUMB_H);
    video_clamp.set_child(Some(&small_video));

    let thumb_stack = gtk::Stack::new();
    thumb_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    thumb_stack.add_named(&thumb, Some("thumb"));
    thumb_stack.add_named(&video_clamp, Some("video"));
    thumb_stack.set_visible_child_name("thumb");
    thumb_stack.add_css_class("rounded");
    thumb_stack.set_halign(gtk::Align::Center);
    thumb_stack.set_valign(gtk::Align::Center);
    thumb_stack.set_hexpand(false);
    thumb_stack.set_vexpand(false);
    thumb_stack.set_margin_start(6);
    // Lock the box to 16:9 and clip the video so it can't grow the bar.
    thumb_stack.set_size_request(THUMB_W, THUMB_H);
    thumb_stack.set_overflow(gtk::Overflow::Hidden);
    thumb_stack.set_tooltip_text(Some(&tr("Toggle Video Window")));
    thumb_stack.set_cursor_from_name(Some("pointer"));
    let thumb_click = gtk::GestureClick::new();
    thumb_stack.add_controller(thumb_click.clone());

    // Title + artist.
    let title_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    title_box.set_valign(gtk::Align::Center);
    let title_lbl = gtk::Label::new(Some(&tr("Unknown Title")));
    title_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title_lbl.set_width_chars(18);
    title_lbl.set_max_width_chars(20);
    title_lbl.set_xalign(0.0);
    let artist_lbl = gtk::Label::new(Some(&tr("Unknown Artist")));
    artist_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
    artist_lbl.set_xalign(0.0);
    artist_lbl.add_css_class("caption");
    artist_lbl.add_css_class("dim-label");
    title_box.append(&title_lbl);
    title_box.append(&artist_lbl);

    // Transport buttons.
    let btn_prev = gtk::Button::from_icon_name("media-skip-backward-symbolic");
    btn_prev.add_css_class("flat");
    btn_prev.set_focus_on_click(false);
    btn_prev.set_tooltip_text(Some(&tr("Previous")));
    btn_prev.set_sensitive(false);
    let btn_play = gtk::Button::from_icon_name("media-playback-start-symbolic");
    btn_play.add_css_class("circular");
    btn_play.set_focus_on_click(false);
    btn_play.set_tooltip_text(Some(&tr("Play/Pause")));
    let btn_stop = gtk::Button::from_icon_name("media-playback-stop-symbolic");
    btn_stop.add_css_class("flat");
    btn_stop.set_focus_on_click(false);
    btn_stop.set_tooltip_text(Some(&tr("Stop")));
    let btn_next = gtk::Button::from_icon_name("media-skip-forward-symbolic");
    btn_next.add_css_class("flat");
    btn_next.set_focus_on_click(false);
    btn_next.set_tooltip_text(Some(&tr("Next")));
    btn_next.set_sensitive(false);
    // Buffering spinner shown in place of the play button while loading.
    let spinner = gtk::Spinner::new();
    spinner.set_visible(false);
    let button_box = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    button_box.set_halign(gtk::Align::Center);
    button_box.append(&btn_prev);
    button_box.append(&btn_play);
    button_box.append(&spinner);
    button_box.append(&btn_stop);
    button_box.append(&btn_next);

    // Seek bar with time labels.
    let time_cur = gtk::Label::new(Some("--:--"));
    time_cur.add_css_class("numeric");
    let scale = gtk::Scale::new(gtk::Orientation::Horizontal, None::<&gtk::Adjustment>);
    scale.set_range(0.0, 1.0);
    scale.set_hexpand(true);
    scale.set_draw_value(false);
    let time_tot = gtk::Label::new(Some("--:--"));
    time_tot.add_css_class("numeric");
    let progress_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    progress_box.append(&time_cur);
    progress_box.append(&scale);
    progress_box.append(&time_tot);

    // The center column groups the buttons over the seek bar, vertically
    // centered so the controls aren't glued to the top edge of the bar.
    let player_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    player_box.set_hexpand(true);
    player_box.set_valign(gtk::Align::Center);
    player_box.set_margin_top(6);
    player_box.set_margin_bottom(4);
    player_box.append(&button_box);
    player_box.append(&progress_box);

    // Volume (ScaleButton configured as a volume control; VolumeButton is
    // deprecated since GTK 4.10).
    let volume = gtk::ScaleButton::new(
        0.0,
        1.0,
        0.02,
        &[
            "audio-volume-muted-symbolic",
            "audio-volume-high-symbolic",
            "audio-volume-low-symbolic",
            "audio-volume-medium-symbolic",
        ],
    );
    volume.set_value(1.0);
    volume.add_css_class("circular");
    volume.set_focus_on_click(false);

    bar.append(&thumb_stack);
    bar.append(&title_box);
    bar.append(&player_box);
    bar.append(&volume);

    let player = Rc::new(Player {
        playbin: playbin.clone(),
        thumb: thumb.clone(),
        title_lbl,
        artist_lbl,
        btn_play: btn_play.clone(),
        btn_prev: btn_prev.clone(),
        btn_next: btn_next.clone(),
        btn_stop: btn_stop.clone(),
        spinner: spinner.clone(),
        scale: scale.clone(),
        time_cur,
        time_tot,
        thumb_stack: thumb_stack.clone(),
        video_available: Cell::new(false),
        showing_frames: Cell::new(false),
        volume: volume.clone(),
        video_window: video_window.clone(),
        seeking: Rc::new(Cell::new(false)),
        duration: Rc::new(Cell::new(0.0)),
        token: Arc::new(AtomicU64::new(0)),
        thumb_token: Arc::new(AtomicU64::new(0)),
        queue: RefCell::new(Vec::new()),
        index: Cell::new(0),
        paused_by_user: Cell::new(false),
    });

    // Play / pause.
    {
        let p = player.clone();
        btn_play.connect_clicked(move |_| p.toggle());
    }
    // Stop.
    {
        let p = player.clone();
        btn_stop.connect_clicked(move |_| p.stop());
    }
    // Previous / next.
    {
        let p = player.clone();
        btn_prev.connect_clicked(move |_| p.prev());
    }
    {
        let p = player.clone();
        btn_next.connect_clicked(move |_| p.next());
    }
    // Volume → playbin.
    {
        let pb = playbin.clone();
        volume.connect_value_changed(move |_, v| pb.set_property("volume", v));
    }
    // Click the thumbnail/inline-video area to pop out the big video window.
    {
        let p = player.clone();
        thumb_click.connect_released(move |_, _, _, _| {
            if p.video_available.get() {
                p.video_window.set_visible(true);
            }
        });
    }
    // Close the big video window with Escape (the X / title-bar close already
    // works via hide_on_close).
    {
        let w = video_window.clone();
        let key = gtk::EventControllerKey::new();
        key.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == gtk::gdk::Key::Escape {
                w.set_visible(false);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        video_window.add_controller(key);
    }
    // Swap the miniature from thumbnail to live video only once real frames are
    // flowing, so it never shows a black surface while loading/buffering.
    {
        let p = player.clone();
        paintable.connect_invalidate_contents(move |_| {
            if !p.showing_frames.get() {
                p.showing_frames.set(true);
                p.update_inline();
            }
        });
    }
    // While the big window is open, show the static thumbnail in the bar; when
    // it closes, return to the inline video.
    {
        let stack = thumb_stack.clone();
        video_window.connect_show(move |_| stack.set_visible_child_name("thumb"));
    }
    {
        let p = player.clone();
        video_window.connect_hide(move |_| p.update_inline());
    }
    // Seek.
    {
        let p = player.clone();
        scale.connect_change_value(move |_, _, value| {
            let dur = p.duration.get();
            if dur > 0.0 {
                p.seeking.set(true);
                let secs = (value * dur).max(0.0);
                let _ = p.playbin.seek_simple(
                    gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                    gst::ClockTime::from_seconds(secs as u64),
                );
                p.seeking.set(false);
            }
            glib::Propagation::Proceed
        });
    }

    // Bus watch: handle buffering (HLS/network streams), advance on EOS, stop on
    // error.
    if let Some(bus) = playbin.bus() {
        let p = player.clone();
        let _ = bus.add_watch_local(move |_, msg| {
            match msg.view() {
                // Network/adaptive (HLS) streams pause to fill the buffer and
                // post BUFFERING messages — we MUST pause until 100%, then
                // resume, or playback stalls and nothing ever shows.
                gst::MessageView::Buffering(b) => {
                    let percent = b.percent();
                    if percent < 100 {
                        let _ = p.playbin.set_state(gst::State::Paused);
                        p.set_loading(true);
                    } else {
                        p.set_loading(false);
                        if !p.paused_by_user.get() {
                            let _ = p.playbin.set_state(gst::State::Playing);
                        }
                    }
                }
                gst::MessageView::Eos(_) => {
                    let advanced = p.advance_after_eos();
                    if !advanced {
                        p.stop();
                    }
                }
                gst::MessageView::Error(err) => {
                    tracing::error!("GStreamer error: {}", err.error());
                    p.stop();
                }
                _ => {}
            }
            glib::ControlFlow::Continue
        });
    }

    // Position/duration update loop.
    {
        let p = player.clone();
        glib::timeout_add_local(Duration::from_millis(500), move || {
            p.update_position();
            glib::ControlFlow::Continue
        });
    }

    // Start idle: nothing playing, all controls disabled.
    player.set_controls_enabled(false);

    (player, bar.upcast())
}

impl Player {
    /// Play a single remote item (resolved via yt-dlp), as a one-item queue.
    pub fn play(self: &Rc<Self>, url: &str, title: &str, artist: &str, thumbnail: &str) {
        if url.is_empty() {
            return;
        }
        self.play_queue(
            vec![QueueItem {
                url: url.to_string(),
                title: title.to_string(),
                artist: artist.to_string(),
                thumbnail: thumbnail.to_string(),
                is_local: false,
                is_video: true,
            }],
            0,
        );
    }

    /// Play a local file directly (no yt-dlp), as a one-item queue.
    pub fn play_local(self: &Rc<Self>, path: &str, title: &str, artist: &str) {
        if path.is_empty() {
            return;
        }
        let is_video = is_video_ext(path);
        self.play_queue(
            vec![QueueItem {
                url: path.to_string(),
                title: title.to_string(),
                artist: artist.to_string(),
                thumbnail: String::new(),
                is_local: true,
                is_video,
            }],
            0,
        );
    }

    /// Seed the queue and start playing `start`.
    pub fn play_queue(self: &Rc<Self>, items: Vec<QueueItem>, start: usize) {
        if items.is_empty() {
            return;
        }
        let start = start.min(items.len() - 1);
        self.queue.replace(items);
        self.play_index(start);
    }

    fn play_index(self: &Rc<Self>, i: usize) {
        let item = {
            let q = self.queue.borrow();
            match q.get(i) {
                Some(it) => it.clone(),
                None => return,
            }
        };
        self.index.set(i);
        self.set_controls_enabled(true);
        // New item: keep the thumbnail until fresh frames arrive.
        self.showing_frames.set(false);

        let shown_title = if item.title.is_empty() {
            tr("Unknown Title")
        } else {
            item.title.clone()
        };
        self.title_lbl.set_text(&shown_title);
        self.load_thumbnail(&item.thumbnail);

        // Audio-only items (e.g. YouTube Music) can't open the video window.
        self.set_video_enabled(item.is_video);

        // Bump the generation; drop any in-flight resolution.
        let gen = self.token.fetch_add(1, Ordering::SeqCst) + 1;

        // Stop the previous stream NOW so it doesn't keep playing (audio/video)
        // while we resolve/buffer the new one — the switch must be clean.
        let _ = self.playbin.set_state(gst::State::Null);

        if item.is_local {
            let shown_artist = if item.artist.is_empty() {
                tr("Unknown Artist")
            } else {
                item.artist.clone()
            };
            self.artist_lbl.set_text(&shown_artist);
            self.start_uri(&to_uri(&item.url));
            return;
        }

        self.artist_lbl.set_text(&tr("Buffering..."));
        self.set_loading(true);
        let (tx, rx) = async_channel::bounded::<String>(1);
        let url_thread = item.url.clone();
        std::thread::spawn(move || {
            let _ = tx.send_blocking(extract_stream_url(&url_thread));
        });

        let this = self.clone();
        let artist = item.artist.clone();
        glib::spawn_future_local(async move {
            let Ok(resolved) = rx.recv().await else {
                return;
            };
            if this.token.load(Ordering::SeqCst) != gen {
                return; // superseded
            }
            this.set_loading(false);
            this.start_uri(&to_uri(&resolved));
            let shown_artist = if artist.is_empty() {
                tr("Unknown Artist")
            } else {
                artist
            };
            this.artist_lbl.set_text(&shown_artist);
        });
    }

    /// Toggle the buffering spinner (replaces the play button while loading).
    fn set_loading(&self, loading: bool) {
        self.btn_play.set_visible(!loading);
        self.spinner.set_visible(loading);
        if loading {
            self.spinner.start();
        } else {
            self.spinner.stop();
        }
    }

    /// Point playbin at `uri` and start playing.
    fn start_uri(&self, uri: &str) {
        self.paused_by_user.set(false);
        let _ = self.playbin.set_state(gst::State::Null);
        self.playbin.set_property("uri", uri);
        let _ = self.playbin.set_state(gst::State::Playing);
        self.btn_play.set_icon_name("media-playback-pause-symbolic");
    }

    fn prev(self: &Rc<Self>) {
        let len = self.queue.borrow().len();
        if len == 0 {
            return;
        }
        let i = self.index.get();
        // Cyclic: wrap to the end from the first item.
        let prev = if i == 0 { len - 1 } else { i - 1 };
        self.play_index(prev);
    }

    fn next(self: &Rc<Self>) {
        let len = self.queue.borrow().len();
        if len == 0 {
            return;
        }
        let i = self.index.get();
        self.play_index((i + 1) % len);
    }

    /// Advance after EOS, cycling back to the start at the end of the list.
    /// Returns false only for an empty queue (so the caller stops).
    fn advance_after_eos(self: &Rc<Self>) -> bool {
        let len = self.queue.borrow().len();
        if len == 0 {
            return false;
        }
        let i = self.index.get();
        self.play_index((i + 1) % len);
        true
    }

    /// Enable prev/next whenever there's more than one item (cyclic).
    fn update_nav(&self) {
        let many = self.queue.borrow().len() > 1;
        self.btn_prev.set_sensitive(many);
        self.btn_next.set_sensitive(many);
    }

    /// Show the inline video for video items; fall back to the thumbnail (and
    /// close the big window) for audio-only items.
    fn set_video_enabled(&self, enabled: bool) {
        self.video_available.set(enabled);
        if !enabled {
            self.video_window.set_visible(false);
        }
        self.update_inline();
    }

    /// Refresh whether the miniature shows the live video or the thumbnail. Shows
    /// the live video only once frames are flowing (never the black loading
    /// surface), and keeps the thumbnail while the big window is open.
    fn update_inline(&self) {
        let show_video = self.video_available.get()
            && self.showing_frames.get()
            && !self.video_window.is_visible();
        self.thumb_stack
            .set_visible_child_name(if show_video { "video" } else { "thumb" });
    }

    /// Enable/disable all transport controls (idle = nothing playing).
    fn set_controls_enabled(&self, on: bool) {
        self.btn_play.set_sensitive(on);
        self.btn_stop.set_sensitive(on);
        self.scale.set_sensitive(on);
        self.volume.set_sensitive(on);
        if on {
            // prev/next + inline video refine themselves per queue/item.
            self.update_nav();
        } else {
            self.btn_prev.set_sensitive(false);
            self.btn_next.set_sensitive(false);
        }
    }

    /// Load the player-bar thumbnail from `url` (off-thread, recycle-safe).
    fn load_thumbnail(self: &Rc<Self>, url: &str) {
        let thumb = self.thumb.clone();
        if url.is_empty() {
            thumb.set_icon_name(Some("audio-x-generic-symbolic"));
            return;
        }
        let gen = self.thumb_token.fetch_add(1, Ordering::SeqCst) + 1;
        let token = self.thumb_token.clone();
        let (tx, rx) = async_channel::bounded::<Option<Vec<u8>>>(1);
        let url = url.to_string();
        std::thread::spawn(move || {
            let _ = tx.send_blocking(crate::row::fetch_bytes(&url));
        });
        glib::spawn_future_local(async move {
            let Ok(Some(bytes)) = rx.recv().await else {
                return;
            };
            if token.load(Ordering::SeqCst) != gen {
                return;
            }
            if let Some(tex) = crate::row::decode_texture_sized(&bytes, 60, 40) {
                thumb.set_paintable(Some(&tex));
            }
        });
    }

    fn toggle(&self) {
        let (_, state, _) = self.playbin.state(Some(gst::ClockTime::ZERO));
        if state == gst::State::Playing {
            self.paused_by_user.set(true);
            let _ = self.playbin.set_state(gst::State::Paused);
            self.btn_play.set_icon_name("media-playback-start-symbolic");
        } else {
            self.paused_by_user.set(false);
            let _ = self.playbin.set_state(gst::State::Playing);
            self.btn_play.set_icon_name("media-playback-pause-symbolic");
        }
    }

    fn stop(&self) {
        // Invalidate any pending resolution and clear the queue.
        self.token.fetch_add(1, Ordering::SeqCst);
        self.queue.replace(Vec::new());
        self.index.set(0);
        self.paused_by_user.set(false);
        self.showing_frames.set(false);
        self.set_loading(false);
        let _ = self.playbin.set_state(gst::State::Null);
        self.btn_play.set_icon_name("media-playback-start-symbolic");
        self.scale.set_value(0.0);
        self.time_cur.set_text("--:--");
        self.time_tot.set_text("--:--");
        self.title_lbl.set_text(&tr("Unknown Title"));
        self.artist_lbl.set_text(&tr("Unknown Artist"));
        self.thumb.set_icon_name(Some("image-x-generic-symbolic"));
        self.set_video_enabled(false);
        // Idle: disable every control until something plays again.
        self.set_controls_enabled(false);
    }

    fn update_position(&self) {
        if self.seeking.get() {
            return;
        }
        let pos = self
            .playbin
            .query_position::<gst::ClockTime>()
            .map(|t| t.seconds() as f64)
            .unwrap_or(0.0);
        let dur = self
            .playbin
            .query_duration::<gst::ClockTime>()
            .map(|t| t.seconds() as f64)
            .unwrap_or(0.0);
        self.duration.set(dur);
        if dur > 0.0 {
            self.scale.set_value(pos / dur);
            self.time_cur.set_text(&fmt_time(pos));
            self.time_tot.set_text(&fmt_time(dur));
        }
    }
}

fn to_uri(url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("file://") {
        url.to_string()
    } else {
        glib::filename_to_uri(url, None)
            .map(|s| s.to_string())
            .unwrap_or_else(|_| url.to_string())
    }
}

/// Whether a local path looks like a video file (vs audio-only).
fn is_video_ext(path: &str) -> bool {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    !matches!(
        ext.as_str(),
        "mp3" | "m4a" | "wav" | "flac" | "ogg" | "opus" | "aac" | "wma"
    )
}

fn fmt_time(secs: f64) -> String {
    let s = secs as u64;
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}
