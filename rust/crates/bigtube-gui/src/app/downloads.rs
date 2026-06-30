//! Downloads page and the full download flow: the list/scheduled UI, the
//! format/quality dialogs, the enqueue pipeline (enqueue_common and friends),
//! plan/codec formatting, schedule restoration and the finished-downloads
//! history. The `DownloadRow` widget, `AppState`, the `UiMsg` channel and the
//! shared list/file/play helpers live in the parent module (reached via `super::`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use gtk::glib;

use bigtube_core::config;
use bigtube_core::download_manager::{self, OnStartFn};
use bigtube_core::downloader::{DownloadParams, VideoDownloader};
use bigtube_core::progress::{Progress, ProgressFn, StatusCode};

use super::converter::{add_converter_file, is_audio_input};
use super::widgets::{combo_row, next_key, page_header_trailing, status_page};
use super::{
    apply_theme_classes, delete_output_file, history_path, history_status_label,
    max_download_history, now_epoch_secs, open_containing_folder, remove_list_card,
    scheduled_downloads_path, wire_play_highlight, AppState, DownloadRow, RescheduleInfo, UiMsg,
    QUALITY_OPTIONS,
};
use crate::dialog;
use crate::i18n::tr;
use crate::objects::VideoObject;

/// The window a download dialog should parent to: the application's currently
/// active window — e.g. an open playlist/album/artist dialog — so the dialog
/// appears on top of it, falling back to the main window. Without this, a dialog
/// opened from the (non-modal) playlist window hides behind it on the main one.
fn dialog_parent(state: &Rc<AppState>) -> Option<gtk::Window> {
    let main = state.window.borrow().clone()?;
    Some(
        main.application()
            .and_then(|a| a.active_window())
            .unwrap_or_else(|| main.upcast()),
    )
}

/// True when the application's active window is the main window (not a secondary
/// dialog like the playlist/album/artist window). The "Processing…" busy card is
/// an overlay on the main window, so it's only the right feedback when the action
/// came from there; from a dialog it would sit behind, under the format dialog.
fn active_is_main(state: &Rc<AppState>) -> bool {
    let Some(main) = state.window.borrow().clone() else {
        return true;
    };
    let main_win: gtk::Window = main.clone().upcast();
    main.application()
        .and_then(|a| a.active_window())
        .map(|active| active == main_win)
        .unwrap_or(true)
}

/// A modal "Processing…" overlay shown over `parent` (a secondary dialog) while a
/// download's formats are fetched — the main window's busy-card overlay can't
/// cover another window, so this reproduces the same design (a dimmed scrim with
/// the centered `.busy-card`) as a borderless window sized to the dialog.
fn show_busy_window(parent: &gtk::Window) -> adw::Window {
    // Same card as the main-window overlay: accent-tinted, rounded, accent spinner.
    let card = gtk::Box::new(gtk::Orientation::Vertical, 18);
    card.set_halign(gtk::Align::Center);
    card.set_valign(gtk::Align::Center);
    card.set_vexpand(true);
    card.add_css_class("busy-card");
    let spinner = gtk::Spinner::new();
    spinner.set_size_request(54, 54);
    spinner.set_margin_top(34);
    spinner.set_margin_start(64);
    spinner.set_margin_end(64);
    spinner.start();
    let label = gtk::Label::new(Some(&tr("Processing...")));
    label.add_css_class("title-2");
    label.set_margin_bottom(34);
    card.append(&spinner);
    card.append(&label);

    // A dim scrim fills the window so the card floats over a darkened dialog,
    // exactly like the main overlay.
    let scrim = gtk::Box::new(gtk::Orientation::Vertical, 0);
    scrim.set_hexpand(true);
    scrim.set_vexpand(true);
    scrim.add_css_class("busy-dim");
    scrim.append(&card);

    // Borderless + transparent background so only the scrim/card show, matching
    // the dialog's footprint (covering it) rather than looking like a new window.
    let (w, h) = (parent.width().max(240), parent.height().max(160));
    let win = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .resizable(false)
        .decorated(false)
        .default_width(w)
        .default_height(h)
        .content(&scrim)
        .build();
    win.add_css_class("busy-scrim-window");
    apply_theme_classes(&win);
    win.present();
    win
}

pub(crate) fn build_downloads_page(state: &Rc<AppState>) -> gtk::Widget {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Header with an icon "clear history" button (disabled while the list is
    // empty; toggled by update_downloads_empty).
    let clear = state.downloads_clear.clone();
    clear.set_icon_name("bigtube-edit-clear-history-symbolic");
    clear.add_css_class("flat");
    clear.set_tooltip_text(Some(&tr("Clear History")));
    clear.set_sensitive(false);
    {
        let state = state.clone();
        clear.connect_clicked(move |_| confirm_clear_all_downloads(&state));
    }
    // Collapsible filter in the header (far right) narrows the rows by title.
    // Disabled until the list has rows (toggled by update_downloads_empty).
    let (filter_ctrl, filter_entry) = super::make_filter_control();
    filter_ctrl.set_sensitive(false);
    state.downloads_filter.replace(Some(filter_ctrl.clone()));
    super::wire_listbox_filter(&filter_entry, &state.downloads_box);
    let header = page_header_trailing(&tr("Downloads Manager"), &[clear], Some(&filter_ctrl));

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&state.downloads_box));

    let empty = status_page(
        "bigtube-download-symbolic",
        &tr("No Downloads"),
        &tr("Your downloads will appear here"),
    );
    state.downloads_stack.set_vexpand(true);
    state.downloads_stack.add_named(&empty, Some("empty"));
    state.downloads_stack.add_named(&scrolled, Some("list"));
    state.downloads_stack.set_visible_child_name("empty");

    page.append(&header);
    page.append(&state.downloads_stack);
    page.upcast()
}

/// Cancel a scheduled download by its persisted id: drop the live (pending) row
/// and its history entry, kill the scheduler task, and remove the store entry.
pub(crate) fn cancel_scheduled_by_id(state: &Rc<AppState>, id: &str) {
    let key = state
        .download_rows
        .borrow()
        .iter()
        .find(|(_, r)| r.sched_id.borrow().as_deref() == Some(id))
        .map(|(k, _)| k.clone());
    if let Some(k) = key {
        if let Some(row) = state.download_rows.borrow_mut().remove(&k) {
            if let Some(d) = row.downloader.borrow().as_ref() {
                d.cancel();
            }
            let fp = row.file_path.borrow().clone();
            if !fp.is_empty() {
                bigtube_core::history::remove_entry_now(&history_path(), &fp);
            }
            remove_list_card(&state.downloads_box, &row.container);
        }
        state.update_downloads_empty();
    }
    download_manager::global().cancel_task(id);
    bigtube_core::scheduled_downloads::ScheduledDownloadStore::new(scheduled_downloads_path())
        .remove(id);
}

