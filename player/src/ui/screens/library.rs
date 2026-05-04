use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::domain::{Screen, Song, SortOption};
use crate::playback::deletion;
use crate::ui::components::song_row;
use crate::ui::App;

pub fn draw(ui: &mut egui::Ui, app: &mut App) {
    draw_header(ui, app, "All Songs");
    draw_sort_picker(ui, app);
    ui.separator();

    let current_id = app.playback.state_snapshot().current_song.as_ref().map(|s| s.id);
    let row_h = ui.spacing().interact_size.y + 4.0;

    let songs = app.library_view();
    let total = songs.len();

    let page_count = if total == 0 {
        1
    } else {
        (total + crate::ui::app::PAGE_SIZE - 1) / crate::ui::app::PAGE_SIZE
    };
    if app.songs_page >= page_count {
        app.songs_page = page_count.saturating_sub(1);
    }
    let page = app.songs_page;
    let start = page * crate::ui::app::PAGE_SIZE;
    let end = ((page + 1) * crate::ui::app::PAGE_SIZE).min(total);

    ui.horizontal(|ui| {
        if ui
            .add_enabled(page > 0, egui::Button::new("◀ Prev"))
            .clicked()
        {
            app.songs_page = page.saturating_sub(1);
        }
        let range_label = if total == 0 {
            "0 of 0".to_string()
        } else {
            format!("{}–{} of {total}", start + 1, end)
        };
        ui.label(format!(
            "Page {} / {page_count}  ({range_label})",
            page + 1
        ));
        if ui
            .add_enabled(page + 1 < page_count, egui::Button::new("Next ▶"))
            .clicked()
        {
            app.songs_page = page + 1;
        }
    });
    ui.separator();

    let mut play_idx: Option<usize> = None;
    let mut delete_request: Option<i64> = None;

    let page_total = end - start;
    egui::ScrollArea::vertical().auto_shrink([false; 2]).show_rows(
        ui,
        row_h,
        page_total,
        |ui, range| {
            for local_i in range {
                let i = start + local_i;
                let song = &songs[i];
                ui.horizontal(|ui| {
                    let row = song_row::draw(ui, i, song, current_id == Some(song.id));
                    if row.clicked {
                        play_idx = Some(i);
                    }
                    if ui.small_button("✕").on_hover_text("Delete").clicked() {
                        delete_request = Some(song.id);
                    }
                });
            }
        },
    );

    if let Some(idx) = play_idx {
        app.playback.play_songs((*songs).clone(), idx, None);
    }
    if let Some(id) = delete_request {
        handle_delete(app, id);
    }
}

pub fn draw_search(ui: &mut egui::Ui, app: &mut App) {
    draw_header(ui, app, "Search");
    let row_h = ui.spacing().interact_size.y + 4.0;
    let songs = app.library_view();
    let total = songs.len();

    let mut play_idx: Option<usize> = None;

    egui::ScrollArea::vertical().auto_shrink([false; 2]).show_rows(
        ui,
        row_h,
        total,
        |ui, range| {
            for i in range {
                let song = &songs[i];
                let row = song_row::draw(ui, i, song, false);
                if row.clicked {
                    play_idx = Some(i);
                }
            }
        },
    );

    if let Some(idx) = play_idx {
        app.playback.play_songs((*songs).clone(), idx, None);
    }
}

pub fn draw_albums(ui: &mut egui::Ui, app: &mut App) {
    draw_header(ui, app, "Albums");
    let songs = app.library.songs_snapshot();
    let mut by_album: BTreeMap<String, usize> = BTreeMap::new();
    for s in &songs {
        *by_album.entry(s.album.clone()).or_default() += 1;
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (album, count) in &by_album {
            ui.horizontal(|ui| {
                if ui.link(format!("{album}")).clicked() {
                    app.navigate(Screen::AlbumDetail(album.clone()));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{count} songs"));
                });
            });
        }
    });
}

pub fn draw_album_detail(ui: &mut egui::Ui, app: &mut App, album: &str) {
    draw_header(ui, app, &format!("Album: {album}"));
    let mut songs: Vec<Song> = app
        .library
        .songs_snapshot()
        .into_iter()
        .filter(|s| s.album == album)
        .collect();
    songs.sort_by_key(|s| s.track_no.unwrap_or(i32::MAX));
    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut play_request: Option<(Vec<Song>, usize)> = None;
        for (i, song) in songs.iter().enumerate() {
            let row = song_row::draw(ui, i, song, false);
            if row.clicked {
                play_request = Some((songs.clone(), i));
            }
        }
        if let Some((list, idx)) = play_request {
            app.playback.play_songs(list, idx, None);
        }
    });
}

