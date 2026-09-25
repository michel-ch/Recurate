//! Regenerates the README screenshots in `docs/screenshots/`.
//!
//! Run from `player/`:
//!
//! ```text
//! cargo run --release --example capture_screenshots
//! cargo run --release --example capture_screenshots -- songs queue --out ../tmp
//! ```
//!
//! Screen names limit the run to those shots; `--out` changes the folder.
//!
//! Opens the real app window with your saved settings, visits each screen and
//! saves one PNG per screen through egui's viewport screenshot command. Nothing
//! is clicked. Library roots that live under the current directory are shown
//! relative, so the images don't carry the Windows user name. A track is
//! started muted and then paused so the mini player has something to show.
//! Settings are never written back: saves are redirected to a temp folder.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use parking_lot::RwLock;

use recurate::data::{Library, LibraryStatus};
use recurate::domain::Screen;
use recurate::engine::Engine;
use recurate::playback::PlaybackController;
use recurate::settings::Settings;
use recurate::ui::App;

const DEFAULT_OUT_DIR: &str = "../docs/screenshots";
const SETTLE: Duration = Duration::from_millis(1500);
const FINGERPRINT_TIMEOUT: Duration = Duration::from_secs(300);
const PLAYLIST_FOLDER: &str = "Chill";

/// Screens in capture order. Duplicates goes last so fingerprinting, started
/// at load time, has the longest head start.
const SHOTS: &[(&str, Screen)] = &[
    ("songs", Screen::AllSongs),
    ("albums", Screen::AlbumsList),
    ("artists", Screen::ArtistsList),
    ("folders", Screen::Folders),
    ("playlists", Screen::Playlists),
    ("playlist", Screen::Playlist),
    ("queue", Screen::Queue),
    ("now-playing", Screen::NowPlaying),
    ("replacer", Screen::Replacer),
    ("missing", Screen::Missing),
    ("settings", Screen::Settings),
    ("duplicates", Screen::Duplicates),
];

enum Phase {
    Loading,
    Priming(Instant),
    Settle(usize, Instant),
    Waiting(usize),
    Done,
}

struct Capture {
    inner: App,
    phase: Phase,
    volume: f32,
    shots: Vec<(&'static str, Screen)>,
    out_dir: PathBuf,
}

impl eframe::App for Capture {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.inner.update(ctx, frame);
        ctx.request_repaint();

