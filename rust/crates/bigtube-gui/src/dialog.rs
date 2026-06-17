//! Format-selection dialog, mirroring `format_dialog.py`. Lists the parsed
//! video/audio formats; picking one invokes `on_pick(format_id, ext)`.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;

use bigtube_core::downloader::{FormatOption, ParsedInfo};

use crate::i18n::tr;

/// Callback: `(format_id, ext)` — download now.
pub type PickFn = Rc<dyn Fn(String, String)>;
/// Callback: `(format_id, ext)` — open the schedule flow for this format.
pub type ScheduleFn = Rc<dyn Fn(String, String)>;
/// Callback: the dialog was closed without picking a format (go back).
pub type CloseFn = Rc<dyn Fn()>;

pub fn show(
    parent: &adw::ApplicationWindow,
    info: &ParsedInfo,
    audio_only: bool,
    on_pick: PickFn,
    on_schedule: ScheduleFn,
    on_close: CloseFn,
) {
    // Normal sources show Video + Audio side by side (two columns, one screen,
    // no Video/Audio prompt); YouTube Music shows the single Audio column.
    let two_col = !audio_only && !info.videos.is_empty();

    let win = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .default_width(if two_col { 860 } else { 520 })
        .title(tr("Select Quality"))
        .build();
    crate::app::apply_theme_classes(&win);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());

    // True once a format is picked/scheduled, so closing the window then doesn't
    // count as "cancelled".
    let picked = Rc::new(Cell::new(false));

    // Builds one column's PreferencesGroup from a list of formats.
    let make_group = |title: String, description: Option<String>, formats: &[FormatOption]| {
        let builder = adw::PreferencesGroup::builder().title(title);
        let group = match description {
            Some(d) => builder.description(d).build(),
            None => builder.build(),
        };
        for f in formats {
            group.add(&format_row(f, &win, &on_pick, &on_schedule, &picked));
        }
        group
    };

    // When every audio row is a virtual "convert" option, the source had no
    // separate audio track — tell the user the audio is extracted/converted.
    let audio_desc = (!info.audios.is_empty()
        && info.audios.iter().all(|f| f.codec.ends_with("_convert")))
    .then(|| {
        tr("This source has no separate audio track. The options below extract and convert its audio.")
    });

    // Outer container: a horizontal row of columns (two-col) or a single column.
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 18);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.set_homogeneous(two_col);

    let mut count = 0;
    if two_col {
        // Left: video formats. Right: audio. Each column top-aligned and equal
        // width, so the dialog's height is the taller column — not their sum.
        let video = make_group(tr("Video Formats"), None, &info.videos);
        video.set_valign(gtk::Align::Start);
        video.set_hexpand(true);
        content.append(&video);
        count += info.videos.len();
        if !info.audios.is_empty() {
            let audio = make_group(tr("Audio Formats"), audio_desc, &info.audios);
            audio.set_valign(gtk::Align::Start);
            audio.set_hexpand(true);
            content.append(&audio);
            count += info.audios.len();
        }
    } else if !info.audios.is_empty() {
        // Audio-only source (YouTube Music): single audio column.
        let audio = make_group(tr("Audio Formats"), audio_desc, &info.audios);
        audio.set_hexpand(true);
        content.append(&audio);
        count += info.audios.len();
    }

    // Empty fallback so the dialog never renders blank.
    if count == 0 {
        let group = adw::PreferencesGroup::new();
        group.add(
            &adw::ActionRow::builder()
                .title(tr("No formats found"))
                .build(),
        );
        group.set_hexpand(true);
        content.append(&group);
    }

    // Grow with the content up to a cap, then scroll — short lists yield a short
    // dialog (no dead space); a very long column still scrolls as a safety net.
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_propagate_natural_height(true);
    scrolled.set_max_content_height(640);
    scrolled.set_child(Some(&content));
    toolbar.set_content(Some(&scrolled));
    win.set_content(Some(&toolbar));

    // Closing without a pick → notify the caller.
    {
        let on_close = on_close.clone();
        let picked = picked.clone();
        win.connect_close_request(move |_| {
            if !picked.get() {
                on_close();
            }
            gtk::glib::Propagation::Proceed
        });
    }
    win.present();
}

