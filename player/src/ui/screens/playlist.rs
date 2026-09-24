use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

use crate::data::scanner::song_id_from_path;
use crate::domain::Screen;
use crate::replacer::download::ffmpeg_available;
use crate::replacer::download_worker::{DownloadRequest, DownloadState};
use crate::replacer::playlist::{self, PlannedTrack, Playlist};
use crate::replacer::youtube::ytdlp_available;
use crate::ui::screens::replacer::download_worker;
use crate::ui::App;

/// Per-screen state for the Playlist page. The fetch runs on a background
/// thread and hands its result back through `fetched`; the UI polls it once
/// per frame while `fetching` is set.
#[derive(Default)]
pub struct PlaylistUi {
    pub url: String,
    pub folder_name: String,
    pub plan: Vec<PlannedTrack>,
    pub playlist_title: String,
    pub fetching: Arc<AtomicBool>,
    pub fetched: Arc<Mutex<Option<anyhow::Result<Playlist>>>>,
    /// `(folder, file names present in it)` — recomputed only when the
    /// folder changes or a download lands, never per frame.
    pub on_disk: Option<(PathBuf, std::collections::HashSet<String>, usize)>,
}

fn on_disk_set(app: &mut App, folder: &PathBuf, done_count: usize) -> std::collections::HashSet<String> {
    let stale = match &app.playlist.on_disk {
        Some((f, _, d)) => f != folder || *d != done_count,
        None => true,
    };
    if stale {
        let mut set = std::collections::HashSet::new();
        if let Ok(rd) = std::fs::read_dir(folder) {
            for e in rd.flatten() {
                set.insert(e.file_name().to_string_lossy().to_string());
            }
        }
        app.playlist.on_disk = Some((folder.clone(), set, done_count));
    }
    app.playlist.on_disk.as_ref().unwrap().1.clone()
}

