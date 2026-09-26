use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use crate::domain::{Screen, Song};
use crate::renumberer;
use crate::replacer::resolve::ResolveJob;
use crate::ui::components::song_menu::{self, MenuAction};
use crate::ui::App;

/// A resolved paste whose enqueue is on hold until the user decides what to
/// do with the entries that already exist in the playlist.
pub struct PendingBatch {
    pub folder: PathBuf,
    pub items: Vec<(crate::replacer::resolve::Resolved, bool)>, // (item, is_duplicate)
    pub outcomes: Vec<crate::replacer::resolve::LineOutcome>,
}

fn norm(s: &str) -> String {
    s.trim().to_lowercase()
}

/// One flag per resolved item: true if the same (title, artist) already
/// exists in the folder or earlier in this same batch.
fn flag_duplicates(items: &[crate::replacer::resolve::Resolved], existing: &[Song]) -> Vec<bool> {
    let mut seen: std::collections::HashSet<(String, String)> = existing
        .iter()
        .map(|s| (norm(&s.title), norm(&s.artist)))
        .collect();
    items
        .iter()
        .map(|r| !seen.insert((norm(&r.title), norm(&r.artist))))
        .collect()
}

/// State for the Playlists hub. `order` is the working copy of the selected
/// folder's songs that reorder controls mutate; nothing touches disk until
/// **Apply order** is clicked.
#[derive(Default)]
pub struct PlaylistsUi {
    pub selected: Option<PathBuf>,
    pub new_name: String,
    pub paste_text: String,
    pub resolve: ResolveJob,
    pub last_outcomes: Vec<crate::replacer::resolve::LineOutcome>,
    /// Folder the in-flight resolve / download batch was started for. Captured
    /// on **Find & download** so switching playlists mid-batch can't redirect
    /// downloads or the final renumber to a different folder.
    pub batch_folder: Option<PathBuf>,
    /// ids queued for download by this screen; when none are still pending
    /// `batch_folder` is renumbered once and this is cleared.
    pub queued_ids: HashSet<i64>,
    pub order: Vec<Song>,
    pub order_version: u64,
    pub order_folder: Option<PathBuf>,
    pub move_target: usize,
    /// Folder list sorted Z→A instead of the default A→Z (case-insensitive).
    pub sort_desc: bool,
    /// Search field above the order list; filters rows with `song_matches`.
    pub filter: String,
    /// Song to scroll to and highlight on the next frame (set by "Go to
    /// playlist" / the location pill on search results). Cleared once the
    /// scroll happened; `highlight` keeps the row tinted until the folder
    /// changes or the user picks another song.
    pub reveal: Option<i64>,
    pub highlight: Option<i64>,
    /// `(library_version, folder, ids sorted by filename)` — refreshed only
    /// when the library version changes, so `is_dirty` doesn't filter the
    /// whole library every frame.
    pub on_disk_cache: Option<(u64, PathBuf, Vec<i64>)>,
    /// Resolved items waiting for the user's answer to "N already exist".
    pub pending_batch: Option<PendingBatch>,
}

pub fn draw(ui: &mut egui::Ui, app: &mut App) {
    ui.horizontal(|ui| {
        if ui.button("← Back").clicked() {
            app.navigate(Screen::AllSongs);
        }
        ui.heading("Playlists");
        ui.label(egui::RichText::new("every folder in the destination root is a playlist").weak());
    });
    ui.separator();

    poll_batch(ui.ctx(), app);
    draw_duplicate_prompt(ui.ctx(), app);

    let dest_root: PathBuf = app
        .settings
        .read()
        .scan
        .roots
        .first()
        .cloned()
        .map(PathBuf::from)
        .unwrap_or_default();
    if dest_root.as_os_str().is_empty() {
        ui.label("Set the Destination root in Settings → Library paths first.");
        return;
    }

    egui::SidePanel::left("playlists_folders")
        .resizable(true)
        .default_width(260.0)
        .show_inside(ui, |ui| draw_folder_list(ui, app, &dest_root));

    egui::CentralPanel::default().show_inside(ui, |ui| {
        let Some(folder) = app.playlists.selected.clone() else {
            ui.label("Select a playlist on the left, or create a new one.");
            return;
        };
        draw_editor(ui, app, &folder);
    });
}

