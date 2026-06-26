//! Settings page: builds the libadwaita preferences UI (appearance, downloads,
//! subtitles, playback, network, storage, converter, search) and the small
//! folder/file pickers and validators behind it. Wiring back to persistence
//! goes through the parent module's `set_cfg`/config helpers.

use std::rc::Rc;

use adw::prelude::*;

use bigtube_core::config;

use super::widgets::{button_row, combo_row, spin_row, spin_row_step, switch_row};
use super::{
    apply_theme, clear_converter_history, clear_search_history, export_history, import_history,
    refresh_version_subtitle, reset_all_data, run_update, set_cfg, tr_markup, AppState,
    QUALITY_OPTIONS,
};
use crate::i18n::tr;

/// In-app preview/player quality options (config values double as labels).
const PREVIEW_QUALITIES: &[&str] = &["144p", "240p", "360p", "480p", "720p"];

/// Human-readable accent-colour name (matches `locales.py` so the catalogs resolve).
fn color_label(value: &str) -> &'static str {
    match value {
        "violet" => "Modern Violet",
        "emerald" => "Emerald Green",
        "sunburst" => "Sunburst Orange",
        "rose" => "Vibrant Rose",
        "cyan" => "Nordic Cyan",
        "nordic" => "Nordic Snow",
        "gruvbox" => "Gruvbox Retro",
        "catppuccin" => "Catppuccin Mocha",
        "dracula" => "Dracula Dark",
        "tokyo_night" => "Tokyo Night",
        "rose_pine" => "Rosé Pine",
        "solarized" => "Solarized Dark",
        "monokai" => "Monokai Pro",
        "cyberpunk" => "Cyberpunk Neon",
        "bigtube" => "BigTube Brand",
        _ => "Default Blue",
    }
}

/// Browsers offered for "Cookies From Browser", detected on PATH (`download_settings.py`).
/// Detect installed browsers by probing the *real* binary at canonical absolute
/// install paths (`/usr/bin`, `/usr/lib`, `/opt`) — deliberately NOT a `$PATH`
/// lookup. Tools like `auto-tweaks-browser` drop wrapper scripts into
/// `/usr/local/bin` (and `~/.local/bin`) for every browser, so a PATH/`which`
/// scan reports browsers that aren't actually installed. We only trust the real
/// package locations and reject anything that resolves into a wrapper.
fn detect_browsers() -> Vec<(&'static str, String)> {
    let candidates: [(&str, &str, &[&str]); 7] = [
        (
            "firefox",
            "Firefox",
            &[
                "/usr/bin/firefox",
                "/usr/lib/firefox/firefox",
                "/opt/firefox/firefox",
            ],
        ),
        (
            "chrome",
            "Chrome",
            &[
                "/usr/bin/google-chrome-stable",
                "/usr/bin/google-chrome",
                "/opt/google/chrome/google-chrome",
            ],
        ),
        (
            "chromium",
            "Chromium",
            &["/usr/bin/chromium", "/usr/lib/chromium/chromium"],
        ),
        (
            "brave",
            "Brave",
            &[
                "/usr/bin/brave",
                "/usr/bin/brave-browser",
                "/opt/brave-bin/brave",
                "/opt/brave.com/brave/brave",
            ],
        ),
        (
            "edge",
            "Microsoft Edge",
            &[
                "/usr/bin/microsoft-edge",
                "/usr/bin/microsoft-edge-stable",
                "/opt/microsoft/msedge/microsoft-edge",
            ],
        ),
        (
            "vivaldi",
            "Vivaldi",
            &[
                "/usr/bin/vivaldi",
                "/usr/bin/vivaldi-stable",
                "/opt/vivaldi/vivaldi",
            ],
        ),
        (
            "opera",
            "Opera",
            &["/usr/bin/opera", "/usr/lib/x86_64-linux-gnu/opera/opera"],
        ),
    ];
    let mut out: Vec<(&str, String)> = vec![("", tr("None"))];
    for (val, label, paths) in candidates {
        if paths.iter().any(|p| is_real_browser_binary(p)) {
            out.push((val, label.to_string()));
        }
    }
    out
}

