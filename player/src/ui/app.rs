use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use eframe::CreationContext;
use parking_lot::RwLock;

use crate::data::duplicates::{find_duplicates, DuplicateGroup};
use crate::data::fingerprint::Fingerprint;
use crate::data::Library;
use crate::domain::{sort_songs, Screen, Song, SortOption};
use crate::playback::PlaybackController;
use crate::replacer::SearchBackend;
use crate::settings::Settings;
use crate::ui::components::{mini_player, top_bar};
use crate::ui::fonts;
use crate::ui::screens;
use crate::ui::toasts::{self, Toast, ToastLevel};

pub const PAGE_SIZE: usize = 50;

fn num_cpus_or(default: usize) -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(default)
}

pub struct LibraryView {
    pub library_version: u64,
    pub sort: SortOption,
    pub query: String,
    pub songs: Arc<Vec<Song>>,
}

pub struct ReplacerView {
    pub source_version: u64,
    pub folder_filters: Vec<PathBuf>,
    pub select_none: bool,
    pub query: String,
    pub songs: Arc<Vec<Song>>,
}

pub struct MissingView {
    pub source_version: u64,
    pub dest_version: u64,
    pub source_root: PathBuf,
    pub dest_root: PathBuf,
    pub missing: Arc<Vec<(PathBuf, PathBuf)>>,
}

pub struct App {
    pub library: Arc<Library>,
    pub source: Arc<Library>,
    pub playback: Arc<PlaybackController>,
    pub settings: Arc<RwLock<Settings>>,
    pub screen: Screen,
    pub sort: SortOption,
    pub search_query: String,
    pub volume: f32,
    pub replacer_folder_filters: Vec<PathBuf>,
    pub replacer_select_none: bool,
    pub replacer_query_filter: String,
    pub replacer_backend: SearchBackend,
    pub replacer_page: usize,
    pub songs_page: usize,
    pub cached_library_view: Option<LibraryView>,
    pub cached_replacer_view: Option<ReplacerView>,
    pub cached_folders: Option<(u64, Arc<Vec<PathBuf>>)>,
    pub cached_source_folders: Option<(u64, Arc<Vec<PathBuf>>)>,
    pub cached_duplicates: Option<(u64, usize, Arc<Vec<DuplicateGroup>>)>,
    pub cached_missing: Option<MissingView>,
    pub last_scanned_source_root: String,
    pub last_scanned_dest_root: String,
    pub previous_screen: Screen,
    pub fingerprints: Arc<RwLock<HashMap<i64, Fingerprint>>>,
    pub fingerprint_failed: Arc<RwLock<std::collections::HashSet<i64>>>,
    pub fingerprint_progress: Arc<AtomicUsize>,
    pub fingerprint_total: Arc<AtomicUsize>,
    pub fingerprint_running: Arc<AtomicBool>,
    pub fingerprint_library_version: u64,
    pub toasts: Vec<Toast>,
}

impl App {
    pub fn new(
        cc: &CreationContext<'_>,
        library: Arc<Library>,
        source: Arc<Library>,
        playback: Arc<PlaybackController>,
        settings: Arc<RwLock<Settings>>,
    ) -> Self {
        fonts::install_unicode_fallbacks(&cc.egui_ctx);
        let initial_volume = settings.read().playback.volume;
        playback.set_volume(initial_volume);
        let initial_source_root = settings.read().scan.source_root.clone();
        let initial_dest_root = settings
            .read()
            .scan
            .roots
            .first()
            .cloned()
            .unwrap_or_default();
        Self {
            library,
            source,
            playback,
            settings,
            screen: Screen::AllSongs,
            sort: SortOption::TitleAsc,
            search_query: String::new(),
            volume: initial_volume,
            replacer_folder_filters: Vec::new(),
            replacer_select_none: false,
            replacer_query_filter: String::new(),
            replacer_backend: SearchBackend::default(),
            replacer_page: 0,
            songs_page: 0,
            cached_library_view: None,
            cached_replacer_view: None,
            cached_folders: None,
            cached_source_folders: None,
            cached_duplicates: None,
            cached_missing: None,
            last_scanned_source_root: initial_source_root,
            last_scanned_dest_root: initial_dest_root,
            previous_screen: Screen::AllSongs,
            fingerprints: Arc::new(RwLock::new(HashMap::new())),
            fingerprint_failed: Arc::new(RwLock::new(std::collections::HashSet::new())),
            fingerprint_progress: Arc::new(AtomicUsize::new(0)),
            fingerprint_total: Arc::new(AtomicUsize::new(0)),
            fingerprint_running: Arc::new(AtomicBool::new(false)),
            fingerprint_library_version: 0,
            toasts: Vec::new(),
        }
    }

