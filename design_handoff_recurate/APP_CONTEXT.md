# Recurate — full app context for UI redesign

Purpose of this file: give a designer (human or Claude Design) everything needed to design a new UI for Recurate without reading the Rust source. It describes what the app is, who uses it, every screen, every control, every piece of state a screen shows, and the constraints the new UI must respect. The current UI is a functional but plain egui (immediate-mode) desktop interface; the redesign is free to change layout, visual language, navigation and grouping, but must keep every listed action reachable.

---

## 1. What Recurate is

Recurate is a **native Windows desktop music player and library curator** for one user's personal MP3 collection. It is written in Rust with egui/eframe, cpal (audio output) and symphonia (decoding). It does three jobs in one window:

1. **Play** a local library of ~2,500 MP3 files organised in flat subfolders (one level deep: `music/<Folder>/<file>.mp3`).
2. **Curate** the library: delete songs, detect acoustic duplicates, keep track-number prefixes contiguous, mirror files between a read-only "source" root and a working "destination" root.
3. **Acquire** audio from YouTube: replace low-quality rips with clean audio-only versions, import whole YouTube playlists, and build folder-playlists from pasted links or "Artist song name" lines. All acquisition goes through `yt-dlp` + `ffmpeg` which must be installed on the machine.

Single user, single machine, offline-first except for YouTube fetches. No accounts, no cloud, no sync.

### Core mental model: a folder is a playlist

Every subfolder of the destination root is a playlist. Track order inside a playlist is encoded in the **filename prefix**: `NN - Title - Artist.mp3` (e.g. `007 - death bed - Powfu.mp3`). There is no database of playlists or order; the filesystem is the single source of truth. Reordering = renaming files. Deleting a song = deleting the file and renumbering the rest so the numbers stay contiguous. Anything that shows a playlist must reflect what is on disk.

### Two roots

- **Source root** (default `music_original/`): read-only catalogue the Replacer enumerates. Never written.
- **Destination root** (default `music/`): what the player plays, what downloads land in, what Playlists shows. Files here mirror the source's per-folder layout.

---

## 2. Domain objects the UI displays

**Song**
- `id` (stable per path), `title`, `artist`, `album`, `album_artist`, `duration` (shown as `m:ss`), `year?`, `genre?`, `composer?`, `track_no?`, `path`, `has_embedded_art`.
- Filenames often carry full-width Unicode punctuation (`＂ ｜ ？ ＃`), stylised math letters (`𝒮𝓁𝑜𝓌𝑒𝒹`), Cyrillic, Chinese, French diacritics, and YouTube boilerplate like `(Official Audio)`. Text must not be assumed ASCII and needs a CJK-capable font fallback.
- Titles can be very long; rows must truncate, never wrap or push trailing controls off-screen.

**PlaybackState**
- `current_song?`, `is_playing`, `current_position_ms`, `duration_ms`, `shuffle_enabled`, `repeat_mode` (Off / All / One).

**Queue**: ordered list of songs plus index of the current one.

**Library status**: Idle / Scanning (with running song count). Two libraries exist: destination (`library`) and source (`source`).

**Folder / playlist**: a directory path; displayed by its last path component.

**SortOption** (Songs page): Title A→Z, Title Z→A, Artist A→Z, Artist Z→A, Album A→Z, Album Z→A, Duration ↑, Duration ↓.

**Search / download worker states** (Replacer, Playlist, Playlists)
- Search per song: Idle → Pending → Done(with N results | no results) | Failed.
- Download per song: Idle → Pending → Done | Failed(message).
- Each worker exposes: paused flag, queued count, per-item states. Search runs 4 in parallel, download 3 in parallel. Pause is non-destructive (queued items wait, in-flight ones finish).

**Fingerprinting**: background pass with `done / total` progress; produces acoustic duplicate groups.

**Toasts**: three levels — info (4 s), warn (6 s), error (9 s) — top-right, auto-dismiss plus manual ✕. Every failure caused by a user action must produce a toast.

