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

    let system = Path::new("/usr/share/locale");
    let dev = std::env::current_dir()
        .map(|d| d.join("target/locale"))
        .unwrap_or_else(|_| system.to_path_buf());

    // Priority: a per-user override (~/.local/share/locale) — but ONLY when it
    // actually has the current locale's catalog, so we never shadow the complete
    // system catalogs for other languages — then system install → dev output.
    let lang = current_lang();
    let dir = match user_locale_dir() {
        Some(u)
            if !lang.is_empty()
                && u.join(&lang)
                    .join("LC_MESSAGES")
                    .join(format!("{DOMAIN}.mo"))
                    .exists() =>
        {
            u
        }
        _ if system.exists() => system.to_path_buf(),
        _ => dev,
    };

    let _ = bindtextdomain(DOMAIN, dir);
    let _ = bind_textdomain_codeset(DOMAIN, "UTF-8");
    let _ = textdomain(DOMAIN);
}

/// Current message locale (e.g. "pt_BR"), stripped of encoding/modifier.
fn current_lang() -> String {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(key) {
            let v = v.split(['.', '@']).next().unwrap_or("").trim();
            if !v.is_empty() && v != "C" && v != "POSIX" {
                return v.to_string();
            }
        }
    }
    String::new()
}

/// Per-user locale dir: `$XDG_DATA_HOME/locale` or `~/.local/share/locale`.
fn user_locale_dir() -> Option<std::path::PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return Some(Path::new(&xdg).join("locale"));
        }
    }
    std::env::var("HOME")
        .ok()
        .filter(|h| !h.is_empty())
        .map(|h| Path::new(&h).join(".local/share/locale"))
}

/// Translate a message id (the English source string).
pub fn tr(msgid: &str) -> String {
    gettextrs::gettext(msgid)
}