fn draw_folder_list(ui: &mut egui::Ui, app: &mut App, dest_root: &PathBuf) {
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.playlists.new_name)
                .hint_text("New playlist name")
                .desired_width(160.0),
        );
        let name = app.playlists.new_name.trim().to_string();
        if ui.add_enabled(!name.is_empty(), egui::Button::new("＋ New")).clicked() {
            let folder = crate::replacer::playlist::dest_folder(dest_root, &name);
            match std::fs::create_dir_all(&folder) {
                Ok(()) => {
                    app.playlists.new_name.clear();
                    app.playlists.selected = Some(folder.clone());
                    if let Err(e) = app.library.refresh_folder(&folder) {
                        tracing::warn!("refresh new playlist failed: {e:#}");
                    }
                    app.cached_folders = None;
                    app.toast_info(format!("Created {}", folder.display()));
                }
                Err(e) => app.toast_error(format!("Could not create folder: {e}")),
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("Sort:");
        let label = if app.playlists.sort_desc { "Name Z→A" } else { "Name A→Z" };
        if ui
            .small_button(label)
            .on_hover_text("Toggle playlist name order")
            .clicked()
        {
            app.playlists.sort_desc = !app.playlists.sort_desc;
        }
    });
    ui.separator();

    let folders: Arc<Vec<PathBuf>> = app.library_folders();
    let mut sorted: Vec<(String, &PathBuf)> = folders
        .iter()
        .map(|f| {
            let name = f.file_name().and_then(|s| s.to_str()).unwrap_or("?").to_string();
            (name, f)
        })
        .collect();
    sorted.sort_by_cached_key(|(name, _)| name.to_lowercase());
    if app.playlists.sort_desc {
        sorted.reverse();
    }
    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        for (label, folder) in &sorted {
            let active = app.playlists.selected.as_ref() == Some(*folder);
            if ui.selectable_label(active, label).clicked() {
                app.playlists.selected = Some((*folder).clone());
            }
        }
        // A freshly created, still-empty folder has no songs and therefore
        // isn't in `library_folders()`; show it anyway so it can be filled.
        if let Some(sel) = app.playlists.selected.clone() {
            if !folders.iter().any(|f| f == &sel) {
                let label = sel.file_name().and_then(|s| s.to_str()).unwrap_or("?");
                let _ = ui.selectable_label(true, format!("{label} (empty)"));
            }
        }
    });
}

fn draw_editor(ui: &mut egui::Ui, app: &mut App, folder: &PathBuf) {
    ui.heading(folder.file_name().and_then(|s| s.to_str()).unwrap_or("?"));
    ui.label(egui::RichText::new(folder.display().to_string()).weak());
    ui.separator();

    draw_add_songs(ui, app, folder);
    ui.separator();
    draw_reorder(ui, app, folder);
}

fn draw_add_songs(ui: &mut egui::Ui, app: &mut App, folder: &PathBuf) {
    use crate::replacer::link_list::parse_lines;

    ui.collapsing("Add songs", |ui| {
        ui.label(
            "One per line: a YouTube video or playlist link, or a title like \
             \"Powfu death bed\". Titles are matched to the best audio-only result.",
        );
        ui.add(
            egui::TextEdit::multiline(&mut app.playlists.paste_text)
                .desired_rows(5)
                .desired_width(f32::INFINITY)
                .hint_text("https://youtu.be/…\nArtist song name\n…"),
        );

        let resolving = app.playlists.resolve.running.load(std::sync::atomic::Ordering::Relaxed);
        if resolving {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
        }
        let cookies_browser = {
            let s = app.settings.read().replacer.cookies_browser.trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        };

        ui.horizontal(|ui| {
            let lines = parse_lines(&app.playlists.paste_text);
            let batch_in_flight =
                !app.playlists.queued_ids.is_empty() || app.playlists.pending_batch.is_some();
            let can = !resolving
                && !batch_in_flight
                && !lines.is_empty()
                && crate::replacer::youtube::ytdlp_available();
            if ui
                .add_enabled(can, egui::Button::new(format!("Find & download {} line(s)", lines.len())))
                .on_hover_text(if batch_in_flight {
                    "Wait for the current batch to finish"
                } else {
                    "Resolves each line, then queues the downloads into this playlist"
                })
                .clicked()
            {
                app.playlists.last_outcomes.clear();
                app.playlists.batch_folder = Some(folder.clone());
                app.playlists.resolve.start(lines, cookies_browser.clone());
            }
            if resolving {
                ui.spinner();
                ui.label(egui::RichText::new("resolving…").weak());
            }
            if ui.button("Clear").clicked() {
                app.playlists.paste_text.clear();
                app.playlists.last_outcomes.clear();
            }
        });

        if app.playlists.batch_folder.as_ref() == Some(folder) {
            draw_batch_status(ui, app);
        }

        // Per-line outcome list (failures first so they are visible).
        if !app.playlists.last_outcomes.is_empty() {
            ui.separator();
            for o in &app.playlists.last_outcomes {
                match &o.result {
                    Ok(items) => {
                        ui.colored_label(
                            egui::Color32::from_rgb(120, 200, 120),
                            format!("✓ {} → {} track(s)", o.line.text(), items.len()),
                        );
                    }
                    Err(e) => {
                        ui.colored_label(
                            egui::Color32::from_rgb(220, 120, 120),
                            format!("✗ {} — {e}", o.line.text()),
                        )
                        .on_hover_text("Fix the line and run again; successful lines were already queued.");
                    }
                }
            }
        }
    });
}

