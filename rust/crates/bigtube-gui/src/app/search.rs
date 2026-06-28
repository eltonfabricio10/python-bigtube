//! Search page: the search bar/results UI, the search execution flow
//! (`run_search`), result playback (`play_from_store`) and the
//! "remove link from history" prompt. Download/schedule actions and the
//! search-history path helper live in the parent module and are reached via
//! `super::`.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};

use bigtube_core::config;
use bigtube_core::search::SearchEngine;

use super::widgets::{loading_page, page_header_trailing, status_page};
use super::{
    a11y_label, apply_theme_classes, download_all, make_filter_control, on_download_clicked,
    schedule_all, search_history_path, AppState,
};
use crate::i18n::tr;
use crate::objects::VideoObject;
use crate::row::{RowAction, SearchResultRow};

/// On-disk known-channels index (`~/.config/bigtube/channels.json`).
fn channels_path() -> std::path::PathBuf {
    bigtube_core::paths::config_dir().join("channels.json")
}

/// A fresh handle to the known-channels store.
fn channels() -> bigtube_core::channels::Channels {
    bigtube_core::channels::Channels::new(channels_path())
}

pub(crate) fn build_search_page(state: &Rc<AppState>) -> gtk::Widget {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Search bar.
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    bar.set_margin_top(8);
    bar.set_margin_bottom(8);
    bar.set_margin_start(12);
    bar.set_margin_end(12);
    let source = gtk::DropDown::from_strings(&[
        tr("YouTube").as_str(),
        tr("YouTube Music").as_str(),
        tr("Direct Link").as_str(),
    ]);
    let entry = gtk::SearchEntry::new();
    entry.set_hexpand(true);
    entry.set_placeholder_text(Some(&tr("Paste URL or type keywords...")));
    state.search_entry.replace(Some(entry.clone()));
    let button = gtk::Button::with_label(&tr("Search"));
    button.add_css_class("suggested-action");
    let btn_select = state.btn_select.clone();
    btn_select.set_icon_name("bigtube-selection-mode-symbolic");
    btn_select.add_css_class("flat");
    btn_select.set_tooltip_text(Some(&tr("Toggle Selection Mode")));
    // No results yet → nothing to select.
    btn_select.set_sensitive(false);
    a11y_label(&btn_select, &tr("Toggle Selection Mode"));
    bar.append(&source);
    bar.append(&entry);
    bar.append(&button);
    bar.append(&btn_select);

    // Results list.
    let on_download: RowAction = {
        let state = state.clone();
        Rc::new(move |item: VideoObject| on_download_clicked(&state, &item))
    };
    let on_play: RowAction = {
        let state = state.clone();
        Rc::new(move |item: VideoObject| {
            let store = state.search_store.clone();
            play_from_store(&state, &store, &item);
        })
    };
    let on_copy: RowAction = {
        let state = state.clone();
        Rc::new(move |item: VideoObject| {
            if let Some(win) = state.window.borrow().clone() {
                win.clipboard().set_text(&item.url());
                state.toast(&tr("Link Copied!"));
            }
        })
    };
    // Opening a playlist: the dialog plays from its own (cyclic) queue.
    let on_open: RowAction = {
        let state = state.clone();
        let on_download = on_download.clone();
        Rc::new(move |item: VideoObject| {
            let (Some(win), Some(player)) =
                (state.window.borrow().clone(), state.player.borrow().clone())
            else {
                return;
            };
            // Batch download (one quality dialog for the whole list).
            let on_download_all: crate::playlist::BatchAction = {
                let state = state.clone();
                Rc::new(move |items: Vec<VideoObject>| download_all(&state, items))
            };
            // Batch schedule (one quality + one time/recurrence for the list).
            let on_schedule_all: crate::playlist::BatchAction = {
                let state = state.clone();
                Rc::new(move |items: Vec<VideoObject>| schedule_all(&state, items))
            };
            crate::playlist::show(
                &win,
                item.url(),
                item.title(),
                player,
                on_download.clone(),
                on_download_all,
                on_schedule_all,
            );
        })
    };
    // A local title filter narrows the fetched results as you type. The filter
    // wraps the store; selection wraps the filter so the ListView shows only the
    // matching rows (the underlying store — and select-all/append — is untouched).
    let needle = Rc::new(RefCell::new(String::new()));
    let f_needle = needle.clone();
    let filter = gtk::CustomFilter::new(move |obj| {
        let n = f_needle.borrow();
        n.is_empty()
            || obj
                .downcast_ref::<VideoObject>()
                .map(|v| v.title().to_lowercase().contains(n.as_str()))
                .unwrap_or(true)
    });
    let filter_model =
        gtk::FilterListModel::new(Some(state.search_store.clone()), Some(filter.clone()));
    // A boxed-list ListBox gives the carded look (matching the playlist + the
    // favorites popover) while keeping the full SearchResultRow (thumbnail,
    // title, every action, selection mode, now-playing highlight). Rows act via
    // their own buttons, so no selection highlight. Per-row handlers on the
    // shared NowPlaying / favorites-watch are disconnected via the row's qdata
    // guards when rows are rebuilt on a new search.
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("boxed-list");
    list.set_valign(gtk::Align::Start);
    list.set_margin_top(12);
    list.set_margin_bottom(12);
    list.set_margin_start(12);
    list.set_margin_end(12);
    {
        let state = state.clone();
        list.bind_model(Some(&filter_model), move |obj| {
            let video = obj.downcast_ref::<VideoObject>().unwrap();
            let row = SearchResultRow::new();
            row.set_handlers(
                on_play.clone(),
                on_download.clone(),
                on_open.clone(),
                on_copy.clone(),
            );
            let (fav_toggle, fav_query) = crate::app::favorites::make_handlers();
            row.set_favorite_handlers(fav_toggle, fav_query, crate::app::favorites::watch());
            if let Some(player) = state.player.borrow().clone() {
                row.set_now_playing(player.now_playing());
            }
            row.set_item(video);
            row.upcast::<gtk::Widget>()
        });
    }
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_child(Some(&list));
    scrolled.set_vexpand(true);

    // Collapsible filter control (pinned to the header below); narrows results.
    // Disabled until there are results to filter (toggled by update_search_empty).
    let (filter_ctrl, filter_entry) = make_filter_control();
    filter_ctrl.set_sensitive(false);
    state.search_filter.replace(Some(filter_ctrl.clone()));
    filter_entry.connect_search_changed(move |e| {
        needle.replace(e.text().to_lowercase());
        filter.changed(gtk::FilterChange::Different);
    });

    // Empty-state / results stack.
    let empty = status_page(
        "bigtube-system-search-symbolic",
        &tr("No Results"),
        &tr("Search for videos or paste a URL"),
    );
    state.search_stack.set_vexpand(true);
    state.search_stack.add_named(&empty, Some("empty"));
    state
        .search_stack
        .add_named(&loading_page(&tr("Searching")), Some("loading"));
    state.search_stack.add_named(&scrolled, Some("list"));
    state.search_stack.set_visible_child_name("empty");

    // Batch selection bar (revealed in selection mode).
    let batch = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    batch.set_halign(gtk::Align::Center);
    batch.set_margin_bottom(6);
    batch.add_css_class("linked");
    let select_all = gtk::Button::with_label(&tr("Select All / None"));
    state
        .select_btn
        .set_label(&tr("Download Selected ({count})").replace("{count}", "0"));
    state.select_btn.add_css_class("suggested-action");
    state.select_btn.set_sensitive(false);
    state.sched_selected_btn.set_label(&tr("Schedule Selected"));
    state.sched_selected_btn.set_sensitive(false);
    batch.append(&select_all);
    batch.append(&state.select_btn);
    batch.append(&state.sched_selected_btn);
    state.select_revealer.set_child(Some(&batch));
    state.select_revealer.set_reveal_child(false);

    page.append(&page_header_trailing(
        &tr("Search Manager"),
        &[],
        Some(&filter_ctrl),
    ));
    page.append(&bar);
    page.append(&state.select_revealer);
    page.append(&state.search_stack);

    // Toggle selection mode: flips the flag on every item and reveals the bar.
    {
        let state = state.clone();
        btn_select.connect_toggled(move |b| {
            let on = b.is_active();
            state.select_mode.set(on);
            for i in 0..state.search_store.n_items() {
                if let Some(o) = state
                    .search_store
                    .item(i)
                    .and_then(|o| o.downcast::<VideoObject>().ok())
                {
                    o.set_selection_mode(on);
                    if !on {
                        o.set_is_selected(false);
                    }
                }
            }
            state.select_revealer.set_reveal_child(on);
            state.refresh_selection_count();
        });
    }
    // Select all / none toggles every item.
    {
        let state = state.clone();
        select_all.connect_clicked(move |_| {
            let store = &state.search_store;
            let mut any_unselected = false;
            for i in 0..store.n_items() {
                if let Some(o) = store.item(i).and_then(|o| o.downcast::<VideoObject>().ok()) {
                    if !o.is_playlist() && !o.is_selected() {
                        any_unselected = true;
                        break;
                    }
                }
            }
            for i in 0..store.n_items() {
                if let Some(o) = store.item(i).and_then(|o| o.downcast::<VideoObject>().ok()) {
                    if !o.is_playlist() {
                        o.set_is_selected(any_unselected);
                    }
                }
            }
            state.refresh_selection_count();
        });
    }
    // Download all selected items.
    {
        let state = state.clone();
        let btn = state.select_btn.clone();
        btn.connect_clicked(move |_| {
            let store = &state.search_store;
            let mut picked = Vec::new();
            for i in 0..store.n_items() {
                if let Some(o) = store.item(i).and_then(|o| o.downcast::<VideoObject>().ok()) {
                    if o.is_selected() {
                        picked.push(o);
                    }
                }
            }
            download_all(&state, picked);
        });
    }
    // Schedule all selected items (same picked set, routed to the schedule flow).
    {
        let state = state.clone();
        let btn = state.sched_selected_btn.clone();
        btn.connect_clicked(move |_| {
            let store = &state.search_store;
            let mut picked = Vec::new();
            for i in 0..store.n_items() {
                if let Some(o) = store.item(i).and_then(|o| o.downcast::<VideoObject>().ok()) {
                    if o.is_selected() {
                        picked.push(o);
                    }
                }
            }
            schedule_all(&state, picked);
        });
    }

    // Suggestion popover (search-history matches while typing).
    let popover = gtk::Popover::new();
    popover.set_parent(&entry);
    popover.set_autohide(false);
    popover.set_has_arrow(false);
    popover.set_position(gtk::PositionType::Bottom);
    popover.add_css_class("menu");
    popover.add_css_class("suggestions-popover");
    // A plain vertical Box (not a ListBox: exact natural height, no ListBoxRow
    // overhead/inflation) inside a ScrolledWindow that PROPAGATES the box's
    // natural height up to a cap. Few matches -> the scroll is exactly the box
    // height (no leftover); many -> it caps and scrolls. No manual min/max-content
    // height (that path hit a Gtk-CRITICAL); the close+reopen on each keystroke
    // gives a fresh surface so the popover never keeps a stale height.
    let sugg_list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sugg_list.set_valign(gtk::Align::Start);
    let sugg_scroll = gtk::ScrolledWindow::new();
    sugg_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    sugg_scroll.set_propagate_natural_height(true);
    sugg_scroll.set_max_content_height(240);
    sugg_scroll.set_min_content_width(320);
    sugg_scroll.set_child(Some(&sugg_list));
    popover.set_child(Some(&sugg_scroll));

    // Dismiss the popover when the entry loses focus (clicking away, minimizing,
    // switching windows) so it never gets stuck on screen.
    {
        let focus = gtk::EventControllerFocus::new();
        let popover = popover.clone();
        focus.connect_leave(move |_| popover.popdown());
        entry.add_controller(focus);
    }
    // focus-leave misses clicks on non-focusable areas (they don't move focus
    // off the entry). Also dismiss on any press in the window that isn't on the
    // entry. The popover is its own surface, so clicks on a suggestion don't
    // reach this gesture.
    if let Some(window) = state.window.borrow().clone() {
        let gesture = gtk::GestureClick::new();
        gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
        {
            let popover = popover.clone();
            let entry = entry.clone();
            gesture.connect_pressed(move |g, _, x, y| {
                if !popover.is_visible() {
                    return;
                }
                let Some(root) = g.widget() else { return };
                let on_entry = root.pick(x, y, gtk::PickFlags::DEFAULT).is_some_and(|w| {
                    w == *entry.upcast_ref::<gtk::Widget>() || w.is_ancestor(&entry)
                });
                if !on_entry {
                    popover.popdown();
                }
            });
        }
        window.add_controller(gesture);

        // Close it when the window is minimized or loses focus — a non-autohide
        // popover would otherwise stay floating (focus-leave doesn't reliably
        // fire on minimize on every compositor).
        let popover = popover.clone();
        window.connect_is_active_notify(move |w| {
            if !w.is_active() {
                popover.popdown();
            }
        });
    }

    // The last query actually searched — guards against the delayed
    // `search-changed` wiping freshly loaded results after a suggestion pick.
    let last_query: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

    // Shared search trigger.
    let trigger: Rc<dyn Fn()> = {
        let state = state.clone();
        let entry = entry.clone();
        let source = source.clone();
        let popover = popover.clone();
        let last_query = last_query.clone();
        Rc::new(move || {
            popover.popdown();
            let query = entry.text().to_string();
            // Empty search: tell the user instead of silently doing nothing.
            if query.trim().is_empty() {
                state.toast(&tr("Type something to search."));
                return;
            }
            *last_query.borrow_mut() = query.trim().to_string();
            // Auto-pick the source from the input, only at search time:
            //  - a URL while on YouTube / YouTube Music → switch to Direct Link;
            //  - plain text while on YouTube / YouTube Music → leave it;
            //  - plain text while on Direct Link → back to YouTube.
            let is_url = bigtube_core::validators::is_valid_url(query.trim());
            match source.selected() {
                2 if !is_url => source.set_selected(0), // Direct Link → YouTube
                0 | 1 if is_url => source.set_selected(2), // YT/YTMusic → Direct Link
                _ => {}
            }
            let src = match source.selected() {
                1 => "youtube_music",
                2 => "url",
                _ => "youtube",
            }
            .to_string();
            run_search(&state, query, src);
        })
    };

    // Expose "paste a URL and search" so the clipboard monitor can drive it.
    {
        let entry = entry.clone();
        let trigger = trigger.clone();
        let stack = state.stack.clone();
        *state.paste_and_search.borrow_mut() = Some(Rc::new(move |url: String| {
            stack.set_visible_child_name("search");
            entry.set_text(&url);
            trigger();
        }));
    }

    // Self-reference so a suggestion's delete button can rebuild (close+reopen)
    // the popover to resize it to the remaining matches.
    #[allow(clippy::type_complexity)]
    let rebuild_slot: Rc<RefCell<Option<Rc<dyn Fn(&str)>>>> = Rc::new(RefCell::new(None));

    // Cache of the last online-autocomplete fetch: (query, suggestions). Lets the
    // rebuild stay synchronous — it shows what's cached and kicks an async fetch
    // (which re-runs rebuild) only when the cache misses the current text.
    let online_cache: Rc<RefCell<(String, Vec<String>)>> =
        Rc::new(RefCell::new((String::new(), Vec::new())));

    // Rebuild the suggestion list for the current text.
    let rebuild: Rc<dyn Fn(&str)> = {
        let sugg_list = sugg_list.clone();
        let popover = popover.clone();
        let entry = entry.clone();
        let trigger = trigger.clone();
        let rebuild_slot = rebuild_slot.clone();
        let online_cache = online_cache.clone();
        Rc::new(move |text: &str| {
            // Close on every keystroke; we re-open (popup) below only when there
            // are matches. Re-opening yields a fresh surface sized to the current
            // content, so the popover never keeps a stale, stretched height — and
            // clearing the text simply leaves it closed.
            popover.popdown();
            while let Some(c) = sugg_list.first_child() {
                sugg_list.remove(&c);
            }
            let (enabled, online_enabled, max) = {
                let c = config::global().read().unwrap_or_else(|e| e.into_inner());
                (
                    c.get_bool("enable_suggestions"),
                    c.get_bool("online_suggestions"),
                    c.get_i64("max_suggestions").max(1) as usize,
                )
            };
            if !enabled || text.trim().is_empty() {
                popover.popdown();
                return;
            }

            // Group 1: local search-query history (with a per-item delete button).
            let history = bigtube_core::search_history::SearchHistory::new(search_history_path())
                .get_matches(text, max);
            // Group 2: known channels harvested from past results (opens the channel).
            let chans = channels().get_matches(text, max.min(6));
            // Group 3: online autocomplete completions (from cache; fetched async).
            let online: Vec<String> = if online_enabled && online_cache.borrow().0 == text {
                online_cache.borrow().1.clone()
            } else {
                Vec::new()
            };

            // Kick an async online fetch when the cache misses the current text;
            // when it returns (and the text is unchanged) it re-runs rebuild, which
            // then renders the online group.
            if online_enabled && online_cache.borrow().0 != text {
                let (otx, orx) = async_channel::bounded::<Vec<String>>(1);
                let q = text.to_string();
                let qmax = max.min(8);
                std::thread::spawn(move || {
                    let _ =
                        otx.send_blocking(bigtube_core::search::fetch_online_suggestions(&q, qmax));
                });
                let entry2 = entry.clone();
                let rebuild_slot2 = rebuild_slot.clone();
                let online_cache2 = online_cache.clone();
                let q2 = text.to_string();
                glib::spawn_future_local(async move {
                    if let Ok(res) = orx.recv().await {
                        // Only apply if the user is still on the same query.
                        if entry2.text() == q2 {
                            *online_cache2.borrow_mut() = (q2.clone(), res);
                            if let Some(rebuild) = rebuild_slot2.borrow().as_ref() {
                                rebuild(&q2);
                            }
                        }
                    }
                });
            }

            // Helper: append a simple (non-deletable) suggestion row and return its
            // pick button for click wiring.
            let add_row = |icon_name: &str, label: &str| -> gtk::Button {
                let rowbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                rowbox.add_css_class("suggestion-row");
                let pick = gtk::Button::new();
                pick.add_css_class("flat");
                pick.set_hexpand(true);
                pick.set_can_focus(false);
                pick.set_focus_on_click(false);
                let inner = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                let icon = gtk::Image::from_icon_name(icon_name);
                icon.add_css_class("dim-label");
                icon.set_pixel_size(14);
                let lbl = gtk::Label::new(Some(label));
                lbl.set_xalign(0.0);
                lbl.set_hexpand(true);
                lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
                inner.append(&icon);
                inner.append(&lbl);
                pick.set_child(Some(&inner));
                rowbox.append(&pick);
                sugg_list.append(&rowbox);
                pick
            };

            // History rows (with delete).
            for m in &history {
                let pick = add_row("bigtube-document-open-recent-symbolic", m);
                let rowbox = pick.parent().unwrap();
                let del = gtk::Button::from_icon_name("bigtube-window-close-symbolic");
                del.add_css_class("flat");
                del.set_valign(gtk::Align::Center);
                del.set_can_focus(false);
                del.set_focus_on_click(false);
                del.set_tooltip_text(Some(&tr("Remove from list")));
                rowbox.downcast_ref::<gtk::Box>().unwrap().append(&del);
                {
                    let entry = entry.clone();
                    let trigger = trigger.clone();
                    let q = m.clone();
                    pick.connect_clicked(move |_| {
                        entry.set_text(&q);
                        trigger();
                    });
                }
                {
                    let entry = entry.clone();
                    let rebuild_slot = rebuild_slot.clone();
                    let q = m.clone();
                    del.connect_clicked(move |_| {
                        bigtube_core::search_history::SearchHistory::new(search_history_path())
                            .remove_item(&q);
                        if let Some(rebuild) = rebuild_slot.borrow().as_ref() {
                            rebuild(&entry.text());
                        }
                    });
                }
            }

            // Channel rows: clicking opens the channel (its URL flows through the
            // Direct Link path, expanding into the channel's videos).
            for c in &chans {
                let pick = add_row("bigtube-channel-symbolic", &c.name);
                let entry = entry.clone();
                let trigger = trigger.clone();
                let url = c.url.clone();
                pick.connect_clicked(move |_| {
                    entry.set_text(&url);
                    trigger();
                });
            }

            // Online completion rows: dedup against history and the typed text.
            let mut seen: std::collections::HashSet<String> =
                history.iter().map(|s| s.to_lowercase()).collect();
            seen.insert(text.to_lowercase());
            let mut shown_online = 0;
            for s in &online {
                if !seen.insert(s.to_lowercase()) {
                    continue;
                }
                let pick = add_row("bigtube-system-search-symbolic", s);
                let entry = entry.clone();
                let trigger = trigger.clone();
                let q = s.clone();
                pick.connect_clicked(move |_| {
                    entry.set_text(&q);
                    trigger();
                });
                shown_online += 1;
            }

            if history.is_empty() && chans.is_empty() && shown_online == 0 {
                popover.popdown();
                return;
            }
            // Match the popover width to the search entry so labels aren't clipped;
            // the popover hugs the box height on its own (no scroll = no leftover).
            popover.set_size_request(entry.width().max(320), -1);
            popover.popup();
        })
    };
    *rebuild_slot.borrow_mut() = Some(rebuild.clone());

    // Typing refreshes suggestions only. Results are NOT cleared on every
    // keystroke — only when the field is fully emptied — so the previous results
    // stay visible until a new search replaces them. (The source is auto-picked
    // on Search, not while typing — see `trigger`.)
    {
        let state = state.clone();
        let rebuild = rebuild.clone();
        let last_query = last_query.clone();
        let filter_entry = filter_entry.clone();
        entry.connect_search_changed(move |e| {
            let text = e.text().to_string();
            // Clear results ONLY when all text is deleted (also closes the popover).
            if text.trim().is_empty() {
                state.search_store.remove_all();
                state.update_search_empty();
                // Clearing the search also clears any active result filter.
                if !filter_entry.text().is_empty() {
                    filter_entry.set_text("");
                }
                rebuild(&text); // rebuild("") -> popdown
                return;
            }
            if text.trim() == *last_query.borrow() {
                return; // results we just loaded for this query — keep them
            }
            rebuild(&text);
        });
    }

    {
        let trigger = trigger.clone();
        button.connect_clicked(move |_| trigger());
    }
    entry.connect_activate(move |_| trigger());

    page.upcast()
}

