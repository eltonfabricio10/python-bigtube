//! Schedule-download dialog, mirroring `schedule_dialog.py`. Presents a calendar
//! plus hour/minute spinners and returns the chosen instant as a Unix timestamp
//! (seconds) via `on_confirm`.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

/// Callback receiving the chosen Unix timestamp (seconds) and the recurrence key
/// ("once" / "daily" / "weekly" / "monthly").
pub type ScheduleFn = Rc<dyn Fn(f64, String)>;

/// Recurrence option keys, indexed to match the "Repeat" dropdown order.
const RECURRENCES: [&str; 4] = ["once", "daily", "weekly", "monthly"];

use crate::i18n::tr;

/// `default_ts` pre-selects the calendar/time (None = now); `default_recurrence`
/// pre-selects the Repeat dropdown. Used both for a fresh schedule and to edit
/// an existing one.
pub fn show(
    parent: &adw::ApplicationWindow,
    default_ts: Option<f64>,
    default_recurrence: &str,
    on_confirm: ScheduleFn,
) {
    let win = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .default_width(400)
        .default_height(600)
        .title(tr("Schedule Download"))
        .build();
    crate::app::apply_theme_classes(&win);

    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    // Confirm lives in the header bar so it's always visible (never pushed below
    // the fold by the calendar). Wired after the inputs exist.
    let confirm = gtk::Button::with_label(&tr("Schedule"));
    confirm.add_css_class("suggested-action");
    header.pack_end(&confirm);
    toolbar.add_top_bar(&header);
    let page = adw::PreferencesPage::new();

    // Preselect the given instant (edit) or now (fresh schedule).
    let preset = default_ts
        .and_then(|ts| glib::DateTime::from_unix_local(ts as i64).ok())
        .or_else(|| glib::DateTime::now_local().ok());

    // Date group with a calendar.
    let date_group = adw::PreferencesGroup::builder().title(tr("Date")).build();
    let calendar = gtk::Calendar::new();
    if let Some(d) = preset.as_ref() {
        if let Ok(sel) = glib::DateTime::new(
            &glib::TimeZone::local(),
            d.year(),
            d.month(),
            d.day_of_month(),
            0,
            0,
            0.0,
        ) {
            calendar.select_day(&sel);
        }
    }
    let date_row = adw::ActionRow::builder().title(tr("Date")).build();
    date_row.add_suffix(&calendar);
    date_group.add(&date_row);
    page.add(&date_group);

    // Time group with hour/minute spinners (defaults to the preset).
    let time_group = adw::PreferencesGroup::builder().title(tr("Time")).build();
    let cur_h = preset.as_ref().map(|d| d.hour()).unwrap_or(0);
    let cur_m = preset.as_ref().map(|d| d.minute()).unwrap_or(0);
    let hour = gtk::SpinButton::with_range(0.0, 23.0, 1.0);
    hour.set_value(cur_h as f64);
    hour.set_valign(gtk::Align::Center);
    let minute = gtk::SpinButton::with_range(0.0, 59.0, 1.0);
    minute.set_value(cur_m as f64);
    minute.set_valign(gtk::Align::Center);
    let time_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    time_box.append(&hour);
    time_box.append(&gtk::Label::new(Some(":")));
    time_box.append(&minute);
    let time_row = adw::ActionRow::builder().title(tr("Time (HH:MM)")).build();
    time_row.add_suffix(&time_box);
    time_group.add(&time_row);
    page.add(&time_group);

    // Recurrence group: "Once" (the manual one-shot) plus daily/weekly/monthly.
    let repeat_group = adw::PreferencesGroup::builder().title(tr("Repeat")).build();
    let repeat = adw::ComboRow::builder().title(tr("Repeat")).build();
    let (once, daily, weekly, monthly) = (tr("Once"), tr("Daily"), tr("Weekly"), tr("Monthly"));
    let repeat_labels = gtk::StringList::new(&[
        once.as_str(),
        daily.as_str(),
        weekly.as_str(),
        monthly.as_str(),
    ]);
    repeat.set_model(Some(&repeat_labels));
    if let Some(idx) = RECURRENCES.iter().position(|r| *r == default_recurrence) {
        repeat.set_selected(idx as u32);
    }
    repeat_group.add(&repeat);
    page.add(&repeat_group);

    toolbar.set_content(Some(&page));
    win.set_content(Some(&toolbar));

    {
        let win = win.clone();
        confirm.connect_clicked(move |_| {
            let date = calendar.date();
            let h = hour.value_as_int();
            let m = minute.value_as_int();
            if let Ok(dt) = glib::DateTime::new(
                &glib::TimeZone::local(),
                date.year(),
                date.month(),
                date.day_of_month(),
                h,
                m,
                0.0,
            ) {
                let recurrence = RECURRENCES
                    .get(repeat.selected() as usize)
                    .copied()
                    .unwrap_or("once")
                    .to_string();
                on_confirm(dt.to_unix() as f64, recurrence);
            }
            win.close();
        });
    }

    win.present();
}