    pub fn toast_info(&mut self, msg: impl Into<String>) {
        self.toasts.push(Toast::new(ToastLevel::Info, msg));
    }

    pub fn toast_warn(&mut self, msg: impl Into<String>) {
        self.toasts.push(Toast::new(ToastLevel::Warn, msg));
    }

    pub fn toast_error(&mut self, msg: impl Into<String>) {
        self.toasts.push(Toast::new(ToastLevel::Error, msg));
    }

    /// Global keyboard shortcuts. Gated on `wants_keyboard_input` so text edits
    /// (search bar, settings fields) keep first dibs on every keystroke. Only
    /// shortcuts that don't conflict with normal typing or browser-style
    /// navigation are bound here:
    ///
    /// * `Space` — play/pause
    /// * `Ctrl+Right` — next track
    /// * `Ctrl+Left` — previous track
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        let pb = self.playback.clone();
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Space) {
                pb.play_pause();
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::ArrowRight) {
                pb.next();
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::ArrowLeft) {
                pb.previous();
            }
        });
    }

    /// True when at least one of the operations the status bar reports on is
    /// currently active. Used to skip declaring the panel entirely when idle
    /// (otherwise it would render an empty strip with margin).
    fn has_active_async_op(&self) -> bool {
        use crate::data::LibraryStatus;
        matches!(self.library.status(), LibraryStatus::Scanning)
            || matches!(self.source.status(), LibraryStatus::Scanning)
            || self.fingerprint_running.load(Ordering::Relaxed)
    }

    /// One-line bar showing whichever async operations are currently in
    /// flight: library scan, source scan, fingerprinting. Caller is
    /// responsible for not invoking this when `has_active_async_op` is false.
    fn draw_status_bar(&self, ui: &mut egui::Ui) {
        use crate::data::LibraryStatus;

        let mut parts: Vec<String> = Vec::new();
        if matches!(self.library.status(), LibraryStatus::Scanning) {
            parts.push(format!(
                "Scanning library… ({} so far)",
                self.library.song_count()
            ));
        }
        if matches!(self.source.status(), LibraryStatus::Scanning) {
            parts.push(format!(
                "Scanning source… ({} so far)",
                self.source.song_count()
            ));
        }
        if self.fingerprint_running.load(Ordering::Relaxed) {
            let total = self.fingerprint_total.load(Ordering::Relaxed);
            let done = self.fingerprint_progress.load(Ordering::Relaxed);
            parts.push(format!("Fingerprinting {done}/{total}"));
        }
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.spinner();
            ui.label(egui::RichText::new(parts.join("  ·  ")).weak());
        });
    }

    pub fn start_fingerprinting(&mut self, force_full: bool) {
        if self.fingerprint_running.load(Ordering::Relaxed) {
            return;
        }
        if force_full {
            self.fingerprints.write().clear();
            self.fingerprint_failed.write().clear();
        }

        let library_version = self.library.version();
        let songs: Vec<(i64, PathBuf)> = self.library.read_songs(|all| {
            all.iter().map(|s| (s.id, s.path.clone())).collect()
        });
        if songs.is_empty() {
            return;
        }

        let live_ids: std::collections::HashSet<i64> =
            songs.iter().map(|(id, _)| *id).collect();
        self.fingerprints
            .write()
            .retain(|id, _| live_ids.contains(id));
        self.fingerprint_failed
            .write()
            .retain(|id| live_ids.contains(id));

        let existing: std::collections::HashSet<i64> =
            self.fingerprints.read().keys().copied().collect();
        let failed: std::collections::HashSet<i64> =
            self.fingerprint_failed.read().iter().copied().collect();
        let needed: Vec<(i64, PathBuf)> = songs
            .into_iter()
            .filter(|(id, _)| !existing.contains(id) && !failed.contains(id))
            .collect();
        if needed.is_empty() {
            self.fingerprint_library_version = library_version;
            return;
        }

        self.fingerprint_library_version = library_version;
        let already_done = existing.len();
        let total = already_done + needed.len();
        self.fingerprint_total.store(total, Ordering::Relaxed);
        self.fingerprint_progress.store(already_done, Ordering::Relaxed);
        self.fingerprint_running.store(true, Ordering::Relaxed);

        let fingerprints = self.fingerprints.clone();
        let failed = self.fingerprint_failed.clone();
        let progress = self.fingerprint_progress.clone();
        let running = self.fingerprint_running.clone();

        std::thread::spawn(move || {
            use crossbeam_channel::{bounded, Receiver};
            let (tx, rx) = bounded::<(i64, PathBuf)>(needed.len());
            for s in needed {
                let _ = tx.send(s);
            }
            drop(tx);

            let workers = num_cpus_or(4);
            let mut handles = Vec::new();
            for _ in 0..workers {
                let rx: Receiver<(i64, PathBuf)> = rx.clone();
                let fps = fingerprints.clone();
                let fail = failed.clone();
                let prog = progress.clone();
                handles.push(std::thread::spawn(move || {
                    while let Ok((id, path)) = rx.recv() {
                        match crate::data::fingerprint::compute_fingerprint(&path) {
                            Ok(fp) => {
                                fps.write().insert(id, fp);
                            }
                            Err(e) => {
                                tracing::debug!(
                                    "fingerprint skip {}: {e:#}",
                                    path.display()
                                );
                                fail.write().insert(id);
                            }
                        }
                        prog.fetch_add(1, Ordering::Relaxed);
                    }
                }));
            }
            for h in handles {
                let _ = h.join();
            }
            running.store(false, Ordering::Relaxed);
        });
    }

    pub fn rescan_if_paths_changed(&mut self) {
        if matches!(self.screen, Screen::Settings) {
            return;
        }
        let source_root = self.settings.read().scan.source_root.clone();
        let dest_root = self
            .settings
            .read()
            .scan
            .roots
            .first()
            .cloned()
            .unwrap_or_default();
        if source_root != self.last_scanned_source_root {
            self.last_scanned_source_root = source_root.clone();
            self.cached_missing = None;
            if !source_root.trim().is_empty() {
                let source_lib = self.source.clone();
                std::thread::spawn(move || {
                    if let Err(e) = source_lib.scan(&[PathBuf::from(source_root)]) {
                        tracing::error!("source rescan failed: {e:#}");
                    }
                });
            }
        }
        if dest_root != self.last_scanned_dest_root {
            self.last_scanned_dest_root = dest_root.clone();
            self.cached_missing = None;
            if !dest_root.trim().is_empty() {
                let library = self.library.clone();
                std::thread::spawn(move || {
                    if let Err(e) = library.scan(&[PathBuf::from(dest_root)]) {
                        tracing::error!("dest rescan failed: {e:#}");
                    }
                });
            }
        }
    }

    pub fn library_folders(&mut self) -> Arc<Vec<PathBuf>> {
        let v = self.library.version();
        let stale = self
            .cached_folders
            .as_ref()
            .map(|(cv, _)| *cv != v)
            .unwrap_or(true);
        if stale {
            self.cached_folders = Some((v, Arc::new(self.library.folders())));
        }
        self.cached_folders.as_ref().unwrap().1.clone()
    }

    pub fn source_folders(&mut self) -> Arc<Vec<PathBuf>> {
        let v = self.source.version();
        let stale = self
            .cached_source_folders
            .as_ref()
            .map(|(cv, _)| *cv != v)
            .unwrap_or(true);
        if stale {
            self.cached_source_folders = Some((v, Arc::new(self.source.folders())));
        }
        self.cached_source_folders.as_ref().unwrap().1.clone()
    }

    pub fn replacer_view(&mut self) -> Arc<Vec<Song>> {
        let version = self.source.version();
        let needs_rebuild = match &self.cached_replacer_view {
            None => true,
            Some(v) => {
                v.source_version != version
                    || v.folder_filters != self.replacer_folder_filters
                    || v.select_none != self.replacer_select_none
                    || v.query != self.replacer_query_filter
            }
        };
        if needs_rebuild {
            let filters = self.replacer_folder_filters.clone();
            let select_none = self.replacer_select_none;
            let mut songs: Vec<Song> = if select_none {
                Vec::new()
            } else {
                self.source.read_songs(|all| {
                    if filters.is_empty() {
                        all.to_vec()
                    } else {
                        all.iter()
                            .filter(|s| {
                                s.folder()
                                    .map(|f| filters.iter().any(|x| x.as_path() == f))
                                    .unwrap_or(false)
                            })
                            .cloned()
                            .collect()
                    }
                })
            };
            if !self.replacer_query_filter.is_empty() {
                let q = self.replacer_query_filter.to_lowercase();
                songs.retain(|s| {
                    s.title.to_lowercase().contains(&q)
                        || s.path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n.to_lowercase().contains(&q))
                            .unwrap_or(false)
                });
            }
            songs.sort_by(|a, b| a.path.cmp(&b.path));
            self.cached_replacer_view = Some(ReplacerView {
                source_version: version,
                folder_filters: self.replacer_folder_filters.clone(),
                select_none,
                query: self.replacer_query_filter.clone(),
                songs: Arc::new(songs),
            });
        }
        self.cached_replacer_view
            .as_ref()
            .map(|v| v.songs.clone())
            .unwrap_or_else(|| Arc::new(Vec::new()))
    }

    pub fn missing(&mut self) -> Arc<Vec<(PathBuf, PathBuf)>> {
        let source_version = self.source.version();
        let dest_version = self.library.version();
        let source_root: PathBuf = PathBuf::from(self.settings.read().scan.source_root.clone());
        let dest_root: PathBuf = self
            .settings
            .read()
            .scan
            .roots
            .first()
            .cloned()
            .map(PathBuf::from)
            .unwrap_or_default();
        let needs_rebuild = match &self.cached_missing {
            None => true,
            Some(v) => {
                v.source_version != source_version
                    || v.dest_version != dest_version
                    || v.source_root != source_root
                    || v.dest_root != dest_root
            }
        };
        if needs_rebuild {
            let missing = if source_root.as_os_str().is_empty() || dest_root.as_os_str().is_empty()
            {
                Vec::new()
            } else {
                let src_root = source_root.clone();
                let dst_root = dest_root.clone();
                self.source.read_songs(|src| {
                    self.library.read_songs(|dst| {
                        crate::replacer::sync::missing_in_dest(src, dst, &src_root, &dst_root)
                    })
                })
            };
            self.cached_missing = Some(MissingView {
                source_version,
                dest_version,
                source_root,
                dest_root,
                missing: Arc::new(missing),
            });
        }
        self.cached_missing
            .as_ref()
            .map(|v| v.missing.clone())
            .unwrap_or_else(|| Arc::new(Vec::new()))
    }

    pub fn navigate(&mut self, screen: Screen) {
        self.screen = screen;
    }

    pub fn duplicates(&mut self) -> Arc<Vec<DuplicateGroup>> {
        let v = self.library.version();
        let progress = self.fingerprint_progress.load(Ordering::Relaxed);
        let stale = self
            .cached_duplicates
            .as_ref()
            .map(|(cv, cp, _)| *cv != v || *cp != progress)
            .unwrap_or(true);
        if stale {
            let fps = self.fingerprints.read().clone();
            let groups = self.library.read_songs(|all| find_duplicates(all, &fps));
            self.cached_duplicates = Some((v, progress, Arc::new(groups)));
        }
        self.cached_duplicates.as_ref().unwrap().2.clone()
    }

    pub fn library_view(&mut self) -> Arc<Vec<Song>> {
        let version = self.library.version();
        let needs_rebuild = match &self.cached_library_view {
            None => true,
            Some(v) => {
                v.library_version != version
                    || v.sort != self.sort
                    || v.query != self.search_query
            }
        };
        if needs_rebuild {
            let mut songs = self.library.read_songs(|all| {
                if self.search_query.is_empty() {
                    all.to_vec()
                } else {
                    let q = self.search_query.to_lowercase();
                    all.iter()
                        .filter(|s| {
                            s.title.to_lowercase().contains(&q)
                                || s.artist.to_lowercase().contains(&q)
                                || s.album.to_lowercase().contains(&q)
                        })
                        .cloned()
                        .collect()
                }
            });
            sort_songs(&mut songs, self.sort);
            self.cached_library_view = Some(LibraryView {
                library_version: version,
                sort: self.sort,
                query: self.search_query.clone(),
                songs: Arc::new(songs),
            });
        }
        self.cached_library_view
            .as_ref()
            .map(|v| v.songs.clone())
            .unwrap_or_else(|| Arc::new(Vec::new()))
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.screen != self.previous_screen {
            if matches!(self.screen, Screen::Missing) {
                self.cached_missing = None;
            }
            self.previous_screen = self.screen.clone();
        }
        self.handle_shortcuts(ctx);
        self.rescan_if_paths_changed();
        if self.playback.state_snapshot().is_playing {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            top_bar::draw(ui, self);
        });

        if self.screen.shows_bottom_nav() {
            egui::TopBottomPanel::bottom("mini_player").show(ctx, |ui| {
                mini_player::draw(ui, self);
            });
        }
        // Status bar sits just above the mini-player (or at the bottom when
        // mini-player is hidden) and only renders when an async op is live —
        // we check the precondition before declaring the panel so the bar
        // takes zero vertical space when idle.
        if self.has_active_async_op() {
            egui::TopBottomPanel::bottom("status_bar")
                .show_separator_line(false)
                .show(ctx, |ui| {
                    self.draw_status_bar(ui);
                });
            // Status updates come from atomics that won't trigger repaints
            // on their own; nudge egui so the spinner animates and the bar
            // disappears promptly when the op finishes.
            ctx.request_repaint_after(std::time::Duration::from_millis(300));
        }

        toasts::draw(ctx, &mut self.toasts);

        egui::CentralPanel::default().show(ctx, |ui| match self.screen.clone() {
            Screen::Library => screens::library::draw(ui, self),
            Screen::AllSongs => screens::library::draw(ui, self),
            Screen::AlbumsList => screens::library::draw_albums(ui, self),
            Screen::AlbumDetail(album) => screens::library::draw_album_detail(ui, self, &album),
            Screen::ArtistsList => screens::library::draw_artists(ui, self),
            Screen::ArtistDetail(artist) => screens::library::draw_artist_detail(ui, self, &artist),
            Screen::Folders => screens::library::draw_folders(ui, self),
            Screen::Playlists => screens::library::draw_playlists(ui, self),
            Screen::NowPlaying => screens::now_playing::draw(ui, self),
            Screen::Equalizer => screens::settings::draw_equalizer(ui, self),
            Screen::Search => screens::library::draw_search(ui, self),
            Screen::Queue => screens::queue::draw(ui, self),
            Screen::Replacer => screens::replacer::draw(ui, self),
            Screen::Duplicates => screens::duplicates::draw(ui, self),
            Screen::Missing => screens::missing::draw(ui, self),
            Screen::Settings => screens::settings::draw(ui, self),
        });
    }
}
