//! Converter page: the file list UI, the one-at-a-time conversion queue
//! (enqueue → pump → run), pending-queue persistence, and the converted-history
//! list. Shared helpers (list-card removal, output deletion, media summaries,
//! play-highlight wiring) live in the parent module and are reached via `super::`.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use adw::prelude::*;
use gtk::glib;

use bigtube_core::config;

use super::widgets::{fmt_eta, page_header_trailing, status_page};
use super::{
    a11y_label, apply_theme_classes, converter_history_path, delete_output_file,
    max_converter_history, media_summary_text, open_containing_folder, remove_list_card,
    wire_play_highlight, AppState,
};
use crate::i18n::tr;

const VIDEO_FORMATS: [&str; 6] = ["mp4", "mkv", "webm", "mp3", "m4a", "wav"];

const AUDIO_FORMATS: [&str; 4] = ["mp3", "m4a", "wav", "flac"];

/// True when the source file is audio-only (by extension). Audio inputs never
/// carry subtitles, so the converter hides that toggle for them (`converter_row.py`).
pub(crate) fn is_audio_input(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_lowercase().as_str(),
                "mp3" | "m4a" | "wav" | "flac" | "ogg" | "opus" | "aac" | "wma"
            )
        })
        .unwrap_or(false)
}

/// Output formats offered for a given source file, by media type (`converter_row.py`).
fn convert_formats_for(path: &std::path::Path) -> &'static [&'static str] {
    if is_audio_input(path) {
        &AUDIO_FORMATS
    } else {
        &VIDEO_FORMATS
    }
}

/// The selected output format string read live from a row's dropdown. Reads the
/// model's current item (not a captured static list), so it stays correct after
/// the Video/Audio toggle repopulates the dropdown.
fn selected_format(dd: &gtk::DropDown) -> String {
    dd.selected_item()
        .and_downcast::<gtk::StringObject>()
        .map(|s| s.string().to_string())
        .unwrap_or_else(|| "mp4".to_string())
}

enum ConvMsg {
    /// `(fraction 0..1, speed_x, eta_seconds)`.
    Progress(f64, Option<f64>, Option<f64>),
    Done(Result<String, String>),
}

pub(crate) fn build_converter_page(state: &Rc<AppState>) -> gtk::Widget {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Header with "add files" + "clear all" buttons.
    let add = gtk::Button::from_icon_name("bigtube-list-add-symbolic");
    add.add_css_class("flat");
    add.set_tooltip_text(Some(&tr("Add Files")));
    {
        let state = state.clone();
        add.connect_clicked(move |_| pick_files(&state));
    }
    // Disabled while the list is empty; toggled by update_converter_empty.
    let clear = state.converter_clear.clone();
    clear.set_icon_name("bigtube-edit-clear-history-symbolic");
    clear.add_css_class("flat");
    clear.set_tooltip_text(Some(&tr("Clear History")));
    clear.set_sensitive(false);
    {
        let state = state.clone();
        clear.connect_clicked(move |_| confirm_clear_all_converter(&state));
    }
    // Collapsible filter in the header (far right) narrows the rows by file name.
    // Disabled until the list has rows (toggled by update_converter_empty).
    let (filter_ctrl, filter_entry) = super::make_filter_control();
    filter_ctrl.set_sensitive(false);
    state.converter_filter.replace(Some(filter_ctrl.clone()));
    super::wire_listbox_filter(&filter_entry, &state.converter_box);
    let header = page_header_trailing(&tr("Converter Manager"), &[add, clear], Some(&filter_ctrl));

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&state.converter_box));

    // Empty-state acts as the drop zone hint.
    let empty = status_page(
        "bigtube-view-refresh-symbolic",
        &tr("Media Converter"),
        &tr("Drag and drop files here to convert"),
    );
    state.converter_stack.set_vexpand(true);
    state.converter_stack.add_named(&empty, Some("empty"));
    state.converter_stack.add_named(&scrolled, Some("list"));
    state.converter_stack.set_visible_child_name("empty");

    page.append(&header);
    page.append(&state.converter_stack);

    // Drag & drop of files onto the page.
    let drop = gtk::DropTarget::new(
        gtk::gdk::FileList::static_type(),
        gtk::gdk::DragAction::COPY,
    );
    {
        let state = state.clone();
        drop.connect_drop(move |_, value, _, _| {
            if let Ok(list) = value.get::<gtk::gdk::FileList>() {
                for file in list.files() {
                    if let Some(path) = file.path() {
                        add_converter_file(&state, path);
                    }
                }
                return true;
            }
            false
        });
    }
    page.add_controller(drop);

    page.upcast()
}

fn pick_files(state: &Rc<AppState>) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = gtk::FileDialog::builder()
        .title(tr("Select Media Files"))
        .build();
    let state = state.clone();
    dialog.open_multiple(Some(&window), gtk::gio::Cancellable::NONE, move |res| {
        if let Ok(files) = res {
            for i in 0..files.n_items() {
                if let Some(obj) = files.item(i) {
                    if let Ok(file) = obj.downcast::<gtk::gio::File>() {
                        if let Some(path) = file.path() {
                            add_converter_file(&state, path);
                        }
                    }
                }
            }
        }
    });
}