        match self.phase {
            Phase::Loading => {
                let idle = |l: &Library| !matches!(l.status(), LibraryStatus::Scanning);
                if idle(&self.inner.library) && idle(&self.inner.source) && self.inner.library.song_count() > 0 {
                    println!("library loaded: {} songs", self.inner.library.song_count());
                    self.inner.start_fingerprinting(false);
                    let view = self.inner.library_view();
                    self.inner.playback.play_songs((*view).clone(), 2.min(view.len() - 1), None);
                    self.phase = Phase::Priming(Instant::now());
                }
            }
            Phase::Priming(since) => {
                let s = self.inner.playback.state_snapshot();
                let started = s.is_playing && s.current_position_ms > 0;
                if started || since.elapsed() > Duration::from_secs(10) {
                    if s.is_playing {
                        self.inner.playback.play_pause();
                    }
                    self.inner.volume = self.volume;
                    self.inner.playback.set_volume(self.volume);
                    self.enter(0);
                }
            }
            Phase::Settle(i, since) => {
                let fingerprinting = self.shots[i].1 == Screen::Duplicates
                    && self.inner.fingerprint_running.load(Ordering::Relaxed)
                    && since.elapsed() < FINGERPRINT_TIMEOUT;
                if since.elapsed() >= SETTLE && !fingerprinting {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
                    self.phase = Phase::Waiting(i);
                }
            }
            Phase::Waiting(i) => {
                let shot = ctx.input(|input| {
                    input.raw.events.iter().find_map(|e| match e {
                        egui::Event::Screenshot { image, .. } => Some(image.clone()),
                        _ => None,
                    })
                });
                if let Some(image) = shot {
                    let path = self.out_dir.join(format!("{}.png", self.shots[i].0));
                    match save_png(&image, &path) {
                        Ok(()) => println!("saved {}", path.display()),
                        Err(e) => eprintln!("failed {}: {e:#}", path.display()),
                    }
                    if i + 1 < self.shots.len() {
                        self.enter(i + 1);
                    } else {
                        self.phase = Phase::Done;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
            Phase::Done => {}
        }
    }
}

impl Capture {
    fn enter(&mut self, i: usize) {
        let screen = self.shots[i].1.clone();
        if screen == Screen::Playlists {
            let folders = self.inner.library.folders();
            self.inner.playlists.selected = folders
                .iter()
                .find(|f| f.file_name().is_some_and(|n| n == PLAYLIST_FOLDER))
                .or(folders.first())
                .cloned();
        }
        println!("capturing {}", self.shots[i].0);
        self.inner.navigate(screen);
        self.phase = Phase::Settle(i, Instant::now());
    }
}

fn save_png(image: &egui::ColorImage, path: &Path) -> Result<()> {
    let [w, h] = image.size;
    let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
    let buf = image::RgbaImage::from_raw(w as u32, h as u32, bytes)
        .ok_or_else(|| anyhow::anyhow!("pixel buffer size mismatch"))?;
    buf.save(path)?;
    Ok(())
}

/// `C:\Users\me\...\player\my_music` → `my_music` when it sits under `cwd`.
fn relative_to(p: &str, cwd: &Path) -> String {
    let norm = |s: &str| s.replace('/', "\\").to_lowercase();
    let base = norm(&cwd.to_string_lossy());
    let full = norm(p);
    match full.strip_prefix(&format!("{base}\\")) {
        Some(_) => p[base.len() + 1..].to_string(),
        None => p.to_string(),
    }
}

fn main() -> Result<()> {
    let mut out_dir = PathBuf::from(DEFAULT_OUT_DIR);
    let mut only: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--out" {
            out_dir = PathBuf::from(args.next().unwrap_or_default());
        } else {
            only.push(a);
        }
    }
    let shots: Vec<(&'static str, Screen)> = SHOTS
        .iter()
        .filter(|(name, _)| only.is_empty() || only.iter().any(|o| o == name))
        .cloned()
        .collect();
    if shots.is_empty() {
        anyhow::bail!("no matching screens; known: {:?}", SHOTS.iter().map(|s| s.0).collect::<Vec<_>>());
    }
    std::fs::create_dir_all(&out_dir)?;
    let cwd = std::env::current_dir()?;

    let mut settings = Settings::load_or_default();
    // From here on any settings save lands in a throwaway folder: the
    // relative roots and muted volume below must never reach the user's file.
    std::env::set_var(
        "RECURATE_CONFIG_DIR",
        std::env::temp_dir().join("recurate-capture-config"),
    );
    settings.scan.roots = settings.scan.roots.iter().map(|r| relative_to(r, &cwd)).collect();
    settings.scan.source_root = relative_to(&settings.scan.source_root, &cwd);
    let volume = settings.playback.volume;
    settings.playback.volume = 0.0;
    let roots: Vec<PathBuf> = settings.scan.roots.iter().map(PathBuf::from).collect();
    let source_root = PathBuf::from(&settings.scan.source_root);
    let settings = Arc::new(RwLock::new(settings));

    let library = Arc::new(Library::new());
    let source = Arc::new(Library::new());
    {
        let library = library.clone();
        std::thread::spawn(move || library.scan(&roots));
    }
    {
        let source = source.clone();
        std::thread::spawn(move || source.scan(&[source_root]));
    }

    let engine = Engine::start()?;
    let playback = PlaybackController::new(engine, library.clone());

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_resizable(false)
            .with_title("Recurate"),
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "recurate-capture",
        options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            let inner = App::new(cc, library, source, playback, settings);
            Ok(Box::new(Capture { inner, phase: Phase::Loading, volume, shots, out_dir }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;
    Ok(())
}