/// Pretty, vendor-neutral codec name for display (avc1 → H.264, mp4a → AAC…).
fn codec_display(codec: &str) -> String {
    let c = codec.to_lowercase();
    if c.contains("avc") || c.contains("h264") {
        "H.264".into()
    } else if c.contains("hev") || c.contains("h265") {
        "H.265".into()
    } else if c.contains("vp9") || c.contains("vp09") {
        "VP9".into()
    } else if c.contains("vp8") {
        "VP8".into()
    } else if c.contains("av01") || c.contains("av1") {
        "AV1".into()
    } else if c.contains("mp4a") || c.contains("aac") {
        "AAC".into()
    } else if c.contains("opus") {
        "Opus".into()
    } else if c.contains("vorbis") {
        "Vorbis".into()
    } else if c.contains("flac") {
        "FLAC".into()
    } else if c.contains("mp3") {
        "MP3".into()
    } else if c.contains("eac3") || c.contains("ac3") {
        "AC-3".into()
    } else if codec.is_empty() {
        String::new()
    } else {
        codec.to_uppercase()
    }
}

/// Compose the row title from the structured format fields, translating the few
/// human words (the codec/ext tokens are proper nouns and stay as-is). Built in
/// the GUI — not the core — so every language gets a localized label.
fn display_label(f: &FormatOption) -> String {
    // Virtual rows, identified by their synthetic codec markers.
    if f.codec == "mkv_merge" {
        return format!("{} · MKV ({}p)", tr("Best"), f.resolution);
    }
    if f.codec == "unknown" {
        return tr("Best available quality");
    }
    if f.codec.ends_with("_convert") {
        return format!("{} {}", tr("Convert to"), f.ext.to_uppercase());
    }
    if f.kind == "audio" {
        let mut s = codec_display(&f.codec);
        let kbps = f.quality as i64;
        if kbps > 0 {
            if !s.is_empty() {
                s.push_str(" · ");
            }
            s.push_str(&format!("{kbps} kbps"));
        }
        if !f.ext.is_empty() {
            s.push_str(&format!(" ({})", f.ext));
        }
        return s;
    }
    // Real video stream: "1080p 60fps · AV1 (webm)".
    let mut s = format!("{}p", f.resolution);
    if f.fps > 30 {
        s.push_str(&format!(" {}fps", f.fps));
    }
    let cd = codec_display(&f.codec);
    if !cd.is_empty() {
        s.push_str(&format!(" · {cd}"));
    }
    if !f.ext.is_empty() {
        s.push_str(&format!(" ({})", f.ext));
    }
    s
}

fn format_row(
    f: &FormatOption,
    win: &adw::Window,
    on_pick: &PickFn,
    on_schedule: &ScheduleFn,
    picked: &Rc<Cell<bool>>,
) -> adw::ActionRow {
    // Virtual "convert" rows have no real size — show a meaningful note instead.
    let subtitle = if f.codec.ends_with("_convert") || f.codec == "unknown" {
        tr("Best available quality")
    } else {
        f.size.clone()
    };
    let row = adw::ActionRow::builder()
        .title(display_label(f))
        .subtitle(subtitle)
        .build();

    let suffix = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    suffix.set_valign(gtk::Align::Center);

    // For video picks, use a height-aware selector so an unavailable exact id
    // falls back to the chosen resolution instead of silently dropping to ~360p.
    let sel_id = if f.kind == "video" {
        bigtube_core::downloader::video_selector(&f.id, f.resolution, &f.codec)
    } else {
        f.id.clone()
    };

    // Schedule for later.
    let schedule = gtk::Button::from_icon_name("alarm-symbolic");
    schedule.add_css_class("flat");
    schedule.set_valign(gtk::Align::Center);
    schedule.set_tooltip_text(Some(&tr("Schedule Download")));
    {
        let id = sel_id.clone();
        let ext = f.ext.clone();
        let on_schedule = on_schedule.clone();
        let win = win.clone();
        let picked = picked.clone();
        schedule.connect_clicked(move |_| {
            picked.set(true);
            on_schedule(id.clone(), ext.clone());
            win.close();
        });
    }

    // Download now.
    let btn = gtk::Button::with_label(&tr("Download"));
    btn.add_css_class("suggested-action");
    btn.add_css_class("pill");
    btn.set_valign(gtk::Align::Center);
    {
        let id = sel_id.clone();
        let ext = f.ext.clone();
        let on_pick = on_pick.clone();
        let win = win.clone();
        let picked = picked.clone();
        btn.connect_clicked(move |_| {
            picked.set(true);
            on_pick(id.clone(), ext.clone());
            win.close();
        });
    }

    suffix.append(&schedule);
    suffix.append(&btn);
    row.add_suffix(&suffix);
    row
}
