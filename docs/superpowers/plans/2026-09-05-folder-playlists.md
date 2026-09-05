# Folder Playlists Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Treat every destination folder as a playlist: create one, add songs by pasting links or "artist song" titles, download + auto-number them, and reorder tracks (drag, position input, first/last) with the order written to the `NN - ` filename prefix.

**Architecture:** A new **Playlists** hub screen (replacing the placeholder `Screen::Playlists`) with a folder list on the left and an editor on the right. Song resolution (link → title/artist, title → best YouTube audio match) runs on a background thread using the existing `fetch_playlist`, `search_ytdlp`, `clean_title`, and `score_results`. Downloads go through the existing shared `DownloadWorker`; files are named `<next NN> - Title - Artist.mp3` and the folder is renumbered once the batch finishes. Reordering builds a `RenumberPlan` from an explicit order and reuses the renumberer's two-phase rename.

**Tech Stack:** Rust, egui 0.28 (`Ui::dnd_drag_source` / `Ui::dnd_drop_zone` for drag), yt-dlp + ffmpeg via `std::process::Command`, `tempfile` for tests.

**Decisions already made with the user:**
- Title lines auto-pick the top scored audio match; no confirmation step. Failures are listed per line.
- Order is persisted by renaming files (the `NN - ` prefix). No sidecar files.
- Everything lives in one Playlists hub. The existing Playlist-link page stays as-is for "whole playlist → new folder".

**Conventions to respect (from `CLAUDE.md`):** no per-frame filesystem calls or subprocess probes (cache them); toast every user-triggered failure; keep `ctx.wants_keyboard_input` gating; don't route bulk operations through `handle_delete`; reuse `download_worker()` from `screens/replacer.rs`.

---

## File map

| File | Responsibility |
|---|---|
| Create `player/src/replacer/link_list.rs` | Parse pasted text into `Vec<LineItem>` (URL vs title). Pure, unit-tested. |
| Create `player/src/replacer/resolve.rs` | Background resolver: `LineItem` → `Resolved { title, artist, video_url }` using yt-dlp. |
| Modify `player/src/replacer/mod.rs` | Register the two modules. |
| Modify `player/src/renumberer.rs` | Add `plan_order(folder, ordered) -> RenumberPlan` and `next_index(folder) -> (usize, usize)`. |
| Modify `player/tests/renumberer.rs` | Integration tests for `plan_order` + `apply`. |
| Create `player/src/ui/screens/playlists.rs` | Playlists hub: folder list, new playlist, add-songs paste box, reorder editor. Holds `PlaylistsUi` state. |
| Modify `player/src/ui/screens/mod.rs` | Register `playlists`. |
| Modify `player/src/ui/screens/library.rs:250-253` | Delete the placeholder `draw_playlists`. |
| Modify `player/src/ui/app.rs` | Add `pub playlists: PlaylistsUi`; route `Screen::Playlists` to the new screen. |
| Modify `player/src/ui/components/top_bar.rs` | Add a `Playlists` nav button. |
| Modify `README.md`, `CLAUDE.md`, `AGENTS.md` | Document the hub and constraint 18. |

All `cargo` commands run from `player/`.

---

### Task 1: Parse pasted lines into links and titles

**Files:**
- Create: `player/src/replacer/link_list.rs`
- Modify: `player/src/replacer/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `player/src/replacer/link_list.rs`:

```rust
//! Turn a pasted block of text (one entry per line) into download intents.
//! A line that parses as a YouTube URL is a link; anything else is a free
//! text title such as `Powfu death bed`.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineItem {
    Link(String),
    Title(String),
}

impl LineItem {
    pub fn text(&self) -> &str {
        match self {
            LineItem::Link(s) | LineItem::Title(s) => s,
        }
    }
}

pub fn parse_lines(text: &str) -> Vec<LineItem> {
    todo!()
}