/// True if `path` is a real browser binary — it exists and does not resolve into
/// a wrapper directory (`/usr/local/...`) or an `auto-tweaks-browser` shim.
fn is_real_browser_binary(path: &str) -> bool {
    let p = std::path::Path::new(path);
    if !p.exists() {
        return false;
    }
    match std::fs::canonicalize(p) {
        Ok(real) => {
            let s = real.to_string_lossy();
            !s.starts_with("/usr/local/") && !s.contains("browser-tweaks")
        }
        Err(_) => true,
    }
}
pub(crate) fn build_settings_page(state: &Rc<AppState>) -> gtk::Widget {
    let page = adw::PreferencesPage::new();

    // Snapshot every config value up front (drops the read lock before wiring).
    let c = {
        let cfg = config::global().read().unwrap();
        Cfg {
            theme_mode: cfg.get_string("theme_mode"),
            theme_color: cfg.get_string("theme_color"),
            default_quality: cfg.get_string("default_quality"),
            preview_quality: cfg.get_string("preview_quality"),
            download_path: cfg.get_string("download_path"),
            max_concurrent: cfg.get_i64("max_concurrent_downloads"),
            concurrent_fragments: cfg.get_i64("concurrent_fragments"),
            max_download_history: cfg.get_i64("max_download_history"),
            max_converter_history: cfg.get_i64("max_converter_history"),
            check_updates_on_startup: cfg.get_bool("check_updates_on_startup"),
            rate_limit: cfg.get_i64("rate_limit"),
            add_metadata: cfg.get_bool("add_metadata"),
            sponsorblock_mode: cfg.get_string("sponsorblock_mode"),
            subtitle_mode: cfg.get_string("subtitle_mode"),
            subtitle_langs: cfg.get_string("subtitle_langs"),
            subtitle_auto: cfg.get_bool("subtitle_auto"),
            system_notifications: cfg.get_bool("system_notifications"),
            monitor_clipboard: cfg.get_bool("monitor_clipboard"),
            remove_on_complete: cfg.get_bool("remove_on_complete"),
            remove_on_cancel: cfg.get_bool("remove_on_cancel"),
            converter_remove_on_complete: cfg.get_bool("converter_remove_on_complete"),
            converter_remove_on_cancel: cfg.get_bool("converter_remove_on_cancel"),
            post_process_cmd: cfg.get_string("post_process_cmd"),
            cookies_file: cfg.get_string("cookies_file"),
            cookies_browser: cfg.get_string("cookies_browser"),
            user_agent: cfg.get_string("user_agent"),
            proxy: cfg.get_string("proxy"),
            save_history: cfg.get_bool("save_history"),
            auto_clear_finished: cfg.get_bool("auto_clear_finished"),
            converter_path: cfg.get_string("converter_path"),
            use_source_folder: cfg.get_bool("use_source_folder"),
            save_converter_history: cfg.get_bool("save_converter_history"),
            search_limit: cfg.get_i64("search_limit"),
            enable_suggestions: cfg.get_bool("enable_suggestions"),
            max_suggestions: cfg.get_i64("max_suggestions"),
            save_search_history: cfg.get_bool("save_search_history"),
        }
    };

    page.add(&build_appearance_group(state, &c));
    page.add(&build_downloads_group(state, &c));
    page.add(&build_postprocessing_group(state, &c));
    page.add(&build_subtitles_group(state, &c));
    page.add(&build_playback_group(state, &c));
    page.add(&build_converter_group(state, &c));
    page.add(&build_search_group(state, &c));
    page.add(&build_network_group(state, &c));
    page.add(&build_storage_group(state, &c));

    page.upcast()
}

/// Snapshot of every setting read once when the page is built.
struct Cfg {
    theme_mode: String,
    theme_color: String,
    default_quality: String,
    preview_quality: String,
    download_path: String,
    max_concurrent: i64,
    concurrent_fragments: i64,
    max_download_history: i64,
    max_converter_history: i64,
    check_updates_on_startup: bool,
    rate_limit: i64,
    add_metadata: bool,
    sponsorblock_mode: String,
    subtitle_mode: String,
    subtitle_langs: String,
    subtitle_auto: bool,
    system_notifications: bool,
    monitor_clipboard: bool,
    remove_on_complete: bool,
    remove_on_cancel: bool,
    converter_remove_on_complete: bool,
    converter_remove_on_cancel: bool,
    post_process_cmd: String,
    cookies_file: String,
    cookies_browser: String,
    user_agent: String,
    proxy: String,
    save_history: bool,
    auto_clear_finished: bool,
    converter_path: String,
    use_source_folder: bool,
    save_converter_history: bool,
    search_limit: i64,
    enable_suggestions: bool,
    max_suggestions: i64,
    save_search_history: bool,
}