**Status bar**: bottom strip shown only while an async op runs: "Scanning library… (N so far)", "Scanning source… (N so far)", "Fingerprinting done/total", with a spinner.

---

## 3. Global window layout (current)

```
┌──────────────────────────────────────────────────────────────────┐
│ Recurate │ Songs Albums Artists Folders Playlists Queue Replacer │
│          │ Missing Playlist Duplicates │ Settings      2457 songs │  ← top bar
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│                        current screen                            │
│                                                                  │
├──────────────────────────────────────────────────────────────────┤
│ ⏮ ▶ ⏭  [======seek======] 1:23 / 3:45  Vol [====]  Now Playing  │  ← mini player
├──────────────────────────────────────────────────────────────────┤
│ ◌ Scanning library… (812 so far)  ·  Fingerprinting 120/2457     │  ← status bar (conditional)
└──────────────────────────────────────────────────────────────────┘
```

- Top bar: app name, one selectable tab per screen (active tab highlighted), Settings button, total song count right-aligned.
- Mini player is hidden on "full-screen" screens: Now Playing, Settings, Replacer, Duplicates, Missing, Playlist. Those screens carry their own "← Back" button.
- Toasts overlay the top-right corner of the central area.
- Global shortcuts: `Space` play/pause, `Ctrl+→` next, `Ctrl+←` previous. They are suppressed while any text field has focus.

Design freedom: the tab strip can become a sidebar, the mini player can become a persistent bottom bar with art, etc. Keep the count of songs and the persistent transport visible somewhere.

---

## 4. Screens

Each entry lists: purpose, what it shows, every control, and behaviours the design must preserve.

### 4.1 Songs (AllSongs) — default screen

- **Purpose**: browse and play the whole destination library.
- **Header**: title "Songs", search text field (filters title/artist/album live), sort dropdown (8 options above).
- **Pagination**: 50 rows per page. Controls: `◀ Prev`, label "Page X / Y (a–b of N)", `Next ▶`. Page is clamped when the filter shrinks the list.
- **Rows** (shared `song_row` component, see §5): row number, ▶ play button, title, then right-aligned cluster: duration · artist (truncated). Click anywhere on the row → play from that row with the current page's list as the queue. Currently-playing row is highlighted.
- Constraint: the list is virtualised; with 2,457 songs nothing may re-sort or re-filter every frame.

### 4.2 Search

Same as Songs without pagination: header "Search", the same live filter, full virtualised list of matches.

### 4.3 Albums / Album detail

- List of album names (derived from tags). Clicking an album opens its detail: header with album name, song rows in track order, click to play.

### 4.4 Artists / Artist detail

- Same pattern as Albums keyed by artist.

### 4.5 Folders

- One collapsible section per destination folder (full path as header). Inside: song rows with a trailing ✕ "Delete". Click row → play the folder; ✕ → delete file from disk, renumber the folder, refresh.

### 4.6 Queue

- Header "Queue". Empty state: "Queue is empty". Otherwise song rows with ✕ "Remove from queue"; current item highlighted; click row → jump to it.

### 4.7 Now Playing (full screen)

- `← Back` button, header "Now Playing".
- Empty state: "Nothing playing".
- Centered: title (large), artist, album (muted). Seek slider, position label `m:ss / m:ss`.
- Transport row: `⏮ Prev`, `▶ Play` / `⏸ Pause`, `Next ⏭`, `Shuffle` (toggle), `Repeat: Off|All|One` (cycles).
- Songs may have embedded cover art (`has_embedded_art`); the current UI does not render it, the redesign may.
- Seek behaviour to keep: dragging shows the in-flight position; the seek is committed on release (or on a bare click on the track); after commit the slider must not snap back.

### 4.8 Mini player (persistent bottom bar)

- "Nothing playing" or: `⏮`, `▶/⏸`, `⏭`, seek slider, `pos / dur`, volume slider 0–1 labelled "Vol", `Now Playing` button.