pub fn is_youtube_url(s: &str) -> bool {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_links_and_titles_and_skips_blanks() {
        let text = "https://www.youtube.com/watch?v=abc123\n\n  Powfu death bed  \nyoutu.be/xyz\n";
        assert_eq!(
            parse_lines(text),
            vec![
                LineItem::Link("https://www.youtube.com/watch?v=abc123".into()),
                LineItem::Title("Powfu death bed".into()),
                LineItem::Link("https://youtu.be/xyz".into()),
            ]
        );
    }

    #[test]
    fn recognises_youtube_hosts_only() {
        assert!(is_youtube_url("https://music.youtube.com/watch?v=a"));
        assert!(is_youtube_url("https://www.youtube.com/playlist?list=PL1"));
        assert!(is_youtube_url("https://youtube.com/shorts/a"));
        assert!(!is_youtube_url("https://example.com/watch?v=a"));
        assert!(!is_youtube_url("Powfu - death bed"));
    }

    #[test]
    fn dedupes_identical_lines() {
        let text = "a song\na song\nhttps://youtu.be/x\nhttps://youtu.be/x";
        assert_eq!(parse_lines(text).len(), 2);
    }
}
```

Add to `player/src/replacer/mod.rs` after `pub mod download_worker;`:

```rust
pub mod link_list;
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test link_list`
Expected: 3 failures panicking on `not yet implemented`.

- [ ] **Step 3: Implement**

Replace the two `todo!()` functions:

```rust
pub fn parse_lines(text: &str) -> Vec<LineItem> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let item = if is_youtube_url(line) {
            let url = if line.starts_with("http://") || line.starts_with("https://") {
                line.to_string()
            } else {
                format!("https://{line}")
            };
            LineItem::Link(url)
        } else {
            LineItem::Title(line.to_string())
        };
        if seen.insert(item.clone()) {
            out.push(item);
        }
    }
    out
}

pub fn is_youtube_url(s: &str) -> bool {
    let s = s.trim().to_ascii_lowercase();
    let rest = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(&s);
    let host = rest.split('/').next().unwrap_or("");
    matches!(
        host,
        "youtube.com" | "www.youtube.com" | "m.youtube.com" | "music.youtube.com" | "youtu.be"
    )
}
```

`LineItem` needs `Hash`: change its derive to `#[derive(Debug, Clone, PartialEq, Eq, Hash)]`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test link_list`
Expected: `3 passed`.

- [ ] **Step 5: Commit**

```bash
git add src/replacer/link_list.rs src/replacer/mod.rs
git commit -m "feat(replacer): parse pasted lines into links and titles"
```

---

### Task 2: Renumberer — explicit order and next index

**Files:**
- Modify: `player/src/renumberer.rs` (append after `renumber_folder`)
- Modify: `player/tests/renumberer.rs` (append)

- [ ] **Step 1: Write the failing integration tests**

Append to `player/tests/renumberer.rs` (the file already imports `recurate::renumberer` and uses `tempfile`; keep its existing helper style):

```rust
#[test]
fn plan_order_renames_to_requested_positions() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["01 - A - X.mp3", "02 - B - X.mp3", "03 - C - X.mp3"] {
        std::fs::write(dir.path().join(name), b"x").unwrap();
    }
    let ordered = vec![
        dir.path().join("03 - C - X.mp3"),
        dir.path().join("01 - A - X.mp3"),
        dir.path().join("02 - B - X.mp3"),
    ];
    let plan = recurate::renumberer::plan_order(dir.path(), &ordered).unwrap();
    assert_eq!(plan.changes(), 3);
    recurate::renumberer::apply(&plan).unwrap();
    let mut names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    assert_eq!(names, vec!["01 - C - X.mp3", "02 - A - X.mp3", "03 - B - X.mp3"]);
}

#[test]
fn plan_order_rejects_paths_outside_folder() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("01 - A - X.mp3"), b"x").unwrap();
    let bogus = vec![dir.path().join("nope.mp3")];
    assert!(recurate::renumberer::plan_order(dir.path(), &bogus).is_err());
}

#[test]
fn next_index_counts_audio_files_and_pad() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(recurate::renumberer::next_index(dir.path()), (1, 2));
    for i in 1..=9 {
        std::fs::write(dir.path().join(format!("0{i} - T - A.mp3")), b"x").unwrap();
    }
    assert_eq!(recurate::renumberer::next_index(dir.path()), (10, 2));
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --test renumberer`
Expected: compile error, `plan_order` and `next_index` not found.

- [ ] **Step 3: Implement**

Append to `player/src/renumberer.rs`:

```rust
/// Build a plan that numbers `ordered` 1..N in the given order. Every path
/// must be an existing file directly inside `folder`; files in the folder
/// that are *not* listed are appended after the listed ones in their
/// current filename order so nothing silently loses its prefix.
pub fn plan_order(folder: &Path, ordered: &[PathBuf]) -> Result<RenumberPlan> {
    let mut on_disk: Vec<PathBuf> = Vec::new();
    for e in std::fs::read_dir(folder).with_context(|| format!("read_dir {}", folder.display()))? {
        let p = match e {
            Ok(v) => v.path(),
            Err(_) => continue,
        };
        let ext = p
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();
        if p.is_file() && SUPPORTED.contains(&ext.as_str()) {
            on_disk.push(p);
        }
    }
    on_disk.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    let mut sequence: Vec<PathBuf> = Vec::with_capacity(on_disk.len());
    for p in ordered {
        if !on_disk.iter().any(|d| d == p) {
            return Err(anyhow!("{} is not an audio file in {}", p.display(), folder.display()));
        }
        if !sequence.contains(p) {
            sequence.push(p.clone());
        }
    }
    for p in on_disk {
        if !sequence.contains(&p) {
            sequence.push(p);
        }
    }

    let pad_width = compute_pad_width(sequence.len()).max(2);
    let mut pairs = Vec::with_capacity(sequence.len());
    for (i, path) in sequence.iter().enumerate() {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("track");
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("mp3");
        let (rest, old_idx) = match split_prefix(stem) {
            Some((digits, rest)) => (rest.to_string(), digits.parse::<i32>().unwrap_or(0)),
            None => (stem.to_string(), 0),
        };
        let new_idx = (i + 1) as i32;
        pairs.push(RenamePair {
            from: path.clone(),
            to: folder.join(format!("{new_idx:0pad_width$} - {rest}.{ext}")),
            old_index: old_idx,
            new_index: new_idx,
        });
    }
    Ok(RenumberPlan {
        pairs,
        skipped_no_prefix: 0,
        total_audio: sequence.len(),
    })
}

/// `(next track number, pad width)` for appending a new file to `folder`.
/// Pad is at least 2 so a fresh playlist starts at `01`.
pub fn next_index(folder: &Path) -> (usize, usize) {
    let count = std::fs::read_dir(folder)
        .map(|rd| {
            rd.flatten()
                .filter(|e| {
                    let p = e.path();
                    let ext = p
                        .extension()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_ascii_lowercase())
                        .unwrap_or_default();
                    p.is_file() && SUPPORTED.contains(&ext.as_str())
                })
                .count()
        })
        .unwrap_or(0);
    (count + 1, compute_pad_width(count + 1).max(2))
}
```

Note: `apply` already does a two-phase temp rename, so a swap like `01↔03` is safe.

- [ ] **Step 4: Run tests**

Run: `cargo test --test renumberer`
Expected: all pass, including the 5 pre-existing ones.

- [ ] **Step 5: Commit**

```bash
git add src/renumberer.rs tests/renumberer.rs
git commit -m "feat(renumberer): plan_order for explicit track order, next_index for appends"
```

---

### Task 3: Background resolver (link/title → title, artist, url)

**Files:**
- Create: `player/src/replacer/resolve.rs`
- Modify: `player/src/replacer/mod.rs`
- Modify: `player/src/replacer/playlist.rs` (make `split_artist_title` `pub`)

- [ ] **Step 1: Make the title splitter reusable**

In `player/src/replacer/playlist.rs` change `fn split_artist_title(` to `pub fn split_artist_title(`.

- [ ] **Step 2: Write the failing unit test**

Create `player/src/replacer/resolve.rs`:

```rust
//! Resolve pasted lines into concrete downloads on a background thread.
//!
//! * `Link` → one `yt-dlp --flat-playlist --dump-single-json` call via
//!   `playlist::fetch_playlist`; a playlist URL expands into all its entries.
//! * `Title` → `search_ytdlp` (10 results) scored by `score_results` with the
//!   cleaned title; the top audio-only candidate wins. An empty result set is
//!   a hard failure for that line (no fallback to music videos, see
//!   CLAUDE.md constraint 9).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use parking_lot::Mutex;

use crate::replacer::link_list::LineItem;
use crate::replacer::playlist::{fetch_playlist, split_artist_title};
use crate::replacer::scoring::score_results;
use crate::replacer::title_cleaner::clean_title;
use crate::replacer::youtube::{search_ytdlp, YtVideo};

#[derive(Debug, Clone)]
pub struct Resolved {
    pub title: String,
    pub artist: String,
    pub video_url: String,
}

#[derive(Debug, Clone)]
pub struct LineOutcome {
    pub line: LineItem,
    pub result: Result<Vec<Resolved>, String>,
}

#[derive(Default)]
pub struct ResolveJob {
    pub running: Arc<AtomicBool>,
    pub outcomes: Arc<Mutex<Option<Vec<LineOutcome>>>>,
}

impl ResolveJob {
    pub fn start(&self, lines: Vec<LineItem>, cookies_browser: Option<String>) {
        self.running.store(true, Ordering::Relaxed);
        let running = self.running.clone();
        let outcomes = self.outcomes.clone();
        std::thread::spawn(move || {
            let out: Vec<LineOutcome> = lines
                .into_iter()
                .map(|line| {
                    let result = resolve_line(&line, cookies_browser.as_deref())
                        .map_err(|e| format!("{e:#}"));
                    LineOutcome { line, result }
                })
                .collect();
            *outcomes.lock() = Some(out);
            running.store(false, Ordering::Relaxed);
        });
    }
}

fn resolve_line(line: &LineItem, cookies: Option<&str>) -> Result<Vec<Resolved>> {
    match line {
        LineItem::Link(url) => {
            let pl = fetch_playlist(url, cookies)?;
            let out: Vec<Resolved> = pl
                .entries
                .iter()
                .flatten()
                .filter(|e| !e.id.is_empty() && !e.title.is_empty())
                .map(|e| {
                    let channel = if e.channel.is_empty() { &e.uploader } else { &e.channel };
                    let (artist, title) = split_artist_title(&e.title, channel);
                    Resolved { title, artist, video_url: e.watch_url() }
                })
                .collect();
            if out.is_empty() {
                return Err(anyhow!("no downloadable video at {url}"));
            }
            Ok(out)
        }
        LineItem::Title(text) => {
            let videos = search_ytdlp(&format!("{text} audio"), 10, cookies)?;
            pick_best(text, &videos).map(|r| vec![r])
        }
    }
}

pub fn pick_best(text: &str, videos: &[YtVideo]) -> Result<Resolved> {
    let cleaned = clean_title(text);
    let scored = score_results(&cleaned, None, videos);
    let top = scored
        .first()
        .ok_or_else(|| anyhow!("no audio-only match for \"{text}\""))?;
    let (artist, title) = split_artist_title(&top.video.title, top.video.channel_or_uploader());
    Ok(Resolved { title, artist, video_url: top.video.watch_url() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vid(title: &str, channel: &str) -> YtVideo {
        YtVideo {
            id: "id".into(),
            title: title.into(),
            channel: channel.into(),
            uploader: String::new(),
            duration: Some(180.0),
            url: String::new(),
            view_count: None,
        }
    }

    #[test]
    fn pick_best_prefers_topic_audio_over_music_video() {
        let videos = vec![
            vid("Powfu - death bed (Official Music Video)", "Powfu"),
            vid("death bed (coffee for your head)", "Powfu - Topic"),
        ];
        let r = pick_best("Powfu death bed", &videos).unwrap();
        assert_eq!(r.artist, "Powfu");
        assert_eq!(r.title, "death bed (coffee for your head)");
    }

    #[test]
    fn pick_best_errors_when_only_videos_exist() {
        let videos = vec![vid("Powfu - death bed (Official Music Video)", "Powfu")];
        assert!(pick_best("Powfu death bed", &videos).is_err());
    }
}
```

Add to `player/src/replacer/mod.rs` after `pub mod playlist;`:

```rust
pub mod resolve;
```

- [ ] **Step 3: Run tests**

Run: `cargo test resolve`
Expected: `2 passed`. If `pick_best_prefers_topic_audio_over_music_video` fails on the title, check `YtVideo` field names against `player/src/replacer/youtube.rs` and adjust the test constructor, not the implementation.

- [ ] **Step 4: Commit**

```bash
git add src/replacer/resolve.rs src/replacer/mod.rs src/replacer/playlist.rs
git commit -m "feat(replacer): background resolver for pasted links and titles"
```

---

### Task 4: Playlists hub skeleton — folder list and New playlist

**Files:**
- Create: `player/src/ui/screens/playlists.rs`
- Modify: `player/src/ui/screens/mod.rs`
- Modify: `player/src/ui/screens/library.rs:250-253` (delete `draw_playlists`)
- Modify: `player/src/ui/app.rs`
- Modify: `player/src/ui/components/top_bar.rs`

- [ ] **Step 1: Create the screen with state struct and left panel**

Create `player/src/ui/screens/playlists.rs`:

```rust
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
                ui.selectable_label(true, format!("{label} (empty)"));
            }
        }
    });
}

fn draw_editor(ui: &mut egui::Ui, app: &mut App, folder: &PathBuf) {
    ui.heading(folder.file_name().and_then(|s| s.to_str()).unwrap_or("?"));
    ui.label(egui::RichText::new(folder.display().to_string()).weak());
    ui.separator();
    ui.label("(add songs and reorder come in the next tasks)");
    let _ = renumberer::next_index(folder);
}
```

- [ ] **Step 2: Wire the module, state, route, and nav**

