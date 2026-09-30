//! Right-click menu shared by every song list, plus the Properties dialog it
//! opens.
//!
//! A row hands its click `Response` to [`show`]; the menu is drawn by egui's
//! `context_menu` and the chosen entry comes back as a [`MenuAction`]. The
//! screen then calls [`perform`] once it no longer holds borrows on `app`
//! (same "collect, then act" shape as `play_idx` / `delete_request`).

use std::path::Path;
use std::time::SystemTime;

use egui::{Response, Ui};

use crate::domain::Song;
use crate::ui::screens::library::handle_delete;
use crate::ui::theme;
use crate::ui::widgets;
use crate::ui::App;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAction {
    Play,
    PlayNext,
    AddToQueue,
    /// Open the Playlists page on the song's folder, scrolled to the song.
    GoToPlaylist,
    ShowInFolder,
    CopyPath,
    Properties,
    /// Rename the file and edit its tags.
    Edit,
    Delete,
}

/// State of the Edit dialog: the working copy of the tags and the file
/// stem, loaded when the dialog opens.
#[derive(Clone, Debug)]
pub struct SongEdit {
    pub song: Song,
    pub stem: String,
    pub tags: crate::data::tags::TagEdit,
}

impl SongEdit {
    pub fn load(song: &Song) -> Self {
        let stem = song
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let tags = crate::data::tags::TagEdit {
            title: song.title.clone(),
            artist: song.artist.clone(),
            album: song.album.clone(),
            album_artist: song.album_artist.clone(),
            year: song.year.map(|y| y.to_string()).unwrap_or_default(),
            genre: song.genre.clone().unwrap_or_default(),
        };
        Self { song: song.clone(), stem, tags }
    }
}

fn short_title(t: &str) -> String {
    if t.chars().count() > 40 {
        format!("{}…", t.chars().take(39).collect::<String>())
    } else {
        t.to_string()
    }
}

/// Attach the menu to `response`. `can_delete` adds the destructive entry
/// (Songs / Folders rows; never the queue, whose ✕ only unqueues).
pub fn show(response: &Response, song: &Song, can_delete: bool) -> Option<MenuAction> {
    let mut picked = None;
    response.context_menu(|ui| {
        ui.set_min_width(180.0);
        let pal = widgets::p(ui);
        ui.label(
            egui::RichText::new(short_title(&song.title))
                .font(theme::semibold(theme::TEXT_CAPTION))
                .color(pal.ink_2),
        );
        ui.separator();
        let mut entry = |ui: &mut Ui, label: &str, action: MenuAction| {
            if ui.button(label).clicked() {
                picked = Some(action);
                ui.close_menu();
            }
        };
        entry(ui, "▶ Play", MenuAction::Play);
        entry(ui, "Play next", MenuAction::PlayNext);
        entry(ui, "Add to queue", MenuAction::AddToQueue);
        ui.separator();
        entry(ui, "Go to playlist", MenuAction::GoToPlaylist);
        entry(ui, "Show in folder", MenuAction::ShowInFolder);
        entry(ui, "Copy full path", MenuAction::CopyPath);
        entry(ui, "Properties…", MenuAction::Properties);
        entry(ui, "Edit name and tags…", MenuAction::Edit);
        if can_delete {
            ui.separator();
            if ui
                .button(egui::RichText::new("Delete file…").color(pal.critical))
                .clicked()
            {
                picked = Some(MenuAction::Delete);
                ui.close_menu();
            }
        }
    });
    picked
}

/// Carry out a menu choice. `context` is the list the row came from, used
/// for `Play` so the queue matches what the user was looking at.
pub fn perform(app: &mut App, action: MenuAction, song: &Song, context: &[Song]) {
    match action {
        MenuAction::Play => {
            let idx = context.iter().position(|s| s.id == song.id).unwrap_or(0);
            let list = if context.is_empty() { vec![song.clone()] } else { context.to_vec() };
            app.playback.play_songs(list, idx, None);
        }
        MenuAction::PlayNext => {
            app.playback.play_next(song.clone());
            app.toast_info(format!("Up next: {}", song.title));
        }
        MenuAction::AddToQueue => {
            app.playback.add_to_queue(song.clone());
            app.toast_info(format!("Queued: {}", song.title));
        }
        MenuAction::GoToPlaylist => reveal_in_playlist(app, song),
        MenuAction::ShowInFolder => {
            if let Err(e) = show_in_folder(&song.path) {
                app.toast_error(format!("Could not open folder: {e}"));
            }
        }
        MenuAction::CopyPath => {
            app.copy_text = Some(song.path.display().to_string());
            app.toast_info("Path copied");
        }
        MenuAction::Properties => {
            app.song_props = Some(SongProps::load(song));
        }
        MenuAction::Edit => {
            app.song_edit = Some(SongEdit::load(song));
        }
        MenuAction::Delete => {
            app.confirm_delete = Some(song.clone());
        }
    }
}

