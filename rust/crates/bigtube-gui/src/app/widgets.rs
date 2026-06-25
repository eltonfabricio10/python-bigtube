//! Small, stateless UI building blocks and formatters shared across the app's
//! pages. Nothing here touches `AppState` — they're pure presentation helpers.

use std::sync::atomic::{AtomicU64, Ordering};

use adw::prelude::*;

/// Human-readable byte size (MiB, or GiB past a gigabyte).
pub(crate) fn human_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1024.0 * 1024.0 * 1024.0 {
        format!("{:.2} GiB", b / 1024.0 / 1024.0 / 1024.0)
    } else {
        format!("{:.1} MiB", b / 1024.0 / 1024.0)
    }
}

/// A centered page title with an optional right-aligned button group.
/// A page title bar: centered title, optional action `buttons` on the right,
/// and an optional `trailing` widget (e.g. the collapsible filter control)
/// pinned to the far-right corner after the buttons.
pub(crate) fn page_header_trailing(
    title: &str,
    buttons: &[gtk::Button],
    trailing: Option<&gtk::Widget>,
) -> gtk::Widget {
    let cb = gtk::CenterBox::new();
    cb.add_css_class("page-title-bar");
    let lbl = gtk::Label::new(Some(title));
    lbl.add_css_class("title-1");
    cb.set_center_widget(Some(&lbl));
    if !buttons.is_empty() || trailing.is_some() {
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        bx.set_halign(gtk::Align::End);
        for b in buttons {
            bx.append(b);
        }
        if let Some(w) = trailing {
            bx.append(w);
        }
        cb.set_end_widget(Some(&bx));
    }
    cb.upcast()
}

/// A centered spinner + label, used as a "loading" stack page.
pub(crate) fn loading_page(label: &str) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 12);
    b.set_valign(gtk::Align::Center);
    b.set_halign(gtk::Align::Center);
    b.set_vexpand(true);
    let spinner = gtk::Spinner::new();
    spinner.set_size_request(48, 48);
    spinner.start();
    b.append(&spinner);
    let lbl = gtk::Label::new(Some(label));
    lbl.add_css_class("dim-label");
    b.append(&lbl);
    b
}

/// A centered empty-state placeholder.
pub(crate) fn status_page(icon: &str, title: &str, desc: &str) -> adw::StatusPage {
    adw::StatusPage::builder()
        .icon_name(icon)
        .title(title)
        .description(desc)
        .vexpand(true)
        .build()
}

pub(crate) fn combo_row(title: &str, options: &[impl AsRef<str>]) -> adw::ComboRow {
    let strs: Vec<&str> = options.iter().map(|s| s.as_ref()).collect();
    let model = gtk::StringList::new(&strs);
    adw::ComboRow::builder().title(title).model(&model).build()
}

pub(crate) fn switch_row(
    title: &str,
    subtitle: &str,
    active: bool,
    on_change: impl Fn(bool) + 'static,
) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder()
        .title(title)
        .subtitle(subtitle)
        .active(active)
        .build();
    row.connect_active_notify(move |r| on_change(r.is_active()));
    row
}

pub(crate) fn spin_row(
    title: &str,
    subtitle: &str,
    min: f64,
    max: f64,
    value: f64,
    on_change: impl Fn(f64) + 'static,
) -> adw::SpinRow {
    spin_row_step(title, subtitle, min, max, 1.0, value, on_change)
}

pub(crate) fn spin_row_step(
    title: &str,
    subtitle: &str,
    min: f64,
    max: f64,
    step: f64,
    value: f64,
    on_change: impl Fn(f64) + 'static,
) -> adw::SpinRow {
    let row = adw::SpinRow::with_range(min, max, step);
    row.set_title(title);
    row.set_subtitle(subtitle);
    row.set_value(value);
    row.connect_value_notify(move |r| on_change(r.value()));
    row
}

/// An action row whose suffix is a single icon button.
pub(crate) fn button_row(
    title: &str,
    subtitle: &str,
    icon: &str,
    destructive: bool,
    on_click: impl Fn() + 'static,
) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .build();
    let btn = gtk::Button::from_icon_name(icon);
    btn.add_css_class("flat");
    if destructive {
        btn.add_css_class("destructive-action");
    }
    btn.set_valign(gtk::Align::Center);
    btn.connect_clicked(move |_| on_click());
    row.add_suffix(&btn);
    row
}

/// Add a titled, icon-bearing page to a view stack.
pub(crate) fn add_page(
    stack: &adw::ViewStack,
    child: &gtk::Widget,
    name: &str,
    title: &str,
    icon: &str,
) {
    let page = stack.add_titled(child, Some(name), title);
    page.set_icon_name(Some(icon));
}

/// A process-unique key for a download row (`dl-<n>`).
pub(crate) fn next_key() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    format!("dl-{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

/// Parse a `"57.3%"`-style string into a 0.0..=1.0 fraction.
pub(crate) fn parse_percent(s: &str) -> Option<f64> {
    s.trim()
        .trim_end_matches('%')
        .parse::<f64>()
        .ok()
        .map(|v| (v / 100.0).clamp(0.0, 1.0))
}

/// Format a seconds count as `M:SS` (or `H:MM:SS` past an hour).
pub(crate) fn fmt_eta(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}