/// Runs every frame the Playlists screen is shown, independent of which
/// folder is selected: collects a finished resolve into download requests
/// for `batch_folder`, and renumbers that folder once every queued download
/// has left the `Pending` state.
fn poll_batch(ctx: &egui::Context, app: &mut App) {
    use crate::replacer::download_worker::DownloadState;
    use crate::ui::screens::replacer::download_worker;

    // Resolver finished: classify the resolved items; the enqueue happens in
    // `enqueue_batch`, either right away or after the duplicate prompt.
    let finished = app.playlists.resolve.outcomes.lock().take();
    if let Some(outcomes) = finished {
        let Some(folder) = app.playlists.batch_folder.clone() else {
            return;
        };
        let items: Vec<crate::replacer::resolve::Resolved> = outcomes
            .iter()
            .filter_map(|o| o.result.as_ref().ok())
            .flat_map(|v| v.iter().cloned())
            .collect();
        let failed = outcomes.iter().filter(|o| o.result.is_err()).count();
        if failed > 0 {
            app.toast_warn(format!("{failed} line(s) could not be resolved — see list below"));
        }
        let existing = app.library.songs_in_folder(&folder);
        let flags = flag_duplicates(&items, &existing);
        let dup_count = flags.iter().filter(|d| **d).count();
        let batch = PendingBatch {
            folder,
            items: items.into_iter().zip(flags).collect(),
            outcomes,
        };
        if dup_count == 0 {
            enqueue_batch(app, batch, false);
        } else {
            app.playlists.pending_batch = Some(batch);
        }
    }

    // Batch bookkeeping: once nothing we queued is still pending, renumber
    // the folder so a partial failure leaves no gap (03 missing → 01,02,03).
    if !app.playlists.queued_ids.is_empty() {
        let Some(folder) = app.playlists.batch_folder.clone() else {
            app.playlists.queued_ids.clear();
            return;
        };
        let folder = &folder;
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
        let dl = download_worker(app.library.clone()).clone();
        let ids: Vec<i64> = app.playlists.queued_ids.iter().copied().collect();
        let (pending, done, _failed) = dl.read_states(|s| {
            let mut p = 0;
            let mut d = 0;
            let mut f = 0;
            for id in &ids {
                match s.get(id) {
                    Some(DownloadState::Pending) => p += 1,
                    Some(DownloadState::Done) => d += 1,
                    Some(DownloadState::Failed { .. }) => f += 1,
                    _ => {}
                }
            }
            (p, d, f)
        });
        if pending == 0 {
            let threshold = app.settings.read().renumber.threshold;
            match renumberer::renumber_folder(folder, threshold) {
                Ok(n) => {
                    if let Err(e) = app.library.refresh_folder(folder) {
                        tracing::warn!("refresh after batch failed: {e:#}");
                    }
                    app.toast_info(format!("{done} added · renumbered {n} file(s)"));
                }
                Err(e) => app.toast_error(format!("Renumber failed: {e}")),
            }
            app.playlists.queued_ids.clear();
            app.playlists.batch_folder = None;
        }
    }

}