fn build_appearance_group(state: &Rc<AppState>, c: &Cfg) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Appearance"))
        .build();

    // Interface theme.
    let theme_modes = ["system", "light", "dark"];
    let theme_row = combo_row(
        &tr("Interface Theme"),
        &[tr("System"), tr("Light"), tr("Dark")],
    );
    theme_row.set_subtitle(&tr("Follow the system or force light/dark"));
    theme_row.set_selected(
        theme_modes
            .iter()
            .position(|m| *m == c.theme_mode)
            .unwrap_or(0) as u32,
    );
    {
        let state = state.clone();
        theme_row.connect_selected_notify(move |row| {
            let val = theme_modes
                .get(row.selected() as usize)
                .copied()
                .unwrap_or("system");
            set_cfg("theme_mode", serde_json::json!(val));
            if let Some(w) = state.window.borrow().clone() {
                apply_theme(&w);
            }
        });
    }
    group.add(&theme_row);

    // Colour scheme (pretty, translated labels).
    let color_values: Vec<&str> = bigtube_core::enums::ThemeColor::ALL
        .iter()
        .map(|c| c.as_value())
        .collect();
    let color_labels: Vec<String> = color_values.iter().map(|v| tr(color_label(v))).collect();
    let color_row = combo_row(&tr("Color Scheme"), &color_labels);
    color_row.set_subtitle(&tr("Accent colour used across the app"));
    color_row.set_selected(
        color_values
            .iter()
            .position(|v| *v == c.theme_color)
            .unwrap_or(0) as u32,
    );
    {
        let state = state.clone();
        let color_values = color_values.clone();
        color_row.connect_selected_notify(move |row| {
            let val = color_values
                .get(row.selected() as usize)
                .copied()
                .unwrap_or("default");
            set_cfg("theme_color", serde_json::json!(val));
            if let Some(w) = state.window.borrow().clone() {
                apply_theme(&w);
            }
        });
    }
    group.add(&color_row);

    // Current version + yt-dlp update.
    let version_row = adw::ActionRow::builder()
        .title(tr("Current Version"))
        .subtitle("yt-dlp v?")
        .build();
    let update_btn = gtk::Button::from_icon_name("bigtube-software-update-symbolic");
    update_btn.add_css_class("flat");
    update_btn.set_valign(gtk::Align::Center);
    version_row.add_suffix(&update_btn);
    group.add(&version_row);
    refresh_version_subtitle(&version_row);
    {
        let state = state.clone();
        let version_row = version_row.clone();
        update_btn.connect_clicked(move |btn| {
            btn.set_sensitive(false);
            run_update(&state, &version_row, btn.clone());
        });
    }

    group.add(&switch_row(
        &tr("Check for Updates on Startup"),
        &tr("On launch, download missing components and notify if yt-dlp has an update"),
        c.check_updates_on_startup,
        |v| set_cfg("check_updates_on_startup", serde_json::json!(v)),
    ));

    group
}

