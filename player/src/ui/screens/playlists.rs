use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use crate::domain::{Screen, Song};
use crate::renumberer;
use crate::replacer::resolve::ResolveJob;
use crate::ui::App;

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
    /// ids queued for download by this screen; when none are still pending
    /// the folder is renumbered once and this is cleared.
    pub queued_ids: HashSet<i64>,
    pub order: Vec<Song>,
    pub order_version: u64,
    pub order_folder: Option<PathBuf>,
    pub move_target: usize,
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
    ui.separator();

    let folders: Arc<Vec<PathBuf>> = app.library_folders();
    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        for folder in folders.iter() {
            let label = folder
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_string();
            let active = app.playlists.selected.as_ref() == Some(folder);
            if ui.selectable_label(active, label).clicked() {
                app.playlists.selected = Some(folder.clone());
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
    use crate::data::scanner::song_id_from_path;
    use crate::replacer::download_worker::{DownloadRequest, DownloadState};
    use crate::replacer::link_list::parse_lines;
    use crate::replacer::playlist::sanitize_filename;
    use crate::ui::screens::replacer::download_worker;

    ui.collapsing("Add songs", |ui| {
        ui.label(
            "One per line: a YouTube video or playlist link, or a title like \n             \"Powfu death bed\". Titles are matched to the best audio-only result.",
        );
        ui.add(
            egui::TextEdit::multiline(&mut app.playlists.paste_text)
                .desired_rows(5)
                .desired_width(f32::INFINITY)
                .hint_text("https://youtu.be/…
Artist song name
…"),
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
            let can = !resolving && !lines.is_empty() && crate::replacer::youtube::ytdlp_available();
            if ui
                .add_enabled(can, egui::Button::new(format!("Find & download {} line(s)", lines.len())))
                .clicked()
            {
                app.playlists.last_outcomes.clear();
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

        // Resolver finished: queue downloads for every resolved item.
        let finished = app.playlists.resolve.outcomes.lock().take();
        if let Some(outcomes) = finished {
            let dl = download_worker(app.library.clone()).clone();
            let (mut next, pad) = renumberer::next_index(folder);
            let mut queued = 0usize;
            let mut failed = 0usize;
            for o in &outcomes {
                match &o.result {
                    Ok(items) => {
                        for r in items {
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
                    }
                    Err(_) => failed += 1,
                }
            }
            app.playlists.last_outcomes = outcomes;
            if queued > 0 {
                app.toast_info(format!("Queued {queued} download(s) into {}", folder.display()));
            }
            if failed > 0 {
                app.toast_warn(format!("{failed} line(s) could not be resolved — see list below"));
            }
        }

        // Batch bookkeeping: once nothing we queued is still pending, renumber
        // the folder so a partial failure leaves no gap (03 missing → 01,02,03).
        if !app.playlists.queued_ids.is_empty() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
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
                if ui.button(if paused { "▶ Resume" } else { "⏸ Pause" }).clicked() {
                    dl.set_paused(!paused);
                }
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
            }
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

fn draw_reorder(ui: &mut egui::Ui, _app: &mut App, _folder: &PathBuf) {
    ui.label("(reorder comes in the next task)");
}