/// Queue a resolved batch into the download worker. `skip_duplicates`
/// drops items flagged as already present.
fn enqueue_batch(app: &mut App, batch: PendingBatch, skip_duplicates: bool) {
    use crate::data::scanner::song_id_from_path;
    use crate::replacer::download_worker::DownloadRequest;
    use crate::replacer::playlist::sanitize_filename;
    use crate::ui::screens::replacer::download_worker;

    let cookies_browser = {
        let s = app.settings.read().replacer.cookies_browser.trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    };
    let folder = &batch.folder;
    let dl = download_worker(app.library.clone()).clone();
    let (mut next, pad) = renumberer::next_index(folder);
    let mut queued = 0usize;
    for (r, is_dup) in &batch.items {
        if skip_duplicates && *is_dup {
            continue;
        }
        let stem = format!("{next:0pad$} - {} - {}", r.title, r.artist);
        let dest = folder.join(format!("{}.mp3", sanitize_filename(&stem)));
        let id = song_id_from_path(&dest);
        dl.enqueue(DownloadRequest {
            song_id: id,
            source_path: dest.clone(),
            dest_path: dest,
            video_url: r.video_url.clone(),
            cookies_browser: cookies_browser.clone(),
        });
        app.playlists.queued_ids.insert(id);
        next += 1;
        queued += 1;
    }
    if queued > 0 {
        app.playlists.last_outcomes = batch.outcomes;
        app.toast_info(format!("Queued {queued} download(s) into {}", folder.display()));
    } else {
        // Nothing was queued, so green "✓ … → N track(s)" lines would lie;
        // keep only the failures worth fixing.
        app.playlists.last_outcomes = batch.outcomes.into_iter().filter(|o| o.result.is_err()).collect();
        app.playlists.batch_folder = None;
    }
}

/// Modal shown when a paste resolved to songs already in the playlist.
fn draw_duplicate_prompt(ctx: &egui::Context, app: &mut App) {
    let Some(batch) = app.playlists.pending_batch.as_ref() else {
        return;
    };
    let dups: Vec<String> = batch
        .items
        .iter()
        .filter(|(_, d)| *d)
        .map(|(r, _)| format!("{} — {}", r.title, r.artist))
        .collect();
    let total = batch.items.len();
    let folder_name = batch
        .folder
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("?")
        .to_string();

    let mut decision: Option<Option<bool>> = None; // Some(Some(skip)) or Some(None)=cancel
    egui::Window::new("Already in playlist")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(format!(
                "{} of {total} song(s) already exist in \"{folder_name}\":",
                dups.len()
            ));
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for d in &dups {
                    ui.label(format!("• {d}"));
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Skip duplicates").clicked() {
                    decision = Some(Some(true));
                }
                if ui.button("Download anyway").clicked() {
                    decision = Some(Some(false));
                }
                if ui.button("Cancel").clicked() {
                    decision = Some(None);
                }
            });
        });

    match decision {
        Some(Some(skip)) => {
            let batch = app.playlists.pending_batch.take().unwrap();
            enqueue_batch(app, batch, skip);
        }
        Some(None) => {
            let batch = app.playlists.pending_batch.take().unwrap();
            app.playlists.last_outcomes =
                batch.outcomes.into_iter().filter(|o| o.result.is_err()).collect();
            app.playlists.batch_folder = None;
            app.toast_info("Nothing queued");
        }
        None => {}
    }
}

/// Pending / done / failed counts for the current batch plus the shared
/// worker's pause toggle. Shown under the folder the batch belongs to.
fn draw_batch_status(ui: &mut egui::Ui, app: &mut App) {
    use crate::replacer::download_worker::DownloadState;
    use crate::ui::screens::replacer::download_worker;

    if app.playlists.queued_ids.is_empty() {
        return;
    }
    let dl = download_worker(app.library.clone()).clone();
    let ids: Vec<i64> = app.playlists.queued_ids.iter().copied().collect();
    let (pending, done, failed) = dl.read_states(|s| {
        let mut p = 0;
        let mut d = 0;
        let mut f = 0;
        for id in &ids {
            match s.get(id) {
                Some(DownloadState::Pending) => p += 1,
                Some(DownloadState::Done) => d += 1,
                Some(DownloadState::Failed { .. }) => f += 1,
                _ => {}
            }
        }
        (p, d, f)
    });
    ui.horizontal(|ui| {
        ui.label(format!("Downloading: {pending} pending, {done} done, {failed} failed"));
        let paused = dl.is_paused();
        if ui
            .button(if paused { "▶ Resume" } else { "⏸ Pause" })
            .on_hover_text("Pauses the shared download worker (also used by Replacer and Playlist)")
            .clicked()
        {
            dl.set_paused(!paused);
        }
    });
}

