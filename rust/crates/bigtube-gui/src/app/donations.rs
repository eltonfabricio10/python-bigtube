//! "Support BigTube" donations dialog: a PIX QR code / copyable key plus links
//! to online donation platforms.

use adw::prelude::*;
use gtk::glib;

use crate::app::{apply_theme_classes, tr_markup};
use crate::i18n::tr;

const PIX_KEY: &str = "a30c24f3-490f-424b-93d3-f1181380bc30";
const PIX_NAME: &str = "ELTON FABRICIO";
const PIX_CITY: &str = "SAO PAULO";
/// Online donation links. Leave a URL empty to hide that row until it's set.
const DONATE_LINKS: &[(&str, &str)] = &[(
    "GitHub Sponsors",
    "https://github.com/sponsors/eltonfabricio10",
)];

/// One EMV TLV field: two-digit id + two-digit length + value.
fn emv(id: &str, value: &str) -> String {
    format!("{id}{:02}{value}", value.len())
}

/// CRC16-CCITT (poly 0x1021, init 0xFFFF) used by the PIX BR Code, as 4 hex
/// uppercase digits.
fn crc16(data: &str) -> String {
    let mut crc: u16 = 0xFFFF;
    for &b in data.as_bytes() {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    format!("{crc:04X}")
}

/// Build the static "Pix Copia e Cola" (BR Code) payload for a key, with no
/// amount (donor chooses). Ends with the CRC16 over the whole string.
fn pix_brcode(key: &str, name: &str, city: &str) -> String {
    let gui = emv("00", "br.gov.bcb.pix");
    let merchant = emv("26", &format!("{gui}{}", emv("01", key)));
    let txid = emv("62", &emv("05", "***"));
    let payload = format!(
        "{}{merchant}{}{}{}{}{}{txid}",
        emv("00", "01"),   // payload format indicator
        emv("52", "0000"), // merchant category code
        emv("53", "986"),  // currency: BRL
        emv("58", "BR"),   // country
        emv("59", name),   // receiver name
        emv("60", city),   // receiver city
    );
    let to_crc = format!("{payload}6304");
    format!("{payload}6304{}", crc16(&to_crc))
}

/// A QR-code widget rendering `data`, drawn with cairo (black modules on a white
/// quiet-zone background so it scans on any theme).
fn qr_widget(data: &str, px: i32) -> gtk::Widget {
    let area = gtk::DrawingArea::new();
    area.set_content_width(px);
    area.set_content_height(px);
    area.set_halign(gtk::Align::Center);
    let Ok(code) = qrcode::QrCode::new(data.as_bytes()) else {
        return area.upcast();
    };
    let width = code.width();
    let modules = code.into_colors();
    area.set_draw_func(move |_, cr, w, h| {
        let quiet = 4_usize;
        let total = (width + quiet * 2) as f64;
        let scale = (w.min(h) as f64) / total;
        // White background (full quiet zone).
        cr.set_source_rgb(1.0, 1.0, 1.0);
        let _ = cr.paint();
        cr.set_source_rgb(0.0, 0.0, 0.0);
        for y in 0..width {
            for x in 0..width {
                if modules[y * width + x] == qrcode::Color::Dark {
                    let px = (x + quiet) as f64 * scale;
                    let py = (y + quiet) as f64 * scale;
                    cr.rectangle(px, py, scale.ceil(), scale.ceil());
                }
            }
        }
        let _ = cr.fill();
    });
    area.upcast()
}

/// "Support BigTube" dialog: a copyable PIX key plus buttons to the online
/// donation platforms (only those with a URL configured are shown).
pub(crate) fn show_donations_dialog(parent: &impl IsA<gtk::Window>) {
    let win = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .default_width(440)
        .title(tr("Donations"))
        .build();
    apply_theme_classes(&win);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());

    let page = gtk::Box::new(gtk::Orientation::Vertical, 18);
    page.set_margin_top(18);
    page.set_margin_bottom(18);
    page.set_margin_start(18);
    page.set_margin_end(18);

    let intro = gtk::Label::new(Some(&tr(
        "If BigTube is useful to you, consider supporting its development. Thank you! ❤️",
    )));
    intro.set_wrap(true);
    intro.set_xalign(0.0);
    page.append(&intro);

    // PIX QR — scan with any bank app. Encodes the full "Copia e Cola" payload.
    let brcode = pix_brcode(PIX_KEY, PIX_NAME, PIX_CITY);
    let qr_frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    qr_frame.set_halign(gtk::Align::Center);
    qr_frame.add_css_class("card");
    qr_frame.set_margin_top(4);
    let qr = qr_widget(&brcode, 200);
    qr.set_margin_top(12);
    qr.set_margin_bottom(12);
    qr.set_margin_start(12);
    qr.set_margin_end(12);
    qr_frame.append(&qr);
    page.append(&qr_frame);

    let group = adw::PreferencesGroup::new();

    // PIX — copyable key (works in any bank app via Pix → key).
    let pix = adw::ActionRow::builder()
        .title("PIX")
        .subtitle(PIX_KEY)
        .build();
    pix.set_subtitle_selectable(true);
    let copy = gtk::Button::with_label(&tr("Copy"));
    copy.add_css_class("flat");
    copy.set_valign(gtk::Align::Center);
    {
        let win = win.clone();
        copy.connect_clicked(move |b| {
            win.clipboard().set_text(PIX_KEY);
            b.set_label(&tr("Copied!"));
            b.set_sensitive(false);
            let b = b.clone();
            glib::timeout_add_seconds_local(2, move || {
                b.set_label(&tr("Copy"));
                b.set_sensitive(true);
                glib::ControlFlow::Break
            });
        });
    }
    pix.add_suffix(&copy);
    pix.set_activatable_widget(Some(&copy));
    group.add(&pix);

    // "Pix Copia e Cola" — copy the full BR Code for apps that paste it.
    let copia = adw::ActionRow::builder()
        .title(tr_markup("Pix Copy & Paste"))
        .build();
    let copy_code = gtk::Button::with_label(&tr("Copy"));
    copy_code.add_css_class("flat");
    copy_code.set_valign(gtk::Align::Center);
    {
        let win = win.clone();
        let brcode = brcode.clone();
        copy_code.connect_clicked(move |b| {
            win.clipboard().set_text(&brcode);
            b.set_label(&tr("Copied!"));
            b.set_sensitive(false);
            let b = b.clone();
            glib::timeout_add_seconds_local(2, move || {
                b.set_label(&tr("Copy"));
                b.set_sensitive(true);
                glib::ControlFlow::Break
            });
        });
    }
    copia.add_suffix(&copy_code);
    copia.set_activatable_widget(Some(&copy_code));
    group.add(&copia);

    // Online platforms (rendered only when a URL is configured).
    for (label, url) in DONATE_LINKS {
        if url.is_empty() {
            continue;
        }
        let row = adw::ActionRow::builder()
            .title(*label)
            .subtitle(*url)
            .build();
        let open = gtk::Button::with_label(&tr("Open"));
        open.add_css_class("flat");
        open.add_css_class("suggested-action");
        open.set_valign(gtk::Align::Center);
        {
            let url = url.to_string();
            let win = win.clone();
            open.connect_clicked(move |_| {
                gtk::UriLauncher::new(&url).launch(Some(&win), gtk::gio::Cancellable::NONE, |_| {});
            });
        }
        row.add_suffix(&open);
        row.set_activatable_widget(Some(&open));
        group.add(&row);
    }

    page.append(&group);
    toolbar.set_content(Some(&page));
    win.set_content(Some(&toolbar));
    win.present();
}

#[cfg(test)]
mod pix_tests {
    use super::{crc16, pix_brcode, PIX_CITY, PIX_KEY, PIX_NAME};

    #[test]
    fn crc16_matches_known_vector() {
        // CRC-16/CCITT-FALSE check value for "123456789" is 0x29B1.
        assert_eq!(crc16("123456789"), "29B1");
    }

    #[test]
    fn pix_brcode_is_well_formed() {
        let code = pix_brcode(PIX_KEY, PIX_NAME, PIX_CITY);
        // Starts with the EMV payload-format indicator and embeds the pix GUI.
        assert!(code.starts_with("000201"));
        assert!(code.contains("br.gov.bcb.pix"));
        assert!(code.contains(PIX_KEY));
        // Final CRC (last 4 chars) must equal crc16 over everything up to "6304".
        let (body, crc) = code.split_at(code.len() - 4);
        assert!(body.ends_with("6304"));
        assert_eq!(crc16(body), crc);
    }
}
