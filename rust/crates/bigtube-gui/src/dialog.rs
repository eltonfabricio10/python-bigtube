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
    let win = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .default_width(520)
        .title(tr("Select Quality"))
        .build();
    crate::app::apply_theme_classes(&win);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());

    // A plain Box of groups (not a PreferencesPage) so the ScrolledWindow can
    // size to the content's natural height — no dead space below the last row.
    let page = gtk::Box::new(gtk::Orientation::Vertical, 18);
    page.set_margin_top(12);
    page.set_margin_bottom(12);
    page.set_margin_start(12);
    page.set_margin_end(12);

    // True once a format is picked/scheduled, so closing the window then doesn't
    // count as "cancelled" (which would re-open the Video/Audio chooser).
    let picked = Rc::new(Cell::new(false));

    // YouTube Music → audio only; normal YouTube → video only.
    let mut count = 0;
    if !audio_only && !info.videos.is_empty() {
        let group = adw::PreferencesGroup::builder()
            .title(tr("Video Formats"))
            .build();
        for f in &info.videos {
            group.add(&format_row(f, &win, &on_pick, &on_schedule, &picked));
        }
        page.append(&group);
        count += info.videos.len();
    }
    if audio_only && !info.audios.is_empty() {
        // When every audio row is a virtual "convert" option, the source had no
        // separate audio track — tell the user the audio is extracted/converted.
        let no_original = info.audios.iter().all(|f| f.codec.ends_with("_convert"));
        let builder = adw::PreferencesGroup::builder().title(tr("Audio Only"));
        let group = if no_original {
            builder
                .description(tr(
                    "This source has no separate audio track. The options below extract and convert its audio.",
                ))
                .build()
        } else {
            builder.build()
        };
        for f in &info.audios {
            group.add(&format_row(f, &win, &on_pick, &on_schedule, &picked));
        }
        page.append(&group);
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
        page.append(&group);
    }

    // Grow with the content up to a cap, then scroll — no fixed height, so a
    // short list yields a short dialog (no dead space) and a long one scrolls.
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_propagate_natural_height(true);
    scrolled.set_max_content_height(620);
    scrolled.set_child(Some(&page));
    toolbar.set_content(Some(&scrolled));
    win.set_content(Some(&toolbar));

    // Closing without a pick → notify the caller (re-opens the kind chooser).
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

fn format_row(
    f: &FormatOption,
    win: &adw::Window,
    on_pick: &PickFn,
    on_schedule: &ScheduleFn,
    picked: &Rc<Cell<bool>>,
) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(&f.label)
        .subtitle(format!("{} • {}", f.size, f.codec))
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