### 4.9 Playlists (hub — the most important new screen)

- `← Back`, header "Playlists", sub-caption "every folder in the destination root is a playlist".
- **Left panel — folder list**
  - Text field "New playlist name" + create button (creates an empty folder; the new empty folder is shown while selected).
  - Sort toggle button: A→Z / Z→A by folder name.
  - One selectable row per folder (name only). Active folder highlighted. An empty selected folder shows as "Name (empty)".
- **Right panel — editor for the selected folder**, three stacked regions:

  **a) Add songs**
  - Multiline paste box, hint text:
    ```
    https://youtu.be/…
    Artist song name
    …
    ```
    One item per line. YouTube URLs (video or playlist) are links; anything else is treated as "Artist + song name" and searched. Blank lines and duplicates are ignored; `youtube.com/...` without scheme is accepted.
  - `Find & download` button (disabled while resolving or while a batch into this folder is running). `Clear` button.
  - Status while resolving: "resolving…" (muted). After resolve: per-line outcomes — only failures are kept on screen as red lines ("no audio match found", "failed to fetch playlist", etc.).
  - **Duplicate prompt** (currently an `egui::Window` titled "Already in playlist"): lists each resolved song whose title+artist (case-insensitive) already exists in the folder or appears twice in the batch. Buttons: `Skip duplicates`, `Download anyway`, `Cancel`. The batch waits for the answer; nothing is auto-skipped. The redesign should make this a real modal.
  - **Batch status** (while downloads are in flight): counts Pending / Done / Failed (colour-coded), `⏸ Pause` / `▶ Resume` for the shared download worker, "(N queued)". Failed items show their error. When the last item lands the folder is renumbered so failed downloads leave no gap.

  **b) Order** (the track list)
  - Caption "Order" + hint "drag rows, or use the buttons; nothing is renamed until you apply".
  - One row per song: drag handle `☰`, position number, title, artist (truncated), then buttons `▲` `▼` `⇱ First` `⇲ Last` `✕`.
  - Drag-and-drop reorder between rows.
  - `Move track` row below the list: `from [n]` `to [n]` numeric inputs (1..N) + `Move` button.
  - `Apply order` button (primary; enabled only when the working order differs from disk and no batch is in flight) and `Reset` (discard edits). Applying renames files via a two-phase temp rename so numbers become `01..N`.
  - `✕` deletes the file from disk immediately (with renumber); it is disabled while the order has unapplied edits or a batch is running, because the renumber would invalidate the pending edit.
  - The list always reflects disk: new downloads and deletions appear immediately even while the user has unapplied reorder edits (vanished songs drop out, new ones append at the end).
  - Bottom controls must stay inside the window: the list reserves space for the Move row and action buttons; the list scrolls, the controls do not.

- Batches are bound to the folder they were started in. Switching the selected folder mid-batch must not move files or renumber the wrong folder. The editor of a folder with a running batch shows its batch status; other folders are editable.

### 4.10 Playlist (import one YouTube playlist, full screen)

- `← Back`, header "Download a YouTube playlist".
- Tool status pill: yt-dlp / ffmpeg found or missing (green / amber / red text).
- URL text field, hint `https://www.youtube.com/playlist?list=…`, `Fetch` button; "fetching…" while the background call runs. Errors toast.
- After fetch: playlist title, `Folder name` text field (default = playlist title, sanitised for Windows), and a preview table of planned files `NN - Title - Artist.mp3` with a marker for entries already on disk.
- `Download N` button → queues all not-yet-on-disk entries into the shared download worker.
- Progress: Pending / Done / Failed counts, pause/resume, failures with messages.

### 4.11 Replacer (full screen, batch tool — deliberately has NO per-song list)