fn build_downloads_group(state: &Rc<AppState>, c: &Cfg) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Downloads"))
        .build();

    // Download folder.
    let folder_row = adw::ActionRow::builder()
        .title(tr("Download Folder"))
        .subtitle(&c.download_path)
        .build();
    let folder_btn = gtk::Button::from_icon_name("bigtube-folder-open-symbolic");
    folder_btn.add_css_class("flat");
    folder_btn.set_valign(gtk::Align::Center);
    {
        let state = state.clone();
        let folder_row = folder_row.clone();
        folder_btn.connect_clicked(move |_| pick_download_folder(&state, &folder_row));
    }
    folder_row.add_suffix(&folder_btn);
    group.add(&folder_row);

    // Preferred quality (translated labels).
    let quality_labels: Vec<String> = QUALITY_OPTIONS.iter().map(|(l, _)| tr(l)).collect();
    let quality_row = combo_row(&tr("Preferred Quality"), &quality_labels);
    quality_row.set_subtitle(&tr("Default quality for new downloads"));
    let qsel = QUALITY_OPTIONS
        .iter()
        .position(|(_, q)| q.as_value() == c.default_quality)
        .unwrap_or(0);
    quality_row.set_selected(qsel as u32);
    quality_row.connect_selected_notify(|row| {
        if let Some((_, q)) = QUALITY_OPTIONS.get(row.selected() as usize) {
            set_cfg("default_quality", serde_json::json!(q.as_value()));
        }
    });
    group.add(&quality_row);

    group.add(&spin_row(
        &tr("Max Simultaneous Downloads"),
        &tr("How many downloads run at the same time"),
        1.0,
        10.0,
        c.max_concurrent as f64,
        |v| set_cfg("max_concurrent_downloads", serde_json::json!(v as i64)),
    ));
    group.add(&switch_row(
        &tr("Save Download History"),
        &tr("Keep a record of completed downloads"),
        c.save_history,
        |v| set_cfg("save_history", serde_json::json!(v)),
    ));
    group.add(&spin_row_step(
        &tr("Maximum History Entries"),
        &tr("How many finished downloads to keep in the list"),
        10.0,
        1000.0,
        10.0,
        c.max_download_history as f64,
        |v| set_cfg("max_download_history", serde_json::json!(v as i64)),
    ));
    group.add(&switch_row(
        &tr("Remove When Complete"),
        &tr("Automatically remove an item from the list once it finishes"),
        c.remove_on_complete,
        |v| set_cfg("remove_on_complete", serde_json::json!(v)),
    ));
    group.add(&switch_row(
        &tr("Remove When Cancelled"),
        &tr("Automatically remove an item from the list when it is cancelled"),
        c.remove_on_cancel,
        |v| set_cfg("remove_on_cancel", serde_json::json!(v)),
    ));
    group.add(&switch_row(
        &tr("System Notifications"),
        &tr("Notify when a download finishes"),
        c.system_notifications,
        |v| set_cfg("system_notifications", serde_json::json!(v)),
    ));
    group.add(&switch_row(
        &tr("Enable ClipBoard Monitor"),
        &tr("Detect copied links and offer to download them"),
        c.monitor_clipboard,
        |v| set_cfg("monitor_clipboard", serde_json::json!(v)),
    ));

    group
}

/// Output post-processing applied to finished downloads (all need ffmpeg).
fn build_postprocessing_group(state: &Rc<AppState>, c: &Cfg) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Post-Processing"))
        .build();

    group.add(&switch_row(
        &tr("Add Metadata to Files"),
        &tr("Embed title, artist and other tags in the file"),
        c.add_metadata,
        |v| set_cfg("add_metadata", serde_json::json!(v)),
    ));

    // SponsorBlock: skip in-video sponsor/self-promo segments using the
    // community database. "Mark" adds chapters (non-destructive); "Remove"
    // cuts them out. Both need ffmpeg.
    let sb_modes = ["off", "mark", "remove"];
    let sb_row = combo_row(
        &tr("SponsorBlock"),
        &[tr("Off"), tr("Mark chapters"), tr("Remove segments")],
    );
    sb_row.set_subtitle(&tr(
        "Skip in-video sponsor segments (SponsorBlock database)",
    ));
    sb_row.set_selected(
        sb_modes
            .iter()
            .position(|m| *m == c.sponsorblock_mode)
            .unwrap_or(0) as u32,
    );
    sb_row.connect_selected_notify(move |row| {
        let val = sb_modes
            .get(row.selected() as usize)
            .copied()
            .unwrap_or("off");
        set_cfg("sponsorblock_mode", serde_json::json!(val));
    });
    group.add(&sb_row);

    group.add(&entry_row_with_presets(
        &tr("Post-Processing Command"),
        &c.post_process_cmd,
        &tr("Common commands"),
        to_presets(&POST_PROCESS_PRESETS),
        "post_process_cmd",
        state,
        validate_post_process,
    ));

    group
}