fn draw_reorder(ui: &mut egui::Ui, app: &mut App, folder: &PathBuf) {
    let version = app.library.version();
    let folder_changed = app.playlists.order_folder.as_ref() != Some(folder);
    if folder_changed {
        app.playlists.order = on_disk_songs(app, folder);
        app.playlists.order_version = version;
        app.playlists.order_folder = Some(folder.clone());
        app.playlists.move_target = 1;
        app.playlists.filter.clear();
        if app.playlists.reveal.is_none() {
            app.playlists.highlight = None;
        }
    } else if app.playlists.order_version != version {
        // Disk changed (download landed, delete, external edit). If the user
        // had no edits, take the disk order wholesale: parallel downloads land
        // out of order and merging them would leave a spurious "dirty" list.
        // With edits pending, merge so they survive and new songs still show.
        let was_clean = matches!(
            &app.playlists.on_disk_cache,
            Some((v, f, ids))
                if *v == app.playlists.order_version
                    && f == folder
                    && ids.iter().copied().eq(app.playlists.order.iter().map(|s| s.id))
        );
        let on_disk = on_disk_songs(app, folder);
        app.playlists.order = if was_clean {
            on_disk
        } else {
            merge_order(std::mem::take(&mut app.playlists.order), &on_disk)
        };
        app.playlists.order_version = version;
    }
    let n = app.playlists.order.len();
    if n == 0 {
        ui.label("This playlist is empty. Add songs above.");
        return;
    }
    let dirty = is_dirty(app, folder);
    let batch_here =
        !app.playlists.queued_ids.is_empty() && app.playlists.batch_folder.as_ref() == Some(folder);

    // Wrapped, not `horizontal`: a row wider than the panel makes egui widen
    // the whole panel, which pushed every song row's buttons (✕, Last,
    // First) past the visible edge.
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("Order").strong());
        ui.label(
            egui::RichText::new("drag rows, or use the buttons; nothing is renamed until you apply")
                .weak(),
        );
        if ui
            .add_enabled(dirty && !batch_here, egui::Button::new("Apply order (rename files)"))
            .on_hover_text(if batch_here {
                "Downloads into this playlist are still running"
            } else {
                "Rewrites the NN - prefixes on disk to match this order"
            })
            .clicked()
        {
            apply_order(app, folder);
        }
        if ui.add_enabled(dirty, egui::Button::new("Revert")).clicked() {
            app.playlists.order_folder = None; // forces reload next frame
        }
    });

    let mut play_from: Option<usize> = None;
    let mut play_shuffled = false;
    let mut toggle_pause = false;
    let state = app.playback.state_snapshot();
    let current_id = state.current_song.as_ref().map(|s| s.id);
    let is_playing = state.is_playing;

    // Search within this playlist. While a filter is active the rows keep
    // their real position numbers but drag-and-drop is ignored: dropping
    // between two visible rows says nothing about the hidden ones.
    ui.horizontal_wrapped(|ui| {
        if crate::ui::widgets::primary_button_sm(ui, true, "▶ Play all")
            .on_hover_text("Play this playlist from the first song, in the order shown")
            .clicked()
        {
            play_from = Some(0);
        }
        if crate::ui::widgets::secondary_button_sm(ui, true, "⤮ Shuffle")
            .on_hover_text("Play this playlist in a random order")
            .clicked()
        {
            play_shuffled = true;
        }
        ui.add_space(crate::ui::theme::SPACE_3);
        crate::ui::widgets::search_input(ui, &mut app.playlists.filter, "Search this playlist", 220.0);
        if !app.playlists.filter.is_empty() {
            let shown = app
                .playlists
                .order
                .iter()
                .filter(|s| crate::ui::app::song_matches(s, &app.playlists.filter))
                .count();
            crate::ui::widgets::caption(ui, format!("{shown} of {n}"));
            if crate::ui::widgets::icon_button(ui, "\u{2715}", 22.0, crate::ui::widgets::p(ui).ink_3)
                .on_hover_text("Clear")
                .clicked()
            {
                app.playlists.filter.clear();
            }
        }
    });
    let filtering = !app.playlists.filter.is_empty();
    let visible: Vec<usize> = (0..n)
        .filter(|&i| !filtering || crate::ui::app::song_matches(&app.playlists.order[i], &app.playlists.filter))
        .collect();

    let row_h = ui.text_style_height(&egui::TextStyle::Body) + 8.0;
    let mut mv: Option<(usize, usize)> = None; // (from, to)
    let mut drop_at: Option<(usize, usize)> = None;
    let mut delete_id: Option<i64> = None;
    let can_delete = !dirty && !batch_here;
    let mut menu: Option<(MenuAction, usize)> = None;

    // Snapshot so the row closures don't hold a borrow on `app`.
    let rows: Vec<Song> = app.playlists.order.clone();

    // Leave room below the list for the "Move track #" row; otherwise the
    // scroll area takes every remaining pixel and that row is pushed out of
    // the window.
    let bottom_reserve = ui.spacing().interact_size.y + ui.spacing().item_spacing.y * 3.0;
    let list_h = (ui.available_height() - bottom_reserve).max(row_h * 3.0);
    // Rows are `row_h` plus the 2px frame margin and the item spacing.
    let row_stride = row_h + 4.0 + ui.spacing().item_spacing.y;
    let mut scroll = egui::ScrollArea::vertical().auto_shrink([false; 2]).max_height(list_h);
    if let Some(id) = app.playlists.reveal.take() {
        if let Some(k) = visible.iter().position(|&i| rows[i].id == id) {
            let offset = (k as f32 * row_stride - list_h / 2.0 + row_stride / 2.0).max(0.0);
            scroll = scroll.vertical_scroll_offset(offset);
            app.playlists.highlight = Some(id);
        }
    }
    let highlight = app.playlists.highlight;
    let pal = crate::ui::widgets::p(ui);
    let accent_soft = pal.accent_soft;
    scroll.show_rows(ui, row_h, visible.len(), |ui, range| {
            for k in range {
                let i = visible[k];
                let song = &rows[i];
                let (song_id, title, artist) = (&song.id, &song.title, &song.artist);
                let id = egui::Id::new(("pl_row", folder, *song_id));
                let is_current = current_id == Some(*song_id);
                let mut frame = egui::Frame::none().inner_margin(2.0);
                if highlight == Some(*song_id) || is_current {
                    frame = frame.fill(accent_soft).rounding(crate::ui::theme::RADIUS_SM);
                }
                let (_, dropped) = ui.dnd_drop_zone::<usize, ()>(frame, |ui| {
                    ui.horizontal(|ui| {
                        ui.dnd_drag_source(id, i, |ui| {
                            ui.label(egui::RichText::new("☰").weak());
                            ui.label(format!("{:02}", i + 1));
                        });
                        // ▶ plays the playlist from this song; on the song
                        // that is already loaded it toggles play / pause.
                        let (glyph, hover) = if is_current && is_playing {
                            ("⏸", "Pause")
                        } else if is_current {
                            ("▶", "Resume")
                        } else {
                            ("▶", "Play from here")
                        };
                        let glyph_c = if is_current { pal.accent } else { pal.ink_2 };
                        if crate::ui::widgets::icon_button(ui, glyph, 22.0, glyph_c)
                            .on_hover_text(hover)
                            .clicked()
                        {
                            if is_current {
                                toggle_pause = true;
                            } else {
                                play_from = Some(i);
                            }
                        }
                        let title_w = (ui.available_width() - 340.0).max(40.0);
                        let title_text = if is_current {
                            egui::RichText::new(title).color(pal.accent_soft_ink).strong()
                        } else {
                            egui::RichText::new(title)
                        };
                        let r = ui
                            .allocate_ui_with_layout(
                                egui::vec2(title_w, row_h),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_min_width(title_w);
                                    ui.add(
                                        egui::Label::new(title_text)
                                            .truncate()
                                            .sense(egui::Sense::click()),
                                    )
                                },
                            )
                            .inner
                            .on_hover_text("Double-click to play, right-click for more");
                        if r.double_clicked() {
                            play_from = Some(i);
                        }
                        if let Some(a) = song_menu::show(&r, song, can_delete) {
                            menu = Some((a, i));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add_enabled(can_delete, egui::Button::new("✕").small())
                                .on_hover_text(if can_delete {
                                    "Delete this file from the playlist folder and renumber"
                                } else if dirty {
                                    "Apply or revert the order first"
                                } else {
                                    "Downloads into this playlist are still running"
                                })
                                .clicked()
                            {
                                delete_id = Some(*song_id);
                            }
                            if ui.small_button("⇲ Last").clicked() {
                                mv = Some((i, n - 1));
                            }
                            if ui.small_button("⇱ First").clicked() {
                                mv = Some((i, 0));
                            }
                            if ui.add_enabled(i + 1 < n, egui::Button::new("▼").small()).clicked() {
                                mv = Some((i, i + 1));
                            }
                            if ui.add_enabled(i > 0, egui::Button::new("▲").small()).clicked() {
                                mv = Some((i, i - 1));
                            }
                            ui.label(
                                egui::RichText::new(song.formatted_duration())
                                    .font(crate::ui::theme::mono(crate::ui::theme::TEXT_CAPTION))
                                    .color(pal.ink_2),
                            );
                            let artist_w = ui.available_width().max(20.0);
                            ui.add_sized(
                                [artist_w, row_h],
                                egui::Label::new(egui::RichText::new(artist).weak()).truncate(),
                            );
                        });
                    });
                });
                if let Some(from) = dropped {
                    if !filtering {
                        drop_at = Some((*from, i));
                    }
                }
            }
        });

    if let Some((a, i)) = menu {
        song_menu::perform(app, a, &rows[i], &rows);
    }
    if toggle_pause {
        app.playback.play_pause();
    }
    if let Some(i) = play_from {
        app.playback.play_songs(rows.clone(), i, None);
    }
    if play_shuffled {
        app.playback.play_songs(shuffled(rows.clone()), 0, None);
    }

    ui.horizontal(|ui| {
        ui.label("Move track #");
        let mut from = app.playlists.move_target.clamp(1, n);
        ui.add(egui::DragValue::new(&mut from).range(1..=n));
        app.playlists.move_target = from;
        ui.label("to position");
        let to_id = ui.make_persistent_id("pl_move_to");
        let mut to: usize = ui.memory(|m| m.data.get_temp(to_id)).unwrap_or(1);
        ui.add(egui::DragValue::new(&mut to).range(1..=n));
        ui.memory_mut(|m| m.data.insert_temp(to_id, to));
        if ui.button("Move").clicked() {
            mv = Some((from - 1, to - 1));
        }
    });

    if let Some((from, to)) = drop_at.or(mv) {
        if from != to && from < n && to < n {
            let s = app.playlists.order.remove(from);
            app.playlists.order.insert(to, s);
        }
    }

    if let Some(id) = delete_id {
        crate::ui::screens::library::handle_delete(app, id);
        // handle_delete renumbers + refreshes; ids changed, so reload from disk.
        app.playlists.order_folder = None;
    }
}

