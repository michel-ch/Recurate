use std::path::PathBuf;
use std::sync::Arc;

use once_cell::sync::{Lazy, OnceCell};

use crate::data::Library;
use crate::domain::Screen;
use crate::replacer::{
    download::ffmpeg_available,
    download_worker::{DownloadRequest, DownloadState, DownloadWorker},
    search_worker::{SearchRequest, SearchWorker, SongState},
    youtube::ytdlp_available,
    SearchBackend,
};
use crate::ui::App;

static WORKER: Lazy<Arc<SearchWorker>> = Lazy::new(|| Arc::new(SearchWorker::start()));
static DOWNLOAD: OnceCell<Arc<DownloadWorker>> = OnceCell::new();

fn download_worker(library: Arc<Library>) -> &'static Arc<DownloadWorker> {
    DOWNLOAD.get_or_init(|| Arc::new(DownloadWorker::start(library)))
}

pub fn draw(ui: &mut egui::Ui, app: &mut App) {
    ui.horizontal(|ui| {
        if ui.button("← Back").clicked() {
            app.navigate(Screen::AllSongs);
        }
        ui.heading("Replacer — YouTube match preview");
    });
    ui.label(
        "Reads filenames from the source root, searches YouTube for an audio-only match, and \
         downloads the replacement into the destination root (mirroring the folder layout). \
         The source folder is never modified — it is the catalog of what to fetch.",
    );
    ui.horizontal(|ui| {
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
                "ffmpeg missing — `winget install Gyan.FFmpeg` (search works, replace will fail)",
            ),
        };
        ui.colored_label(col, msg);
    });

    let api_key = std::env::var("YOUTUBE_API_KEY").ok().filter(|s| !s.is_empty()).or_else(|| {
        let k = app.settings.read().replacer.youtube_api_key.clone();
        if k.is_empty() {
            None
        } else {
            Some(k)
        }
    });

    let cookies_browser = {
        let c = app.settings.read().replacer.cookies_browser.trim().to_string();
        if c.is_empty() {
            None
        } else {
            Some(c)
        }
    };

    ui.horizontal(|ui| {
        ui.label("Backend:");
        let current = app.replacer_backend;
        egui::ComboBox::from_id_source("replacer_backend")
            .selected_text(current.label())
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(current == SearchBackend::Ytdlp, SearchBackend::Ytdlp.label())
                    .clicked()
                {
                    app.replacer_backend = SearchBackend::Ytdlp;
                }
                if ui
                    .selectable_label(current == SearchBackend::Api, SearchBackend::Api.label())
                    .clicked()
                {
                    app.replacer_backend = SearchBackend::Api;
                }
            });
        match app.replacer_backend {
            SearchBackend::Ytdlp => {
                if ytdlp_available() {
                    ui.colored_label(
                        egui::Color32::from_rgb(120, 200, 120),
                        "yt-dlp on PATH",
                    );
                } else {
                    ui.colored_label(
                        egui::Color32::from_rgb(220, 80, 80),
                        "yt-dlp not on PATH (`winget install yt-dlp`)",
                    );
                }
            }
            SearchBackend::Api => match &api_key {
                Some(k) if !k.is_empty() => {
                    ui.colored_label(
                        egui::Color32::from_rgb(120, 200, 120),
                        format!("API key set (…{})", suffix4(k)),
                    );
                }
                _ => {
                    ui.colored_label(
                        egui::Color32::from_rgb(220, 80, 80),
                        "No API key — set in Settings → Replacer or export YOUTUBE_API_KEY",
                    );
                }
            },
        }
    });

    if app.replacer_backend == SearchBackend::Ytdlp && !ytdlp_available() {
        ui.separator();
        return;
    }

    ui.separator();

    let folders_arc = app.source_folders();
    let folders: &[PathBuf] = &folders_arc;
    let total_folders = folders.len();
    let popup_id = ui.make_persistent_id("replacer_folder_multi");

    ui.horizontal(|ui| {
        ui.label("Folders:");
        let selected_count = if app.replacer_select_none {
            0
        } else if app.replacer_folder_filters.is_empty() {
            total_folders
        } else {
            app.replacer_folder_filters.len()
        };
        let resp = ui.button(format!("{selected_count}/{total_folders} ▾"));
        if resp.clicked() {
            ui.memory_mut(|m| m.toggle_popup(popup_id));
        }
        egui::popup::popup_below_widget(
            ui,
            popup_id,
            &resp,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
            ui.set_min_width(260.0);
            ui.set_max_height(380.0);
            ui.horizontal(|ui| {
                if ui.small_button("Reset to all").clicked() {
                    app.replacer_folder_filters.clear();
                    app.replacer_select_none = false;
                }
                if ui.small_button("Deselect all").clicked() {
                    app.replacer_folder_filters.clear();
                    app.replacer_select_none = true;
                }
                ui.label(
                    egui::RichText::new(format!("{selected_count} of {total_folders}")).weak(),
                );
            });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                let is_none = app.replacer_select_none;
                let is_all = !is_none && app.replacer_folder_filters.is_empty();
                for folder in folders {
                    let label = folder
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("?")
                        .to_string();
                    let mut sel = is_all
                        || (!is_none
                            && app.replacer_folder_filters.iter().any(|x| x == folder));
                    if ui.checkbox(&mut sel, &label).changed() {
                        if sel {
                            if is_none {
                                app.replacer_select_none = false;
                                app.replacer_folder_filters = vec![folder.clone()];
                            } else {
                                if !app.replacer_folder_filters.iter().any(|x| x == folder) {
                                    app.replacer_folder_filters.push(folder.clone());
                                }
                                if app.replacer_folder_filters.len() == folders.len() {
                                    app.replacer_folder_filters.clear();
                                }
                            }
                        } else if is_all {
                            app.replacer_folder_filters = folders
                                .iter()
                                .filter(|f| f.as_path() != folder.as_path())
                                .cloned()
                                .collect();
                        } else {
                            app.replacer_folder_filters.retain(|x| x != folder);
                            if app.replacer_folder_filters.is_empty() {
                                app.replacer_select_none = true;
                            }
                        }
                    }
                }
            });
        });
        ui.separator();
        ui.label("Filter:");
        ui.add(
            egui::TextEdit::singleline(&mut app.replacer_query_filter).desired_width(200.0),
        );
        ui.separator();
        if ui.button("Clear results").clicked() {
            WORKER.clear();
        }
    });

    ui.separator();

    let songs_arc = app.replacer_view();
    let songs: &[crate::domain::Song] = &songs_arc;

    let backend = app.replacer_backend;
    let api_key_val = api_key.clone();
    let cookies_val = cookies_browser.clone();

    ui.horizontal(|ui| {
        let cap = match backend {
            SearchBackend::Ytdlp => 50,
            SearchBackend::Api => 25,
        };
        let n = songs.len();
        let take = cap.min(n);
        if ui
            .button(format!("Search first {take}"))
            .on_hover_text(match backend {
                SearchBackend::Ytdlp => "Each search is ~2-3s via yt-dlp",
                SearchBackend::Api => "Each search costs ~101 quota units (search.list + videos.list)",
            })
            .clicked()
        {
            for song in songs.iter().take(cap) {
                enqueue_search(song, backend, api_key_val.clone(), cookies_val.clone());
            }
        }
        let all_label = match backend {
            SearchBackend::Ytdlp => format!("Search all {n} (~{}m)", (n as u64 * 3 + 59) / 60),
            SearchBackend::Api => format!("Search all {n} (~{} units)", n as u64 * 101),
        };
        let all_hover = match backend {
            SearchBackend::Ytdlp => format!("yt-dlp: ~3s per search → ~{}s total", n * 3),
            SearchBackend::Api => format!(
                "API: {n} × 101 = {} units. Daily default quota = 10,000.",
                n as u64 * 101
            ),
        };
        let dangerous = matches!(backend, SearchBackend::Api) && n > 99;
        let all_btn = if dangerous {
            egui::Button::new(
                egui::RichText::new(&all_label).color(egui::Color32::from_rgb(220, 80, 80)),
            )
        } else {
            egui::Button::new(&all_label)
        };
        if ui.add(all_btn).on_hover_text(all_hover).clicked() {
            for song in songs.iter() {
                enqueue_search(song, backend, api_key_val.clone(), cookies_val.clone());
            }
        }
        ui.separator();
        ui.label(format!("{} songs visible", n));
    });

    let dl_for_controls = download_worker(app.library.clone()).clone();
    let source_root: PathBuf = PathBuf::from(app.settings.read().scan.source_root.clone());
    let dest_root: PathBuf = app
        .settings
        .read()
        .scan
        .roots
        .first()
        .cloned()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./music"));
    let cookies_browser = {
        let s = app.settings.read().replacer.cookies_browser.trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    };

    let stats = WORKER.read_states(|search_states| {
        dl_for_controls.read_states(|dl_states| {
            let mut s = ScopeStats::default();
            for song in songs {
                s.total += 1;
                match search_states.get(&song.id) {
                    None | Some(SongState::Idle) => s.search_idle += 1,
                    Some(SongState::Pending) => s.search_pending += 1,
                    Some(SongState::Failed { .. }) => s.search_failed += 1,
                    Some(SongState::Done { results, .. }) => {
                        if results.is_empty() {
                            s.search_no_results += 1;
                        } else {
                            s.search_done += 1;
                            let dl = dl_states.get(&song.id);
                            match dl {
                                None | Some(DownloadState::Idle) => {
                                    s.ready_to_replace.push((
                                        song.id,
                                        song.path.clone(),
                                        results[0].video.watch_url(),
                                    ));
                                }
                                Some(DownloadState::Pending) => s.dl_pending += 1,
                                Some(DownloadState::Done { .. }) => s.dl_done += 1,
                                Some(DownloadState::Failed { .. }) => s.dl_failed += 1,
                            }
                        }
                    }
                }
            }
            s
        })
    });
    let any_pending_search = stats.search_pending > 0;
    let ready_to_replace = &stats.ready_to_replace;

    if any_pending_search || !ready_to_replace.is_empty() {
        ui.horizontal(|ui| {
            let n = ready_to_replace.len();
            let label = if any_pending_search {
                format!("Start replace top match ({n} ready, more incoming)")
            } else {
                format!("Start replace top match ({n} ready)")
            };
            if ui
                .add_enabled(n > 0, egui::Button::new(label))
                .on_hover_text(
                    "Queues a replace for each visible song whose search is complete with at \
                     least one result. Safe to click again as more searches finish.",
                )
                .clicked()
            {
                for (song_id, source_path, video_url) in ready_to_replace {
                    let rel = source_path
                        .strip_prefix(&source_root)
                        .ok()
                        .map(PathBuf::from)
                        .or_else(|| source_path.file_name().map(PathBuf::from))
                        .unwrap_or_else(|| PathBuf::from("song.mp3"));
                    let dest_path = dest_root.join(rel);
                    dl_for_controls.enqueue(DownloadRequest {
                        song_id: *song_id,
                        source_path: source_path.clone(),
                        dest_path,
                        video_url: video_url.clone(),
                        cookies_browser: cookies_browser.clone(),
                    });
                }
            }
            if any_pending_search {
                ui.spinner();
                ui.label(
                    egui::RichText::new("searches still running…").weak(),
                );
            }
        });
    }

    ui.horizontal(|ui| {
        let s_paused = WORKER.is_paused();
        let s_pending = WORKER.pending_count();
        let s_label = if s_paused {
            format!("▶ Resume search ({s_pending} queued)")
        } else {
            format!("⏸ Pause search ({s_pending} queued)")
        };
        if ui
            .add_enabled(s_pending > 0 || s_paused, egui::Button::new(s_label))
            .on_hover_text("Pauses the search worker; queue is preserved and resumes where it left off")
            .clicked()
        {
            WORKER.set_paused(!s_paused);
        }

        let d_paused = dl_for_controls.is_paused();
        let d_pending = dl_for_controls.pending_count();
        let d_label = if d_paused {
            format!("▶ Resume replace ({d_pending} queued)")
        } else {
            format!("⏸ Pause replace ({d_pending} queued)")
        };
        if ui
            .add_enabled(d_pending > 0 || d_paused, egui::Button::new(d_label))
            .on_hover_text("Pauses the download worker; queue is preserved and resumes where it left off")
            .clicked()
        {
            dl_for_controls.set_paused(!d_paused);
        }
    });

    ui.separator();

    ui.label(egui::RichText::new("Search progress").strong());
    ui.horizontal(|ui| {
        ui.label(format!("Total in scope: {}", stats.total));
        ui.separator();
        ui.label(format!("Idle: {}", stats.search_idle));
        ui.separator();
        ui.colored_label(
            egui::Color32::from_rgb(180, 180, 220),
            format!("Pending: {}", stats.search_pending),
        );
        ui.separator();
        ui.colored_label(
            egui::Color32::from_rgb(120, 200, 120),
            format!("Done w/ results: {}", stats.search_done),
        );
        ui.separator();
        ui.label(format!("No results: {}", stats.search_no_results));
        ui.separator();
        ui.colored_label(
            egui::Color32::from_rgb(220, 120, 120),
            format!("Failed: {}", stats.search_failed),
        );
    });

    ui.label(egui::RichText::new("Replace progress").strong());
    ui.horizontal(|ui| {
        ui.colored_label(
            egui::Color32::from_rgb(180, 180, 220),
            format!("Pending: {}", stats.dl_pending),
        );
        ui.separator();
        ui.colored_label(
            egui::Color32::from_rgb(120, 200, 120),
            format!("Done: {}", stats.dl_done),
        );
        ui.separator();
        ui.colored_label(
            egui::Color32::from_rgb(220, 120, 120),
            format!("Failed: {}", stats.dl_failed),
        );
        ui.separator();
        ui.label(format!("Ready to start: {}", stats.ready_to_replace.len()));
    });
}

#[derive(Default)]
struct ScopeStats {
    total: usize,
    search_idle: usize,
    search_pending: usize,
    search_done: usize,
    search_no_results: usize,
    search_failed: usize,
    dl_pending: usize,
    dl_done: usize,
    dl_failed: usize,
    ready_to_replace: Vec<(i64, PathBuf, String)>,
}

fn enqueue_search(
    song: &crate::domain::Song,
    backend: SearchBackend,
    api_key: Option<String>,
    cookies_browser: Option<String>,
) {
    let stem = song
        .path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    if stem.is_empty() {
        return;
    }
    let req = SearchRequest {
        song_id: song.id,
        filename_stem: stem,
        expected_duration_secs: Some(song.duration.as_secs()),
        max_results: 10,
        backend,
        api_key,
        cookies_browser,
    };
    WORKER.enqueue(req);
}

fn suffix4(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    if n <= 4 {
        s.to_string()
    } else {
        chars[n - 4..].iter().collect()
    }
}