/// Subtitle download settings: mode, languages, and auto-generated captions.
fn build_subtitles_group(_state: &Rc<AppState>, c: &Cfg) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Subtitles"))
        .build();

    // Mode: off / embed in the video / separate file / both.
    let modes = ["off", "embed", "file", "both"];
    let mode_row = combo_row(
        &tr("Subtitles"),
        &[
            tr("Off"),
            tr("Embed in video"),
            tr("Separate file"),
            tr("Embed + file"),
        ],
    );
    mode_row.set_subtitle(&tr("Download subtitles and how to store them"));
    mode_row.set_selected(
        modes
            .iter()
            .position(|m| *m == c.subtitle_mode)
            .unwrap_or(0) as u32,
    );
    mode_row.connect_selected_notify(move |row| {
        let val = modes.get(row.selected() as usize).copied().unwrap_or("off");
        set_cfg("subtitle_mode", serde_json::json!(val));
    });
    group.add(&mode_row);

    // Languages (comma-separated, validated lightly).
    let lang_row = adw::EntryRow::builder()
        .title(tr("Languages"))
        .text(&c.subtitle_langs)
        .show_apply_button(true)
        .build();
    lang_row.set_tooltip_text(Some(&tr("Comma-separated language codes, e.g. pt, en, es")));
    lang_row.connect_apply(|r| {
        let txt = r.text().trim().to_string();
        set_cfg("subtitle_langs", serde_json::json!(txt));
    });
    group.add(&lang_row);

    group.add(&switch_row(
        &tr("Include Auto-generated"),
        &tr("Also fetch automatic (machine) captions"),
        c.subtitle_auto,
        |v| set_cfg("subtitle_auto", serde_json::json!(v)),
    ));

    group
}

/// In-app player / preview settings.
fn build_playback_group(_state: &Rc<AppState>, c: &Cfg) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Playback"))
        .build();

    // In-app preview/player quality. 360p is progressive (rock-solid); 480p/720p
    // stream via HLS. Takes effect on the next item played.
    let preview_row = combo_row(&tr("Preview Quality"), PREVIEW_QUALITIES);
    preview_row.set_subtitle(&tr("Quality used by the in-app player"));
    let psel = PREVIEW_QUALITIES
        .iter()
        .position(|q| *q == c.preview_quality)
        .unwrap_or(0);
    preview_row.set_selected(psel as u32);
    preview_row.connect_selected_notify(|row| {
        if let Some(q) = PREVIEW_QUALITIES.get(row.selected() as usize) {
            set_cfg("preview_quality", serde_json::json!(q));
        }
    });
    group.add(&preview_row);
    group
}

/// Network, authentication and advanced (cookies, proxy, UA, post-processing).
fn build_network_group(state: &Rc<AppState>, c: &Cfg) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr_markup("Network & Advanced"))
        .build();

    group.add(&spin_row(
        &tr("Concurrent Fragments"),
        &tr("Parallel fragments per download (faster, uses more bandwidth)"),
        1.0,
        16.0,
        c.concurrent_fragments as f64,
        |v| set_cfg("concurrent_fragments", serde_json::json!(v as i64)),
    ));
    group.add(&spin_row_step(
        &tr("Download Speed Limit (KB/s)"),
        &tr("Cap the download rate (0 = unlimited)"),
        0.0,
        100_000.0,
        100.0,
        c.rate_limit as f64,
        |v| set_cfg("rate_limit", serde_json::json!(v as i64)),
    ));

    // Cookies file.
    let cookies_row = adw::ActionRow::builder()
        .title(tr("Cookies File"))
        .subtitle(&c.cookies_file)
        .build();
    let cookies_btn = gtk::Button::from_icon_name("bigtube-document-open-symbolic");
    cookies_btn.add_css_class("flat");
    cookies_btn.set_valign(gtk::Align::Center);
    {
        let state = state.clone();
        let cookies_row = cookies_row.clone();
        cookies_btn.connect_clicked(move |_| pick_cookies_file(&state, &cookies_row));
    }
    cookies_row.add_suffix(&cookies_btn);
    group.add(&cookies_row);

    // Cookies from browser (detected on PATH).
    let browsers = detect_browsers();
    let browser_labels: Vec<String> = browsers.iter().map(|(_, l)| l.clone()).collect();
    let browser_row = combo_row(&tr("Cookies From Browser"), &browser_labels);
    browser_row.set_subtitle(&tr("Use this browser's cookies for restricted videos"));
    let bsel = browsers
        .iter()
        .position(|(v, _)| *v == c.cookies_browser)
        .unwrap_or(0);
    browser_row.set_selected(bsel as u32);
    {
        let browsers: Vec<&str> = browsers.iter().map(|(v, _)| *v).collect();
        browser_row.connect_selected_notify(move |row| {
            let val = browsers.get(row.selected() as usize).copied().unwrap_or("");
            set_cfg("cookies_browser", serde_json::json!(val));
        });
    }
    group.add(&browser_row);

    group.add(&entry_row_with_presets(
        &tr("User Agent"),
        &c.user_agent,
        &tr("Installed browsers"),
        user_agent_presets(),
        "user_agent",
        state,
        validate_user_agent,
    ));
    group.add(&entry_row_with_presets(
        &tr("Proxy"),
        &c.proxy,
        &tr("Known proxies"),
        to_presets(&PROXY_PRESETS),
        "proxy",
        state,
        validate_proxy,
    ));

    group
}