/// Write the tags, then rename the file if the stem changed, and refresh
/// the folder so the library picks up both. A renamed file gets a new id,
/// so it is dropped from the queue.
fn save_edit(app: &mut App, edit: &SongEdit) -> anyhow::Result<()> {
    let path = &edit.song.path;
    crate::data::tags::write_tags(path, &edit.tags)?;
    let stem = edit.stem.trim();
    let old_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if !stem.is_empty() && stem != old_stem {
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("mp3");
        let new_path = path.with_file_name(format!(
            "{}.{ext}",
            crate::replacer::playlist::sanitize_filename(stem)
        ));
        if new_path.exists() {
            anyhow::bail!("{} already exists", new_path.display());
        }
        std::fs::rename(path, &new_path)?;
        app.playback.remove_from_queue(edit.song.id);
    }
    if let Some(folder) = path.parent() {
        app.library.refresh_folder(folder)?;
    }
    Ok(())
}

/// Jump to the Playlists page with the song's folder selected; the order
/// list scrolls to the song and highlights it (`PlaylistsUi.reveal`).
pub fn reveal_in_playlist(app: &mut App, song: &Song) {
    let Some(folder) = song.path.parent() else { return };
    app.playlists.selected = Some(folder.to_path_buf());
    app.playlists.filter.clear();
    app.playlists.reveal = Some(song.id);
    app.navigate(crate::domain::Screen::Playlists);
}

/// Windows Explorer with the file selected; other platforms open the folder.
fn show_in_folder(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn()
            .map(|_| ())
    }
    #[cfg(not(windows))]
    {
        let folder = path.parent().unwrap_or(path);
        std::process::Command::new("xdg-open").arg(folder).spawn().map(|_| ())
    }
}

/// Everything the Properties dialog shows, read once when it opens so the
/// modal never touches the disk per frame.
pub struct SongProps {
    pub song: Song,
    pub rows: Vec<(&'static str, String)>,
}

impl SongProps {
    pub fn load(song: &Song) -> Self {
        let meta = std::fs::metadata(&song.path).ok();
        let file_name = song
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let folder = song
            .path
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let mut rows: Vec<(&'static str, String)> = vec![
            ("File name", file_name),
            ("Folder", folder),
            ("Full path", song.path.display().to_string()),
            (
                "Size",
                meta.as_ref()
                    .map(|m| format_size(m.len()))
                    .unwrap_or_else(|| "file not found".into()),
            ),
            (
                "Modified",
                meta.as_ref()
                    .and_then(|m| m.modified().ok())
                    .map(format_time)
                    .unwrap_or_default(),
            ),
            ("Duration", song.formatted_duration()),
            ("Title", song.title.clone()),
            ("Artist", song.artist.clone()),
            ("Album", song.album.clone()),
        ];
        if !song.album_artist.is_empty() && song.album_artist != song.artist {
            rows.push(("Album artist", song.album_artist.clone()));
        }
        if let Some(n) = song.track_no {
            rows.push(("Track", n.to_string()));
        }
        if let Some(y) = song.year {
            rows.push(("Year", y.to_string()));
        }
        if let Some(g) = &song.genre {
            rows.push(("Genre", g.clone()));
        }
        if let Some(c) = &song.composer {
            rows.push(("Composer", c.clone()));
        }
        rows.push(("Cover art", if song.has_embedded_art { "embedded" } else { "none" }.into()));
        rows.retain(|(_, v)| !v.is_empty());
        Self { song: song.clone(), rows }
    }
}

pub fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b >= KB * KB * KB {
        format!("{:.2} GB ({bytes} bytes)", b / (KB * KB * KB))
    } else if b >= KB * KB {
        format!("{:.2} MB ({bytes} bytes)", b / (KB * KB))
    } else if b >= KB {
        format!("{:.1} KB ({bytes} bytes)", b / KB)
    } else {
        format!("{bytes} bytes")
    }
}

/// Local date and time without pulling in a date crate: civil-from-days
/// (Howard Hinnant's algorithm) applied to the UTC timestamp, then the local
/// offset from the C runtime is not available, so this is UTC and says so.
fn format_time(t: SystemTime) -> String {
    let secs = t
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{s:02} UTC")
}

