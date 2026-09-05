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

fn draw_editor(ui: &mut egui::Ui, _app: &mut App, folder: &PathBuf) {
    ui.heading(folder.file_name().and_then(|s| s.to_str()).unwrap_or("?"));
    ui.label(egui::RichText::new(folder.display().to_string()).weak());
    ui.separator();
    ui.label("(add songs and reorder come in the next tasks)");
    let _ = renumberer::next_index(folder);
}