/// Reopen the schedule dialog pre-filled for a pending scheduled row's pencil
/// button, then re-arm with the new time/recurrence (cancelling the old first).
#[allow(clippy::too_many_arguments)]
pub(crate) fn open_schedule_editor(
    state: &Rc<AppState>,
    sched_id: &str,
    url: &str,
    title: &str,
    thumbnail: &str,
    uploader: &str,
    format_id: &str,
    ext: &str,
    ts: f64,
    recurrence: &str,
) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let st = state.clone();
    let (sid, url, title, thumb, uploader, fmt, ext) = (
        sched_id.to_string(),
        url.to_string(),
        title.to_string(),
        thumbnail.to_string(),
        uploader.to_string(),
        format_id.to_string(),
        ext.to_string(),
    );
    crate::schedule::show(
        &window,
        Some(ts),
        recurrence,
        Rc::new(move |new_ts: f64, new_rec: String| {
            // Drop the old occurrence, then create the edited one afresh.
            cancel_scheduled_by_id(&st, &sid);
            enqueue_scheduled(
                &st, &url, &title, &thumb, &uploader, &fmt, &ext, new_ts, &new_rec,
            );
        }),
    );
}

/// Fetch metadata for `item`, then present the format-selection dialog.
pub(crate) fn on_download_clicked(state: &Rc<AppState>, item: &VideoObject) {
    let url = item.url();
    if url.is_empty() {
        state.toast(&tr("Invalid URL format"));
        return;
    }
    let title = item.title();
    let thumb = item.thumbnail();
    let uploader = item.uploader();
    let audio_only = !item.is_video();
    // "Processing…" feedback while formats are fetched. From the main window it's
    // the centered busy-card overlay; from a secondary window (the playlist/album/
    // artist dialog) that overlay would hide behind the format dialog, so show a
    // small spinner window over that dialog instead.
    let on_main = active_is_main(state);
    let busy_win: Rc<RefCell<Option<adw::Window>>> = Rc::new(RefCell::new(None));
    if on_main {
        state.busy_begin();
    } else if let Some(parent) = dialog_parent(state) {
        *busy_win.borrow_mut() = Some(show_busy_window(&parent));
    }

    let (tx, rx) = async_channel::bounded::<
        std::result::Result<bigtube_core::downloader::ParsedInfo, StatusCode>,
    >(1);
    let url_thread = url.clone();
    std::thread::spawn(move || {
        let info = match VideoDownloader::new() {
            Ok(d) => d.fetch_video_info_checked(&url_thread),
            Err(_) => Err(StatusCode::UnknownError),
        };
        let _ = tx.send_blocking(info);
    });

    let state = state.clone();
    glib::spawn_future_local(async move {
        let received = rx.recv().await;
        // Keep the busy spinner running until the format dialog is actually on
        // screen — on slow machines, parsing + building the dialog takes a beat,
        // and ending "busy" early left a dead gap with no feedback. End it on
        // each terminal branch instead (right as the dialog/toast appears).
        let end_busy = || {
            if on_main {
                state.busy_end();
            }
            if let Some(w) = busy_win.borrow_mut().take() {
                w.close();
            }
        };
        let info = match received {
            Ok(Ok(info)) => info,
            Ok(Err(StatusCode::BotBlocked)) => {
                end_busy();
                state.notify_bot_block();
                return;
            }
            _ => {
                end_busy();
                state.toast(&tr("No formats found"));
                return;
            }
        };
        // The secondary-window spinner is itself modal, so close it before the
        // format dialog opens over the same parent. The main-window busy card,
        // by contrast, stays up until the dialog is on screen (no flicker gap).
        if let Some(w) = busy_win.borrow_mut().take() {
            w.close();
        }
        run_download_flow(&state, info, url, title, thumb, uploader, audio_only);
        if on_main {
            state.busy_end();
        }
    });
}

/// Drive the post-fetch flow: a single format dialog. YouTube Music
/// (`audio_only`) shows just the Audio column; a normal source shows Video and
/// Audio side by side (two columns) in one screen — no Video/Audio prompt.
fn run_download_flow(
    state: &Rc<AppState>,
    info: bigtube_core::downloader::ParsedInfo,
    url: String,
    title: String,
    thumb: String,
    uploader: String,
    audio_only: bool,
) {
    let info = Rc::new(info);
    show_format_dialog(state, info, url, title, thumb, uploader, audio_only);
}

/// Present the format-selection dialog for already-fetched `info`, wiring its
/// Download and Schedule buttons. Closing without a pick just closes.
#[allow(clippy::too_many_arguments)]
fn show_format_dialog(
    state: &Rc<AppState>,
    info: Rc<bigtube_core::downloader::ParsedInfo>,
    url: String,
    title: String,
    thumb: String,
    uploader: String,
    audio_only: bool,
) {
    let Some(window) = dialog_parent(state) else {
        return;
    };
    let on_pick: dialog::PickFn = {
        let st = state.clone();
        let url = url.clone();
        let title = title.clone();
        let thumb = thumb.clone();
        let uploader = uploader.clone();
        Rc::new(move |format_id: String, ext: String| {
            enqueue_download_checked(&st, &url, &title, &thumb, &uploader, &format_id, &ext);
        })
    };
    let on_schedule: dialog::ScheduleFn = {
        let st = state.clone();
        let url = url.clone();
        let title = title.clone();
        let thumb = thumb.clone();
        let uploader = uploader.clone();
        Rc::new(move |format_id: String, ext: String| {
            let Some(win) = st.window.borrow().clone() else {
                return;
            };
            let st = st.clone();
            let url = url.clone();
            let title = title.clone();
            let thumb = thumb.clone();
            let uploader = uploader.clone();
            crate::schedule::show(
                &win,
                None,
                "once",
                Rc::new(move |ts: f64, recurrence: String| {
                    enqueue_scheduled(
                        &st,
                        &url,
                        &title,
                        &thumb,
                        &uploader,
                        &format_id,
                        &ext,
                        ts,
                        &recurrence,
                    );
                }),
            );
        })
    };
    let on_close: dialog::CloseFn = Rc::new(|| {});
    dialog::show(&window, &info, audio_only, on_pick, on_schedule, on_close);
}

/// File extension that pairs with a quality selector (audio/MKV/MP4).
fn quality_ext(q: bigtube_core::enums::VideoQuality) -> &'static str {
    use bigtube_core::enums::VideoQuality::*;
    match q {
        AudioMp3 => "mp3",
        AudioM4a => "m4a",
        AudioOpus => "opus",
        AudioFlac => "flac",
        AudioWav => "wav",
        AudioAac => "aac",
        Best => "mkv",
        _ => "mp4",
    }
}