pub fn draw(ui: &mut egui::Ui, app: &mut App) {
    ui.horizontal(|ui| {
        if ui.button("← Back").clicked() {
            app.navigate(Screen::AllSongs);
        }
        ui.heading("Download a YouTube playlist");
    });
    ui.label(
        "Paste a playlist link. Every track is downloaded as mp3 into a new folder under the \
         destination root, named \"NN - Title - Artist.mp3\" in playlist order.",
    );

    let yt_ok = ytdlp_available();
    let ff_ok = ffmpeg_available();
    let (col, msg) = match (yt_ok, ff_ok) {
        (true, true) => (egui::Color32::from_rgb(120, 200, 120), "yt-dlp + ffmpeg ready"),
        (false, _) => (
            egui::Color32::from_rgb(220, 80, 80),
            "yt-dlp missing — `winget install yt-dlp`",
        ),
        (_, false) => (
            egui::Color32::from_rgb(220, 160, 80),
            "ffmpeg missing — `winget install Gyan.FFmpeg` (downloads will fail)",
        ),
    };
    ui.colored_label(col, msg);
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
    let cookies_browser = {
        let s = app.settings.read().replacer.cookies_browser.trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    };

    // Collect a finished fetch, if any.
    let fetched = app.playlist.fetched.lock().take();
    if let Some(result) = fetched {
        match result {
            Ok(pl) => {
                app.playlist.plan = playlist::plan_entries(&pl);
                app.playlist.playlist_title = pl.title.clone();
                if app.playlist.folder_name.trim().is_empty() {
                    app.playlist.folder_name = pl.title.clone();
                }
                let n = app.playlist.plan.len();
                if n == 0 {
                    app.toast_warn("Playlist has no downloadable entries");
                } else {
                    app.toast_info(format!("Found {n} tracks in \"{}\"", pl.title));
                }
            }
            Err(e) => app.toast_error(format!("Playlist fetch failed: {e:#}")),
        }
    }
    let fetching = app.playlist.fetching.load(Ordering::Relaxed);
    if fetching {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
    }

    ui.horizontal(|ui| {
        ui.label("Playlist URL:");
        ui.add(
            egui::TextEdit::singleline(&mut app.playlist.url)
                .desired_width(420.0)
                .hint_text("https://www.youtube.com/playlist?list=…"),
        );
        let can_fetch = yt_ok && !fetching && !app.playlist.url.trim().is_empty();
        if ui
            .add_enabled(can_fetch, egui::Button::new("Fetch"))
            .on_hover_text("Lists the playlist via yt-dlp without downloading anything")
            .clicked()
        {
            app.playlist.plan.clear();
            app.playlist.folder_name.clear();
            spawn_fetch(
                app.playlist.url.clone(),
                cookies_browser.clone(),
                app.playlist.fetching.clone(),
                app.playlist.fetched.clone(),
            );
        }
        if fetching {
            ui.spinner();
            ui.label(egui::RichText::new("fetching…").weak());
        }
    });

    if app.playlist.plan.is_empty() {
        return;
    }

    ui.horizontal(|ui| {
        ui.label("Folder name:");
        ui.add(egui::TextEdit::singleline(&mut app.playlist.folder_name).desired_width(300.0));
        let folder = playlist::dest_folder(&dest_root, &app.playlist.folder_name);
        ui.label(egui::RichText::new(format!("→ {}", folder.display())).weak());
    });
    ui.separator();

    let folder = playlist::dest_folder(&dest_root, &app.playlist.folder_name);
    let dl = download_worker(app.library.clone()).clone();
    let done_count = dl.read_states(|s| s.values().filter(|v| matches!(v, DownloadState::Done)).count());
    let on_disk = on_disk_set(app, &folder, done_count);

    // One pass over the plan: per-row state for display plus the "to start" set.
    let mut rows: Vec<(usize, String, PathBuf, String, RowState)> = Vec::with_capacity(app.playlist.plan.len());
    dl.read_states(|states| {
        for t in &app.playlist.plan {
            let dest = folder.join(&t.file_name);
            let id = song_id_from_path(&dest);
            let st = match states.get(&id) {
                Some(DownloadState::Pending) => RowState::Pending,
                Some(DownloadState::Done) => RowState::Done,
                Some(DownloadState::Failed { error }) => RowState::Failed(error.clone()),
                None | Some(DownloadState::Idle) => {
                    if on_disk.contains(&t.file_name) {
                        RowState::Exists
                    } else {
                        RowState::Ready
                    }
                }
            };
            rows.push((t.index, t.file_name.clone(), dest, t.video_url.clone(), st));
        }
    });
    let ready: Vec<&(usize, String, PathBuf, String, RowState)> =
        rows.iter().filter(|r| matches!(r.4, RowState::Ready)).collect();
    let pending = rows.iter().filter(|r| matches!(r.4, RowState::Pending)).count();
    let done = rows.iter().filter(|r| matches!(r.4, RowState::Done)).count();
    let failed = rows.iter().filter(|r| matches!(r.4, RowState::Failed(_))).count();
    let exists = rows.iter().filter(|r| matches!(r.4, RowState::Exists)).count();
    if pending > 0 {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
    }

    ui.horizontal(|ui| {
        let n = ready.len();
        let folder_ok = !app.playlist.folder_name.trim().is_empty();
        if ui
            .add_enabled(n > 0 && folder_ok && ff_ok, egui::Button::new(format!("Download {n} tracks")))
            .on_hover_text("Queues every track not already on disk into the download worker")
            .clicked()
        {
            for (_, _, dest, url, _) in &ready {
                dl.enqueue(DownloadRequest {
                    song_id: song_id_from_path(dest),
                    source_path: dest.clone(),
                    dest_path: dest.clone(),
                    video_url: url.clone(),
                    cookies_browser: cookies_browser.clone(),
                });
            }
            app.toast_info(format!("Queued {n} downloads"));
        }
        let d_paused = dl.is_paused();
        let d_pending = dl.pending_count();
        let d_label = if d_paused {
            format!("▶ Resume ({d_pending} queued)")
        } else {
            format!("⏸ Pause ({d_pending} queued)")
        };
        if ui
            .add_enabled(d_pending > 0 || d_paused, egui::Button::new(d_label))
            .clicked()
        {
            dl.set_paused(!d_paused);
        }
        ui.separator();
        ui.label(format!("{} tracks", rows.len()));
        ui.separator();
        ui.colored_label(egui::Color32::from_rgb(180, 180, 220), format!("Pending: {pending}"));
        ui.separator();
        ui.colored_label(egui::Color32::from_rgb(120, 200, 120), format!("Done: {done}"));
        ui.separator();
        ui.label(format!("Already on disk: {exists}"));
        ui.separator();
        ui.colored_label(egui::Color32::from_rgb(220, 120, 120), format!("Failed: {failed}"));
    });
    ui.separator();

    let row_h = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show_rows(ui, row_h, rows.len(), |ui, range| {
            for (_, name, _, _, st) in &rows[range] {
                ui.horizontal(|ui| {
                    let (col, tag) = match st {
                        RowState::Ready => (ui.visuals().weak_text_color(), "ready"),
                        RowState::Exists => (ui.visuals().weak_text_color(), "on disk"),
                        RowState::Pending => (egui::Color32::from_rgb(180, 180, 220), "pending"),
                        RowState::Done => (egui::Color32::from_rgb(120, 200, 120), "done"),
                        RowState::Failed(_) => (egui::Color32::from_rgb(220, 120, 120), "failed"),
                    };
                    let tag_resp = ui.add_sized([60.0, row_h], egui::Label::new(egui::RichText::new(tag).color(col)));
                    if let RowState::Failed(err) = st {
                        tag_resp.on_hover_text(err);
                    }
                    ui.add(egui::Label::new(name).truncate());
                });
            }
        });
}

enum RowState {
    Ready,
    Exists,
    Pending,
    Done,
    Failed(String),
}

fn spawn_fetch(
    url: String,
    cookies_browser: Option<String>,
    fetching: Arc<AtomicBool>,
    fetched: Arc<Mutex<Option<anyhow::Result<Playlist>>>>,
) {
    fetching.store(true, Ordering::Relaxed);
    std::thread::spawn(move || {
        let result = playlist::fetch_playlist(&url, cookies_browser.as_deref());
        *fetched.lock() = Some(result);
        fetching.store(false, Ordering::Relaxed);
    });
}