/// The per-row widgets a conversion updates (bundled to keep arg lists sane).
#[derive(Clone)]
struct ConvUi {
    progress: gtk::ProgressBar,
    status: gtk::Label,
    detail: gtk::Label,
    loc_lbl: gtk::Label,
    convert: gtk::Button,
    cancel: gtk::Button,
    folder: gtk::Button,
    play: gtk::Button,
    favorite: gtk::Button,
    format: gtk::DropDown,
    meta_chk: gtk::CheckButton,
    subs_chk: gtk::CheckButton,
    out_path: Rc<RefCell<String>>,
    // The row's card, so "remove when finished/cancelled" can drop it.
    container: gtk::Box,
}

impl ConvUi {
    /// Lock/unlock the format dropdown and option toggles while a conversion
    /// runs (mirrors `converter_row.py` disabling `combo_format`).
    fn set_inputs_sensitive(&self, on: bool) {
        self.format.set_sensitive(on);
        self.meta_chk.set_sensitive(on);
        self.subs_chk.set_sensitive(on);
    }
}

/// A queued conversion job, awaiting its turn in the single-slot pipeline.
pub(crate) struct PendingConv {
    path: std::path::PathBuf,
    fmt: String,
    add_metadata: bool,
    add_subtitles: bool,
    ui: ConvUi,
    cancel_flag: Arc<AtomicBool>,
    // Replace an existing output file instead of writing a " (n)" copy.
    overwrite: bool,
}

/// Queue a conversion and try to start it. The row shows "Queued" until its
/// turn comes (`converter_controller.py::_on_row_request_start`).
fn enqueue_conversion(state: &Rc<AppState>, job: PendingConv) {
    job.ui.convert.set_visible(false);
    job.ui.cancel.set_visible(true);
    job.ui.folder.set_visible(false);
    job.ui.play.set_visible(false);
    job.ui.favorite.set_visible(false);
    job.ui.set_inputs_sensitive(false);
    job.ui.set_progress_class("");
    job.ui.status.set_text(&tr("Queued"));
    state.conv_queue.borrow_mut().push_back(job);
    pump_conversion(state);
}

/// Start the next queued conversion if none is running. Jobs cancelled while
/// still queued are dropped (their row removed) and the next is tried.
fn pump_conversion(state: &Rc<AppState>) {
    if state.conv_active.get() {
        return;
    }
    let job = match state.conv_queue.borrow_mut().pop_front() {
        Some(j) => j,
        None => return,
    };
    if job.cancel_flag.load(Ordering::SeqCst) {
        // Cancelled while still queued: reset the row (keep it for re-convert).
        job.ui.reset_ready();
        pump_conversion(state);
        return;
    }
    state.conv_active.set(true);
    run_conversion(
        job.path,
        job.fmt,
        job.add_metadata,
        job.add_subtitles,
        job.ui,
        job.cancel_flag,
        job.overwrite,
        state.clone(),
    );
}

impl ConvUi {
    fn set_progress_class(&self, class: &str) {
        for c in ["success", "warning", "error"] {
            self.progress.remove_css_class(c);
        }
        if !class.is_empty() {
            self.progress.add_css_class(class);
        }
    }

    /// Reset the row to its initial idle look (after a cancel), so another
    /// format can be chosen and converted again — the row is NOT removed.
    fn reset_ready(&self) {
        self.cancel.set_visible(false);
        self.convert.set_visible(true);
        self.set_inputs_sensitive(true);
        self.set_progress_class("");
        self.progress.set_fraction(0.0);
        self.status.set_text(&tr("Ready"));
        self.detail.set_text("");
    }
}

pub(crate) fn add_converter_file(state: &Rc<AppState>, path: std::path::PathBuf) {
    add_converter_row(state, path, None);
}