/// Draw the Properties modal and the delete confirmation, if either is open.
/// Called once per frame from `App::update` after the screens.
pub fn draw_dialogs(ctx: &egui::Context, app: &mut App) {
    if let Some(props) = &app.song_props {
        let mut close = false;
        let mut copy: Option<String> = None;
        widgets::modal(ctx, "song_props", "Properties", 560.0, |ui| {
            let pal = widgets::p(ui);
            egui::Grid::new("song_props_grid")
                .num_columns(2)
                .spacing([theme::SPACE_4, theme::SPACE_2])
                .show(ui, |ui| {
                    for (k, v) in &props.rows {
                        ui.label(egui::RichText::new(*k).color(pal.ink_3).size(theme::TEXT_CAPTION));
                        ui.add(
                            egui::Label::new(egui::RichText::new(v).font(theme::mono(theme::TEXT_CAPTION)))
                                .wrap(),
                        );
                        ui.end_row();
                    }
                });
            ui.add_space(theme::SPACE_4);
            ui.horizontal(|ui| {
                if widgets::secondary_button(ui, true, "Copy full path").clicked() {
                    copy = Some(props.song.path.display().to_string());
                }
                if widgets::secondary_button(ui, true, "Show in folder").clicked() {
                    let _ = show_in_folder(&props.song.path);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::primary_button(ui, true, "Close").clicked() {
                        close = true;
                    }
                });
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if let Some(text) = copy {
            app.copy_text = Some(text);
            app.toast_info("Path copied");
        }
        if close {
            app.song_props = None;
        }
    }

    if let Some(song) = app.confirm_delete.clone() {
        let mut decision: Option<bool> = None;
        widgets::modal(ctx, "song_delete", "Delete file?", 460.0, |ui| {
            let pal = widgets::p(ui);
            ui.label(egui::RichText::new(&song.title).font(theme::semibold(theme::TEXT_BODY)));
            ui.label(
                egui::RichText::new(song.path.display().to_string())
                    .font(theme::mono(theme::TEXT_CAPTION))
                    .color(pal.ink_2),
            );
            ui.add_space(theme::SPACE_2);
            widgets::caption(ui, "The file is removed from disk and the folder is renumbered.");
            ui.add_space(theme::SPACE_4);
            ui.horizontal(|ui| {
                if widgets::danger_button(ui, true, "Delete").clicked() {
                    decision = Some(true);
                }
                if widgets::secondary_button(ui, true, "Cancel").clicked() {
                    decision = Some(false);
                }
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            decision = Some(false);
        }
        match decision {
            Some(true) => {
                app.confirm_delete = None;
                handle_delete(app, song.id);
            }
            Some(false) => app.confirm_delete = None,
            None => {}
        }
    }

    if let Some(mut edit) = app.song_edit.take() {
        let mut decision: Option<bool> = None;
        widgets::modal(ctx, "song_edit", "Edit song", 520.0, |ui| {
            let pal = widgets::p(ui);
            let field = |ui: &mut Ui, label: &str, value: &mut String| {
                ui.label(egui::RichText::new(label).color(pal.ink_3).size(theme::TEXT_CAPTION));
                widgets::text_input(ui, value, "", f32::INFINITY);
                ui.end_row();
            };
            egui::Grid::new("song_edit_grid")
                .num_columns(2)
                .spacing([theme::SPACE_4, theme::SPACE_2])
                .show(ui, |ui| {
                    field(ui, "File name", &mut edit.stem);
                    field(ui, "Title", &mut edit.tags.title);
                    field(ui, "Artist", &mut edit.tags.artist);
                    field(ui, "Album", &mut edit.tags.album);
                    field(ui, "Album artist", &mut edit.tags.album_artist);
                    field(ui, "Year", &mut edit.tags.year);
                    field(ui, "Genre", &mut edit.tags.genre);
                });
            ui.add_space(theme::SPACE_2);
            widgets::caption(
                ui,
                "Tags are written into the file. Renaming keeps the extension; keep the                  NN - prefix if you want the playlist order to stay.",
            );
            ui.add_space(theme::SPACE_4);
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, true, "Save").clicked() {
                    decision = Some(true);
                }
                if widgets::secondary_button(ui, true, "Cancel").clicked() {
                    decision = Some(false);
                }
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            decision = Some(false);
        }
        match decision {
            Some(true) => match save_edit(app, &edit) {
                Ok(()) => app.toast_info("Song saved"),
                Err(e) => {
                    tracing::warn!("edit song failed: {e:#}");
                    app.toast_error(format!("Could not save: {e}"));
                    app.song_edit = Some(edit);
                }
            },
            Some(false) => {}
            None => app.song_edit = Some(edit),
        }
    }

    if let Some(text) = app.copy_text.take() {
        ctx.copy_text(text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_units() {
        assert_eq!(format_size(512), "512 bytes");
        assert_eq!(format_size(2048), "2.0 KB (2048 bytes)");
        assert_eq!(format_size(5 * 1024 * 1024), "5.00 MB (5242880 bytes)");
    }

    #[test]
    fn epoch_formats() {
        assert_eq!(format_time(SystemTime::UNIX_EPOCH), "1970-01-01 00:00:00 UTC");
        let t = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        assert_eq!(format_time(t), "2023-11-14 22:13:20 UTC");
    }
}