/// Common `yt-dlp --exec` post-processing commands (`{}` = the output file).
const POST_PROCESS_PRESETS: [(&str, &str); 5] = [
    ("Choose a preset…", ""),
    ("Desktop notification", "notify-send 'BigTube' 'Done: {}'"),
    ("Open output folder", "xdg-open \"$(dirname \"{}\")\""),
    ("Make read-only", "chmod 444 {}"),
    ("Update timestamp", "touch {}"),
];

/// Well-known *local* proxy endpoints. Public free-proxy IPs are ephemeral and
/// untrustworthy, so we offer reliable local setups (Tor/Privoxy) instead — the
/// field stays free-text for any custom proxy.
const PROXY_PRESETS: [(&str, &str); 4] = [
    ("Choose a preset…", ""),
    ("Tor (SOCKS5)", "socks5://127.0.0.1:9050"),
    ("Local HTTP proxy", "http://127.0.0.1:8080"),
    ("Privoxy (HTTP)", "http://127.0.0.1:8118"),
];

/// A current Linux User-Agent for a detected browser key (`detect_browsers`).
fn browser_ua(key: &str) -> Option<&'static str> {
    const CHROME: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
    Some(match key {
        "firefox" => "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0",
        "chrome" | "chromium" | "brave" | "vivaldi" => CHROME,
        "edge" => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0",
        "opera" => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 OPR/111.0.0.0",
        _ => return None,
    })
}

/// User-Agent presets for browsers actually installed on this machine — one
/// entry per installed browser ([`detect_browsers`] already filters to real
/// installs, not `$PATH` wrappers).
fn user_agent_presets() -> Vec<(String, String)> {
    let mut out = vec![(tr("Choose a preset…"), String::new())];
    for (key, label) in detect_browsers() {
        if key.is_empty() {
            continue; // the "None" sentinel
        }
        if let Some(ua) = browser_ua(key) {
            out.push((label, ua.to_string()));
        }
    }
    out
}

/// Translate the labels of a static preset table into an owned preset list.
fn to_presets(arr: &[(&str, &str)]) -> Vec<(String, String)> {
    arr.iter().map(|(l, v)| (tr(l), v.to_string())).collect()
}

/// Validator: empty (no proxy) or a `scheme://host:port` with a known scheme.
fn validate_proxy(s: &str) -> Option<String> {
    let (ok, _, _) = bigtube_core::config::ConfigManager::validate_proxy_url(s);
    if ok {
        None
    } else {
        Some(tr("Invalid proxy address — use scheme://host:port."))
    }
}

/// Validator: a User-Agent must be a single printable line (no control chars).
fn validate_user_agent(s: &str) -> Option<String> {
    if s.chars().any(char::is_control) {
        Some(tr("Invalid user agent."))
    } else {
        None
    }
}

/// Validator: the post-processing command's program must exist on `$PATH`.
fn validate_post_process(s: &str) -> Option<String> {
    let prog = s.split_whitespace().next().unwrap_or("");
    if prog.is_empty() || bigtube_core::util::which(prog).is_some() {
        None
    } else {
        Some(format!("{} {}", tr("Command not found on PATH:"), prog))
    }
}

