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

    gstreamer::init().expect("GStreamer init failed");
    i18n::init();

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| load_css());
    app.connect_activate(app::build_window);
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
