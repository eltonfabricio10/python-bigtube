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

    if two_col {
        // Two columns: video (left) + audio (right), equal width and top-aligned.
        // Only the VIDEO column scrolls (capped height), so its long codec ladder
        // doesn't blow up the dialog while the short audio column stays fully
        // visible. No outer scroll — the dialog height tracks the taller column.
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 18);
        row.set_margin_top(12);
        row.set_margin_bottom(12);
        row.set_margin_start(12);
        row.set_margin_end(12);
        row.set_homogeneous(true);

        let video = make_group(tr("Video Formats"), None, &info.videos);
        video.set_valign(gtk::Align::Start);
        let video_scroll = gtk::ScrolledWindow::new();
        video_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        video_scroll.set_propagate_natural_height(true);
        video_scroll.set_max_content_height(460);
        video_scroll.set_hexpand(true);
        video_scroll.set_valign(gtk::Align::Start);
        video_scroll.set_child(Some(&video));
        row.append(&video_scroll);

        if !info.audios.is_empty() {
            let audio = make_group(tr("Audio Formats"), audio_desc, &info.audios);
            audio.set_valign(gtk::Align::Start);
            audio.set_hexpand(true);
            row.append(&audio);
        }
        toolbar.set_content(Some(&row));
    } else {
        // Single column (YouTube Music audio, or fallback) inside one scroll that
        // grows with the content up to a cap, then scrolls.
        let page = gtk::Box::new(gtk::Orientation::Vertical, 18);
        page.set_margin_top(12);
        page.set_margin_bottom(12);
        page.set_margin_start(12);
        page.set_margin_end(12);

        if !info.audios.is_empty() {
            let audio = make_group(tr("Audio Formats"), audio_desc, &info.audios);
            audio.set_hexpand(true);
            page.append(&audio);
        } else {
            // Empty fallback so the dialog never renders blank.
            let group = adw::PreferencesGroup::new();
            group.add(
                &adw::ActionRow::builder()
                    .title(tr("No formats found"))
                    .build(),
            );
            page.append(&group);
        }

        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scrolled.set_propagate_natural_height(true);
        scrolled.set_max_content_height(640);
        scrolled.set_child(Some(&page));
        toolbar.set_content(Some(&scrolled));
    }
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
