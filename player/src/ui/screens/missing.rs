use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use once_cell::sync::Lazy;

use crate::domain::Screen;
use crate::renumberer;
use crate::replacer::sync as sync_files;
use crate::ui::App;

static SYNC_RUNNING: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));
static RENUMBER_RUNNING: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

pub fn draw(ui: &mut egui::Ui, app: &mut App) {
    let source_root = app.settings.read().scan.source_root.clone();
    let dest_root = app
        .settings
        .read()
        .scan
        .roots
        .first()
        .cloned()
        .unwrap_or_default();

    ui.horizontal(|ui| {
        if ui.button("← Back").clicked() {
            app.navigate(Screen::AllSongs);
        }
        ui.heading("Missing from destination");
    });
    ui.label(
        egui::RichText::new(format!(
            "Source: {} → Destination: {}",
            if source_root.is_empty() { "(unset)" } else { source_root.as_str() },
            if dest_root.is_empty() { "(unset)" } else { dest_root.as_str() },
        ))
        .weak(),
    );
    ui.separator();

    if source_root.is_empty() || dest_root.is_empty() {
        ui.label("Set Source root and Destination root in Settings → Library paths first.");
        return;
    }

    let missing = app.missing();
    let count = missing.len();
    let sync_running = SYNC_RUNNING.load(Ordering::Relaxed);

    ui.horizontal(|ui| {
        let label = if sync_running {
            format!("Copying… ({count} pending)")
        } else if count == 0 {
            "Nothing to copy".to_string()
        } else {
            format!("Copy all {count} missing from source")
        };
        let enabled = !sync_running && count > 0;
        if ui
            .add_enabled(enabled, egui::Button::new(label))
            .on_hover_text(
                "Direct file copy from the source root to the destination root, mirroring the \
                 folder layout. No YouTube, no transcode.",
            )
            .clicked()
        {
            spawn_copy_all(missing.clone(), app.library.clone());
        }
        if ui.button("Refresh").clicked() {
            app.cached_missing = None;
        }
        ui.separator();
        let renumber_running = RENUMBER_RUNNING.load(Ordering::Relaxed);
        let threshold = app.settings.read().renumber.threshold;
        let renumber_label = if renumber_running {
            "Renumbering…".to_string()
        } else {
            "Renumber all folders".to_string()
        };
        if ui
            .add_enabled(!renumber_running, egui::Button::new(renumber_label))
            .on_hover_text(
                "For every destination folder, renumber the prefixed audio files so the digits \
                 are contiguous (1, 2, 3 …). Folders where fewer than the threshold of files \
                 carry a digit prefix are skipped.",
            )
            .clicked()
        {
            spawn_renumber_all(app.library.clone(), threshold);
        }
        ui.separator();
        ui.label(format!("{count} file{}", if count == 1 { "" } else { "s" }));
    });

    ui.add_space(6.0);
    ui.separator();

    if count == 0 {
        ui.label("Destination has every source file. Nothing to do.");
        return;
    }

    let mut by_folder: BTreeMap<String, Vec<(PathBuf, PathBuf)>> = BTreeMap::new();
    for (src, dest) in missing.iter() {
        let folder = src
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("(root)")
            .to_string();
        by_folder.entry(folder).or_default().push((src.clone(), dest.clone()));
    }

    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        for (folder, items) in &by_folder {
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(format!("{folder}  ({} missing)", items.len()))
                    .strong()
                    .size(14.0),
            );
            ui.separator();
            for (src, dest) in items {
                ui.horizontal(|ui| {
                    let filename = src
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("?");
                    ui.add(egui::Label::new(filename).truncate());
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            let copy_one_enabled = !SYNC_RUNNING.load(Ordering::Relaxed);
                            if ui
                                .add_enabled(copy_one_enabled, egui::Button::new("Copy"))
                                .on_hover_text("Copy just this file")
                                .clicked()
                            {
                                spawn_copy_one(
                                    src.clone(),
                                    dest.clone(),
                                    app.library.clone(),
                                );
                            }
                        },
                    );
                });
            }
        }
    });
}

fn spawn_copy_all(
    pairs: Arc<Vec<(PathBuf, PathBuf)>>,
    library: Arc<crate::data::Library>,
) {
    SYNC_RUNNING.store(true, Ordering::Relaxed);
    let flag = SYNC_RUNNING.clone();
    std::thread::spawn(move || {
        let mut copied = 0usize;
        let mut failed = 0usize;
        let mut touched_folders: std::collections::HashSet<PathBuf> =
            std::collections::HashSet::new();
        for (src, dest) in pairs.iter() {
            if dest.exists() {
                continue;
            }
            match sync_files::copy_file(src, dest) {
                Ok(()) => {
                    copied += 1;
                    if let Some(parent) = dest.parent() {
                        touched_folders.insert(parent.to_path_buf());
                    }
                }
                Err(e) => {
                    failed += 1;
                    tracing::warn!("copy failed: {e:#}");
                }
            }
        }
        for folder in &touched_folders {
            if let Err(e) = library.refresh_folder(folder) {
                tracing::warn!("refresh after copy failed: {e:#}");
            }
        }
        tracing::info!("source→dest copy: {copied} ok, {failed} failed");
        flag.store(false, Ordering::Relaxed);
    });
}

fn spawn_renumber_all(library: Arc<crate::data::Library>, threshold: f32) {
    RENUMBER_RUNNING.store(true, Ordering::Relaxed);
    let flag = RENUMBER_RUNNING.clone();
    std::thread::spawn(move || {
        let folders = library.folders();
        let mut renamed_total = 0usize;
        let mut folders_changed = 0usize;
        let mut failed = 0usize;
        for folder in &folders {
            match renumberer::renumber_folder(folder, threshold) {
                Ok(0) => {}
                Ok(n) => {
                    renamed_total += n;
                    folders_changed += 1;
                    if let Err(e) = library.refresh_folder(folder) {
                        tracing::warn!("refresh after renumber failed: {e:#}");
                    }
                }
                Err(e) => {
                    failed += 1;
                    tracing::warn!("renumber failed for {}: {e:#}", folder.display());
                }
            }
        }
        tracing::info!(
            "renumber all: {renamed_total} files renamed across {folders_changed} folder(s), {failed} failed"
        );
        flag.store(false, Ordering::Relaxed);
    });
}

fn spawn_copy_one(src: PathBuf, dest: PathBuf, library: Arc<crate::data::Library>) {
    SYNC_RUNNING.store(true, Ordering::Relaxed);
    let flag = SYNC_RUNNING.clone();
    std::thread::spawn(move || {
        match sync_files::copy_file(&src, &dest) {
            Ok(()) => {
                if let Some(parent) = dest.parent() {
                    if let Err(e) = library.refresh_folder(parent) {
                        tracing::warn!("refresh after copy failed: {e:#}");
                    }
                }
            }
            Err(e) => tracing::warn!("copy failed: {e:#}"),
        }
        flag.store(false, Ordering::Relaxed);
    });
}