pub fn draw_artists(ui: &mut egui::Ui, app: &mut App) {
    draw_header(ui, app, "Artists");
    let songs = app.library.songs_snapshot();
    let mut by_artist: BTreeMap<String, usize> = BTreeMap::new();
    for s in &songs {
        *by_artist.entry(s.artist.clone()).or_default() += 1;
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (artist, count) in &by_artist {
            ui.horizontal(|ui| {
                if ui.link(artist).clicked() {
                    app.navigate(Screen::ArtistDetail(artist.clone()));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{count} songs"));
                });
            });
        }
    });
}

pub fn draw_artist_detail(ui: &mut egui::Ui, app: &mut App, artist: &str) {
    draw_header(ui, app, &format!("Artist: {artist}"));
    let songs: Vec<Song> = app
        .library
        .songs_snapshot()
        .into_iter()
        .filter(|s| s.artist == artist)
        .collect();
    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut play_request: Option<(Vec<Song>, usize)> = None;
        for (i, song) in songs.iter().enumerate() {
            let row = song_row::draw(ui, i, song, false);
            if row.clicked {
                play_request = Some((songs.clone(), i));
            }
        }
        if let Some((list, idx)) = play_request {
            app.playback.play_songs(list, idx, None);
        }
    });
}

pub fn draw_folders(ui: &mut egui::Ui, app: &mut App) {
    draw_header(ui, app, "Folders");
    let folders: Vec<PathBuf> = app.library.folders();
    egui::ScrollArea::vertical().show(ui, |ui| {
        for folder in &folders {
            let songs = app.library.songs_in_folder(folder);
            ui.collapsing(folder.display().to_string(), |ui| {
                let mut play_request: Option<(Vec<Song>, usize)> = None;
                let mut delete_request: Option<i64> = None;
                for (i, song) in songs.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let row = song_row::draw(ui, i, song, false);
                        if row.clicked {
                            play_request = Some((songs.clone(), i));
                        }
                        if ui.small_button("✕").clicked() {
                            delete_request = Some(song.id);
                        }
                    });
                }
                if let Some((list, idx)) = play_request {
                    app.playback.play_songs(list, idx, None);
                }
                if let Some(id) = delete_request {
                    handle_delete(app, id);
                }
            });
        }
    });
}

pub fn draw_playlists(ui: &mut egui::Ui, _app: &mut App) {
    ui.heading("Playlists");
    ui.label("(playlists not implemented in MVP)");
}

fn draw_header(ui: &mut egui::Ui, app: &mut App, title: &str) {
    ui.horizontal(|ui| {
        ui.heading(title);
        ui.separator();
        ui.label("Search:");
        ui.add(egui::TextEdit::singleline(&mut app.search_query).desired_width(200.0));
    });
}

fn draw_sort_picker(ui: &mut egui::Ui, app: &mut App) {
    ui.horizontal(|ui| {
        ui.label("Sort:");
        let current = app.sort;
        egui::ComboBox::from_id_source("sort_combo")
            .selected_text(current.label())
            .show_ui(ui, |ui| {
                for &opt in SortOption::ALL {
                    if ui
                        .selectable_label(current == opt, opt.label())
                        .clicked()
                    {
                        app.sort = opt;
                    }
                }
            });
    });
}

pub fn handle_delete(app: &mut App, song_id: i64) {
    let renumber = app.settings.read().renumber.enabled;
    let threshold = app.settings.read().renumber.threshold;
    let library = app.playback.library().clone();
    app.playback.remove_from_queue(song_id);
    match deletion::delete_song(&library, song_id, renumber, threshold) {
        Ok(r) => {
            tracing::info!(
                "deleted {} (renumbered {})",
                r.deleted_path.display(),
                r.renumbered
            );
            let name = r
                .deleted_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("track")
                .to_string();
            app.toast_info(if r.renumbered > 0 {
                format!("Deleted {name} · renumbered {} files", r.renumbered)
            } else {
                format!("Deleted {name}")
            });
        }
        Err(e) => {
            tracing::warn!("delete failed: {e:#}");
            app.toast_error(format!("Delete failed: {e}"));
        }
    }
}