/// Batch-download every item with a SINGLE quality dialog (instead of one
/// per-item format dialog). The chosen quality's yt-dlp selector is applied to
/// all items.
pub(crate) fn download_all(state: &Rc<AppState>, items: Vec<VideoObject>) {
    let items: Vec<VideoObject> = items
        .into_iter()
        .filter(|o| !o.is_playlist() && !o.url().is_empty())
        .collect();
    if items.is_empty() {
        state.toast(&tr("No results found!"));
        return;
    }
    let Some(window) = dialog_parent(state) else {
        return;
    };
    // Group a playlist / multi-selection under a folder named after the first
    // item's artist (uploader), so batches don't scatter across the download dir.
    let artist = bigtube_core::validators::sanitize_filename(&items[0].uploader(), 100);
    let subfolder = if artist.is_empty() {
        None
    } else {
        Some(artist)
    };

    let st = state.clone();
    let win = window.clone();
    show_quality_dialog(&window, move |q| {
        let sel = q.as_value().to_string();
        let ext = quality_ext(q);

        // How many of the batch would overwrite an existing file?
        let collisions = items
            .iter()
            .filter(|o| {
                std::path::Path::new(&output_path(&o.title(), &sel, ext, subfolder.as_deref()))
                    .exists()
            })
            .count();

        // One enqueue pass applied to the whole batch: colliding items either get
        // overwritten or a unique " (n)" title; the rest enqueue as-is.
        let enqueue_batch: Rc<dyn Fn(bool)> = {
            let st = st.clone();
            let items = items.clone();
            let subfolder = subfolder.clone();
            let sel = sel.clone();
            Rc::new(move |overwrite: bool| {
                for o in &items {
                    let collides = std::path::Path::new(&output_path(
                        &o.title(),
                        &sel,
                        ext,
                        subfolder.as_deref(),
                    ))
                    .exists();
                    let (title, force) = match (collides, overwrite) {
                        (true, true) => (o.title(), true),
                        (true, false) => (
                            unique_title(&o.title(), &sel, ext, subfolder.as_deref()),
                            false,
                        ),
                        (false, _) => (o.title(), false),
                    };
                    enqueue_common(
                        &st,
                        &o.url(),
                        &title,
                        &o.thumbnail(),
                        &o.uploader(),
                        &sel,
                        ext,
                        None,
                        force,
                        subfolder.as_deref(),
                        None,
                        "once",
                    );
                }
                let msg = match &subfolder {
                    Some(s) => format!("{} → {s}/", tr("Added to downloads")),
                    None => tr("Added to downloads"),
                };
                st.toast(&msg);
            })
        };

        if collisions == 0 {
            enqueue_batch(false);
            return;
        }
        // Ask once and apply the choice to every colliding item in the batch.
        let dialog = adw::MessageDialog::new(
            Some(&win),
            Some(&tr("Some files already exist")),
            Some(&format!(
                "{} {}",
                collisions,
                tr("file(s) in this batch are already in the download folder.")
            )),
        );
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("keep", &tr("Keep Both"));
        dialog.add_response("overwrite", &tr("Overwrite"));
        dialog.set_response_appearance("overwrite", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("keep"));
        dialog.set_close_response("cancel");
        apply_theme_classes(&dialog);
        dialog.connect_response(None, move |dlg, resp| {
            match resp {
                "overwrite" => enqueue_batch(true),
                "keep" => enqueue_batch(false),
                _ => {}
            }
            dlg.close();
        });
        dialog.present();
    });
}

/// Like [`download_all`] but routes the batch through the schedule dialog: ONE
/// quality pick + ONE time/recurrence for every item (playlist or selection).
pub(crate) fn schedule_all(state: &Rc<AppState>, items: Vec<VideoObject>) {
    let items: Vec<VideoObject> = items
        .into_iter()
        .filter(|o| !o.is_playlist() && !o.url().is_empty())
        .collect();
    if items.is_empty() {
        state.toast(&tr("No results found!"));
        return;
    }
    let Some(window) = dialog_parent(state) else {
        return;
    };
    let artist = bigtube_core::validators::sanitize_filename(&items[0].uploader(), 100);
    let subfolder = if artist.is_empty() {
        None
    } else {
        Some(artist)
    };

    let st = state.clone();
    show_quality_dialog(&window, move |q| {
        let sel = q.as_value().to_string();
        let ext = quality_ext(q);
        let Some(win) = dialog_parent(&st) else {
            return;
        };
        // One time + recurrence applied to the whole batch.
        let st2 = st.clone();
        let items2 = items.clone();
        let subfolder2 = subfolder.clone();
        crate::schedule::show(
            &win,
            None,
            "once",
            Rc::new(move |ts: f64, recurrence: String| {
                for o in &items2 {
                    enqueue_common(
                        &st2,
                        &o.url(),
                        &o.title(),
                        &o.thumbnail(),
                        &o.uploader(),
                        &sel,
                        ext,
                        Some(ts),
                        false,
                        subfolder2.as_deref(),
                        None,
                        &recurrence,
                    );
                }
                st2.toast(&tr("Scheduled!"));
            }),
        );
    });
}

/// True for the audio-only batch qualities (MP3 / M4A).
fn is_audio_quality(q: bigtube_core::enums::VideoQuality) -> bool {
    use bigtube_core::enums::VideoQuality::*;
    matches!(
        q,
        AudioMp3 | AudioM4a | AudioOpus | AudioFlac | AudioWav | AudioAac
    )
}