/// An entry row with a suffix dropdown of presets and an apply-time validator;
/// choosing a preset fills the entry and persists `cfg_key`. The first preset is
/// a no-op placeholder. An invalid entry is rejected with a toast and reverted.
fn entry_row_with_presets(
    title: &str,
    value: &str,
    tooltip: &str,
    presets: Vec<(String, String)>,
    cfg_key: &'static str,
    state: &Rc<AppState>,
    validate: fn(&str) -> Option<String>,
) -> adw::EntryRow {
    let row = adw::EntryRow::builder()
        .title(title)
        .text(value)
        .show_apply_button(true)
        .build();
    {
        let state = state.clone();
        row.connect_apply(move |r| {
            let txt = r.text().trim().to_string();
            if !txt.is_empty() {
                if let Some(err) = validate(&txt) {
                    state.toast(&err);
                    // Revert to the last saved value so the bad input doesn't stick.
                    let saved = config::global().read().unwrap().get_string(cfg_key);
                    r.set_text(&saved);
                    return;
                }
            }
            set_cfg(cfg_key, serde_json::json!(txt));
        });
    }

    let dd = {
        let labels: Vec<&str> = presets.iter().map(|(l, _)| l.as_str()).collect();
        gtk::DropDown::from_strings(&labels)
    };
    dd.set_valign(gtk::Align::Center);
    dd.set_tooltip_text(Some(tooltip));
    {
        let row = row.clone();
        dd.connect_selected_notify(move |d| {
            if let Some((_, val)) = presets.get(d.selected() as usize) {
                if !val.is_empty() {
                    row.set_text(val);
                    set_cfg(cfg_key, serde_json::json!(val));
                }
            }
        });
    }
    row.add_suffix(&dd);
    row
}

fn build_storage_group(state: &Rc<AppState>, c: &Cfg) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Storage"))
        .build();

    group.add(&switch_row(
        &tr("Clear All Data on Exit"),
        &tr("Wipe history and finished items when the app closes"),
        c.auto_clear_finished,
        |v| set_cfg("auto_clear_finished", serde_json::json!(v)),
    ));

    // Export history.
    {
        let state = state.clone();
        group.add(&button_row(
            &tr("Export History"),
            &tr("Save your download history to a file"),
            "bigtube-document-export-symbolic",
            false,
            move || export_history(&state),
        ));
    }
    // Import history.
    {
        let state = state.clone();
        group.add(&button_row(
            &tr("Import History"),
            &tr("Restore history from a backup file"),
            "bigtube-document-import-symbolic",
            false,
            move || import_history(&state),
        ));
    }
    // Reset all data (destructive).
    {
        let state = state.clone();
        group.add(&button_row(
            &tr("Clear All App Data (Reset)"),
            &tr("Permanently delete all settings and history"),
            "bigtube-user-trash-symbolic",
            true,
            move || reset_all_data(&state),
        ));
    }

    group
}

fn build_converter_group(state: &Rc<AppState>, c: &Cfg) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Media Converter"))
        .build();

    let folder_row = adw::ActionRow::builder()
        .title(tr("Default Output Folder"))
        .subtitle(&c.converter_path)
        .build();
    let folder_btn = gtk::Button::from_icon_name("bigtube-folder-open-symbolic");
    folder_btn.add_css_class("flat");
    folder_btn.set_valign(gtk::Align::Center);
    {
        let state = state.clone();
        let folder_row = folder_row.clone();
        folder_btn.connect_clicked(move |_| pick_converter_folder(&state, &folder_row));
    }
    folder_row.add_suffix(&folder_btn);
    group.add(&folder_row);

    group.add(&switch_row(
        &tr("Save in Source Directory"),
        &tr("Write the converted file next to the original"),
        c.use_source_folder,
        |v| set_cfg("use_source_folder", serde_json::json!(v)),
    ));
    group.add(&switch_row(
        &tr("Save Conversion History"),
        &tr("Keep a record of converted files"),
        c.save_converter_history,
        |v| set_cfg("save_converter_history", serde_json::json!(v)),
    ));
    group.add(&switch_row(
        &tr("Remove When Complete"),
        &tr("Automatically remove an item from the list once it finishes"),
        c.converter_remove_on_complete,
        |v| set_cfg("converter_remove_on_complete", serde_json::json!(v)),
    ));
    group.add(&switch_row(
        &tr("Remove When Cancelled"),
        &tr("Automatically remove an item from the list when it is cancelled"),
        c.converter_remove_on_cancel,
        |v| set_cfg("converter_remove_on_cancel", serde_json::json!(v)),
    ));
    group.add(&spin_row_step(
        &tr("Maximum History Entries"),
        &tr("How many finished conversions to keep in the list"),
        10.0,
        500.0,
        10.0,
        c.max_converter_history as f64,
        |v| set_cfg("max_converter_history", serde_json::json!(v as i64)),
    ));
    {
        let state = state.clone();
        group.add(&button_row(
            &tr("Clear Conversion History"),
            &tr("Delete all previous conversion entries"),
            "bigtube-user-trash-symbolic",
            false,
            move || clear_converter_history(&state),
        ));
    }

    group
}