`player/src/ui/screens/mod.rs`: add `pub mod playlists;` after `pub mod playlist;`.

`player/src/ui/screens/library.rs`: delete the whole `pub fn draw_playlists` function (lines ~250-253).

`player/src/ui/app.rs`:
- add `use crate::ui::screens::playlists::PlaylistsUi;` next to the `PlaylistUi` import;
- add field `pub playlists: PlaylistsUi,` after `pub playlist: PlaylistUi,`;
- init with `playlists: PlaylistsUi::default(),` after `playlist: PlaylistUi::default(),`;
- change the route line to `Screen::Playlists => screens::playlists::draw(ui, self),`.

`player/src/ui/components/top_bar.rs`: after the `"Folders"` nav button add
`nav_button(ui, app, "Playlists", Screen::Playlists);`.

`player/src/domain/model.rs`: in `shows_bottom_nav`, leave `Screen::Playlists` out of the exclusion list (the mini-player should stay visible while editing).

- [ ] **Step 3: Check and run**

Run: `cargo check --all-targets && cargo test`
Expected: clean check, all tests pass.

Run: `cargo run --release`, click **Playlists**, type a name, click **＋ New**. Expected: a folder appears under the destination root and shows as selected with "(empty)". Close the app.

- [ ] **Step 4: Commit**

```bash
git add src/ui/screens/playlists.rs src/ui/screens/mod.rs src/ui/screens/library.rs src/ui/app.rs src/ui/components/top_bar.rs
git commit -m "feat(ui): Playlists hub with folder list and new-playlist creation"
```

---

### Task 5: Add songs by pasting links or titles

**Files:**
- Modify: `player/src/ui/screens/playlists.rs` (replace `draw_editor`)

- [ ] **Step 1: Implement the paste box, resolve, and download queueing**

Replace `draw_editor` in `player/src/ui/screens/playlists.rs` with:

```rust
fn draw_editor(ui: &mut egui::Ui, app: &mut App, folder: &PathBuf) {
    ui.heading(folder.file_name().and_then(|s| s.to_str()).unwrap_or("?"));
    ui.label(egui::RichText::new(folder.display().to_string()).weak());
    ui.separator();

    draw_add_songs(ui, app, folder);
    ui.separator();
    draw_reorder(ui, app, folder);
}

fn draw_add_songs(ui: &mut egui::Ui, app: &mut App, folder: &PathBuf) {
    use crate::data::scanner::song_id_from_path;
    use crate::replacer::download_worker::{DownloadRequest, DownloadState};
    use crate::replacer::link_list::parse_lines;
    use crate::replacer::playlist::sanitize_filename;
    use crate::ui::screens::replacer::download_worker;

    ui.collapsing("Add songs", |ui| {
        ui.label(
            "One per line: a YouTube video or playlist link, or a title like \
             \"Powfu death bed\". Titles are matched to the best audio-only result.",
        );
        ui.add(
            egui::TextEdit::multiline(&mut app.playlists.paste_text)
                .desired_rows(5)
                .desired_width(f32::INFINITY)
                .hint_text("https://youtu.be/…\nArtist song name\n…"),
        );

        let resolving = app.playlists.resolve.running.load(std::sync::atomic::Ordering::Relaxed);
        if resolving {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
        }
        let cookies_browser = {
            let s = app.settings.read().replacer.cookies_browser.trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        };

        ui.horizontal(|ui| {
            let lines = parse_lines(&app.playlists.paste_text);
            let can = !resolving && !lines.is_empty() && crate::replacer::youtube::ytdlp_available();
            if ui
                .add_enabled(can, egui::Button::new(format!("Find & download {} line(s)", lines.len())))
                .clicked()
            {
                app.playlists.last_outcomes.clear();
                app.playlists.resolve.start(lines, cookies_browser.clone());
            }
            if resolving {
                ui.spinner();
                ui.label(egui::RichText::new("resolving…").weak());
            }
            if ui.button("Clear").clicked() {
                app.playlists.paste_text.clear();
                app.playlists.last_outcomes.clear();
            }
        });

        // Resolver finished: queue downloads for every resolved item.
        let finished = app.playlists.resolve.outcomes.lock().take();
        if let Some(outcomes) = finished {
            let dl = download_worker(app.library.clone()).clone();
            let (mut next, pad) = renumberer::next_index(folder);
            let mut queued = 0usize;
            let mut failed = 0usize;
            for o in &outcomes {
                match &o.result {
                    Ok(items) => {
                        for r in items {
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
                    }
                    Err(_) => failed += 1,
                }
            }
            app.playlists.last_outcomes = outcomes;
            if queued > 0 {
                app.toast_info(format!("Queued {queued} download(s) into {}", folder.display()));
            }
            if failed > 0 {
                app.toast_warn(format!("{failed} line(s) could not be resolved — see list below"));
            }
        }

        // Batch bookkeeping: once nothing we queued is still pending, renumber
        // the folder so a partial failure leaves no gap (03 missing → 01,02,03).
        if !app.playlists.queued_ids.is_empty() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
            let dl = download_worker(app.library.clone()).clone();
            let (pending, done, failed) = dl.read_states(|s| {
                let mut p = 0; let mut d = 0; let mut f = 0;
                for id in &app.playlists.queued_ids {
                    match s.get(id) {
                        Some(DownloadState::Pending) => p += 1,
                        Some(DownloadState::Done) => d += 1,
                        Some(DownloadState::Failed { .. }) => f += 1,
                        _ => {}
                    }
                }
                (p, d, f)
            });
            ui.horizontal(|ui| {
                ui.label(format!("Downloading: {pending} pending, {done} done, {failed} failed"));
                let paused = dl.is_paused();
                if ui.button(if paused { "▶ Resume" } else { "⏸ Pause" }).clicked() {
                    dl.set_paused(!paused);
                }
            });
            if pending == 0 {
                let threshold = app.settings.read().renumber.threshold;
                match renumberer::renumber_folder(folder, threshold) {
                    Ok(n) => {
                        if let Err(e) = app.library.refresh_folder(folder) {
                            tracing::warn!("refresh after batch failed: {e:#}");
                        }
                        app.toast_info(format!("{done} added · renumbered {n} file(s)"));
                    }
                    Err(e) => app.toast_error(format!("Renumber failed: {e}")),
                }
                app.playlists.queued_ids.clear();
            }
        }

        // Per-line outcome list (failures first so they are visible).
        if !app.playlists.last_outcomes.is_empty() {
            ui.separator();
            for o in &app.playlists.last_outcomes {
                match &o.result {
                    Ok(items) => {
                        ui.colored_label(
                            egui::Color32::from_rgb(120, 200, 120),
                            format!("✓ {} → {} track(s)", o.line.text(), items.len()),
                        );
                    }
                    Err(e) => {
                        ui.colored_label(
                            egui::Color32::from_rgb(220, 120, 120),
                            format!("✗ {} — {e}", o.line.text()),
                        )
                        .on_hover_text("Fix the line and run again; successful lines were already queued.");
                    }
                }
            }
        }
    });
}

fn draw_reorder(ui: &mut egui::Ui, _app: &mut App, _folder: &PathBuf) {
    ui.label("(reorder comes in the next task)");
}
```

- [ ] **Step 2: Check, then manual test**

Run: `cargo check --all-targets`
Expected: clean.

Run the app, open Playlists, select a folder, paste one `https://youtu.be/…` link and one title such as `Powfu death bed`, click **Find & download 2 line(s)**. Expected: after resolving, a toast "Queued 2 download(s)", the pending/done counter, then "2 added · renumbered N file(s)". The folder now has the two files as `NN - Title - Artist.mp3` and they show under Songs.

Also paste a nonsense title like `zzzz qqqq 12345 asdf`. Expected: red ✗ line with "no audio-only match", no download.

- [ ] **Step 3: Commit**

```bash
git add src/ui/screens/playlists.rs
git commit -m "feat(ui): add songs to a playlist by pasting links or titles"
```

---

### Task 6: Reorder editor — move buttons, position input, first/last, drag

**Files:**
- Modify: `player/src/ui/screens/playlists.rs` (replace `draw_reorder`)

- [ ] **Step 1: Implement**

Replace `draw_reorder` with:

```rust
fn draw_reorder(ui: &mut egui::Ui, app: &mut App, folder: &PathBuf) {
    // Refresh the working copy when the folder or library changes, but never
    // while the user has unapplied edits (dirty = order differs from disk).
    let version = app.library.version();
    let folder_changed = app.playlists.order_folder.as_ref() != Some(folder);
    if folder_changed || (app.playlists.order_version != version && !is_dirty(app, folder)) {
        app.playlists.order = app.library.songs_in_folder(folder);
        app.playlists.order.sort_by(|a, b| a.path.file_name().cmp(&b.path.file_name()));
        app.playlists.order_version = version;
        app.playlists.order_folder = Some(folder.clone());
        app.playlists.move_target = 1;
    }
    let n = app.playlists.order.len();
    if n == 0 {
        ui.label("This playlist is empty. Add songs above.");
        return;
    }
    let dirty = is_dirty(app, folder);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Order").strong());
        ui.label(egui::RichText::new("drag rows, or use the buttons; nothing is renamed until you apply").weak());
        if ui.add_enabled(dirty, egui::Button::new("Apply order (rename files)")).clicked() {
            apply_order(app, folder);
        }
        if ui.add_enabled(dirty, egui::Button::new("Revert")).clicked() {
            app.playlists.order_folder = None; // forces reload next frame
        }
    });

    let row_h = ui.text_style_height(&egui::TextStyle::Body) + 8.0;
    let mut mv: Option<(usize, usize)> = None; // (from, to)
    let mut drop_at: Option<(usize, usize)> = None;

    egui::ScrollArea::vertical().auto_shrink([false; 2]).show_rows(ui, row_h, n, |ui, range| {
        for i in range {
            let song = app.playlists.order[i].clone();
            let id = egui::Id::new(("pl_row", folder, song.id));
            let frame = egui::Frame::none().inner_margin(2.0);
            let (_, dropped) = ui.dnd_drop_zone::<usize, ()>(frame, |ui| {
                ui.horizontal(|ui| {
                    ui.dnd_drag_source(id, i, |ui| {
                        ui.label(egui::RichText::new("☰").weak());
                        ui.label(format!("{:02}", i + 1));
                    });
                    ui.add(egui::Label::new(&song.title).truncate());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("⇲ Last").clicked() { mv = Some((i, n - 1)); }
                        if ui.small_button("⇱ First").clicked() { mv = Some((i, 0)); }
                        if ui.add_enabled(i + 1 < n, egui::Button::new("▼").small()).clicked() { mv = Some((i, i + 1)); }
                        if ui.add_enabled(i > 0, egui::Button::new("▲").small()).clicked() { mv = Some((i, i - 1)); }
                        ui.label(egui::RichText::new(&song.artist).weak());
                    });
                });
            });
            if let Some(from) = dropped {
                drop_at = Some((*from, i));
            }
        }
    });

    ui.horizontal(|ui| {
        ui.label("Move track #");
        let mut from = app.playlists.move_target.clamp(1, n);
        ui.add(egui::DragValue::new(&mut from).clamp_range(1..=n));
        app.playlists.move_target = from;
        ui.label("to position");
        let to_id = ui.make_persistent_id("pl_move_to");
        let mut to: usize = ui.memory(|m| m.data.get_temp(to_id)).unwrap_or(1);
        ui.add(egui::DragValue::new(&mut to).clamp_range(1..=n));
        ui.memory_mut(|m| m.data.insert_temp(to_id, to));
        if ui.button("Move").clicked() {
            mv = Some((from - 1, to - 1));
        }
    });

    if let Some((from, to)) = drop_at.or(mv) {
        if from != to && from < n && to < n {
            let s = app.playlists.order.remove(from);
            app.playlists.order.insert(to, s);
        }
    }
}

fn is_dirty(app: &App, folder: &PathBuf) -> bool {
    if app.playlists.order_folder.as_ref() != Some(folder) {
        return false;
    }
    let mut on_disk = app.library.songs_in_folder(folder);
    on_disk.sort_by(|a, b| a.path.file_name().cmp(&b.path.file_name()));
    on_disk.iter().map(|s| s.id).ne(app.playlists.order.iter().map(|s| s.id))
}

fn apply_order(app: &mut App, folder: &PathBuf) {
    let ordered: Vec<PathBuf> = app.playlists.order.iter().map(|s| s.path.clone()).collect();
    match renumberer::plan_order(folder, &ordered).and_then(|p| renumberer::apply(&p)) {
        Ok(n) => {
            if let Err(e) = app.library.refresh_folder(folder) {
                tracing::warn!("refresh after reorder failed: {e:#}");
            }
            app.playlists.order_folder = None; // reload from disk
            app.toast_info(format!("Renamed {n} file(s)"));
        }
        Err(e) => app.toast_error(format!("Reorder failed: {e}")),
    }
}
```

Performance note: `is_dirty` calls `songs_in_folder` (a filter over the library) up to twice per frame. A folder holds at most a few hundred songs, so this is cheap, but if the Playlists page ever feels laggy, cache the on-disk id list keyed by `library.version()` in `PlaylistsUi`, following the `cached_*` pattern in `app.rs`.

- [ ] **Step 2: Check and manual test**