/// A single quality picker for batch downloads: a Video/Audio radio chooses the
/// kind, and the dropdown lists the qualities for that kind. Defaults to the
/// configured preferred quality.
fn show_quality_dialog(
    parent: &impl IsA<gtk::Window>,
    on_pick: impl Fn(bigtube_core::enums::VideoQuality) + 'static,
) {
    use bigtube_core::enums::VideoQuality;
    // Split the (non-"Ask") options into video and audio sets.
    let video: Vec<(&str, VideoQuality)> = QUALITY_OPTIONS
        .iter()
        .copied()
        .filter(|(_, q)| !matches!(q, VideoQuality::Ask) && !is_audio_quality(*q))
        .collect();
    let audio: Vec<(&str, VideoQuality)> = QUALITY_OPTIONS
        .iter()
        .copied()
        .filter(|(_, q)| is_audio_quality(*q))
        .collect();
    let default_quality = config::global()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .get_string("default_quality");
    let start_audio = audio.iter().any(|(_, q)| q.as_value() == default_quality);

    let win = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(tr("Select Quality"))
        .default_width(400)
        // Size to content so there's no dead space below the dropdown.
        .resizable(false)
        .build();
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    let dl_btn = gtk::Button::with_label(&tr("Download"));
    dl_btn.add_css_class("suggested-action");
    dl_btn.set_focus_on_click(false);
    header.pack_end(&dl_btn);
    toolbar.add_top_bar(&header);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);

    // Video/Audio radio.
    let kind = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    kind.set_halign(gtk::Align::Center);
    let video_radio = gtk::CheckButton::with_label(&tr("Video"));
    let audio_radio = gtk::CheckButton::with_label(&tr("Audio"));
    audio_radio.set_group(Some(&video_radio));
    video_radio.set_active(!start_audio);
    audio_radio.set_active(start_audio);
    kind.append(&video_radio);
    kind.append(&audio_radio);
    content.append(&kind);

    let group = adw::PreferencesGroup::new();
    let combo = combo_row(&tr("Preferred Quality"), &[] as &[&str]);
    group.add(&combo);
    content.append(&group);

    // The qualities currently shown in the dropdown (kept in sync with the radio).
    let current: Rc<RefCell<Vec<VideoQuality>>> = Rc::new(RefCell::new(Vec::new()));

    // Repopulate the dropdown for the selected kind, preselecting the default.
    let populate: Rc<dyn Fn(bool)> = {
        let combo = combo.clone();
        let current = current.clone();
        let video = video.clone();
        let audio = audio.clone();
        let default_quality = default_quality.clone();
        Rc::new(move |as_audio: bool| {
            let list = if as_audio { &audio } else { &video };
            let labels: Vec<String> = list.iter().map(|(l, _)| tr(l)).collect();
            let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
            combo.set_model(Some(&gtk::StringList::new(&refs)));
            let sel = list
                .iter()
                .position(|(_, q)| q.as_value() == default_quality)
                .unwrap_or(0);
            combo.set_selected(sel as u32);
            *current.borrow_mut() = list.iter().map(|(_, q)| *q).collect();
        })
    };
    populate(start_audio);
    {
        let populate = populate.clone();
        audio_radio.connect_toggled(move |b| populate(b.is_active()));
    }

    toolbar.set_content(Some(&content));
    win.set_content(Some(&toolbar));
    apply_theme_classes(&win);
    win.present();

    let on_pick = Rc::new(on_pick);
    dl_btn.connect_clicked(move |_| {
        if let Some(q) = current.borrow().get(combo.selected() as usize) {
            on_pick(*q);
        }
        win.close();
    });
}

/// Output file path the downloader will use
/// (`{download}/[subfolder/]{safe_title}.{ext}`).
fn output_path(title: &str, format_id: &str, ext: &str, subfolder: Option<&str>) -> String {
    let dir = config::global()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .get_string("download_path");
    let mut safe = bigtube_core::validators::sanitize_filename(title, 200);
    if safe.is_empty() {
        safe = format!("video_{format_id}");
    }
    match subfolder {
        Some(sub) if !sub.is_empty() => format!("{dir}/{sub}/{safe}.{ext}"),
        _ => format!("{dir}/{safe}.{ext}"),
    }
}

#[allow(clippy::too_many_arguments)]
fn enqueue_download(
    state: &Rc<AppState>,
    url: &str,
    title: &str,
    thumbnail: &str,
    uploader: &str,
    format_id: &str,
    ext: &str,
) {
    enqueue_common(
        state, url, title, thumbnail, uploader, format_id, ext, None, false, None, None, "once",
    );
}

/// Like [`enqueue_download`] but first checks whether the target file already
/// exists and, if so, asks the user to Overwrite / Keep both / Cancel.
#[allow(clippy::too_many_arguments)]
fn enqueue_download_checked(
    state: &Rc<AppState>,
    url: &str,
    title: &str,
    thumbnail: &str,
    uploader: &str,
    format_id: &str,
    ext: &str,
) {
    let path = output_path(title, format_id, ext, None);
    if !std::path::Path::new(&path).exists() {
        enqueue_download(state, url, title, thumbnail, uploader, format_id, ext);
        return;
    }
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = adw::MessageDialog::new(
        Some(&window),
        Some(&tr("File already exists")),
        Some(&format!(
            "{}\n\n{}",
            tr("A file with this name is already in the download folder."),
            path
        )),
    );
    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("keep", &tr("Keep Both"));
    dialog.add_response("overwrite", &tr("Overwrite"));
    dialog.set_response_appearance("overwrite", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("keep"));
    dialog.set_close_response("cancel");
    apply_theme_classes(&dialog);

    // Own the strings for the response closure.
    let (state, url, title, thumbnail, uploader, format_id, ext) = (
        state.clone(),
        url.to_string(),
        title.to_string(),
        thumbnail.to_string(),
        uploader.to_string(),
        format_id.to_string(),
        ext.to_string(),
    );
    dialog.connect_response(None, move |dlg, resp| {
        match resp {
            "overwrite" => enqueue_common(
                &state, &url, &title, &thumbnail, &uploader, &format_id, &ext, None, true, None,
                None, "once",
            ),
            "keep" => {
                let t = unique_title(&title, &format_id, &ext, None);
                enqueue_download(&state, &url, &t, &thumbnail, &uploader, &format_id, &ext);
            }
            _ => {}
        }
        dlg.close();
    });
    dialog.present();
}

/// A title whose `output_path` doesn't collide, appending " (n)" as needed.
fn unique_title(title: &str, format_id: &str, ext: &str, subfolder: Option<&str>) -> String {
    if !std::path::Path::new(&output_path(title, format_id, ext, subfolder)).exists() {
        return title.to_string();
    }
    for n in 1..1000 {
        let candidate = format!("{title} ({n})");
        if !std::path::Path::new(&output_path(&candidate, format_id, ext, subfolder)).exists() {
            return candidate;
        }
    }
    format!("{title} ({})", glib::real_time())
}