/// Extract a clean, catalog-translatable message from a search error. The
/// error's Display adds a prefix ("search error: …") that would stop tr() from
/// matching, so for the message-carrying variants we return the inner string
/// (which IS a catalog msgid) and let the toast translate it.
fn search_error_message(e: &bigtube_core::errors::BigTubeError) -> String {
    use bigtube_core::errors::BigTubeError::*;
    match e {
        Search(m) | Network(m) | Config(m) => m.clone(),
        BinaryNotFound(b) => format!("{} {}", tr("Command not found on PATH:"), b),
        other => other.to_string(),
    }
}

fn run_search(state: &Rc<AppState>, query: String, source: String) {
    let query = query.trim().to_string();
    if query.is_empty() {
        return;
    }
    state.search_store.remove_all();

    // Persist the query to search history (honouring the setting).
    let (save, show_playlists) = {
        let c = config::global().read().unwrap_or_else(|e| e.into_inner());
        (
            c.get_bool("save_search_history"),
            c.get_bool("show_playlists"),
        )
    };
    bigtube_core::search_history::SearchHistory::new(search_history_path()).add(&query, save);

    // Show the spinner page while the search runs.
    state.search_stack.set_visible_child_name("loading");

    // A direct link (incl. a playlist URL) is expanded into its videos by the
    // core, so suppress any playlist wrapper rows — a pasted playlist lists its
    // videos inline, never a "playlist menu" row. Mirror the core's own
    // direct-link routing (source "url" OR a pasted http/www string).
    let is_url_search = source == "url" || query.starts_with("http") || query.starts_with("www");
    // Kept for the "link has no media" prompt, since `query` is moved into the
    // worker thread below.
    let query_for_prompt = query.clone();
    let (tx, rx) =
        async_channel::bounded::<Result<Vec<bigtube_core::search::SearchResult>, String>>(1);
    std::thread::spawn(move || {
        let result = SearchEngine::new()
            .map_err(|e| search_error_message(&e))
            .and_then(|eng| {
                eng.search(&query, &source)
                    .map_err(|e| search_error_message(&e))
            });
        let _ = tx.send_blocking(result);
    });

    let state = state.clone();
    glib::spawn_future_local(async move {
        if let Ok(result) = rx.recv().await {
            match result {
                Ok(list) => {
                    let mode = state.select_mode.get();
                    for r in &list {
                        // Direct-link results are already expanded videos; drop any
                        // stray playlist wrapper so no "open playlist" row appears.
                        // Also honour the "Show Playlists" setting for keyword
                        // searches (default on).
                        if r.is_playlist && (is_url_search || !show_playlists) {
                            continue;
                        }
                        let obj = VideoObject::from_result(r);
                        obj.set_selection_mode(mode);
                        let st = state.clone();
                        obj.connect_is_selected_notify(move |_| st.refresh_selection_count());
                        state.search_store.append(&obj);
                    }
                    // Remember the channels behind these results so they can be
                    // suggested next time the user types.
                    channels().record_many(
                        list.iter()
                            .map(|r| (r.uploader.as_str(), r.uploader_url.as_str())),
                    );
                    state.update_search_empty();
                    state.refresh_selection_count();
                    // Nothing playable came back.
                    if state.search_store.n_items() == 0 {
                        if is_url_search && save {
                            // A pasted link with no media: offer to drop the junk
                            // query from the search history.
                            ask_remove_link_from_history(
                                &state,
                                &query_for_prompt,
                                tr("This link has no video or audio. Remove it from the search history?"),
                            );
                        } else {
                            state.toast(&tr("No results found!"));
                        }
                    }
                }
                Err(e) => {
                    state.update_search_empty();
                    // The core returns a known English message; translate it via
                    // the catalog (tr() returns the input unchanged if unknown).
                    if is_url_search && save {
                        let body = format!(
                            "{}\n\n{}",
                            tr("Couldn't get video or audio from this link."),
                            tr(&e)
                        );
                        ask_remove_link_from_history(&state, &query_for_prompt, body);
                    } else {
                        state.toast(&tr(&e));
                    }
                }
            }
        }
    });
}