/// Fisher-Yates with a time-seeded xorshift: good enough for picking a play
/// order, and avoids a `rand` dependency for one call site.
fn shuffled(mut songs: Vec<Song>) -> Vec<Song> {
    let mut x = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
        | 1;
    for i in (1..songs.len()).rev() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let j = (x % (i as u64 + 1)) as usize;
        songs.swap(i, j);
    }
    songs
}

fn is_dirty(app: &mut App, folder: &PathBuf) -> bool {
    if app.playlists.order_folder.as_ref() != Some(folder) {
        return false;
    }
    let version = app.library.version();
    let stale = match &app.playlists.on_disk_cache {
        Some((v, f, _)) => *v != version || f != folder,
        None => true,
    };
    if stale {
        let ids: Vec<i64> = on_disk_songs(app, folder).iter().map(|s| s.id).collect();
        app.playlists.on_disk_cache = Some((version, folder.clone(), ids));
    }
    let ids = &app.playlists.on_disk_cache.as_ref().unwrap().2;
    ids.iter().copied().ne(app.playlists.order.iter().map(|s| s.id))
}

/// Reconcile the user's working order with what is on disk: songs that
/// vanished are dropped, songs that appeared are appended (in filename
/// order), everything else keeps its current relative position.
fn merge_order(current: Vec<Song>, on_disk: &[Song]) -> Vec<Song> {
    let live: std::collections::HashSet<i64> = on_disk.iter().map(|s| s.id).collect();
    let mut merged: Vec<Song> = current.into_iter().filter(|s| live.contains(&s.id)).collect();
    let have: std::collections::HashSet<i64> = merged.iter().map(|s| s.id).collect();
    let mut new: Vec<&Song> = on_disk.iter().filter(|s| !have.contains(&s.id)).collect();
    new.sort_by(|a, b| a.path.file_name().cmp(&b.path.file_name()));
    merged.extend(new.into_iter().cloned());
    merged
}

