# Handoff: Recurate desktop UI redesign

## Overview
Recurate is a native desktop music player + library curator for one user's ~2,500-file MP3 collection (Rust, egui today). This package specifies a complete UI redesign in direction **1a**: a persistent left sidebar, a persistent bottom mini player, and the **Claude Design** language — cream paper, sepia ink, a single terracotta accent used like punctuation, serif page titles, hairlines instead of borders, no shadows, no gradients. Every action in the current app stays reachable; screens and behaviours are defined by `APP_CONTEXT.md` (bundled) and this README. The brand spec itself lives at `docs/claude-design.md` in the repo and is the source of truth for every token below.

Target platforms: Windows first; the layout is identical on macOS and in a web/Tauri shell (see "Platform notes").

## About the design files
The `.dc.html` files in this bundle are **design references built in HTML** — they show intended look and behaviour. They are not production code. Recreate them in the target environment:
- **Keep egui**: keep `App`'s cached views and worker APIs, rewrite `ui/` only (`ui/components/{top_bar→sidebar, mini_player, song_row}`, `ui/screens/*`). Map the tokens below to `egui::Visuals` once in `ui/theme.rs`; never hardcode a colour in a screen.
- **Tauri / web**: keep `data/`, `engine/`, `replacer/`, `renumberer.rs` as the backend; implement this spec in the frontend with the CSS custom properties from `docs/claude-design.md`.

## Fidelity
**High-fidelity.** Colours, type sizes, spacing and states are final. Icons are glyph placeholders (`♫ ▤ ◎ ▣ ≡ ☷ ⇄ ⤓ ◫ ◌ ⚙`) — replace with a 16px **outlined** line icon set of thin consistent stroke (Lucide / Phosphor Light), single colour, never filled. Cover art tiles are flat `--surface-sunk` squares with a hairline — render embedded art (`has_embedded_art`) and fall back to the flat tile. Never use emoji in chrome.

## Files
- `Recurate Mockups.dc.html` — turn 1: **1a** = Songs shell + Playlists hub (+ duplicate modal). The rejected 1b/1c alternatives have been removed.
- `Recurate Desktop.dc.html` — turn 2: 2a Now Playing · 2b Queue · 2c Folders · 2d Albums/Artists · 2e Replacer · 2f Import playlist · 2g Duplicates · 2h Missing · 2i Settings.
- `APP_CONTEXT.md` — product/behaviour source of truth.
- `../docs/claude-design.md` — brand language, tokens, component rules, accessibility gates.