/// Build a converter row. `restore` carries the saved (format, metadata,
/// subtitles) when re-creating a *pending* row on startup; `None` for a fresh
/// add (which also pulls the user over to the Converter tab).
fn add_converter_row(
    state: &Rc<AppState>,
    path: std::path::PathBuf,
    restore: Option<(String, bool, bool)>,
) {
    let source = path.to_string_lossy().to_string();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());

    let container = gtk::Box::new(gtk::Orientation::Vertical, 4);
    // Tag the card so the converter filter can match it by file name/path.
    super::set_row_filter_key(&container, &format!("{name} {source}"));
    container.add_css_class("card");
    container.set_margin_top(6);
    container.set_margin_bottom(6);
    container.set_margin_start(8);
    container.set_margin_end(8);
    let pad = gtk::Box::new(gtk::Orientation::Vertical, 4);
    pad.set_margin_top(8);
    pad.set_margin_bottom(8);
    pad.set_margin_start(12);
    pad.set_margin_end(12);

    let formats = convert_formats_for(&path);
    let is_video = !is_audio_input(&path);
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name_lbl = gtk::Label::new(Some(&name));
    name_lbl.set_xalign(0.0);
    name_lbl.set_hexpand(true);
    name_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
    name_lbl.add_css_class("heading");
    let format = gtk::DropDown::from_strings(formats);
    if let Some((fmt, _, _)) = &restore {
        if let Some(i) = formats.iter().position(|f| *f == fmt.as_str()) {
            format.set_selected(i as u32);
        }
    }
    // Per-file output type (Video / Audio). Switching it repopulates the format
    // dropdown, so a video can be converted to an audio format (extract audio)
    // and vice-versa. Defaults to the source's own media type.
    let type_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    type_box.add_css_class("linked");
    let t_video = gtk::ToggleButton::with_label(&tr("Video"));
    let t_audio = gtk::ToggleButton::with_label(&tr("Audio"));
    t_audio.set_group(Some(&t_video));
    if is_video {
        t_video.set_active(true);
    } else {
        t_audio.set_active(true);
    }
    type_box.append(&t_video);
    type_box.append(&t_audio);
    let convert = gtk::Button::from_icon_name("bigtube-view-refresh-symbolic");
    convert.add_css_class("flat");
    convert.set_tooltip_text(Some(&tr("Convert")));
    a11y_label(&convert, &tr("Convert"));
    let cancel = gtk::Button::from_icon_name("bigtube-process-stop-symbolic");
    cancel.add_css_class("flat");
    cancel.add_css_class("destructive-action");
    cancel.set_tooltip_text(Some(&tr("Cancel")));
    a11y_label(&cancel, &tr("Cancel"));
    cancel.set_visible(false);
    let folder = gtk::Button::from_icon_name("bigtube-folder-open-symbolic");
    folder.add_css_class("flat");
    folder.set_tooltip_text(Some(&tr("Open Folder")));
    a11y_label(&folder, &tr("Open Folder"));
    folder.set_visible(false);
    let play = gtk::Button::from_icon_name("bigtube-media-playback-start-symbolic");
    play.add_css_class("flat");
    play.set_tooltip_text(Some(&tr("Play Video")));
    a11y_label(&play, &tr("Play Video"));
    play.set_visible(false);
    let favorite = gtk::Button::from_icon_name("bigtube-emblem-favorite-symbolic");
    favorite.add_css_class("flat");
    favorite.set_tooltip_text(Some(&tr("Add to Favorites")));
    a11y_label(&favorite, &tr("Add to Favorites"));
    favorite.set_visible(false);
    let remove = gtk::Button::from_icon_name("bigtube-user-trash-symbolic");
    remove.add_css_class("flat");
    remove.set_tooltip_text(Some(&tr("Remove from list")));
    a11y_label(&remove, &tr("Remove from list"));
    // Short status word lives in the top row (like the downloads list); the
    // detailed progress / media summary goes in the bottom row.
    let status = gtk::Label::new(Some(tr("Ready").as_str()));
    status.set_ellipsize(gtk::pango::EllipsizeMode::End);
    status.add_css_class("dim-label");
    status.add_css_class("caption");
    // Top row: name + format input + convert/cancel (next to the dropdown) +
    // status + delete. The bottom row carries the detail line + play/folder/fav.
    header.append(&name_lbl);
    header.append(&type_box);
    header.append(&format);
    header.append(&convert);
    header.append(&cancel);
    header.append(&status);
    header.append(&remove);

    // Output folder under the title ("Location: <folder>"); starts at the source
    // file's folder and is updated to the real output folder once it succeeds.
    let loc_lbl = gtk::Label::new(Some(&crate::app::location_label(&path.to_string_lossy())));
    loc_lbl.set_xalign(0.0);
    loc_lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    loc_lbl.add_css_class("dim-label");
    loc_lbl.add_css_class("caption");

    // Conversion options (mirrors `converter_row.py`): both default on; the
    // subtitle toggle only applies to video inputs.
    let opts = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let meta_chk = gtk::CheckButton::with_label(&tr("Add Metadata"));
    meta_chk.set_active(restore.as_ref().map(|r| r.1).unwrap_or(true));
    let subs_chk = gtk::CheckButton::with_label(&tr("Add Subtitles"));
    subs_chk.set_active(restore.as_ref().map(|r| r.2).unwrap_or(true));
    // Subtitles only apply when producing a video output from a video source.
    subs_chk.set_visible(is_video);
    opts.append(&meta_chk);
    opts.append(&subs_chk);

    // Switching the Video/Audio toggle repopulates the format list and toggles
    // the subtitle option.
    {
        let format = format.clone();
        let subs_chk = subs_chk.clone();
        let input_is_video = is_video;
        t_video.connect_toggled(move |b| {
            let video_out = b.is_active();
            let fmts: &[&str] = if video_out {
                &VIDEO_FORMATS
            } else {
                &AUDIO_FORMATS
            };
            format.set_model(Some(&gtk::StringList::new(fmts)));
            subs_chk.set_visible(video_out && input_is_video);
        });
    }

    let progress = gtk::ProgressBar::new();
    progress.set_fraction(0.0);

    // Live progress detail ("45% · 1.2x · ETA 00:10") while converting, then the
    // media summary once done — bottom-left, on the same row as the actions.
    let detail = gtk::Label::new(None);
    detail.set_xalign(0.0);
    detail.set_hexpand(true);
    detail.set_ellipsize(gtk::pango::EllipsizeMode::End);
    detail.add_css_class("dim-label");
    detail.add_css_class("caption");

    // Bottom row: detail on the left, play/folder/favorite (post-conversion) on
    // the right.
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    actions.set_halign(gtk::Align::End);
    actions.append(&folder);
    actions.append(&play);
    actions.append(&favorite);
    footer.append(&detail);
    footer.append(&actions);

    pad.append(&header);
    pad.append(&loc_lbl);
    pad.append(&opts);
    pad.append(&progress);
    pad.append(&footer);
    container.append(&pad);
    state.converter_box.append(&container);
    state.update_converter_empty();
    // Don't yank the user to the Converter tab when restoring rows at startup.
    if restore.is_none() {
        state.stack.set_visible_child_name("converter");
    }

    let ui = ConvUi {
        progress,
        status,
        detail,
        loc_lbl,
        convert: convert.clone(),
        cancel: cancel.clone(),
        folder: folder.clone(),
        play: play.clone(),
        favorite: favorite.clone(),
        format: format.clone(),
        meta_chk: meta_chk.clone(),
        subs_chk: subs_chk.clone(),
        out_path: Rc::new(RefCell::new(String::new())),
        container: container.clone(),
    };

    // Persist this as a pending item so it survives a restart even if it's never
    // converted, and keep the stored format/options in sync as the user tweaks
    // them. The entry is dropped on removal (confirm_delete_converter) or once
    // the conversion succeeds (run_conversion).
    save_pending_conv(
        &source,
        &selected_format(&format),
        meta_chk.is_active(),
        subs_chk.is_active(),
    );
    {
        let (src, meta, subs) = (source.clone(), meta_chk.clone(), subs_chk.clone());
        format.connect_selected_notify(move |dd| {
            save_pending_conv(
                &src,
                &selected_format(dd),
                meta.is_active(),
                subs.is_active(),
            );
        });
    }
    {
        let (src, dd, subs) = (source.clone(), format.clone(), subs_chk.clone());
        meta_chk.connect_toggled(move |m| {
            save_pending_conv(&src, &selected_format(&dd), m.is_active(), subs.is_active());
        });
    }
    {
        let (src, dd, meta) = (source.clone(), format.clone(), meta_chk.clone());
        subs_chk.connect_toggled(move |s| {
            save_pending_conv(&src, &selected_format(&dd), meta.is_active(), s.is_active());
        });
    }

    // Remove this row from the list.
    {
        let state = state.clone();
        let container = container.clone();
        let source = path.to_string_lossy().to_string();
        let out_path = ui.out_path.clone();
        remove.connect_clicked(move |_| {
            confirm_delete_converter(&state, &container, &source, None, &out_path.borrow());
        });
    }
    // Open the converted file's folder.
    {
        let state = state.clone();
        let out_path = ui.out_path.clone();
        folder.connect_clicked(move |_| open_containing_folder(&state, &out_path.borrow()));
    }
    // Play the converted file, seeding a cyclic queue of all converted files.
    {
        let state = state.clone();
        let out_path = ui.out_path.clone();
        play.connect_clicked(move |_| {
            // Toggle play/pause in sync with the bar if this output is active.
            if let Some(player) = state.player.borrow().clone() {
                let p = out_path.borrow().clone();
                if !p.is_empty() && player.now_playing().url() == p {
                    player.now_playing().request_toggle();
                    return;
                }
            }
            play_converter_at(&state, &out_path.borrow());
        });
    }
    // Highlight this row while its output is the one playing, and sync its glyph.
    wire_play_highlight(state, &container, ui.out_path.clone(), &play);
    // Favorite the converted local file (heart appears once it succeeds).
    {
        let out_path = ui.out_path.clone();
        favorite.connect_clicked(move |b| {
            let path = out_path.borrow().clone();
            if path.is_empty() {
                return;
            }
            let now = crate::app::favorites::toggle_local(&path, "");
            crate::app::favorites::set_heart_icon(b, now);
        });
    }
    crate::app::favorites::watch_heart(&favorite, ui.out_path.clone());

    // Convert (with a cancel flag the cancel button flips).
    {
        let ui = ui.clone();
        let format = format.clone();
        let cancel = cancel.clone();
        let state = state.clone();
        convert.connect_clicked(move |btn| {
            let _ = btn;
            let fmt = selected_format(&format);
            // Read the per-row option toggles; subtitles never apply to audio.
            let add_metadata = ui.meta_chk.is_active();
            let add_subtitles = is_video && ui.subs_chk.is_active();
            let source = path.to_string_lossy().to_string();

            // One enqueue, parameterised by the overwrite choice.
            let do_enqueue: Rc<dyn Fn(bool)> = {
                let state = state.clone();
                let path = path.clone();
                let ui = ui.clone();
                let cancel = cancel.clone();
                let fmt = fmt.clone();
                Rc::new(move |overwrite: bool| {
                    let flag = Arc::new(AtomicBool::new(false));
                    {
                        let flag = flag.clone();
                        cancel.connect_clicked(move |_| flag.store(true, Ordering::SeqCst));
                    }
                    enqueue_conversion(
                        &state,
                        PendingConv {
                            path: path.clone(),
                            fmt: fmt.clone(),
                            add_metadata,
                            add_subtitles,
                            ui: ui.clone(),
                            cancel_flag: flag,
                            overwrite,
                        },
                    );
                })
            };

            // If the output already exists, ask: Overwrite / Keep Both / Cancel.
            let planned = bigtube_core::converter::planned_output_path(&source, &fmt);
            if !std::path::Path::new(&planned).exists() {
                do_enqueue(false);
                return;
            }
            let Some(window) = state.window.borrow().clone() else {
                do_enqueue(false);
                return;
            };
            let dialog = adw::MessageDialog::new(
                Some(&window),
                Some(&tr("File already exists")),
                Some(&format!(
                    "{}\n\n{}",
                    tr("A file with this name is already in the output folder."),
                    planned
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
                    "overwrite" => do_enqueue(true),
                    "keep" => do_enqueue(false),
                    _ => {}
                }
                dlg.close();
            });
            dialog.present();
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn run_conversion(
    path: std::path::PathBuf,
    fmt: String,
    add_metadata: bool,
    add_subtitles: bool,
    ui: ConvUi,
    cancel_flag: Arc<AtomicBool>,
    overwrite: bool,
    state: Rc<AppState>,
) {
    use bigtube_core::converter::{convert_media, ConvertProgressFn};

    let (tx, rx) = async_channel::unbounded::<ConvMsg>();
    let tx_progress = tx.clone();
    let cb: ConvertProgressFn = Arc::new(move |p, speed, eta| {
        let _ = tx_progress.send_blocking(ConvMsg::Progress(p, speed, eta));
    });

    let input = path.to_string_lossy().to_string();
    let source = input.clone();
    let fmt_hist = fmt.clone();
    let flag = cancel_flag.clone();
    std::thread::spawn(move || {
        let result = convert_media(
            &input,
            &fmt,
            Some(&cb),
            add_metadata,
            add_subtitles,
            Some(&flag),
            overwrite,
        )
        .map_err(|e| e.to_string());
        let _ = tx.send_blocking(ConvMsg::Done(result));
    });

    glib::spawn_future_local(async move {
        while let Ok(msg) = rx.recv().await {
            match msg {
                ConvMsg::Progress(p, speed, eta) => {
                    ui.progress.set_fraction(p);
                    let mut parts: Vec<String> = vec![format!("{:.0}%", p * 100.0)];
                    if let Some(s) = speed.filter(|s| *s > 0.0) {
                        parts.push(format!("{s:.1}x"));
                    }
                    if let Some(e) = eta.filter(|e| *e > 0.0) {
                        parts.push(format!("ETA {}", fmt_eta(e)));
                    }
                    ui.status.set_text(&tr("Converting"));
                    ui.detail.set_text(&parts.join(" · "));
                }
                ConvMsg::Done(Ok(out)) => {
                    // Converted: it graduates from the pending queue (it'll be
                    // recorded in converter history below if enabled).
                    remove_pending_conv(&source);
                    // "Remove when finished": drop the row and skip history, so
                    // it leaves no trace (and won't reappear on the next launch).
                    let remove = config::global()
                        .read()
                        .map(|c| c.get_bool("converter_remove_on_complete"))
                        .unwrap_or(false);
                    if remove {
                        remove_list_card(&state.converter_box, &ui.container);
                        state.update_converter_empty();
                        continue;
                    }
                    ui.progress.set_fraction(1.0);
                    ui.set_progress_class("success");
                    ui.status.set_text(&tr("Completed"));
                    ui.cancel.set_visible(false);
                    ui.convert.set_visible(true);
                    ui.set_inputs_sensitive(true);
                    ui.out_path.replace(out.clone());
                    ui.loc_lbl.set_text(&crate::app::location_label(&out));
                    ui.loc_lbl.set_tooltip_text(Some(&out));
                    ui.folder.set_visible(true);
                    ui.play.set_visible(true);
                    ui.favorite.set_visible(true);
                    crate::app::favorites::set_heart_icon(
                        &ui.favorite,
                        crate::app::favorites::favorites().contains(&out),
                    );
                    // Probe the converted file (codecs + real size) and show it as
                    // the bottom detail line (the top keeps the "Success!" word).
                    {
                        let (itx, irx) = async_channel::bounded::<String>(1);
                        let outp = out.clone();
                        std::thread::spawn(move || {
                            let s = bigtube_core::converter::probe_media_summary(&outp);
                            let _ = itx.send_blocking(media_summary_text(&s, &outp));
                        });
                        let detail_lbl = ui.detail.clone();
                        glib::spawn_future_local(async move {
                            if let Ok(text) = irx.recv().await {
                                if !text.is_empty() {
                                    detail_lbl.set_text(&text);
                                }
                            }
                        });
                    }
                    if config::global()
                        .read()
                        .unwrap_or_else(|e| e.into_inner())
                        .get_bool("save_converter_history")
                    {
                        bigtube_core::converter_history::ConverterHistoryManager::with_max(
                            bigtube_core::paths::config_dir().join("converter_history.json"),
                            max_converter_history(),
                        )
                        .add_entry(&source, &out, &fmt_hist);
                    }
                }
                ConvMsg::Done(Err(e)) => {
                    if cancel_flag.load(Ordering::SeqCst) {
                        // Cancelled by the user: the core removed the partial
                        // output. "Remove when cancelled" drops the row (and its
                        // pending entry); otherwise keep it, reset for a retry.
                        let remove = config::global()
                            .read()
                            .map(|c| c.get_bool("converter_remove_on_cancel"))
                            .unwrap_or(false);
                        if remove {
                            remove_pending_conv(&source);
                            remove_list_card(&state.converter_box, &ui.container);
                            state.update_converter_empty();
                        } else {
                            ui.reset_ready();
                        }
                    } else {
                        ui.cancel.set_visible(false);
                        ui.convert.set_visible(true);
                        ui.set_inputs_sensitive(true);
                        ui.set_progress_class("error");
                        // Friendly status; keep the raw engine error in the log
                        // and the row's tooltip instead of dumping it on screen.
                        tracing::error!("conversion failed: {e}");
                        ui.status.set_text(&tr("Conversion failed"));
                        ui.status.set_tooltip_text(Some(&e));
                        ui.detail.set_text("");
                    }
                }
            }
        }
        // This conversion finished (ok, error, or cancel): free the slot and
        // let the next queued job start.
        state.conv_active.set(false);
        pump_conversion(&state);
    });
}

/// Play a converted file, seeding the queue from every converted output (from
/// history) so prev/next/EOS cycle through them. Falls back to a single play if
/// the clicked output isn't in history (e.g. converter history disabled).
fn play_converter_at(state: &Rc<AppState>, clicked: &str) {
    let Some(player) = state.player.borrow().clone() else {
        state.toast(&tr(
            "Playback unavailable — install the GStreamer gtk4 plugin",
        ));
        return;
    };
    if clicked.is_empty() {
        return;
    }
    let history: Vec<serde_json::Value> =
        bigtube_core::json_store::load_json(converter_history_path(), Vec::new());
    let mut items = Vec::new();
    let mut start = 0usize;
    let mut found = false;
    for it in &history {
        let out = it.get("output").and_then(|v| v.as_str()).unwrap_or("");
        if out.is_empty() || !std::path::Path::new(out).exists() {
            continue;
        }
        if out == clicked {
            start = items.len();
            found = true;
        }
        let title = std::path::Path::new(out)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        items.push(crate::player::QueueItem {
            url: out.to_string(),
            title,
            artist: String::new(),
            thumbnail: String::new(),
            is_local: true,
            is_video: !is_audio_input(std::path::Path::new(out)),
        });
    }
    if found && !items.is_empty() {
        player.play_queue(items, start);
    } else {
        // Not in history: just play the one file.
        let title = std::path::Path::new(clicked)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        player.play_local(clicked, &title, "");
    }
}

/// Path to the persisted *pending* converter queue — files added to the
/// Converter but not yet converted. Kept apart from converter_history.json
/// (completed runs) so queued items survive a restart even if never converted.
fn converter_pending_path() -> std::path::PathBuf {
    bigtube_core::paths::config_dir().join("converter_pending.json")
}

/// Insert or update a pending converter entry, keyed by source path.
fn save_pending_conv(source: &str, format: &str, metadata: bool, subtitles: bool) {
    let mut items: Vec<serde_json::Value> =
        bigtube_core::json_store::load_json(converter_pending_path(), Vec::new());
    items.retain(|it| it.get("source").and_then(|v| v.as_str()) != Some(source));
    items.push(serde_json::json!({
        "source": source,
        "format": format,
        "metadata": metadata,
        "subtitles": subtitles,
    }));
    bigtube_core::json_store::save_json(converter_pending_path(), &items, Some(2));
}

/// Drop a pending converter entry by source (no-op if absent).
fn remove_pending_conv(source: &str) {
    let mut items: Vec<serde_json::Value> =
        bigtube_core::json_store::load_json(converter_pending_path(), Vec::new());
    let before = items.len();
    items.retain(|it| it.get("source").and_then(|v| v.as_str()) != Some(source));
    if items.len() != before {
        bigtube_core::json_store::save_json(converter_pending_path(), &items, Some(2));
    }
}

/// Restore pending converter rows (added but not converted) on startup, pruning
/// entries whose source file no longer exists.
pub(crate) fn load_pending_conv(state: &Rc<AppState>) {
    let items: Vec<serde_json::Value> =
        bigtube_core::json_store::load_json(converter_pending_path(), Vec::new());
    let mut kept = 0usize;
    for it in &items {
        let source = it.get("source").and_then(|v| v.as_str()).unwrap_or("");
        if source.is_empty() || !std::path::Path::new(source).exists() {
            continue; // source gone — drop the dead entry
        }
        let format = it
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let metadata = it.get("metadata").and_then(|v| v.as_bool()).unwrap_or(true);
        let subtitles = it
            .get("subtitles")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        add_converter_row(
            state,
            std::path::PathBuf::from(source),
            Some((format, metadata, subtitles)),
        );
        kept += 1;
    }
    // If we skipped any dead entries, rewrite the file without them (each
    // restored row already re-saved itself via add_converter_row).
    if kept != items.len() {
        let alive: Vec<serde_json::Value> = items
            .into_iter()
            .filter(|it| {
                it.get("source")
                    .and_then(|v| v.as_str())
                    .map(|s| !s.is_empty() && std::path::Path::new(s).exists())
                    .unwrap_or(false)
            })
            .collect();
        bigtube_core::json_store::save_json(converter_pending_path(), &alive, Some(2));
    }
    state.update_converter_empty();
}

/// Restore past conversions into the Converter list as completed rows.
pub(crate) fn load_converter_history(state: &Rc<AppState>) {
    // Pure read: do NOT construct a ConverterHistoryManager here — its debouncer
    // flushes on drop, which would turn this load into a write and could clobber
    // the file with an empty list on a transient read race.
    let items: Vec<serde_json::Value> =
        bigtube_core::json_store::load_json(converter_history_path(), Vec::new());
    for it in items.iter().take(max_converter_history()) {
        let source = it.get("source").and_then(|v| v.as_str()).unwrap_or("");
        let output = it.get("output").and_then(|v| v.as_str()).unwrap_or("");
        let format = it.get("format").and_then(|v| v.as_str()).unwrap_or("");
        if output.is_empty() {
            continue;
        }
        add_converted_history_row(state, source, output, format);
    }
    state.update_converter_empty();
}

/// A finished converter row restored from history: shows the output name, a
/// completed bar, and open-folder / play / remove actions (no convert flow).
fn add_converted_history_row(state: &Rc<AppState>, source: &str, output: &str, format: &str) {
    let name = std::path::Path::new(output)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| output.to_string());

    let container = gtk::Box::new(gtk::Orientation::Vertical, 4);
    // Tag the card so the converter filter can match it by name/source/output.
    super::set_row_filter_key(&container, &format!("{name} {source} {output}"));
    container.add_css_class("card");
    container.set_margin_top(6);
    container.set_margin_bottom(6);
    container.set_margin_start(8);
    container.set_margin_end(8);
    let pad = gtk::Box::new(gtk::Orientation::Vertical, 4);
    pad.set_margin_top(8);
    pad.set_margin_bottom(8);
    pad.set_margin_start(12);
    pad.set_margin_end(12);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name_lbl = gtk::Label::new(Some(&name));
    name_lbl.set_xalign(0.0);
    name_lbl.set_hexpand(true);
    name_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
    name_lbl.add_css_class("heading");
    let folder = gtk::Button::from_icon_name("bigtube-folder-open-symbolic");
    folder.add_css_class("flat");
    folder.set_tooltip_text(Some(&tr("Open Folder")));
    let play = gtk::Button::from_icon_name("bigtube-media-playback-start-symbolic");
    play.add_css_class("flat");
    play.set_tooltip_text(Some(&tr("Play Video")));
    let favorite = gtk::Button::from_icon_name("bigtube-emblem-favorite-symbolic");
    favorite.add_css_class("flat");
    favorite.set_tooltip_text(Some(&tr("Add to Favorites")));
    let remove = gtk::Button::from_icon_name("bigtube-user-trash-symbolic");
    remove.add_css_class("flat");
    remove.set_tooltip_text(Some(&tr("Remove from list")));
    // Top row: name + status + delete (matching the downloads list).
    let status = gtk::Label::new(Some(tr("Completed").as_str()));
    status.set_ellipsize(gtk::pango::EllipsizeMode::End);
    status.add_css_class("dim-label");
    status.add_css_class("caption");
    header.append(&name_lbl);
    header.append(&status);
    header.append(&remove);

    // Output folder under the name ("Location: <folder>"); full path as tooltip.
    let loc_lbl = gtk::Label::new(Some(&crate::app::location_label(output)));
    loc_lbl.set_xalign(0.0);
    loc_lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    loc_lbl.set_tooltip_text(Some(output));
    loc_lbl.add_css_class("dim-label");
    loc_lbl.add_css_class("caption");

    // Bottom row: media-summary detail on the left, play/folder/favorite right.
    let detail = gtk::Label::new(None);
    detail.set_xalign(0.0);
    detail.set_hexpand(true);
    detail.set_ellipsize(gtk::pango::EllipsizeMode::End);
    detail.add_css_class("dim-label");
    detail.add_css_class("caption");
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    actions.set_halign(gtk::Align::End);
    actions.append(&folder);
    actions.append(&play);
    actions.append(&favorite);
    footer.append(&detail);
    footer.append(&actions);

    pad.append(&header);
    pad.append(&loc_lbl);
    pad.append(&footer);
    container.append(&pad);
    state.converter_box.append(&container);

    let exists = std::path::Path::new(output).exists();
    folder.set_visible(exists);
    play.set_visible(exists);
    favorite.set_visible(exists);
    crate::app::favorites::set_heart_icon(
        &favorite,
        exists && crate::app::favorites::favorites().contains(output),
    );
    // Probe the file for the media summary (codecs + size), shown as the detail.
    if exists {
        let (itx, irx) = async_channel::bounded::<String>(1);
        let outp = output.to_string();
        std::thread::spawn(move || {
            let s = bigtube_core::converter::probe_media_summary(&outp);
            let _ = itx.send_blocking(media_summary_text(&s, &outp));
        });
        let detail_lbl = detail.clone();
        glib::spawn_future_local(async move {
            if let Ok(text) = irx.recv().await {
                if !text.is_empty() {
                    detail_lbl.set_text(&text);
                }
            }
        });
    }

    let out = output.to_string();
    {
        let state = state.clone();
        let out = out.clone();
        folder.connect_clicked(move |_| open_containing_folder(&state, &out));
    }
    {
        let state = state.clone();
        let out = out.clone();
        play.connect_clicked(move |_| {
            // Toggle play/pause in sync with the bar if this output is active.
            if let Some(player) = state.player.borrow().clone() {
                if !out.is_empty() && player.now_playing().url() == out {
                    player.now_playing().request_toggle();
                    return;
                }
            }
            play_converter_at(&state, &out);
        });
    }
    // Highlight this row while its output is the one playing, and sync its glyph.
    let out_rc = Rc::new(RefCell::new(output.to_string()));
    wire_play_highlight(state, &container, out_rc.clone(), &play);
    // Favorite the converted local file.
    {
        let out = out.clone();
        favorite.connect_clicked(move |b| {
            if out.is_empty() {
                return;
            }
            let now = crate::app::favorites::toggle_local(&out, "");
            crate::app::favorites::set_heart_icon(b, now);
        });
    }
    crate::app::favorites::watch_heart(&favorite, out_rc);
    {
        let state = state.clone();
        let container = container.clone();
        let source = source.to_string();
        let format = format.to_string();
        let out = out.clone();
        remove.connect_clicked(move |_| {
            confirm_delete_converter(&state, &container, &source, Some(&format), &out);
        });
    }
}

/// "Clear all" conversions: ask history vs files, then wipe every row.
fn confirm_clear_all_converter(state: &Rc<AppState>) {
    if state.converter_box.first_child().is_none() {
        return;
    }
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = adw::MessageDialog::new(
        Some(&window),
        Some(&tr("Clear all conversions?")),
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
            // Stop anything still queued (a running one finishes on its own).
            state.conv_queue.borrow_mut().clear();
            let mgr = bigtube_core::converter_history::ConverterHistoryManager::new(
                converter_history_path(),
            );
            if resp == "file" {
                for it in mgr.load() {
                    if let Some(out) = it.get("output").and_then(|v| v.as_str()) {
                        delete_output_file(out);
                    }
                }
            }
            mgr.clear_all();
            // Also drop queued-but-unconverted items so they don't reappear.
            let _ = std::fs::remove_file(converter_pending_path());
            while let Some(c) = state.converter_box.first_child() {
                state.converter_box.remove(&c);
            }
            state.update_converter_empty();
        }
        dlg.close();
    });
    dialog.present();
}

/// Ask "remove from history" vs "delete output file too" for one conversion.
fn confirm_delete_converter(
    state: &Rc<AppState>,
    container: &gtk::Box,
    source: &str,
    format: Option<&str>,
    out_path: &str,
) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog = adw::MessageDialog::new(
        Some(&window),
        Some(&tr("Remove conversion?")),
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
    let source = source.to_string();
    let format = format.map(str::to_string);
    let out_path = out_path.to_string();
    dialog.connect_response(None, move |dlg, resp| {
        if resp == "history" || resp == "file" {
            if resp == "file" {
                delete_output_file(&out_path);
            }
            bigtube_core::converter_history::ConverterHistoryManager::new(converter_history_path())
                .remove_entry(&source, format.as_deref());
            // Drop it from the pending queue too (no-op for finished rows).
            remove_pending_conv(&source);
            remove_list_card(&state.converter_box, &container);
            state.update_converter_empty();
        }
        dlg.close();
    });
    dialog.present();
}