- `← Back`, header "Replacer — YouTube match preview".
- Tool status pill (yt-dlp / ffmpeg / API key).
- Backend dropdown: `yt-dlp (default)` or `YouTube Data API v3`. In API mode the "Search all" button turns red and shows the quota cost when N > 99; "Search first N" is capped at 25.
- Scope controls: folder multi-select popup button `"k/22 ▾"` with a checkbox per source folder plus `Reset to all` and `Deselect all`; three states exist (all / some / none). Query text filter. `Clear results`.
- Actions: `Search first N`, `Search all N`, `Start replace top match` (appears only when there is at least one pending or done-with-results search; queues a download for every song with a top match; safe to click repeatedly as more searches finish).
- Two worker control rows: search worker and download worker, each with `⏸ Pause`/`▶ Resume` and "(N queued)".
- Stats block: "Search progress" — total / idle / pending / done-with-results / no-results / failed; "Replace progress" — pending / done / failed / ready-to-start. Colour-coded labels. "searches still running…" hint.
- Important product rule: audio-only. Music videos are filtered out and an empty result is the correct answer when no audio version exists. Slowed / Reverb / Sped Up / Nightcore tags are intentional and preserved.
- Do not add a per-song row list here: with 2,457 rows it caused unacceptable lag.

### 4.12 Missing (full screen)

- `← Back`, header "Missing from destination".
- Toolbar: `Copy all N` (direct file copy from source to destination, disabled while running or when 0), `Refresh`, `Renumber all folders` (shows "Renumbering…" while running), count label "N files".
- Empty state: "Destination has every source file. Nothing to do."
- Otherwise: grouped by folder name, each folder listing the missing filenames.

### 4.13 Duplicates (full screen)

- `← Back`, header "Duplicates by audio fingerprint".
- Toolbar: `Recompute` (full re-fingerprint), `✕ Delete N extras (keep highest name)` with two-click confirmation (second state: `⚠ Confirm delete N files?`), group count.
- Progress bar "Fingerprinting done / total" with a note while the pass is incomplete; groups update live as fingerprints arrive.
- Groups: each group = songs in the same folder with identical acoustic fingerprint and duration. Per song: filename/title, `▶ Play`, `✕ Delete`.

### 4.14 Settings (full screen)

- Header "Settings", collapsible sections:
  - **Library paths**: Source root path field (caption: read-only catalogue), Destination root path field (caption: downloads land here; player scans it), `Rescan source + destination`. Edits auto-rescan when leaving the page.
  - **Renumber**: checkbox "Renumber after delete", slider "Prefix threshold" 0–1 (folders with fewer prefixed files than this fraction are skipped).
  - **Equalizer**: checkbox "Enable equalizer", 10 vertical band sliders −12..+12 dB labelled with Hz, "Bass boost (dB)" slider 0–12. Also reachable as its own screen.
  - **Replacer**: YouTube Data API v3 key (password field; env var `YOUTUBE_API_KEY` overrides), Cookies-from-browser free text (`chrome` / `firefox` / `edge` / `brave`; empty = off) used to bypass YouTube's "confirm you're not a bot" wall; hint that the browser must be closed.
  - `Save settings` button. Settings persist at `%APPDATA%/MusicSuite/Player/settings.toml`.

---

## 5. Shared components

**song_row** — the one row widget used by Songs, Search, Albums, Artists, Folders, Queue.
- Layout: `[28 px #] [▶] [title, fills] … [✕?] [m:ss] [·] [artist]`.
- The whole row is one click target (play). The trailing cluster is right-aligned and reserved (max 60 % of width) so long titles truncate instead of pushing it off-screen. Optional ✕ with a hover text ("Delete" / "Remove from queue"). Highlight state for the playing row.

**Seek slider** — shared by mini player and Now Playing; stashes the in-flight drag value and commits on release.

**Tool status pill** — green/amber/red text: "yt-dlp ✓ ffmpeg ✓", or which one is missing.

**Worker controls** — `⏸ Pause` / `▶ Resume` + "(N queued)" — appear in Replacer, Playlist, Playlists.