Reference frame is **1000×700** (the app's minimum comfortable window). Everything must remain inside that frame; lists scroll, control rows never do. The frames follow the OS colour scheme (`prefers-color-scheme`) and can be forced with `data-theme="light"` or `data-theme="dark"` on `<html>`; both palettes share the same tokens.

---

## Design tokens
Canonical values are in `docs/claude-design.md`. Both palettes are listed here so the implementer needs no second file. All components reference the token, never the hex.

### Colour
| Token | Light | Dark | Use in Recurate |
|---|---|---|---|
| `--paper` | `#F6F3EC` | `#22201B` | content background, Now Playing background |
| `--surface` | `#EFEADF` | `#2A2823` | sidebar, mini player, status bar, cards/panels, toasts, modal, popover |
| `--surface-sunk` | `#E7E1D4` | `#1C1A16` | inputs, textareas, dropdowns, slider tracks, art placeholder tiles, progress-bar background |
| `--ink` | `#33302A` | `#EAE4D6` | primary text, big stat values |
| `--ink-2` | `#6B6356` | `#B3AB9A` | sidebar items, artist, durations, secondary-button text, captions |
| `--ink-3` | `#978C7B` | `#847C6D` | section labels, counts, hints, placeholders, disabled controls |
| `--line` | `#E2DBCE` | `#38352D` | every divider, panel border, sidebar/mini-player edge |
| `--line-strong` | `#D6CDBC` | `#46423A` | input/button outlines, borders on hover, popover/modal outline |
| `--accent` | `#B3502F` | `#D98A66` | links, focus ring, icons, seek/volume fill and knob, selected-tile ring, "pending" counter |
| `--accent-fill` | `#A8492B` | `#AE4F30` | primary button, play circle, logo mark |
| `--accent-hover` | `#8F3D22` | `#984229` | primary button hover/active |
| `--accent-ink` | `#FBF8F1` | `#FBF6EE` | text/icon on an accent fill |
| `--accent-soft` | `#F1E3D8` | `rgba(205,117,81,.15)` | active nav item, playing row, selected folder row, dragged row, art tint on playing row, "downloading" pill, tool status pill |
| `--accent-soft-ink` | `#8A3C22` | `#E7A98A` | text on the soft wash (active nav, title/artist on playing row), disabled `✕` on a dirty list |
| `--positive` | `#5C6B3F` | `#8A9A66` | done, "on disk ✓", tool found, `keep` tag — text and dots only |
| `--critical` | `#9C3B2E` | `#D07A66` | failed, error toast edge, failure lines — text and dots; the only filled use is the armed delete-confirm button |
| `--caution` * | `#7A5A1E` | `#D9B26A` | unapplied-edit marker "03 ←09", API key missing, cookie warning, warn toast edge |
| `--critical-soft` * | `#F2DFD9` | `rgba(208,122,102,.15)` | row pending deletion on hover, duplicate extras rows |
| `--shadow` | `0 1px 2px rgba(54,46,36,.06)` | `0 1px 2px rgba(0,0,0,.25)` | modal and popover only — optional |

\* Derived tokens, not in the canonical set: a warm ochre for "attention, not danger" and a warm wash for rows about to be deleted. Both stay inside the cream/clay spectrum; never signal with them alone — always pair with a label or glyph.

Logo mark: 22px square, `--radius-sm`, solid `--accent-fill`. No gradient. Modal scrim: `rgba(54,46,36,.35)`.

### Typography
- Display / page titles: **Newsreader** (serif), weight 400, never bold, never below `--h3`. Fallback Georgia.
- UI: **Inter** 400/500/600 (600 is the maximum). Fallback `Segoe UI Variable` / system humanist sans. CJK fallback **Noto Sans SC**, then `Microsoft YaHei UI`. Never assume ASCII.
- Data (durations, prefixes, paths, filenames, counters, key/value): **JetBrains Mono** 400/500 with tabular numerals.
- Scale — page title `--h2` 26 serif · panel title / modal title / sidebar wordmark `--h3` 21 serif · Now Playing title `--h1` 34 serif · body row 13 sans · caption 12 · meta 12 · micro 11 · section label 11/600/+0.06em uppercase · nav label 10/600/+0.08em uppercase · big stat 20/500 mono. Line-height `--leading-snug` 1.4 for labels, `--leading-tight` 1.15 for headings.
- This is a data-dense surface: the 11–13px range is a documented exception to the 16px body default (spec §Pragmatics). Prose (info cards, captions longer than a line) goes back to 14–16 and stays under `--measure`.
- Long titles: `white-space:nowrap; overflow:hidden; text-overflow:ellipsis`. Never wrap a row; never let trailing columns move.

### Spacing / radius / elevation
- 8pt grid, fine control at 4: only `--space-1…9` (4 8 12 16 24 32 48 64 96). Content padding 24px horizontal, 16px top. Row padding 8px horizontal. Sidebar padding 16px 8px. Default to more air than feels necessary.
- Radii: rows / buttons / art tiles / mini buttons `--radius-sm` 6 · inputs / dropdowns / cards / popover `--radius` 10 · modal / Now Playing art `--radius-lg` 14 · pills `--radius-pill`.
- Elevation: none. Modal and popover may carry `--shadow`; nothing else does. No glow on the play button, no shadow under Now Playing art, no inset colour bars on nav or toasts.
- Borders are hairlines (`--border` 1px `--line`); `--line-strong` only on hover/focus and around inputs.

---

## Global shell (all screens)

```
┌──────────┬────────────────────────────────────────────┐
│ sidebar  │ content (scrolls internally)               │ 1fr
│ 200px    │                                            │
├──────────┴────────────────────────────────────────────┤
│ mini player 72px (hidden on Now Playing)              │
└───────────────────────────────────────────────────────┘
```
CSS-grid: `grid-template-columns: 200px 1fr; grid-template-rows: 1fr 72px`. Status bar (conditional, 24px) is a flow element between content and mini player, never an overlay. Content never exceeds `--w-app` 1120px; on wider windows it stays left-aligned with a generous right gutter rather than stretching.

### Sidebar (200px, `--surface`, hairline right edge)
- Header: logo mark + "Recurate" in `--h3` serif. (macOS: add 28px top inset for traffic lights. Windows: this header doubles as the custom title-bar drag region.)
- Section labels `LIBRARY`, `CURATE` (nav label style, `--ink-3`).
- Items 13px sans, padding 8px, `--radius-sm`, icon column 16px, gap 8: Songs · Albums · Artists · Folders · Queue(count) — Playlists · Replacer · Import playlist · Duplicates(count, `--accent` when >0) · Missing(count).
- States: default `--ink-2`; hover `--surface-sunk`; **active** `background: --accent-soft; color: --accent-soft-ink; weight 600`. No vertical bar, no underline.
- Bottom (margin-top:auto): Settings item, then "2 457 songs · 22 folders" 11px `--ink-3`.
- Replaces the old top tab strip **and every "← Back" button** — every screen is one click away.

### Mini player (72px, `--surface`, hairline top edge) — grid `260px 1fr 220px`, padding 0 16px, gap 16
- Left: 44px art (`--radius-sm`) + title 13/600 (ellipsis) + "artist · folder" 12 `--ink-2`. Empty state: "Nothing playing".
- Centre: transport row (shuffle · ⏮ · **32px `--accent-fill` circle** play/pause with `--accent-ink` glyph · ⏭ · repeat) 14px, gap 16; below it the seek row: `pos` mono 11 · 4px track (`--surface-sunk` bg, `--accent` fill, 12px `--accent` knob) · `dur`.
- Right: volume icon + 96px 4px slider (0–1) + "Now Playing" secondary button.
- Seek: dragging shows the in-flight position; commit on release or on bare click; no snap-back after commit (stash drag value, apply on release).
- Shortcuts anywhere outside a text field: `Space` play/pause, `Ctrl+→` next, `Ctrl+←` previous. Suppressed while any text field has focus. No bare arrow keys.

### Shared components
**song_row** (Songs, Search, Albums, Artists, Folders, Queue) — height 40, `--radius-sm`, grid `32px 32px 1fr 44px 200px [28px]` gap 8, padding 0 8:
`# (12 --ink-3)` · `28px art` · `title (13, ellipsis)` · `duration (mono 12 --ink-2, right)` · `artist (13 --ink-2, ellipsis)` · optional `✕` (Delete / Remove from queue, tooltip). Whole row is the click target (play). Hover `--surface`. **Playing row**: `background --accent-soft`, `#` becomes an `--accent` ▶ glyph, title `--accent-soft-ink` 600, duration/artist `--accent-soft-ink`, art tinted `--accent-soft`. Trailing cluster reserved max 60% width.

**Buttons** — height 32 (toolbar) / 28 (inline), `--radius-sm`, padding 4–6px 12–16px, 13px sans 500. Primary: `--accent-fill` bg, `--accent-ink` text; hover `--accent-hover`; no lift, no glow. Secondary: transparent, hairline `--line` border, `--ink` text; hover fills `--surface` and border firms to `--line-strong`. Danger-confirm (armed state only): `--critical` bg, `--accent-ink` text, 600. Text button / link: `--accent`, underline on hover. Disabled: 45% opacity, no pointer. All get a visible `:focus-visible` ring: 2px `--accent` outline with a 2px `--paper` inset halo.

**Inputs** — height 32 (toolbar) / 28 (in panel), `--radius-sm`, `--surface-sunk` + hairline `--line-strong`, placeholder `--ink-3`. Search inputs carry a leading `⌕`. Focus: 1px `--accent` outline + paper halo, never the browser blue. Error: border tints `--critical` + small `--critical` message below; never a filled banner.

**Dropdown** — like input, trailing `▾` in `--ink-3`.

**Tool status pill** — `--radius-pill`, `--accent-soft` wash, `--accent-soft-ink` text, mono 11: `● yt-dlp ● ffmpeg [● API key]`; dots `--positive` / `--caution` / `--critical`, each followed by the word so state never rests on colour alone.

**Worker controls** — `⏸ Pause` / `▶ Resume` secondary 12 (Resume shows `--accent` border + `--accent` text) + "(N queued)" `--ink-3`.

**Batch counters** — mono 12: `● N pending` `--accent` · `● N done` `--positive` · `● N failed` `--critical`.

**Toast** — top-right of content, 280px, `--surface` bg, hairline `--line-strong`, `--radius`, a 2px left edge by level (info `--accent`, warn `--caution`, error `--critical`) plus a leading word "Info / Warning / Error", title 600 + body `--ink-2` 12, `✕`. Auto-dismiss info 4 s / warn 6 s / error 9 s. Every user-triggered failure toasts.

**Status bar** — 24px, `--surface`, mono 11 `--ink-3`: a single pulsing dot (`--accent`, 1.2 s ease, no spin) + "Scanning library… (N so far)" · "Scanning source… (N so far)" · "Fingerprinting done/total" with a 120px 4px bar. Shown only while an async op runs.

**Modal** — 420px, `--surface` bg, hairline `--line-strong`, `--radius-lg`, padding 24, `--shadow`, over the scrim. Title `--h3` serif, body 13 `--ink-2`, list rows on `--paper` `--radius-sm`, buttons right-aligned (text · secondary · primary).

**Popover** — `--surface`, hairline `--line-strong`, `--radius`, padding 8, `--shadow`.

**Cards / panels** — `--surface`, hairline `--line`, `--radius`, padding 16–24, no shadow.

---

## Screens

### 1a-1 Songs (default)
- Header: "Songs" `--h2` serif · right: filter input 280px · sort dropdown (8 options).
- Column header row (11 uppercase `--ink-3`): `#`, `TITLE`, `TIME`, `ARTIST`, bottom border.
- 50 song_rows per page, virtualised. Pagination footer (top border): `◀ Prev` (disabled on p.1) · "Page X / Y (a–b of N)" · `Next ▶`. Clamp page when filter shrinks list. Filter/sort run on cached views, never per frame.
- Search screen = same without pagination.

### 1a-2 Playlists (hub)
Content splits `230px | 1fr`.
- **Left**: "Playlists" `--h2` serif + caption "every folder in the destination root is a playlist" 12 `--ink-3` · row: input "New playlist name" + 28px primary `+` (creates empty folder, selects it) · "22 folders" + `A→Z ⇅` sort toggle · folder rows 13 (name + count 11 `--ink-3`). Active row `--accent-soft` + 600. Empty folder: "Name (empty)" `--ink-3`. A folder with a running batch shows an `--accent-soft` pill `↓ N`.
- **Right** header: 40px art + folder name `--h3` serif + "music/Folder · N tracks · N downloading".
- **Add songs** panel (`--surface`, `--radius`): label `ADD SONGS` + hint "one per line · YouTube URL or “Artist song name”" · 56px mono textarea (placeholder `https://youtu.be/…` / `Artist song name`) · `Find & download` primary (disabled while resolving or batch in flight) · `Clear` · right-aligned failure lines in danger (only failures persist) · "resolving…" `--ink-2` while resolving. Bottom row: batch counters + Pause/Resume + "(N queued)". When the last item lands, renumber the folder.
- **Duplicate modal** "Already in playlist": subtitle "N of M resolved songs match a title + artist already in **Folder**." · rows "Title — Artist" + reason ("already #01" / "twice in batch") · `Cancel` · `Download anyway` · `Skip duplicates` (primary). Batch waits for the answer; nothing auto-skips.
- **Order** list: label `ORDER` + hint "drag rows or use the buttons; nothing is renamed until you apply". Rows 32px, grid `16px 24px 24px 1fr 130px 120px 22px`: `☰` handle · `01` mono · 22px art · title · artist · `▲ ▼ ⇱ ⇲` mini buttons (hairline, `--radius-sm`) · `✕`. Dragged/moved row: `--accent-soft` + 1px dashed `--accent` outline; when moved show "03 ←09" in `--caution`. Rows still downloading: 55% opacity, dashed art, italic "— downloading…", no buttons. List scrolls; footer doesn't.
- **Footer** (top border): "Move" `[from] → [to]` numeric inputs 36×24 + `Move` · right: `Reset` secondary · `Apply order` primary (enabled only when working order ≠ disk and no batch in flight). `✕` disabled (`--accent-soft-ink` colour) while there are unapplied edits or a batch is running.
- Batches are folder-bound: switching folders never moves/renumbers another folder. List reflects disk immediately (vanished songs drop, new ones append) even with unapplied edits.

### 2a Now Playing (mini player hidden; content grid `1fr | 260px`)
- Background plain `--paper`; the art carries the picture, the page stays quiet.
- Centre column: 280px art (`--radius-lg`, hairline, no shadow) · title `--h1` serif (ellipsis, max 560) · artist 14 `--accent` · "album · year · folder" 13 `--ink-2` · seek 6px track with 16px knob + mono times · transport: SHUFFLE (icon + 9.5 label, `--accent` when on) · ⏮ · **56px `--accent-fill` circle** ⏸ (no glow) · ⏭ · REPEAT · OFF/ALL/ONE (cycles) · volume 120px.
- Right "Up next" panel (`--surface`, left border): header + "9 left · 31 min" · compact rows (30px art, title, artist, duration) · footer link "Open queue →".
- Empty state: "Nothing playing" centred, `--ink-2`. Top-right hint "Space · Ctrl+← →" mono `--ink-3`.

### 2b Queue
Header "Queue" + "12 tracks · 41 min · playing 3 of 12" + `Clear queue` secondary. song_rows with trailing `✕` (Remove from queue). Already-played rows: text `--ink-2`, art 60% opacity. Current row highlighted. Click row → jump. Empty: "Queue is empty".

### 2c Folders
Header + "22 folders in music/" + filter 240px + `Collapse all`. One card per folder (`--surface`, `--radius`): header row 8px 10px — `▸/▾` · 22px art · name 600 · path mono 11 `--ink-3` · right "N tracks · duration" · when open `▶ Play folder` `--accent` text link. Expanded rows 34px, indent 36px, grid `32px 1fr 44px 180px 28px`: prefix mono · title · duration · artist · `✕`. Hovering `✕` turns the row `--critical-soft` with inline text "Delete from disk? renumbers N → N-1" and the `✕` becomes a red filled chip; click deletes, renumbers, refreshes. Click row → play the folder.

### 2d Albums / Artists
Content grid `1fr | 340px`. Left: header + count + filter; 4-column tile grid (square art radius 6, name 13/600 ellipsis, "artist · count" 11 `--ink-2`); selected tile 2px `--accent` ring. Right detail panel: 96px art + "ALBUM" label + name `--h3` serif + "artist · year · N tracks" + `▶ Play` primary; song rows 38px (`24px 1fr 40px`) in track order. Artists: identical, keyed by artist ("ARTIST" label, tiles show name + track count).

### 2e Replacer (batch tool — **no per-song list**, deliberately)
- Header "Replacer" + caption "replace low-quality rips with clean audio-only versions from YouTube" + tool status pill (yt-dlp/ffmpeg/API key).
- Row 1: Backend dropdown (`yt-dlp (default)` / `YouTube Data API v3`) · Folders scope button "k / 22 ▾" (`--accent` border; opens popover with `Reset to all · Deselect all · k/22`, checkbox rows with counts) · Query filter 220px · `Clear results` right.
- Row 2: `Search first N` secondary (N capped at 25 in API mode) · `Search all N` — in API mode with N>99 render in the armed danger-confirm style with "· ~X quota units" · `⇄ Start replace top match (N)` primary, shown only when ≥1 pending or done-with-results search exists; safe to click repeatedly · "searches still running…" `--ink-3` right.
- Two worker cards (`--surface`): `SEARCH WORKER · 4 parallel` and `REPLACE WORKER · 3 parallel`, each with Pause/Resume + queued, a 4px stacked progress bar (`--positive` / `--line-strong` / `--critical` / `--accent` segments, each with a mono legend), and 3×2 / 2×2 stat grid (20/500 mono value + 12 `--ink-3` label): search = total, idle, pending, with results, no results ⓘ, failed; replace = pending, done, failed, ready to start.
- Info card: "**Audio-only rule.** Music videos are filtered out — “no results” is correct when no audio version exists. Slowed / Reverb / Sped Up / Nightcore tags are preserved."
- `RECENT FAILURES` list (`--critical`, mono, ellipsis) — max ~5 lines, no full list.

### 2f Import playlist
Header "Import a YouTube playlist" + caption "files land as `NN - Title - Artist.mp3` in music/<folder>/" + tool pill. URL input (mono) + `Fetch` ("fetching…" while running; errors toast). After fetch: panel with `PLAYLIST` title + "N videos · k already on disk · m to download" and `FOLDER NAME` input (default = title sanitised for Windows). Preview table (mono 12, rows 28): `#` · planned filename · status (`on disk ✓` success, or `queued`; rows already on disk `--ink-2`). Footer: Progress counters + Pause + `⤓ Download N` primary (N = not-on-disk).

### 2g Duplicates
Header "Duplicates" + caption "same folder · identical acoustic fingerprint · same duration" · `↻ Recompute` · bulk button: state 1 `✕ Delete N extras (keep highest name)` secondary → state 2 `⚠ Confirm delete N files?` danger-confirm (reverts after 5 s or on blur). Progress line: spinner + "Fingerprinting done / total" mono + bar + "N groups so far · groups update live". Group cards: header mono accent path · "N files · m:ss each" · `fp xxxx…xxxx`; rows 32px mono: filename (kept row tagged `keep` success; extras `--critical-soft`) · `▶ Play` `--accent` · `✕ Delete` (danger on extras, disabled on kept).

### 2h Missing
Header "Missing from destination" + caption "in music_original/ but not in music/ · N files" · `↻ Refresh` · `Renumber all folders` ("Renumbering…" while running) · `⧉ Copy all N` primary (disabled when 0 or running). Folder cards: name 600 · mono "src → dst" path · "N files" · `Copy folder` `--accent` text link; mono filename list (ellipsis) + "… k more". Empty state: "Destination has every source file. Nothing to do."

### 2i Settings
Header "Settings" + mono path `%APPDATA%/MusicSuite/Player/settings.toml`. 2-column card grid:
- **LIBRARY PATHS**: Source root (caption "read-only catalogue") · Destination root ("downloads land here; player scans it") — mono inputs with `…` browse · `↻ Rescan source + destination` + hint "edits auto-rescan on leaving the page".
- **RENUMBER**: toggle "Renumber after delete" (32×16 pill, `--accent-fill` when on) · "Prefix threshold" slider 0–1 with mono value + caption.
- **EQUALIZER**: toggle "Enable" + "Open as screen ↗" link · 10 vertical band sliders (4px `--surface-sunk` tracks, `--accent` fill from centre, labels 31 62 125 250 500 1k 2k 4k 8k 16k mono 10) range −12..+12 dB · "Bass boost" slider 0–12 dB.
- **REPLACER**: API key password field with eye toggle (caption "env YOUTUBE_API_KEY overrides") · "Cookies from browser" input (chrome/firefox/edge/brave; empty = off) + `--caution` line "Close the browser before downloading — its cookie DB is locked while it runs."
Footer: "N unsaved changes" `--ink-3` · `Discard` · `Save settings` primary.

---

## Interactions & motion
- Hover: rows/nav → `--surface` / `--surface-sunk`, 80 ms ease-out. Buttons: +6% lightness.
- Selection/playing highlight changes instantly (no fade) so the virtualised list stays cheap.
- Seek knob appears on hover of the track; visible always in Now Playing.
- Modal/popover: fade + 4px translate, `--dur` 200 ms. Toasts fade in with a 4px translate, `--dur` in, `--dur-fast` out. No staggered entrances.
- Progress bars animate width `--dur` 200 ms linear. All motion honours `prefers-reduced-motion`.
- Two-click confirmation for bulk deletes; single-row `✕` is immediate but shows the inline red row state on hover.

## State (per screen, minimal)
Global: `PlaybackState`, `Queue`, `library`/`source` status, worker states (search, download), fingerprint progress, toasts, active screen, sidebar counts.
Songs: filter string, sort, page. Playlists: selected folder, new-name text, sort dir, paste text, resolve state, duplicate prompt, per-folder working order + dirty flag, move from/to. Replacer: backend, folder scope set, query, worker stats. Import: url, fetch state, title, folder name, plan rows. Duplicates: confirm-armed flag. Settings: draft + dirty count.

## Platform notes
- **Windows**: frameless window with custom title bar merged into the sidebar header (min/max/close at top-right of content, 46×32 hit targets). Font fallback `Segoe UI Variable` if Inter is unavailable; Georgia if Newsreader is unavailable.
- **macOS**: standard title bar hidden, traffic lights inset; sidebar header shifts down 28px. `Cmd` replaces `Ctrl` in shortcuts.
- **Web / Tauri**: identical layout; min viewport 1000×700; below that the sidebar collapses to 56px icon rail (labels hidden, tooltips on).

## Assets
No bitmap assets. Fonts: Newsreader (400/500, italic 400), Inter (400/500/600), JetBrains Mono (400/500), Noto Sans SC (Google Fonts / bundled). Icons: glyph placeholders → replace with a 16px outlined line icon set, single colour. Art placeholders → embedded cover art via `lofty`, else a flat `--surface-sunk` tile.