fn build_search_group(state: &Rc<AppState>, c: &Cfg) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Search Settings"))
        .build();

    group.add(&switch_row(
        &tr("Save Search History"),
        &tr("Remember past searches for suggestions"),
        c.save_search_history,
        |v| set_cfg("save_search_history", serde_json::json!(v)),
    ));
    group.add(&spin_row(
        &tr("Maximum Search Results"),
        &tr("How many results to fetch per search"),
        5.0,
        100.0,
        c.search_limit as f64,
        |v| set_cfg("search_limit", serde_json::json!(v as i64)),
    ));
    group.add(&switch_row(
        &tr("Enable Search Suggestions"),
        &tr("Show matches from your history while typing"),
        c.enable_suggestions,
        |v| set_cfg("enable_suggestions", serde_json::json!(v)),
    ));
    group.add(&spin_row(
        &tr("Maximum Suggestions"),
        &tr("How many suggestions to show"),
        1.0,
        50.0,
        c.max_suggestions as f64,
        |v| set_cfg("max_suggestions", serde_json::json!(v as i64)),
    ));
    {
        let state = state.clone();
        group.add(&button_row(
            &tr("Clear Search History"),
            &tr("Delete all previous search entries"),
            "bigtube-user-trash-symbolic",
            false,
            move || clear_search_history(&state),
        ));
    }

    group
}

/// A page banner (large title strip) shown at the top of each page.
/// A page title strip (full-width highlighted bar, matching the header bar
/// colour) with optional icon action buttons at the end.
fn pick_download_folder(state: &Rc<AppState>, folder_row: &adw::ActionRow) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = gtk::FileDialog::builder().title(tr("Pick Folder")).build();
    let folder_row = folder_row.clone();
    dialog.select_folder(Some(&window), gtk::gio::Cancellable::NONE, move |res| {
        if let Ok(file) = res {
            if let Some(path) = file.path() {
                let p = path.to_string_lossy().to_string();
                set_cfg("download_path", serde_json::json!(p));
                folder_row.set_subtitle(&p);
            }
        }
    });
}

fn pick_converter_folder(state: &Rc<AppState>, folder_row: &adw::ActionRow) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = gtk::FileDialog::builder()
        .title(tr("Default Output Folder"))
        .build();
    let folder_row = folder_row.clone();
    dialog.select_folder(Some(&window), gtk::gio::Cancellable::NONE, move |res| {
        if let Ok(file) = res {
            if let Some(path) = file.path() {
                let p = path.to_string_lossy().to_string();
                set_cfg("converter_path", serde_json::json!(p));
                folder_row.set_subtitle(&p);
            }
        }
    });
}

fn pick_cookies_file(state: &Rc<AppState>, cookies_row: &adw::ActionRow) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = gtk::FileDialog::builder().title(tr("Cookies File")).build();
    let cookies_row = cookies_row.clone();
    dialog.open(Some(&window), gtk::gio::Cancellable::NONE, move |res| {
        if let Ok(file) = res {
            if let Some(path) = file.path() {
                let p = path.to_string_lossy().to_string();
                set_cfg("cookies_file", serde_json::json!(p));
                cookies_row.set_subtitle(&p);
            }
        }
    });
}