Run: `cargo check --all-targets`
Expected: clean. If `dnd_drop_zone` signature complains, the egui 0.28 form is `ui.dnd_drop_zone::<Payload, R>(frame, add_contents) -> (InnerResponse<R>, Option<Arc<Payload>>)`; dereference the `Arc<usize>` with `*from` as shown.

Run the app → Playlists → pick a folder with 3+ songs. Verify each of:
1. ▲/▼ swap neighbours; **Apply order** becomes enabled.
2. ⇱ First / ⇲ Last move a row to the ends.
3. "Move track #2 to position 5" + **Move** relocates it.
4. Dragging the ☰ handle onto another row drops it there.
5. **Apply order** renames files; Songs page shows the new numbers; **Apply** is disabled again.
6. **Revert** discards unapplied edits.
7. While a song from this folder is playing, apply an order. Expected: playback continues (the engine holds an open handle; the queue's paths are stale but `next` still works because `refresh_folder` re-derives ids). If `next` skips, note it in the commit message; fixing queue path refresh is out of scope.

- [ ] **Step 3: Commit**

```bash
git add src/ui/screens/playlists.rs
git commit -m "feat(ui): reorder playlist tracks with drag, buttons, and position input"
```

---

### Task 7: Docs and graph

**Files:**
- Modify: `README.md` (add a `### Playlists` section before `### Playlist`)
- Modify: `CLAUDE.md`, `AGENTS.md` (constraint 18)

- [ ] **Step 1: README**

Insert before `### Playlist`:

```markdown
### Playlists

Every folder in the destination root is a playlist. The Playlists hub
lists them on the left (**＋ New** creates an empty folder) and edits the
selected one on the right:

- **Add songs** — paste one entry per line: a YouTube video or playlist
  link, or a plain title such as `Powfu death bed`. Titles are resolved to
  the best audio-only match with the same scoring the Replacer uses; lines
  with no audio-only result are shown in red and skipped. Downloads are
  named `NN - Title - Artist.mp3` continuing the folder's numbering and the
  folder is renumbered once the batch finishes.
- **Order** — drag rows by the ☰ handle, use ▲ ▼ / ⇱ First / ⇲ Last, or
  type a track number and target position. Nothing touches disk until
  **Apply order**, which rewrites the `NN - ` prefixes with the
  renumberer's two-phase rename.
```

- [ ] **Step 2: CLAUDE.md and AGENTS.md**

Append after constraint 17 in both files:

```markdown
18. **Playlists hub = folders; order lives in the filename prefix.** `Screen::Playlists` / `screens/playlists.rs`. Adding songs: `replacer/link_list.rs::parse_lines` classifies pasted lines (YouTube host → `Link`, else `Title`), `replacer/resolve.rs::ResolveJob` resolves them on one background thread (`Link` → `fetch_playlist`, so a playlist URL expands; `Title` → `search_ytdlp` + `score_results` top hit, empty = failure, no video fallback), then the screen queues `DownloadRequest`s into the shared `DownloadWorker` named `<renumberer::next_index> - Title - Artist.mp3` and tracks them in `PlaylistsUi.queued_ids`; when none are `Pending` it runs `renumber_folder` once so failed downloads leave no gap. Reordering mutates `PlaylistsUi.order` (working copy) and only **Apply order** calls `renumberer::plan_order` + `apply` (two-phase temp rename; unlisted files are appended, foreign paths are rejected). Don't write order anywhere else (no sidecar files) — the prefix is the single source of truth the Songs page, the renumberer, and external file managers all agree on.
```

- [ ] **Step 3: Refresh the graph and commit**

Run from the repo root: `graphify update .`

```bash
git add README.md CLAUDE.md AGENTS.md graphify-out
git commit -m "docs: Playlists hub"
```

---

## Self-review

- **Spec coverage:** create playlist (Task 4), paste list of links or titles and download with auto-numbering (Tasks 1, 3, 5), find by link or by title (Task 3), minimum one link (any non-empty parse enables the button), reorder via drag / position input / first / last (Task 6, plus ▲▼), documentation (Task 7).
- **Placeholders:** none; every step has code or an exact command.
- **Type consistency:** `LineItem::{Link, Title}` + `text()` (T1) used in T3/T5; `Resolved { title, artist, video_url }` and `LineOutcome { line, result }` (T3) used in T5; `plan_order(folder, &[PathBuf]) -> Result<RenumberPlan>` and `next_index(folder) -> (usize, usize)` (T2) used in T5/T6; `PlaylistsUi` fields (T4) match every use in T5/T6.
- **Out of scope, deliberately:** confirming title matches, per-song delete inside the hub (Folders page already has it), refreshing the playback queue's paths after a rename.