/// Format a unix timestamp as a local "DD/MM/YYYY HH:MM" string for the
/// "Scheduled for:" message.
fn format_schedule_ts(ts: f64) -> String {
    glib::DateTime::from_unix_local(ts as i64)
        .ok()
        .and_then(|dt| dt.format("%d/%m/%Y %H:%M").ok())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// Schedule a download for the Unix timestamp `ts` (seconds), with an optional
/// recurrence ("once" / "daily" / "weekly" / "monthly").
#[allow(clippy::too_many_arguments)]
fn enqueue_scheduled(
    state: &Rc<AppState>,
    url: &str,
    title: &str,
    thumbnail: &str,
    uploader: &str,
    format_id: &str,
    ext: &str,
    ts: f64,
    recurrence: &str,
) {
    enqueue_common(
        state,
        url,
        title,
        thumbnail,
        uploader,
        format_id,
        ext,
        Some(ts),
        false,
        None,
        None,
        recurrence,
    );
}

/// Human label for a recurrence key (for the scheduled row's status line).
fn recurrence_label(recurrence: &str) -> Option<String> {
    match recurrence {
        "daily" => Some(tr("Daily")),
        "weekly" => Some(tr("Weekly")),
        "monthly" => Some(tr("Monthly")),
        _ => None,
    }
}

/// Next future occurrence of a recurring schedule: advance `base_ts` by the
/// recurrence interval (calendar-aware, so DST and month lengths are honoured)
/// until it lands strictly after `now`. None for "once"/unknown keys.
pub(crate) fn next_occurrence(base_ts: f64, recurrence: &str, now: f64) -> Option<f64> {
    let mut dt = glib::DateTime::from_unix_local(base_ts as i64).ok()?;
    loop {
        dt = match recurrence {
            "daily" => dt.add_days(1).ok()?,
            "weekly" => dt.add_weeks(1).ok()?,
            "monthly" => dt.add_months(1).ok()?,
            _ => return None,
        };
        if dt.to_unix() as f64 > now {
            return Some(dt.to_unix() as f64);
        }
    }
}

/// Pretty codec name for the resolved-plan summary line.
pub(crate) fn codec_pretty(c: &str) -> String {
    let l = c.to_lowercase();
    if l.contains("avc") || l.contains("h264") {
        "H.264".into()
    } else if l.contains("av01") || l.contains("av1") {
        "AV1".into()
    } else if l.contains("vp9") || l.contains("vp09") {
        "VP9".into()
    } else if l.contains("vp8") {
        "VP8".into()
    } else if l.contains("mp4a") || l.contains("aac") {
        "AAC".into()
    } else if l.contains("opus") {
        "Opus".into()
    } else if l.contains("mp3") {
        "MP3".into()
    } else if l.contains("ac-3") || l.contains("ac3") {
        "AC3".into()
    } else if l.is_empty() {
        String::new()
    } else {
        c.split('.').next().unwrap_or(c).to_uppercase()
    }
}

/// One-line summary of a resolved plan, e.g. "H.264 + AAC · 1080p60 · 275.0 MiB".
fn plan_summary(p: &bigtube_core::downloader::ResolvedPlan) -> String {
    let mut parts: Vec<String> = Vec::new();
    let (v, a) = (codec_pretty(&p.vcodec), codec_pretty(&p.acodec));
    let codecs = match (v.is_empty(), a.is_empty()) {
        (false, false) => format!("{v} + {a}"),
        (false, true) => v,
        (true, false) => a,
        _ => String::new(),
    };
    if !codecs.is_empty() {
        parts.push(codecs);
    }
    if p.height > 0 {
        parts.push(if p.fps > 30 {
            format!("{}p{}", p.height, p.fps)
        } else {
            format!("{}p", p.height)
        });
    }
    if p.size_mb > 0.0 {
        // Pre-download size from yt-dlp is an estimate (often overshoots); the
        // live progress line shows the real bytes once the transfer starts.
        parts.push(if p.size_mb >= 1024.0 {
            format!("~{:.2} GiB", p.size_mb / 1024.0)
        } else {
            format!("~{:.0} MiB", p.size_mb)
        });
    }
    parts.join(" · ")
}

/// Should we probe the real plan before downloading? Only for immediate video
/// downloads — scheduled tasks may run hours later (concrete ids could go
/// stale), and audio-convert picks re-encode so a probed size wouldn't match.
fn should_probe_plan(ext: &str, schedule_ts: Option<f64>) -> bool {
    schedule_ts.is_none() && matches!(ext, "mp4" | "mkv" | "webm")
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn enqueue_common(
    state: &Rc<AppState>,
    url: &str,
    title: &str,
    thumbnail: &str,
    uploader: &str,
    format_id: &str,
    ext: &str,
    schedule_ts: Option<f64>,
    force_overwrite: bool,
    subfolder: Option<&str>,
    restore_id: Option<String>,
    recurrence: &str,
) {
    let key = next_key();
    let file_path = output_path(title, format_id, ext, subfolder);

    // Restoring a persisted schedule: the entry is already in history and in the
    // scheduled store, so don't re-add either.
    let restoring = restore_id.is_some();
    // Stable id shared by the scheduler task and the persisted store entry, so we
    // can remove the right one when the download starts.
    let sched_id = restore_id.unwrap_or_else(|| key.clone());

    // Record a pending history entry up front (so it survives a crash mid-download).
    let save_history = config::global()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .get_bool("save_history");
    if save_history && !restoring {
        let video_info = serde_json::json!({
            "title": title, "url": url, "webpage_url": url,
            "thumbnail": thumbnail, "uploader": uploader,
            "scheduled_time": schedule_ts,
        });
        let format_data = serde_json::json!({ "id": format_id, "ext": ext });
        bigtube_core::history::add_entry_now(
            &history_path(),
            max_download_history(),
            &video_info,
            &format_data,
            &file_path,
        );
    }

    let tx = state.ui_tx.clone();
    let k = key.clone();
    let fp_cb = file_path.clone();
    let cb: ProgressFn = Arc::new(move |p: Progress| {
        // Persist terminal states to history.
        if save_history && (p.status == StatusCode::Completed || p.status.is_error()) {
            use bigtube_core::enums::DownloadStatus;
            let st = if p.status == StatusCode::Completed {
                DownloadStatus::Completed
            } else {
                DownloadStatus::Error
            };
            bigtube_core::history::update_status_now(
                &history_path(),
                &fp_cb,
                st,
                Some(if p.status == StatusCode::Completed {
                    1.0
                } else {
                    0.0
                }),
            );
        }
        let _ = tx.send_blocking(UiMsg::Progress {
            key: k.clone(),
            percent: p.percent.clone(),
            status: p.status,
            detail: p.detail.clone(),
        });
    });

    let row = DownloadRow::new(title, &file_path, uploader);
    wire_row_footer(state, &row);
    // Store the progress callback so the row's pause/resume can re-run it.
    row.progress_fn.replace(Some(cb.clone()));
    // For a scheduled download, show "Scheduled for: <date time>" on the row and
    // as a toast, and make Cancel drop the still-pending timer (the core emits no
    // progress for a not-yet-started task, so we clean up the row here directly).
    if let Some(ts) = schedule_ts {
        let mut msg = format!("{} {}", tr("Scheduled for:"), format_schedule_ts(ts));
        if let Some(rep) = recurrence_label(recurrence) {
            msg = format!("{msg} · {rep}");
        }
        row.status.set_text(&msg);
        row.set_progress_class("warning");
        // Tag the row with its schedule id and reveal the edit pencil.
        row.sched_id.replace(Some(sched_id.clone()));
        row.edit.set_visible(true);
        state.toast(&msg);

        // Pencil → reopen the schedule dialog pre-filled and re-arm on confirm.
        {
            let st = state.clone();
            let (sid, url, title, thumb, uploader, fmt, ext2, rec) = (
                sched_id.clone(),
                url.to_string(),
                title.to_string(),
                thumbnail.to_string(),
                uploader.to_string(),
                format_id.to_string(),
                ext.to_string(),
                recurrence.to_string(),
            );
            row.edit.connect_clicked(move |_| {
                open_schedule_editor(
                    &st, &sid, &url, &title, &thumb, &uploader, &fmt, &ext2, ts, &rec,
                );
            });
        }

        let st = state.clone();
        let key_c = key.clone();
        let sid = sched_id.clone();
        let dl_slot = row.downloader.clone();
        row.cancel.connect_clicked(move |_| {
            // Already running: the active-downloader handler cancels it and the
            // core's Cancelled progress cleans up — nothing to do here.
            if dl_slot.borrow().is_some() {
                return;
            }
            download_manager::global().cancel_task(&sid);
            bigtube_core::scheduled_downloads::ScheduledDownloadStore::new(
                scheduled_downloads_path(),
            )
            .remove(&sid);
            if let Some(row) = st.download_rows.borrow_mut().remove(&key_c) {
                let fp = row.file_path.borrow().clone();
                if !fp.is_empty() {
                    bigtube_core::history::remove_entry_now(&history_path(), &fp);
                }
                remove_list_card(&st.downloads_box, &row.container);
            }
            st.update_downloads_empty();
        });
    }
    // Labels to update once the real plan is resolved (clones share the widget).
    let status_lbl = row.status.clone();
    let detail_lbl = row.detail.clone();
    state.downloads_box.append(&row.container);
    state.download_rows.borrow_mut().insert(key.clone(), row);
    state.update_downloads_empty();
    state.stack.set_visible_child_name("downloads");

    // Capture the VideoDownloader when the task starts (for cancel/pause). A
    // scheduled task also drops its persisted store entry here — once it's
    // running it must not be restored again on the next launch.
    let tx_started = state.ui_tx.clone();
    let k2 = key.clone();
    let was_scheduled = schedule_ts.is_some();
    let start_sched_id = sched_id.clone();
    // Owned bundle so a recurring task can spawn its next occurrence on fire.
    let rec_start = recurrence.to_string();
    let base_ts = schedule_ts;
    let reschedule = RescheduleInfo {
        url: url.to_string(),
        title: title.to_string(),
        thumbnail: thumbnail.to_string(),
        uploader: uploader.to_string(),
        format_id: format_id.to_string(),
        ext: ext.to_string(),
        force_overwrite,
        recurrence: rec_start,
    };
    let on_start: OnStartFn = Arc::new(move |dl: Arc<VideoDownloader>| {
        if was_scheduled {
            bigtube_core::scheduled_downloads::ScheduledDownloadStore::new(
                scheduled_downloads_path(),
            )
            .remove(&start_sched_id);
            // A recurring task arms its next occurrence the moment this one
            // starts (the main loop computes the next instant and re-enqueues).
            if reschedule.recurrence != "once" {
                if let Some(ts) = base_ts {
                    let _ = tx_started.send_blocking(UiMsg::Reschedule {
                        info: reschedule.clone(),
                        base_ts: ts,
                    });
                }
            }
        }
        let _ = tx_started.send_blocking(UiMsg::Started {
            key: k2.clone(),
            downloader: dl,
        });
    });

    // Build params + enqueue, given the (possibly probe-resolved) format and size.
    let (url_o, title_o, ext_o) = (url.to_string(), title.to_string(), ext.to_string());
    let sub_o = subfolder.map(str::to_string);
    let mgr = download_manager::global();
    // Owned copies for persisting the schedule (so the entry can recreate the
    // download after a restart). The original selector is stored, not a probed
    // concrete id (scheduled tasks run later, when those ids may be stale).
    let persist = schedule_ts.is_some() && !restoring;
    let rec_persist = recurrence.to_string();
    let (s_url, s_title, s_thumb, s_uploader, s_fmt, s_ext, s_path) = (
        url.to_string(),
        title.to_string(),
        thumbnail.to_string(),
        uploader.to_string(),
        format_id.to_string(),
        ext.to_string(),
        file_path.clone(),
    );
    let finalize: Box<dyn FnOnce(String, Option<f64>)> = Box::new(move |fmt, size| {
        let params = DownloadParams {
            url: url_o,
            format_id: fmt,
            title: title_o,
            ext: ext_o,
            force_overwrite,
            estimated_size_mb: size,
            subfolder: sub_o,
        };
        match schedule_ts {
            Some(ts) => {
                if persist {
                    let item = serde_json::json!({
                        "id": sched_id,
                        "scheduled_time": ts,
                        "recurrence": rec_persist,
                        "video_info": {
                            "url": s_url, "title": s_title,
                            "thumbnail": s_thumb, "uploader": s_uploader,
                        },
                        "format_data": { "id": s_fmt, "ext": s_ext },
                        "full_path": s_path,
                        "force_overwrite": force_overwrite,
                        "estimated_size_mb": size,
                    });
                    bigtube_core::scheduled_downloads::ScheduledDownloadStore::new(
                        scheduled_downloads_path(),
                    )
                    .upsert(&item);
                }
                mgr.schedule_download(ts, params, cb, Some(on_start), 0, Some(sched_id));
            }
            None => {
                mgr.add_download(params, cb, Some(on_start), 0);
            }
        }
    });

    if should_probe_plan(ext, schedule_ts) {
        // Exact "sondagem": ask yt-dlp what `format_id` actually resolves to, pin
        // those concrete ids (so the file matches what we show), and display the
        // real codecs/size. The original selector stays as a safety fallback.
        status_lbl.set_text(&tr("Resolving format…"));
        let orig_sel = format_id.to_string();
        let (ptx, prx) =
            async_channel::bounded::<Option<bigtube_core::downloader::ResolvedPlan>>(1);
        let url_p = url.to_string();
        let sel_p = format_id.to_string();
        std::thread::spawn(move || {
            let plan = VideoDownloader::new()
                .ok()
                .and_then(|d| d.resolve_plan(&url_p, &sel_p));
            let _ = ptx.send_blocking(plan);
        });
        glib::spawn_future_local(async move {
            let plan = prx.recv().await.ok().flatten();
            let (fmt, size) = match &plan {
                Some(p) if !p.format_id.is_empty() => {
                    detail_lbl.set_text(&plan_summary(p));
                    detail_lbl.set_visible(true);
                    // Pin concrete ids; keep the codec-aware chain as a fallback.
                    let f = format!("{}/{}", p.format_id, orig_sel);
                    (f, Some(p.size_mb).filter(|s| *s > 0.0))
                }
                _ => (orig_sel, None),
            };
            finalize(fmt, size);
        });
    } else {
        finalize(format_id.to_string(), None);
    }
}

/// Wire a completed download row's footer actions (open folder / play / convert).
pub(crate) fn wire_row_footer(state: &Rc<AppState>, row: &DownloadRow) {
    {
        let state = state.clone();
        let fp = row.file_path.clone();
        row.btn_folder
            .connect_clicked(move |_| open_containing_folder(&state, &fp.borrow()));
    }
    {
        let state = state.clone();
        let container = row.container.clone();
        let fp = row.file_path.clone();
        row.btn_play.connect_clicked(move |_| {
            // If this row's file is the active track, the button toggles
            // play/pause in sync with the bar; otherwise it starts playback.
            if let Some(player) = state.player.borrow().clone() {
                let p = fp.borrow().clone();
                if !p.is_empty() && player.now_playing().url() == p {
                    player.now_playing().request_toggle();
                    return;
                }
            }
            play_download_at(&state, &container);
        });
    }
    // Highlight this row while its file is the one playing, and sync its glyph.
    wire_play_highlight(state, &row.container, row.file_path.clone(), &row.btn_play);
    {
        let state = state.clone();
        let fp = row.file_path.clone();
        row.btn_convert.connect_clicked(move |_| {
            let path = std::path::PathBuf::from(&*fp.borrow());
            if path.exists() {
                add_converter_file(&state, path);
            }
        });
    }
    // Favorite toggle for the downloaded local file.
    {
        let fp = row.file_path.clone();
        let artist = row.artist.clone();
        let btn = row.btn_favorite.clone();
        crate::app::favorites::set_heart_icon(
            &btn,
            crate::app::favorites::favorites().contains(&fp.borrow()),
        );
        btn.connect_clicked(move |b| {
            let path = fp.borrow().clone();
            if path.is_empty() {
                return;
            }
            let now = crate::app::favorites::toggle_local(&path, &artist.borrow());
            crate::app::favorites::set_heart_icon(b, now);
        });
        // Keep the heart in sync if this file is unfavorited elsewhere (popover).
        crate::app::favorites::watch_heart(&row.btn_favorite, row.file_path.clone());
    }
    // Per-row delete: ask whether to drop just the history entry or the file too.
    {
        let state = state.clone();
        let container = row.container.clone();
        let fp = row.file_path.clone();
        let downloader = row.downloader.clone();
        row.btn_delete.connect_clicked(move |_| {
            confirm_delete_download(&state, &container, &fp.borrow(), &downloader);
        });
    }
}

/// The card box backing a `ListBox` child (the child may be wrapped in an
/// auto-created `ListBoxRow`).
pub(crate) fn card_of(child: &gtk::Widget) -> Option<gtk::Box> {
    if let Ok(row) = child.clone().downcast::<gtk::ListBoxRow>() {
        row.child().and_then(|w| w.downcast::<gtk::Box>().ok())
    } else {
        child.clone().downcast::<gtk::Box>().ok()
    }
}

/// Play the clicked completed download, seeding the player queue from every
/// playable file in the list (in visual order) so prev/next/EOS cycle through
/// them. Highlights follow via the shared NowPlaying handle.
pub(crate) fn play_download_at(state: &Rc<AppState>, clicked: &gtk::Box) {
    let Some(player) = state.player.borrow().clone() else {
        state.toast(&tr(
            "Playback unavailable — install the GStreamer gtk4 plugin",
        ));
        return;
    };
    let rows = state.download_rows.borrow();
    // Index rows by their card widget so the per-child lookup below is O(1).
    // Scanning every row for every child was O(n^2) on large histories.
    let by_card: HashMap<usize, &DownloadRow> = rows
        .values()
        .map(|r| (r.container.as_ptr() as usize, r))
        .collect();
    let mut items = Vec::new();
    let mut start = 0usize;
    let mut child = state.downloads_box.first_child();
    while let Some(c) = child {
        let next = c.next_sibling();
        if let Some(card) = card_of(&c) {
            if let Some(row) = by_card.get(&(card.as_ptr() as usize)) {
                let path = row.file_path.borrow().clone();
                if !path.is_empty() && std::path::Path::new(&path).exists() {
                    if card == *clicked {
                        start = items.len();
                    }
                    let title = std::path::Path::new(&path)
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    items.push(crate::player::QueueItem {
                        url: path.clone(),
                        title,
                        artist: row.artist.borrow().clone(),
                        thumbnail: String::new(),
                        is_local: true,
                        is_video: !is_audio_input(std::path::Path::new(&path)),
                    });
                }
            }
        }
        child = next;
    }
    drop(rows);
    if !items.is_empty() {
        player.play_queue(items, start);
    }
}

/// "Clear all" downloads: ask history-only vs file-too, then wipe every row.
pub(crate) fn confirm_clear_all_downloads(state: &Rc<AppState>) {
    if state.download_rows.borrow().is_empty() {
        return;
    }
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = adw::MessageDialog::new(
        Some(&window),
        Some(&tr("Clear all downloads?")),
        Some(&tr(
            "Remove only the history entries, or delete the files too?",
        )),
    );
    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("history", &tr("Remove from history"));
    dialog.add_response("file", &tr("Delete files too"));
    dialog.set_response_appearance("file", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("history"));
    dialog.set_close_response("cancel");
    apply_theme_classes(&dialog);

    let state = state.clone();
    dialog.connect_response(None, move |dlg, resp| {
        if resp == "history" || resp == "file" {
            let delete_files = resp == "file";
            let sched_store = bigtube_core::scheduled_downloads::ScheduledDownloadStore::new(
                scheduled_downloads_path(),
            );
            let mut rows = state.download_rows.borrow_mut();
            for (_, row) in rows.drain() {
                if let Some(d) = row.downloader.borrow().as_ref() {
                    d.cancel();
                }
                // Pending scheduled rows: kill the timer and drop the store entry
                // so they don't fire or reappear on restart.
                if let Some(sid) = row.sched_id.borrow().clone() {
                    download_manager::global().cancel_task(&sid);
                    sched_store.remove(&sid);
                }
                if delete_files {
                    delete_output_file(&row.file_path.borrow());
                }
                remove_list_card(&state.downloads_box, &row.container);
            }
            drop(rows);
            // Wipe the saved history so nothing reloads on restart.
            bigtube_core::history::clear_all_now(&history_path());
            state.update_downloads_empty();
        }
        dlg.close();
    });
    dialog.present();
}

/// Remove a download row (by widget identity) from the list and the row map.
fn remove_download_row(state: &Rc<AppState>, container: &gtk::Box) {
    let mut rows = state.download_rows.borrow_mut();
    let key = rows
        .iter()
        .find(|(_, r)| &r.container == container)
        .map(|(k, _)| k.clone());
    if let Some(k) = key {
        if let Some(r) = rows.remove(&k) {
            remove_list_card(&state.downloads_box, &r.container);
        }
    }
    drop(rows);
    state.update_downloads_empty();
}

/// Ask "remove from history" vs "delete file too" for one download, then apply.
pub(crate) fn confirm_delete_download(
    state: &Rc<AppState>,
    container: &gtk::Box,
    file_path: &str,
    downloader: &Rc<RefCell<Option<Arc<VideoDownloader>>>>,
) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = adw::MessageDialog::new(
        Some(&window),
        Some(&tr("Remove download?")),
        Some(&tr(
            "Remove only the history entry, or delete the file too?",
        )),
    );
    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("history", &tr("Remove from history"));
    dialog.add_response("file", &tr("Delete file too"));
    dialog.set_response_appearance("file", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("history"));
    dialog.set_close_response("cancel");
    apply_theme_classes(&dialog);

    let state = state.clone();
    let container = container.clone();
    let downloader = downloader.clone();
    let file_path = file_path.to_string();
    dialog.connect_response(None, move |dlg, resp| {
        if resp == "history" || resp == "file" {
            // Stop the download first if it's still running.
            if let Some(d) = downloader.borrow().as_ref() {
                d.cancel();
            }
            if resp == "file" {
                delete_output_file(&file_path);
            }
            if !file_path.is_empty() {
                bigtube_core::history::remove_entry_now(&history_path(), &file_path);
            }
            remove_download_row(&state, &container);
        }
        dlg.close();
    });
    dialog.present();
}

/// Recreate persisted scheduled downloads after startup, mirroring
/// `download_workflow.restore_scheduled_downloads`. Re-arms each future timer;
/// any whose time already passed (app was closed) downloads immediately.
pub(crate) fn restore_scheduled_downloads(state: &Rc<AppState>) {
    let store =
        bigtube_core::scheduled_downloads::ScheduledDownloadStore::new(scheduled_downloads_path());
    let now = now_epoch_secs();
    for item in store.load() {
        let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let video_info = item.get("video_info").and_then(|v| v.as_object());
        let format_data = item.get("format_data").and_then(|v| v.as_object());
        let full_path = item.get("full_path").and_then(|v| v.as_str()).unwrap_or("");

        // Drop corrupt entries we can't recreate.
        let (Some(video_info), Some(format_data)) = (video_info, format_data) else {
            if !id.is_empty() {
                store.remove(id);
            }
            continue;
        };
        if full_path.is_empty() || id.is_empty() {
            if !id.is_empty() {
                store.remove(id);
            }
            continue;
        }

        let get = |m: &serde_json::Map<String, serde_json::Value>, k: &str| {
            m.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
        };
        let url = get(video_info, "url");
        let title = get(video_info, "title");
        let thumbnail = get(video_info, "thumbnail");
        let uploader = get(video_info, "uploader");
        let format_id = get(format_data, "id");
        let ext = get(format_data, "ext");
        let force_overwrite = item
            .get("force_overwrite")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let recurrence = item
            .get("recurrence")
            .and_then(|v| v.as_str())
            .unwrap_or("once")
            .to_string();

        // Past its time while the app was closed. For a one-shot, download right
        // away (schedule_ts = None). For a recurring one, skip the missed runs
        // (cron-style) and re-arm the next future occurrence in place.
        let mut schedule_ts = item.get("scheduled_time").and_then(|v| v.as_f64());
        if let Some(ts) = schedule_ts {
            if ts <= now {
                if recurrence != "once" {
                    schedule_ts = next_occurrence(ts, &recurrence, now);
                    match schedule_ts {
                        Some(next) => {
                            // Persist the advanced time on the same entry so it
                            // isn't restored as past-due again next launch.
                            let mut updated = item.clone();
                            updated["scheduled_time"] = serde_json::json!(next);
                            store.upsert(&updated);
                        }
                        None => store.remove(id),
                    }
                } else {
                    store.remove(id);
                    schedule_ts = None;
                }
            }
        }

        enqueue_common(
            state,
            &url,
            &title,
            &thumbnail,
            &uploader,
            &format_id,
            &ext,
            schedule_ts,
            force_overwrite,
            None,
            Some(id.to_string()),
            &recurrence,
        );
    }
}

/// Load persisted download history into the Downloads list on startup.
pub(crate) fn load_download_history(state: &Rc<AppState>) {
    // Pure read (see load_converter_history): avoid the manager's drop-flush.
    let items: Vec<serde_json::Value> =
        bigtube_core::json_store::load_json(history_path(), Vec::new());

    // Items that are still scheduled are restored as live scheduled rows by
    // `restore_scheduled_downloads`; skip them here so they don't show twice
    // (once as a stale "Scheduled" history row, once as the live one).
    let scheduled_paths: std::collections::HashSet<String> =
        bigtube_core::scheduled_downloads::ScheduledDownloadStore::new(scheduled_downloads_path())
            .load()
            .iter()
            .filter_map(|e| {
                e.get("full_path")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .collect();

    for it in items.iter().take(max_download_history()) {
        let title = it
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Title");
        let fp = it.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
        if !fp.is_empty() && scheduled_paths.contains(fp) {
            continue;
        }
        let uploader = it.get("uploader").and_then(|v| v.as_str()).unwrap_or("");
        let status = it
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("completed");

        let row = DownloadRow::new(title, fp, uploader);
        row.pause.set_visible(false);
        row.cancel.set_visible(false);
        row.progress.set_visible(false);
        // Past-session rows are terminal, so "Clear" can remove them.
        row.pause.set_sensitive(false);
        row.cancel.set_sensitive(false);
        row.status.set_text(&history_status_label(status));
        let exists = !fp.is_empty() && std::path::Path::new(fp).exists();
        row.actions.set_visible(exists);

        // Restore the saved media summary (codecs/resolution/size) bottom-left.
        let media = it
            .get("media_summary")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !media.is_empty() {
            row.detail.set_text(media);
            row.detail.set_visible(true);
        }

        // A failed download comes back retryable: red bar + a Retry button that
        // re-enqueues from the stored url/format and drops the stale entry.
        if status == "error" {
            row.progress.set_visible(true);
            row.progress.set_fraction(0.0);
            row.set_progress_class("error");
            row.pause.set_visible(true);
            row.pause.set_sensitive(true);
            row.pause.set_icon_name("bigtube-view-refresh-symbolic");
            row.pause.set_tooltip_text(Some(&tr("Retry")));

            let state2 = state.clone();
            let container = row.container.clone();
            let u = it
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let t = title.to_string();
            let th = it
                .get("thumbnail")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let up = uploader.to_string();
            let f = it
                .get("format_id")
                .and_then(|v| v.as_str())
                .unwrap_or("best")
                .to_string();
            let e = it
                .get("ext")
                .and_then(|v| v.as_str())
                .unwrap_or("mp4")
                .to_string();
            let fp_owned = fp.to_string();
            row.pause.connect_clicked(move |_| {
                if u.is_empty() {
                    return;
                }
                remove_download_row(&state2, &container);
                if !fp_owned.is_empty() {
                    bigtube_core::history::remove_entry_now(&history_path(), &fp_owned);
                }
                enqueue_download(&state2, &u, &t, &th, &up, &f, &e);
            });
        }

        wire_row_footer(state, &row);
        state.downloads_box.append(&row.container);
        // Track in the row map so "Clear" can find and remove them too.
        state.download_rows.borrow_mut().insert(next_key(), row);
    }
    state.update_downloads_empty();
}
