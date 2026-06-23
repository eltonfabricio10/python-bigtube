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

const STYLE_CSS: &str = include_str!("../assets/style.css");

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Force GSK to fully redraw every frame. GTK's partial-damage optimization
    // under-damages while scrolling the results list on some GTK4/Mesa/KWin
    // stacks, leaving stale "ghost" text/thumbnails behind until a hover
    // repaints the row. Full redraws sidestep that at a negligible cost for an
    // app this light. Append so an explicit GSK_DEBUG from the environment wins.
    //
    // Set BIGTUBE_NO_FULL_REDRAW=1 to skip the workaround (to check whether the
    // underlying driver/GTK bug still reproduces on the current stack).
    if std::env::var_os("BIGTUBE_NO_FULL_REDRAW").is_none() {
        let gsk_debug = match std::env::var("GSK_DEBUG") {
            Ok(v) if !v.is_empty() => format!("{v},full-redraw"),
            _ => "full-redraw".to_string(),
        };
        std::env::set_var("GSK_DEBUG", gsk_debug);
    }

    gstreamer::init().expect("GStreamer init failed");
    i18n::init();

    // Register the bundled symbolic icons so the app never shows broken/missing
    // glyphs when the system icon theme lacks them. The matching resource path is
    // added to the icon theme in `connect_startup` (once a display exists).
    register_bundled_icons();

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
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::IconTheme::for_display(&display)
                .add_resource_path("/io/github/eltonfabricio10/bigtube/icons");
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

/// Register the icon GResource compiled by `build.rs` so the bundled symbolic
/// icons are available to the icon theme.
fn register_bundled_icons() {
    let bytes = gtk::glib::Bytes::from_static(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/bigtube.gresource"
    )));
    match gtk::gio::Resource::from_data(&bytes) {
        Ok(res) => gtk::gio::resources_register(&res),
        Err(e) => tracing::warn!("failed to register bundled icons: {e}"),
    }
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
