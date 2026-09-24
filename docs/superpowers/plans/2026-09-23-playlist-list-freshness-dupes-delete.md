# Playlist List Freshness, Duplicate Prompt, Delete Button — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On the Playlists hub, the song list always reflects disk (including mid-batch and while reordering), adding a song that already exists in the playlist asks the user before downloading, and each row has a delete button.

**Architecture:** Three small changes to `player/src/ui/screens/playlists.rs` plus two pure helpers with unit tests. (1) Replace the "freeze while dirty" rule with a merge: on library change, drop vanished ids and append new songs, keeping the user's relative order. (2) Insert a confirmation step between resolve and enqueue: `poll_batch` parks resolved items in `pending_batch` when any match an existing title+artist, and a modal window lets the user skip or keep them. (3) A per-row ✕ that calls the existing `library::handle_delete`, disabled while the order is dirty or a batch is running.

**Tech Stack:** Rust, egui 0.28 (`egui::Window` modal), existing `Library`, `renumberer`, `DownloadWorker`.

**Assumptions (stated, not asked):**
- "Already present" = same title and artist, case-insensitive, as a song already in that folder, or as another line in the same paste. Video URLs are not stored on disk, so they can't be used.
- Deleting from a playlist deletes the file (same as the Folders page) and renumbers. No confirmation dialog, matching the Folders page.
- The user must Apply or Revert pending order edits before deleting; otherwise the renumber would invalidate every id in the working copy.

---

## File map

| File | Change |
|---|---|
| `player/src/ui/screens/playlists.rs` | `merge_order` helper + on-disk id cache; `pending_batch` state + modal; ✕ per row. |
| `player/src/data/scanner.rs` | Skip dot-prefixed files in `scan_dir` (verify first; only if not already). |
| `player/src/ui/screens/library.rs` | No change; `handle_delete` is already `pub`. |
| `AGENTS.md` (and `CLAUDE.md` on disk) | Extend constraint 18. |

All `cargo` commands run from `player/`.

---

### Task 1: Song list always reflects disk (merge instead of freeze)

**Files:** modify `player/src/ui/screens/playlists.rs`, possibly `player/src/data/scanner.rs`.

- [ ] **Step 1: Check whether the scanner lists in-flight dot-files**

