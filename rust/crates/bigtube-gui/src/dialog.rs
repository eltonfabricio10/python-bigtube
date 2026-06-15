//! Format-selection dialog, mirroring `format_dialog.py`. Lists the parsed
//! video/audio formats; picking one invokes `on_pick(format_id, ext)`.

use std::rc::Rc;

use adw::prelude::*;

use bigtube_core::downloader::{FormatOption, ParsedInfo};

use crate::i18n::tr;

/// Callback: `(format_id, ext)` — download now.
pub type PickFn = Rc<dyn Fn(String, String)>;
/// Callback: `(format_id, ext)` — open the schedule flow for this format.
pub type ScheduleFn = Rc<dyn Fn(String, String)>;

pub fn show(
    parent: &adw::ApplicationWindow,
    info: &ParsedInfo,
    audio_only: bool,
    on_pick: PickFn,
    on_schedule: ScheduleFn,
) {
    let win = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .default_width(520)
        .default_height(640)
        .title(tr("Select Quality"))
        .build();
    crate::app::apply_theme_classes(&win);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());

    let page = adw::PreferencesPage::new();

    // YouTube Music → audio only; normal YouTube → video only.
    let mut count = 0;
    if !audio_only && !info.videos.is_empty() {
        let group = adw::PreferencesGroup::builder()
            .title(tr("Video Formats"))
            .build();
        for f in &info.videos {
            group.add(&format_row(f, &win, &on_pick, &on_schedule));
        }
        page.add(&group);
        count += info.videos.len();
    }
    if audio_only && !info.audios.is_empty() {
        let group = adw::PreferencesGroup::builder()
            .title(tr("Audio Only"))
            .build();
        for f in &info.audios {
            group.add(&format_row(f, &win, &on_pick, &on_schedule));
        }
        page.add(&group);
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
        page.add(&group);
    }

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&page));
    toolbar.set_content(Some(&scrolled));
    win.set_content(Some(&toolbar));
    win.present();
}

fn format_row(
    f: &FormatOption,
    win: &adw::Window,
    on_pick: &PickFn,
    on_schedule: &ScheduleFn,
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
        schedule.connect_clicked(move |_| {
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
        btn.connect_clicked(move |_| {
            on_pick(id.clone(), ext.clone());
            win.close();
        });
    }

    suffix.append(&schedule);
    suffix.append(&btn);
    row.add_suffix(&suffix);
    row
}
