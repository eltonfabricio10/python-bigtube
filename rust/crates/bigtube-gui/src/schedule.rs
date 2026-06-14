//! Schedule-download dialog, mirroring `schedule_dialog.py`. Presents a calendar
//! plus hour/minute spinners and returns the chosen instant as a Unix timestamp
//! (seconds) via `on_confirm`.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

/// Callback receiving the chosen Unix timestamp (seconds).
pub type ScheduleFn = Rc<dyn Fn(f64)>;

use crate::i18n::tr;

pub fn show(parent: &adw::ApplicationWindow, on_confirm: ScheduleFn) {
    let win = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .default_width(400)
        .default_height(600)
        .title(tr("Schedule Download"))
        .build();
    crate::app::apply_theme_classes(&win);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    let page = adw::PreferencesPage::new();

    // Date group with a calendar.
    let date_group = adw::PreferencesGroup::builder().title(tr("Date")).build();
    let calendar = gtk::Calendar::new();
    let date_row = adw::ActionRow::builder().title(tr("Date")).build();
    date_row.add_suffix(&calendar);
    date_group.add(&date_row);
    page.add(&date_group);

    // Time group with hour/minute spinners (defaults to now).
    let time_group = adw::PreferencesGroup::builder().title(tr("Time")).build();
    let now = glib::DateTime::now_local().ok();
    let cur_h = now.as_ref().map(|d| d.hour()).unwrap_or(0);
    let cur_m = now.as_ref().map(|d| d.minute()).unwrap_or(0);
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

    // Confirm button.
    let confirm_group = adw::PreferencesGroup::new();
    let confirm = gtk::Button::with_label(&tr("Schedule"));
    confirm.add_css_class("pill");
    confirm.add_css_class("suggested-action");
    confirm.set_halign(gtk::Align::Center);
    confirm.set_margin_top(12);
    confirm_group.add(&confirm);
    page.add(&confirm_group);

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&page));
    toolbar.set_content(Some(&scrolled));
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
                on_confirm(dt.to_unix() as f64);
            }
            win.close();
        });
    }

    win.present();
}