**Toasts** — info / warn / error, top-right, timed, dismissible.

---

## 6. Key user flows

1. **Listen**: Songs → filter/sort → click row → mini player shows transport → Now Playing for the big view. Space / Ctrl+arrows work anywhere outside text fields.
2. **Build a playlist from links**: Playlists → New playlist name → create → paste lines → Find & download → (answer duplicate prompt) → watch Pending/Done/Failed → songs appear in Order list numbered → optionally drag / First / Last / Move → Apply order.
3. **Import a YouTube playlist**: Playlist → paste URL → Fetch → adjust folder name → Download N → files land as `NN - Title - Artist.mp3` in `<dest>/<folder>/`.
4. **Replace bad rips**: Missing → Copy all (bootstrap destination from source) → Replacer → pick folders → Search all → Start replace top match → repeat as searches finish.
5. **Clean duplicates**: Duplicates → wait for fingerprinting → review groups → delete per song or bulk "keep highest name" with confirmation.
6. **Delete a song**: Folders or Playlists ✕ → file removed → folder renumbered → all lists refresh.

---

## 7. Non-negotiable constraints for any new UI

- **Performance**: 2,457+ songs. Lists must be virtualised and views cached; no per-frame filesystem, subprocess, sorting or cloning of the whole library. Availability probes (yt-dlp, ffmpeg) are cached once.
- **Filesystem is truth**: playlist order lives in filename prefixes; there are no sidecar playlist files. Any order UI edits a working copy and applies with one explicit action.
- **Freshness**: lists must reflect disk changes (downloads, deletes) immediately, even with unapplied edits.
- **Ask, don't auto-skip**: duplicates found during add must prompt the user.
- **Batches are folder-bound**: a running download batch belongs to the folder it started in; switching selection must not affect it.
- **Audio-only**: no video fallback in search results; remix tags (Slowed/Reverb/…) preserved.
- **Every user-triggered failure gets a toast**; long-running background work shows in the status bar.
- **Text**: long, Unicode-heavy titles; truncate, never overflow; CJK fallback font.
- **Keyboard**: Space, Ctrl+→, Ctrl+← gated on no text field focus; avoid bare arrow keys (scroll areas use them).
- **Bottom controls must stay within the window** on every screen at modest window sizes (~1000×700).
- **Two-click confirmation** for bulk destructive actions.
- **No accounts, no network except yt-dlp/YouTube API calls.**

---

## 8. Technical stack (for the implementer, not the designer)

- Rust 2021, `eframe`/`egui` 0.28, `cpal`, `symphonia`, `lofty` (tags), `crossbeam_channel`, `serde`, `tracing`.
- Source layout: `player/src/{main,lib}.rs`, `data/` (scanner, tags, fingerprint, duplicates, library), `engine/` + `playback/` (decode/output/queue), `replacer/` (title cleaner, YouTube search, scoring, download, playlist fetch, link-list parser, resolver, sync), `renumberer.rs`, `ui/{app,fonts,toasts}.rs`, `ui/components/{top_bar,mini_player,song_row}.rs`, `ui/screens/{library,now_playing,queue,settings,replacer,duplicates,missing,playlist,playlists}.rs`, `domain/model.rs` (`Song`, `PlaybackState`, `Screen`, `SortOption`, `RepeatMode`).
- `Screen` enum: Library, AllSongs, AlbumsList, AlbumDetail(name), ArtistsList, ArtistDetail(name), Folders, Playlists, NowPlaying, Equalizer, Search, Queue, Replacer, Duplicates, Missing, Playlist, Settings.
- Launch: `start.bat` at repo root, or `cargo run --release` from `player/`. 68 tests via `cargo test`.
- A UI rewrite that keeps egui should keep the `App` struct's cached views and worker APIs; a rewrite in another toolkit (e.g. Tauri/web) would keep everything under `data/`, `engine/`, `replacer/`, `renumberer.rs` as the backend and replace `ui/` only.
