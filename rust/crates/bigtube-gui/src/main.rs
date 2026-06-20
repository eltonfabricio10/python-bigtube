//! BigTube GUI (Rust port).
//!
//! libadwaita front-end over `bigtube-core`. Fase 2 = shell; Fase 3 wires the
//! Search and Downloads pages to the core engine. Reuses the project `style.css`.

mod app;
mod dialog;
mod i18n;
mod objects;
mod player;
mod playlist;
mod row;
mod schedule;

use adw::prelude::*;

use bigtube_core::enums::APP_ID;

const STYLE_CSS: &str = include_str!("../../../../src/bigtube/data/style.css");

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Pick the GSK rendering engine from settings (default "cairo", software).
    // The cairo renderer dodges GL/Vulkan scroll flicker seen on some Mesa
    // drivers, but its SHM buffers can themselves glitch under some newer
    // Mesa/compositor combos — so the engine is user-selectable in Settings
    // (Appearance → Rendering Engine). An explicit GSK_RENDERER in the
    // environment always wins; "default" leaves the choice to GTK.
    if std::env::var_os("GSK_RENDERER").is_none() {
        let choice = bigtube_core::config::global()
            .read()
            .map(|c| c.get_string("gsk_renderer"))
            .unwrap_or_else(|_| "cairo".to_string());
        match choice.as_str() {
            "" | "default" | "auto" => {} // let GTK pick (GPU when available)
            other => std::env::set_var("GSK_RENDERER", other),
        }
    }

    gstreamer::init().expect("GStreamer init failed");
    i18n::init();

    // Drop libadwaita's one cosmetic warning about the desktop's legacy
    // `gtk-application-prefer-dark-theme` setting (we theme via AdwStyleManager).
    // Everything else from the Adwaita domain is passed through unchanged.
    gtk::glib::log_set_handler(
        Some("Adwaita"),
        gtk::glib::LogLevels::all(),
        false,
        false,
        |domain, level, message| {
            if message.contains("gtk-application-prefer-dark-theme") {
                return;
            }
            gtk::glib::log_default_handler(domain, level, Some(message));
        },
    );

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| {
        // Silence libadwaita's "gtk-application-prefer-dark-theme is unsupported"
        // warning: many desktops set that in ~/.config/gtk-4.0/settings.ini, but
        // we drive dark/light via AdwStyleManager. Reset the legacy flag so the
        // two don't fight (and the warning doesn't spam the log).
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_application_prefer_dark_theme(false);
        }
        load_css();
    });
    // Single instance: GApplication is already unique via APP_ID (a second launch
    // forwards `activate` to the running process). Without this guard, that second
    // activation would build ANOTHER window in the same process — so re-opening
    // BigTube must just raise the existing window instead of duplicating it.
    app.connect_activate(|app| match app.active_window() {
        Some(win) => win.present(),
        None => app::build_window(app),
    });
    app.run();
}

fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(STYLE_CSS);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