/// A link search returned no playable media; ask whether to drop the query from
/// the search history (it was already saved before the search ran).
fn ask_remove_link_from_history(state: &Rc<AppState>, query: &str, body: String) {
    let Some(window) = state.window.borrow().clone() else {
        return;
    };
    let dialog =
        adw::MessageDialog::new(Some(&window), Some(&tr("No video or audio")), Some(&body));
    dialog.add_response("keep", &tr("Keep"));
    dialog.add_response("remove", &tr("Remove from history"));
    dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("keep"));
    dialog.set_close_response("keep");
    apply_theme_classes(&dialog);

    let query = query.to_string();
    dialog.connect_response(None, move |dlg, resp| {
        if resp == "remove" {
            bigtube_core::search_history::SearchHistory::new(search_history_path())
                .remove_item(&query);
        }
        dlg.close();
    });
    dialog.present();
}

/// Play `clicked`, seeding the player queue from the playable items of `store`
/// (so prev/next walk the list). Falls back to a one-item queue if `clicked`
/// isn't in the store (e.g. invoked from the playlist dialog).
fn play_from_store(state: &Rc<AppState>, store: &gio::ListStore, clicked: &VideoObject) {
    let Some(player) = state.player.borrow().clone() else {
        state.toast(&tr(
            "Playback unavailable — install the GStreamer gtk4 plugin",
        ));
        return;
    };
    let mut items = Vec::new();
    let mut start = None;
    for i in 0..store.n_items() {
        let Some(obj) = store.item(i).and_then(|o| o.downcast::<VideoObject>().ok()) else {
            continue;
        };
        if obj.is_playlist() {
            continue;
        }
        if obj.url() == clicked.url() {
            start = Some(items.len());
        }
        items.push(crate::player::QueueItem {
            url: obj.url(),
            title: obj.title(),
            artist: obj.uploader(),
            thumbnail: obj.thumbnail(),
            is_local: false,
            is_video: obj.is_video(),
        });
    }
    match start {
        Some(s) => player.play_queue(items, s),
        None => player.play(
            &clicked.url(),
            &clicked.title(),
            &clicked.uploader(),
            &clicked.thumbnail(),
        ),
    }
}