fn on_disk_songs(app: &App, folder: &PathBuf) -> Vec<Song> {
    let mut v = app.library.songs_in_folder(folder);
    v.sort_by(|a, b| a.path.file_name().cmp(&b.path.file_name()));
    v
}

fn apply_order(app: &mut App, folder: &PathBuf) {
    let ordered: Vec<PathBuf> = app.playlists.order.iter().map(|s| s.path.clone()).collect();
    match renumberer::plan_order(folder, &ordered).and_then(|p| renumberer::apply(&p)) {
        Ok(n) => {
            if let Err(e) = app.library.refresh_folder(folder) {
                tracing::warn!("refresh after reorder failed: {e:#}");
            }
            app.playlists.order_folder = None; // reload from disk
            app.toast_info(format!("Renamed {n} file(s)"));
        }
        Err(e) => app.toast_error(format!("Reorder failed: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shuffled_keeps_every_song_once() {
        let songs: Vec<Song> = (0..50).map(|i| song(i, &format!("{i:02} - s.mp3"))).collect();
        let mut ids: Vec<i64> = shuffled(songs).iter().map(|s| s.id).collect();
        ids.sort();
        assert_eq!(ids, (0..50).collect::<Vec<i64>>());
    }
    use std::time::Duration;

    fn song(id: i64, name: &str) -> Song {
        Song {
            id,
            title: name.into(),
            artist: String::new(),
            album: String::new(),
            album_artist: String::new(),
            duration: Duration::ZERO,
            year: None,
            genre: None,
            composer: None,
            track_no: None,
            path: PathBuf::from(format!("C:/pl/{name}.mp3")),
            has_embedded_art: false,
        }
    }

    #[test]
    fn merge_keeps_user_order_drops_missing_appends_new() {
        let current = vec![song(3, "c"), song(1, "a"), song(2, "b")];
        let on_disk = vec![song(1, "a"), song(2, "b"), song(4, "d")];
        let merged = merge_order(current, &on_disk);
        let ids: Vec<i64> = merged.iter().map(|s| s.id).collect();
        assert_eq!(ids, vec![1, 2, 4]);
    }

    #[test]
    fn merge_with_no_changes_is_identity() {
        let current = vec![song(2, "b"), song(1, "a")];
        let on_disk = vec![song(1, "a"), song(2, "b")];
        let ids: Vec<i64> = merge_order(current, &on_disk).iter().map(|s| s.id).collect();
        assert_eq!(ids, vec![2, 1]);
    }

    #[test]
    fn merge_appends_several_new_songs_in_filename_order() {
        let current = vec![song(1, "01 - a")];
        let on_disk = vec![song(3, "03 - c"), song(1, "01 - a"), song(2, "02 - b")];
        let ids: Vec<i64> = merge_order(current, &on_disk).iter().map(|s| s.id).collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    use crate::replacer::resolve::Resolved;

    fn resolved(title: &str, artist: &str) -> Resolved {
        Resolved { title: title.into(), artist: artist.into(), video_url: "u".into() }
    }

    #[test]
    fn flags_case_insensitive_matches_against_folder_and_within_batch() {
        let mut existing = song(1, "01 - death bed - Powfu");
        existing.title = "death bed".into();
        existing.artist = "Powfu".into();
        let items = vec![
            resolved("Death Bed", "powfu"), // in folder
            resolved("New One", "X"),       // fresh
            resolved("new one", "x"),       // dup of previous line
        ];
        assert_eq!(flag_duplicates(&items, &[existing]), vec![true, false, true]);
    }
}
