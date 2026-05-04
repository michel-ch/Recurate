use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use crate::domain::Screen;
use crate::playback::deletion;
use crate::renumberer;
use crate::ui::screens::library::handle_delete;
use crate::ui::App;

/// Pure helper: derive the visible row label `<folder>  /  <filename>` for a
/// path. Extracted so the duplicates rendering can be unit-tested without an
/// egui context — the bug report ("different songs name but the same source
/// song") hinges on each row showing its *own* path, never a sibling's.
pub(crate) fn row_label(path: &Path) -> String {
    let folder = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("?");
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("?");
    format!("{folder}  /  {filename}")
}

pub fn draw(ui: &mut egui::Ui, app: &mut App) {
    let running = app.fingerprint_running.load(Ordering::Relaxed);
    let total = app.fingerprint_total.load(Ordering::Relaxed);
    let done = app.fingerprint_progress.load(Ordering::Relaxed);
    let song_count = app.library.song_count();

    let any_unfingerprinted = {
        let fps = app.fingerprints.read();
        let failed = app.fingerprint_failed.read();
        app.library.read_songs(|all| {
            all.iter()
                .any(|s| !fps.contains_key(&s.id) && !failed.contains(&s.id))
        })
    };

    let needs_start = !running && song_count > 0 && any_unfingerprinted;
    if needs_start {
        app.start_fingerprinting(false);
    }

    // Bulk-delete plan: per-group, sort by filename DESC and keep the first
    // (lexicographically-largest filename, e.g. `70 - foo.mp3` over
    // `23 - foo.mp3`); every other file in the group gets deleted.
    let groups_for_header = app.duplicates();
    let bulk_delete_ids: Vec<i64> = groups_for_header
        .iter()
        .flat_map(|g| {
            let mut sorted = g.songs.clone();
            sorted.sort_by(|a, b| {
                b.path
                    .file_name()
                    .cmp(&a.path.file_name())
                    .then_with(|| b.path.cmp(&a.path))
            });
            sorted.into_iter().skip(1).map(|s| s.id).collect::<Vec<_>>()
        })
        .collect();
    let bulk_count = bulk_delete_ids.len();

    let confirm_id = egui::Id::new("dup_bulk_delete_confirm");
    let confirming =
        ui.ctx()
            .memory(|m| m.data.get_temp::<bool>(confirm_id).unwrap_or(false));

    let mut bulk_delete_now = false;

    ui.horizontal(|ui| {
        if ui.button("← Back").clicked() {
            app.navigate(Screen::AllSongs);
        }
        ui.heading("Duplicates by audio fingerprint");
        ui.separator();
        let total_groups = groups_for_header.len();
        let total_dupes: usize = groups_for_header.iter().map(|g| g.songs.len()).sum();
        ui.label(format!(
            "{total_groups} group{}  ·  {total_dupes} file{}",
            if total_groups == 1 { "" } else { "s" },
            if total_dupes == 1 { "" } else { "s" },
        ));
        ui.separator();
        if ui
            .button("Recompute")
            .on_hover_text("Re-fingerprint the entire library from scratch")
            .clicked()
            && !running
        {
            app.start_fingerprinting(true);
        }
        ui.separator();
        if bulk_count > 0 {
            let label = if confirming {
                format!("⚠ Confirm delete {bulk_count} files?")
            } else {
                format!("✕ Delete {bulk_count} extras (keep highest name)")
            };
            let resp = ui.button(label).on_hover_text(
                "For each group, keep the file whose name sorts last (DESC) \
                 and delete the rest. Two-click confirmation.",
            );
            if resp.clicked() {
                if confirming {
                    bulk_delete_now = true;
                    ui.ctx()
                        .memory_mut(|m| m.data.remove::<bool>(confirm_id));
                } else {
                    ui.ctx()
                        .memory_mut(|m| m.data.insert_temp(confirm_id, true));
                }
            }
        }
    });

    ui.separator();

    if running || (total > 0 && done < total) {
        let pct = if total == 0 {
            0.0
        } else {
            done as f32 / total as f32
        };
        ui.add(egui::ProgressBar::new(pct).text(format!("Fingerprinting {done} / {total}")));
        ui.label(
            egui::RichText::new(
                "Decoding the first 30s of each track for a 128-bit amplitude-envelope hash. \
                 Songs are grouped only when both the fingerprint AND duration (rounded to a \
                 second) match. Groups update live as fingerprints arrive.",
            )
            .weak(),
        );
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(300));
    } else if total == 0 {
        ui.label("Library is empty — nothing to fingerprint.");
        return;
    } else {
        ui.label(
            egui::RichText::new(format!("Fingerprinted {total} files.")).weak(),
        );
    }
    ui.separator();

    let groups = app.duplicates();
    if groups.is_empty() {
        ui.label("No duplicates found.");
        return;
    }

    let mut delete_request: Option<i64> = None;
    let mut play_request: Option<crate::domain::Song> = None;

    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        for group in groups.iter() {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(format!(
                    "fp {:016x}{:016x}{:016x}{:016x}  ·  {}s  ·  {} copies",
                    group.fingerprint[0],
                    group.fingerprint[1],
                    group.fingerprint[2],
                    group.fingerprint[3],
                    group.duration_secs,
                    group.songs.len()
                ))
                .strong()
                .size(14.0),
            );
            ui.separator();

            for song in &group.songs {
                // Composite push_id: (group.fingerprint, song.id). Defensive
                // against the (extremely unlikely) case where two rows in a
                // group end up with the same id from a path-hash collision —
                // egui needs a unique scope per row so button clicks route
                // to the row that was actually clicked, not a sibling.
                let row_id = (group.fingerprint, song.id);
                ui.push_id(row_id, |ui| {
                    ui.horizontal(|ui| {
                        let label = row_label(&song.path);
                        let full_path = song.path.to_string_lossy().to_string();
                        // Tooltip shows the full path so the user can verify
                        // two rows really are distinct files when the visible
                        // label is truncated to fit the row width.
                        ui.add(egui::Label::new(label).truncate())
                            .on_hover_text(full_path);
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui
                                    .small_button("✕ Delete")
                                    .on_hover_text("Delete this file")
                                    .clicked()
                                {
                                    delete_request = Some(song.id);
                                }
                                if ui
                                    .small_button("▶ Play")
                                    .on_hover_text("Play this file")
                                    .clicked()
                                {
                                    play_request = Some(song.clone());
                                }
                                ui.label(song.formatted_duration());
                                ui.separator();
                                ui.label(if song.artist.is_empty() {
                                    "—"
                                } else {
                                    song.artist.as_str()
                                });
                            },
                        );
                    });
                });
            }
            ui.add_space(4.0);
        }
    });

    if let Some(song) = play_request {
        app.playback.play_songs(vec![song], 0, None);
    }
    if let Some(id) = delete_request {
        handle_delete(app, id);
    }
    if bulk_delete_now {
        // Per-file renumber would refresh each affected folder mid-loop, which
        // re-derives song IDs from the new (renumbered) paths. Subsequent ids
        // in `bulk_delete_ids` would then resolve to nothing in the library and
        // their files would silently survive on disk — the bug that made this
        // button look like it left duplicates behind. Delete with renumber off,
        // then run a single renumber + refresh per affected folder at the end.
        let library = app.playback.library().clone();
        let mut affected_folders: HashSet<PathBuf> = HashSet::new();
        let mut deleted = 0usize;
        let mut skipped = 0usize;
        for group in groups_for_header.iter() {
            let mut sorted = group.songs.clone();
            sorted.sort_by(|a, b| {
                b.path
                    .file_name()
                    .cmp(&a.path.file_name())
                    .then_with(|| b.path.cmp(&a.path))
            });
            for song in sorted.into_iter().skip(1) {
                if let Some(folder) = song.path.parent() {
                    affected_folders.insert(folder.to_path_buf());
                }
                app.playback.remove_from_queue(song.id);
                match deletion::delete_song(&library, song.id, false, 0.0) {
                    Ok(_) => deleted += 1,
                    Err(e) => {
                        skipped += 1;
                        tracing::warn!(
                            "bulk delete skipped {} (id={}): {e:#}",
                            song.path.display(),
                            song.id
                        );
                    }
                }
            }
        }
        let renumber_enabled = app.settings.read().renumber.enabled;
        let threshold = app.settings.read().renumber.threshold;
        let mut renumber_failures = 0usize;
        if renumber_enabled {
            for folder in &affected_folders {
                match renumberer::renumber_folder(folder, threshold) {
                    Ok(0) => {}
                    Ok(_) => {
                        if let Err(e) = library.refresh_folder(folder) {
                            renumber_failures += 1;
                            tracing::warn!(
                                "refresh_folder after bulk renumber failed ({}): {e:#}",
                                folder.display()
                            );
                        }
                    }
                    Err(e) => {
                        renumber_failures += 1;
                        tracing::warn!(
                            "renumber failed for {}: {e:#}",
                            folder.display()
                        );
                    }
                }
            }
        }
        tracing::info!(
            "bulk-deleted {}/{} duplicate extras across {} folders",
            deleted,
            bulk_delete_ids.len(),
            affected_folders.len()
        );
        if deleted > 0 {
            app.toast_info(format!(
                "Deleted {deleted} duplicate{} across {} folder{}",
                if deleted == 1 { "" } else { "s" },
                affected_folders.len(),
                if affected_folders.len() == 1 { "" } else { "s" },
            ));
        }
        if skipped > 0 {
            app.toast_warn(format!(
                "{skipped} duplicate{} could not be deleted (see log for paths)",
                if skipped == 1 { "" } else { "s" }
            ));
        }
        if renumber_failures > 0 {
            app.toast_warn(format!(
                "{renumber_failures} folder{} failed to renumber after delete",
                if renumber_failures == 1 { "" } else { "s" }
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn row_label_uses_per_path_folder_and_filename() {
        // Regression for the user's "different songs name but the same source
        // song as different songs in the duplicates page interface" report:
        // each row's label MUST be derived from that row's own path, not from
        // a sibling. If a closure ever captures `group.songs[0]` instead of
        // the per-iteration song, this test would flag the equality.
        let a = PathBuf::from("/m/AlbumA/01 - first.mp3");
        let b = PathBuf::from("/m/AlbumB/02 - second.mp3");
        assert_ne!(row_label(&a), row_label(&b));
        assert_eq!(row_label(&a), "AlbumA  /  01 - first.mp3");
        assert_eq!(row_label(&b), "AlbumB  /  02 - second.mp3");
    }

    #[test]
    fn row_label_handles_path_without_parent() {
        // Defensive: a path with no parent folder (root-level file) must still
        // render something readable rather than panicking.
        let p = PathBuf::from("loose.mp3");
        let label = row_label(&p);
        assert!(label.contains("loose.mp3"));
    }

    #[test]
    fn row_label_handles_unicode_filenames() {
        // The dataset includes full-width Unicode and CJK; row_label must
        // pass them through unchanged.
        let p = PathBuf::from("/m/AK/＂Slowed＂ ｜ track.mp3");
        let label = row_label(&p);
        assert!(label.contains("＂Slowed＂"));
        assert!(label.contains("AK"));
    }
}
