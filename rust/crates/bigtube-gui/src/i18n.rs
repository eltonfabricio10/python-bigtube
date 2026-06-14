//! Localization, mirroring `locales.py`. Reuses the project's existing
//! `.po`/`.mo` catalogs (16 languages) via gettext — the Rust `msgid`s are the
//! same English source strings the Python `N_()` markers used.

use std::path::Path;

use gettextrs::{bind_textdomain_codeset, bindtextdomain, setlocale, textdomain, LocaleCategory};

const DOMAIN: &str = "bigtube";

/// Initialize gettext: pick the system locale and bind the `bigtube` domain to
/// the installed catalogs (falling back to a local dev directory).
pub fn init() {
    let _ = setlocale(LocaleCategory::LcAll, "");

    // Priority: system install → local dev build output.
    let system = Path::new("/usr/share/locale");
    let dir = if system.exists() {
        system.to_path_buf()
    } else {
        // <repo>/src/bigtube/data/locales (where Python compiles .mo for dev)
        std::env::current_dir()
            .map(|d| d.join("src/bigtube/data/locales"))
            .unwrap_or_else(|_| system.to_path_buf())
    };

    let _ = bindtextdomain(DOMAIN, dir);
    let _ = bind_textdomain_codeset(DOMAIN, "UTF-8");
    let _ = textdomain(DOMAIN);
}

/// Translate a message id (the English source string).
pub fn tr(msgid: &str) -> String {
    gettextrs::gettext(msgid)
}
