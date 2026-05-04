use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use parking_lot::RwLock;
use tracing_subscriber::EnvFilter;

use recurate::data::Library;
use recurate::engine::Engine;
use recurate::playback::PlaybackController;
use recurate::settings::Settings;
use recurate::ui::App;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                // `symphonia_bundle_mp3=error` silences the noisy `invalid
                // main_data_begin, underflow by N bytes` warnings emitted on
                // nearly every track in the corpus — symphonia recovers from
                // them on its own and decoding continues, so they're not
                // actionable. Real decode failures still surface as errors.
                EnvFilter::new("recurate=info,warn,symphonia_bundle_mp3=error")
            }),
        )
        .init();

    let settings = Arc::new(RwLock::new(Settings::load_or_default()));

    let library = Arc::new(Library::new());
    let source = Arc::new(Library::new());
    {
        let library = library.clone();
        let roots: Vec<PathBuf> = settings
            .read()
            .scan
            .roots
            .iter()
            .map(PathBuf::from)
            .collect();
        std::thread::spawn(move || {
            if let Err(e) = library.scan(&roots) {
                tracing::error!("library scan failed: {e:#}");
            }
        });
    }
    {
        let source = source.clone();
        let source_root = settings.read().scan.source_root.clone();
        if !source_root.trim().is_empty() {
            let roots = vec![PathBuf::from(source_root)];
            std::thread::spawn(move || {
                if let Err(e) = source.scan(&roots) {
                    tracing::error!("source scan failed: {e:#}");
                }
            });
        }
    }

    let engine = Engine::start()?;
    let playback = PlaybackController::new(engine, library.clone());

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([700.0, 480.0])
            .with_title("Recurate"),
        ..Default::default()
    };

    eframe::run_native(
        "recurate",
        native_options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(App::new(cc, library, source, playback, settings)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}