Run: `grep -n "starts_with('.')\|file_name" src/data/scanner.rs`
If `scan_dir` does not skip names starting with `.`, add this inside its per-entry loop right after the path is obtained (match the file's existing style):

```rust
if path
    .file_name()
    .and_then(|s| s.to_str())
    .map_or(false, |n| n.starts_with('.'))
{
    continue;
}
```

Reason: the download worker calls `refresh_folder` per file; without this, `.01 - X.dl.mp3` transcodes would flash in every song list.

- [ ] **Step 2: Write the failing unit test for the merge helper**

Append to `playlists.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn song(id: i64, name: &str) -> Song {
        Song {
            id,
            title: name.into(),
            artist: String::new(),
            album: String::new(),
            album_artist: String::new(),
            duration: Duration::ZERO,
            year: None,
            genre: None,
            composer: None,
            track_no: None,
            path: PathBuf::from(format!("C:/pl/{name}.mp3")),
            has_embedded_art: false,
        }
    }

    #[test]
    fn merge_keeps_user_order_drops_missing_appends_new() {
        let current = vec![song(3, "c"), song(1, "a"), song(2, "b")];
        let on_disk = vec![song(1, "a"), song(2, "b"), song(4, "d")];
        let merged = merge_order(current, &on_disk);
        let ids: Vec<i64> = merged.iter().map(|s| s.id).collect();
        assert_eq!(ids, vec![1, 2, 4]);
    }

    #[test]
    fn merge_with_no_changes_is_identity() {
        let current = vec![song(2, "b"), song(1, "a")];
        let on_disk = vec![song(1, "a"), song(2, "b")];
        let ids: Vec<i64> = merge_order(current, &on_disk).iter().map(|s| s.id).collect();
        assert_eq!(ids, vec![2, 1]);
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test merge_` → compile error, `merge_order` not found.

- [ ] **Step 4: Implement the helper and the cache, replace the freeze**

Add fields to `PlaylistsUi`:

```rust
    /// `(library_version, folder, ids sorted by filename)` — refreshed only
    /// when the library version changes, so `is_dirty` doesn't filter the
    /// whole library every frame.
    pub on_disk_cache: Option<(u64, PathBuf, Vec<i64>)>,
```

Add the helper near `is_dirty`:

```rust
/// Reconcile the user's working order with what is on disk: songs that
/// vanished are dropped, songs that appeared are appended (in filename
/// order), everything else keeps its current relative position.
fn merge_order(current: Vec<Song>, on_disk: &[Song]) -> Vec<Song> {
    let live: std::collections::HashSet<i64> = on_disk.iter().map(|s| s.id).collect();
    let mut merged: Vec<Song> = current.into_iter().filter(|s| live.contains(&s.id)).collect();
    let have: std::collections::HashSet<i64> = merged.iter().map(|s| s.id).collect();
    let mut new: Vec<&Song> = on_disk.iter().filter(|s| !have.contains(&s.id)).collect();
    new.sort_by(|a, b| a.path.file_name().cmp(&b.path.file_name()));
    merged.extend(new.into_iter().cloned());
    merged
}

fn on_disk_songs(app: &App, folder: &PathBuf) -> Vec<Song> {
    let mut v = app.library.songs_in_folder(folder);
    v.sort_by(|a, b| a.path.file_name().cmp(&b.path.file_name()));
    v
}
```

Replace the top of `draw_reorder` (the block from `let version = ...` through the closing `}` of the reload `if`) with:

```rust
    let version = app.library.version();
    let folder_changed = app.playlists.order_folder.as_ref() != Some(folder);
    if folder_changed {
        app.playlists.order = on_disk_songs(app, folder);
        app.playlists.order_version = version;
        app.playlists.order_folder = Some(folder.clone());
        app.playlists.move_target = 1;
    } else if app.playlists.order_version != version {
        // Disk changed (download landed, delete, external edit): merge so
        // new songs show up immediately and unapplied edits survive.
        let on_disk = on_disk_songs(app, folder);
        let current = std::mem::take(&mut app.playlists.order);
        app.playlists.order = merge_order(current, &on_disk);
        app.playlists.order_version = version;
    }
```

Replace `is_dirty` with the cached version:

```rust
fn is_dirty(app: &mut App, folder: &PathBuf) -> bool {
    if app.playlists.order_folder.as_ref() != Some(folder) {
        return false;
    }
    let version = app.library.version();
    let stale = match &app.playlists.on_disk_cache {
        Some((v, f, _)) => *v != version || f != folder,
        None => true,
    };
    if stale {
        let ids: Vec<i64> = on_disk_songs(app, folder).iter().map(|s| s.id).collect();
        app.playlists.on_disk_cache = Some((version, folder.clone(), ids));
    }
    let ids = &app.playlists.on_disk_cache.as_ref().unwrap().2;
    ids.iter().copied().ne(app.playlists.order.iter().map(|s| s.id))
}
```

`is_dirty` now takes `&mut App`; its two call sites already have `app: &mut App`, so no other change.

Also make the "Revert" button work with the merge model: it already sets `order_folder = None`, which triggers the `folder_changed` reload. Keep it.

- [ ] **Step 5: Verify**

Run: `cargo test merge_` → 2 passed. `cargo check --all-targets` clean. `cargo test` all pass.
Manual: with the app open on a playlist, drop an mp3 into that folder from Explorer and click any other page then back (or wait for a batch download to land): the song appears at the end of the list without losing reorder edits.

- [ ] **Step 6: Commit**

```bash
git add src/ui/screens/playlists.rs src/data/scanner.rs
git commit -m "fix(playlists): merge disk changes into the order list instead of freezing it"
```

---

### Task 2: Ask before adding a song already in the playlist

**Files:** modify `player/src/ui/screens/playlists.rs`.

- [ ] **Step 1: Write the failing unit test for duplicate detection**

Add inside the `tests` module from Task 1:

```rust
    use crate::replacer::resolve::Resolved;

    fn resolved(title: &str, artist: &str) -> Resolved {
        Resolved { title: title.into(), artist: artist.into(), video_url: "u".into() }
    }

    #[test]
    fn flags_case_insensitive_matches_against_folder_and_within_batch() {
        let mut existing = song(1, "01 - death bed - Powfu");
        existing.title = "death bed".into();
        existing.artist = "Powfu".into();
        let items = vec![
            resolved("Death Bed", "powfu"), // in folder
            resolved("New One", "X"),       // fresh
            resolved("new one", "x"),       // dup of previous line
        ];
        assert_eq!(flag_duplicates(&items, &[existing]), vec![true, false, true]);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test flags_case` → compile error, `flag_duplicates` not found.

- [ ] **Step 3: Implement detection, pending state, and the modal**

Add to `PlaylistsUi`:

```rust
    /// Resolved items waiting for the user's answer to "N already exist".
    pub pending_batch: Option<PendingBatch>,
```

Add the types + helper near the top of the file:

```rust
/// A resolved paste whose enqueue is on hold until the user decides what to
/// do with the entries that already exist in the playlist.
pub struct PendingBatch {
    pub folder: PathBuf,
    pub items: Vec<(crate::replacer::resolve::Resolved, bool)>, // (item, is_duplicate)
    pub outcomes: Vec<crate::replacer::resolve::LineOutcome>,
}

fn norm(s: &str) -> String {
    s.trim().to_lowercase()
}

/// One flag per resolved item: true if the same (title, artist) already
/// exists in the folder or earlier in this same batch.
fn flag_duplicates(items: &[crate::replacer::resolve::Resolved], existing: &[Song]) -> Vec<bool> {
    let mut seen: std::collections::HashSet<(String, String)> = existing
        .iter()
        .map(|s| (norm(&s.title), norm(&s.artist)))
        .collect();
    items
        .iter()
        .map(|r| !seen.insert((norm(&r.title), norm(&r.artist))))
        .collect()
}
```

In `poll_batch`, replace the body of `if let Some(outcomes) = finished { ... }` so it **only resolves and classifies**; the enqueue moves to a new function:

```rust
    let finished = app.playlists.resolve.outcomes.lock().take();
    if let Some(outcomes) = finished {
        let Some(folder) = app.playlists.batch_folder.clone() else {
            return;
        };
        let items: Vec<crate::replacer::resolve::Resolved> = outcomes
            .iter()
            .filter_map(|o| o.result.as_ref().ok())
            .flat_map(|v| v.iter().cloned())
            .collect();
        let failed = outcomes.iter().filter(|o| o.result.is_err()).count();
        if failed > 0 {
            app.toast_warn(format!("{failed} line(s) could not be resolved — see list below"));
        }
        let existing = app.library.songs_in_folder(&folder);
        let flags = flag_duplicates(&items, &existing);
        let dup_count = flags.iter().filter(|d| **d).count();
        let batch = PendingBatch {
            folder,
            items: items.into_iter().zip(flags).collect(),
            outcomes,
        };
        if dup_count == 0 {
            enqueue_batch(app, batch, false);
        } else {
            app.playlists.pending_batch = Some(batch);
        }
    }
```

Add `enqueue_batch` (this is the old enqueue loop, parameterised):

```rust
/// Queue a resolved batch into the download worker. `skip_duplicates`
/// drops items flagged as already present.
fn enqueue_batch(app: &mut App, batch: PendingBatch, skip_duplicates: bool) {
    use crate::data::scanner::song_id_from_path;
    use crate::replacer::download_worker::DownloadRequest;
    use crate::replacer::playlist::sanitize_filename;
    use crate::ui::screens::replacer::download_worker;

    let cookies_browser = {
        let s = app.settings.read().replacer.cookies_browser.trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    };
    let folder = &batch.folder;
    let dl = download_worker(app.library.clone()).clone();
    let (mut next, pad) = renumberer::next_index(folder);
    let mut queued = 0usize;
    for (r, is_dup) in &batch.items {
        if skip_duplicates && *is_dup {
            continue;
        }
        let stem = format!("{next:0pad$} - {} - {}", r.title, r.artist);
        let dest = folder.join(format!("{}.mp3", sanitize_filename(&stem)));
        let id = song_id_from_path(&dest);
        dl.enqueue(DownloadRequest {
            song_id: id,
            source_path: dest.clone(),
            dest_path: dest,
            video_url: r.video_url.clone(),
            cookies_browser: cookies_browser.clone(),
        });
        app.playlists.queued_ids.insert(id);
        next += 1;
        queued += 1;
    }
    app.playlists.last_outcomes = batch.outcomes;
    if queued > 0 {
        app.toast_info(format!("Queued {queued} download(s) into {}", folder.display()));
    } else {
        app.playlists.batch_folder = None;
    }
}
```

Remove the now-unused `cookies_browser` binding and the three `use` lines at the top of `poll_batch` (`song_id_from_path`, `DownloadRequest`, `sanitize_filename`, `download_worker`), keeping `DownloadState` and `download_worker` if the bookkeeping block below still uses them.

Add the modal, drawn from `draw` right after `poll_batch(ui.ctx(), app);`:

```rust
    draw_duplicate_prompt(ui.ctx(), app);
```

```rust
/// Modal shown when a paste resolved to songs already in the playlist.
fn draw_duplicate_prompt(ctx: &egui::Context, app: &mut App) {
    let Some(batch) = app.playlists.pending_batch.as_ref() else {
        return;
    };
    let dups: Vec<String> = batch
        .items
        .iter()
        .filter(|(_, d)| *d)
        .map(|(r, _)| format!("{} — {}", r.title, r.artist))
        .collect();
    let total = batch.items.len();
    let folder_name = batch
        .folder
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("?")
        .to_string();

    let mut decision: Option<Option<bool>> = None; // Some(Some(skip)) or Some(None)=cancel
    egui::Window::new("Already in playlist")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(format!(
                "{} of {total} song(s) already exist in \"{folder_name}\":",
                dups.len()
            ));
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for d in &dups {
                    ui.label(format!("• {d}"));
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Skip duplicates").clicked() {
                    decision = Some(Some(true));
                }
                if ui.button("Download anyway").clicked() {
                    decision = Some(Some(false));
                }
                if ui.button("Cancel").clicked() {
                    decision = Some(None);
                }
            });
        });

    match decision {
        Some(Some(skip)) => {
            let batch = app.playlists.pending_batch.take().unwrap();
            enqueue_batch(app, batch, skip);
        }
        Some(None) => {
            let batch = app.playlists.pending_batch.take().unwrap();
            app.playlists.last_outcomes = batch.outcomes;
            app.playlists.batch_folder = None;
            app.toast_info("Nothing queued");
        }
        None => {}
    }
}
```

Also keep the Find button disabled while a prompt is open: in `draw_add_songs`, extend `batch_in_flight` to `!app.playlists.queued_ids.is_empty() || app.playlists.pending_batch.is_some()`.

- [ ] **Step 4: Verify**

Run: `cargo test flags_case` → 1 passed. `cargo check --all-targets` clean (no unused imports). `cargo test` all pass.
Manual: paste a title that is already in the selected playlist → modal lists it; **Skip duplicates** queues nothing and clears; **Download anyway** downloads it with the next number; pasting two fresh lines shows no modal.

- [ ] **Step 5: Commit**

```bash
git add src/ui/screens/playlists.rs
git commit -m "feat(playlists): ask before adding songs already in the playlist"
```

---

### Task 3: Delete-from-playlist button on each row

**Files:** modify `player/src/ui/screens/playlists.rs`.

- [ ] **Step 1: Add the button to the row's right-to-left cluster**

In `draw_reorder`, before the `egui::ScrollArea` add:

```rust
    let mut delete_id: Option<i64> = None;
    let can_delete = !dirty && !batch_here;
```

Inside the `right_to_left` cluster, as the **first** widget (so it sits at the far right), add:

```rust
                            if ui
                                .add_enabled(can_delete, egui::Button::new("✕").small())
                                .on_hover_text(if can_delete {
                                    "Delete this file from the playlist folder and renumber"
                                } else if dirty {
                                    "Apply or revert the order first"
                                } else {
                                    "Downloads into this playlist are still running"
                                })
                                .clicked()
                            {
                                delete_id = Some(*song_id);
                            }
```

After the `drop_at.or(mv)` block at the end of `draw_reorder`, add:

```rust
    if let Some(id) = delete_id {
        crate::ui::screens::library::handle_delete(app, id);
        // handle_delete renumbers + refreshes; ids changed, so reload from disk.
        app.playlists.order_folder = None;
    }
```

`handle_delete` already removes the song from the playback queue, deletes, renumbers when the setting is on, refreshes the folder, and toasts the result (constraint 13).

Widen the title reservation so the extra button fits: change `- 260.0` to `- 290.0` in the `title_w` computation.

- [ ] **Step 2: Verify**

Run: `cargo check --all-targets` clean, `cargo test` all pass.
Manual: click ✕ on a track → file gone from the folder, remaining files renumbered, list updated, toast shown. With unapplied edits the button is disabled with the hover reason.

- [ ] **Step 3: Commit**

```bash
git add src/ui/screens/playlists.rs
git commit -m "feat(playlists): delete a song from the playlist row"
```

---

### Task 4: Docs

**Files:** `AGENTS.md`, `CLAUDE.md` (on disk; gitignored), `README.md`.

- [ ] **Step 1: Extend constraint 18** (both files), append to the paragraph:

```markdown
The order list is a **merge**, not a snapshot: `merge_order` drops vanished ids and appends new songs on every `library.version()` bump, so downloads and deletes appear immediately even with unapplied edits; `is_dirty` compares against a cached `(version, folder, ids)` triple. Before enqueueing, `flag_duplicates` compares each resolved `(title, artist)` case-insensitively against the folder's songs and the batch itself; any hit parks the batch in `PlaylistsUi.pending_batch` behind a modal (Skip / Download anyway / Cancel) — never auto-skip, the user decides. Row ✕ routes through `library::handle_delete` and is disabled while the order is dirty (the renumber would invalidate the working copy's ids) or a batch is in flight.
```

- [ ] **Step 2: README** — in the `### Playlists` section add two bullets:

```markdown
- **Duplicates** — if a pasted line resolves to a song already in the playlist
  (same title and artist), a dialog lists them and lets you skip or download
  anyway.
- **Delete** — ✕ on a row deletes the file and renumbers the folder. Apply or
  revert pending order edits first.
```

- [ ] **Step 3: Commit**

```bash
git add README.md AGENTS.md
git commit -m "docs: playlist list merge, duplicate prompt, row delete"
```

---

## Self-review

- **Coverage:** list always updated (Task 1, incl. mid-batch via scanner dot-file skip and merge-while-dirty); ask on duplicate (Task 2, modal with three outcomes); delete button (Task 3); docs (Task 4).
- **Placeholders:** none.
- **Type consistency:** `merge_order(Vec<Song>, &[Song]) -> Vec<Song>`, `on_disk_songs(&App, &PathBuf)`, `is_dirty(&mut App, &PathBuf)`, `flag_duplicates(&[Resolved], &[Song]) -> Vec<bool>`, `PendingBatch { folder, items: Vec<(Resolved, bool)>, outcomes }`, `enqueue_batch(&mut App, PendingBatch, bool)` used consistently across Tasks 1–3.
- **Scope:** three tightly related changes to one screen; one plan is right.
