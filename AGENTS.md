# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## Repository state

**Player working with an embedded Replacer (search + download + atomic replace into a destination root), a Playlist import page, a Playlists hub (folder = playlist: add by link/title, reorder, delete), and a token-driven UI (sidebar shell, light/dark theme, bundled fonts, YD logo as window/exe icon); Python Replacer not started.** Layout:

```
Youtube/
├── plan.md                # Authoritative spec for both components
├── AGENTS.md              # This file
└── player/                # Rust desktop player (compiles, P1–P15 partial)
    ├── Cargo.toml
    ├── src/               # main.rs + lib.rs + data/ engine/ playback/ replacer/ ui/ etc.
    ├── tests/renumberer.rs
    └── music/             # 2,457 mp3 dataset (22 folders) — referenced as `player/music/`
```

`plan.md` is still authoritative for design decisions; update it when design changes. The Python Replacer directory does not exist yet — the `player/src/replacer/` module now implements the full search → score → download → backup → in-place replace pipeline as a Rust port, wired into a Player screen. The Python pipeline as originally specified is therefore optional rather than required for the user's workflow.

## What this project does

Two components sharing one library:
- **Replacer** (Python, not started): walks the music dataset, extracts a title from each filename, searches YouTube for the cleanest audio match, downloads via yt-dlp + ffmpeg, replaces each file with a `<title> [audio].mp3` version while keeping a backup.
- **Player** (Rust, scaffolded): native desktop player using egui + cpal + symphonia. Plays the same dataset, deletes songs, calls the shared Renumberer. Also embeds a **full Replacer screen**: title-cleaner + YouTube search (yt-dlp or API) + scoring + audio-only filter + yt-dlp+ffmpeg download + backup + in-place replace. The user driving this project does the replacement entirely from the Player; the Python pipeline is not currently needed.

Both components share the music folder as source of truth and call the same **Renumberer** spec (Part III of `plan.md`). The Rust copy lives at `player/src/renumberer.rs`.

## The dataset (`player/music/`)

This is the only environment the tool will run against in practice — design decisions are calibrated to it.

- **2,457 `.mp3` files** in **22 flat subfolders** (no nesting).
- Naming patterns vary widely: numbered album tracks (`120 - death bed (coffee for your head) - Powfu.mp3`), YouTube-rip titles with `(Official Audio)`, full-width Unicode punctuation (`＂`, `｜`, `？`, `＃` — yt-dlp's substitutes for Windows-illegal chars), stylized math Unicode (`𝒮𝓁𝑜𝓌𝑒𝒹 𝒟𝑜𝓌𝓃`), multi-script (Cyrillic, Mandarin, French diacritics), and `(free) ... type beat - "<title>"` boilerplate.
- See *Dataset Analysis* in `plan.md` for the full pattern table — title-extraction logic must handle every variant listed there.

## Non-obvious constraints (read before coding)

These are the design points most likely to be missed by re-deriving from the data:

1. **`SKIP_FOLDERS`** — `Long`, `Ambient Playlist (1 Hour)`, `Suno`, `Suno2` must be skipped by default. The first two are single multi-hour DJ mixes (not tracks); the last two are AI-generated and have no YouTube source. Processing them produces garbage matches.

2. **Slowed/reverb files are *intentional* remixes, not bad rips.** Concentrated in `Mix - Lost Within the End (Slowed)`. The cleaner must **preserve** `(Slowed)` / `(Reverb)` / `(Sped Up)` / `(Nightcore)` tags in the search query, and the searcher must **prefer** results that also contain the tag. Stripping the tag and replacing with the studio original destroys the user's curation.

3. **Default `SEARCH_BACKEND` should be `ytdlp`, not `api`.** YouTube Data API v3 default quota is 10k units/day; `search.list` is 100 units → 100 searches/day → ~25 days for 2,457 files. The yt-dlp backend (`yt-dlp ytsearch10:"<query>"`) bypasses quota entirely at ~2-3s per query and runs **4 in parallel** in the Player, finishing the full library in ~30 minutes. Only switch to `api` if the user has applied for higher quota. The Player's Replacer screen exposes both backends behind a dropdown; the API path costs ~101 units per search (search.list 100 + videos.list 1 for duration) and the "Search first N" button caps at 25 in API mode while the "Search all N" button turns red and shows the unit total when N>99.

4. **Resume is required, not optional.** `./logs/processed.json` and `./logs/search_cache.json` (atomic writes via temp + `os.replace`) let runs pick up where they left off and avoid re-spending API quota on retries. A run without resume support is incorrect at this scale.

5. **Replace order matters: source-only read → download-to-tmp → atomic rename into destination.** The pipeline is now a **two-root layout**, not an in-place replace. `ScanSettings.source_root` (default `<cwd>/music_original`, editable in Settings → Library paths) is the read-only catalog the Replacer enumerates — `App` scans it into a second `Arc<Library>` named `source` at startup. **Path-edit auto-rescan**: `App::rescan_if_paths_changed()` runs once per `update()` and spawns a fresh `Library::scan` whenever `settings.scan.source_root` or `roots[0]` drifts from the last-scanned values cached on `App` (`last_scanned_source_root`, `last_scanned_dest_root`). It deliberately bails out while `Screen::Settings` is active so per-keystroke edits don't trigger one rescan per character — the rescan kicks in the moment the user navigates to any other page, and `cached_missing` is nulled at the same time so the Missing page reflects the new roots. `ScanSettings.roots[0]` (default `<cwd>/music`, also editable in Settings) is the destination — the existing Library scans it for playback. `DownloadRequest = {song_id, source_path, dest_path, video_url, cookies_browser}` where `dest_path = dest_root.join(source_path.strip_prefix(source_root))` mirrors the per-folder layout. yt-dlp downloads to a hidden sibling `.<stem>.dl.mp3` next to `dest_path`, then atomic-renames over `dest_path`. **There is no `backup/` mirror anymore** — the source root is the backup, by virtue of being read-only. A failed download leaves `dest_path` absent and the source untouched; the in-place hazard is gone. ID3 tags and cover art are written into the mp3 *before* the rename via yt-dlp's `--embed-metadata --embed-thumbnail --convert-thumbnails jpg`, plus two `--parse-metadata` rules: one strips ` - Topic` from `%(uploader)s`, the other splits `Artist - Title (suffix)` video titles. After yt-dlp finishes, `download.rs::override_tags_from_filename` parses the **target filename's stem** — only when it matches the track-prefixed pattern `<digits> - <title> - <artist>` (the user's library convention) — and uses `lofty` to overwrite Title / Artist / Track on the tmp mp3. The yt-dlp-derived tags are the fallback for filenames without that pattern; the cover art is preserved. After the rename, `Library::refresh_folder(dest_path.parent())` (the **destination** library) re-reads via `lofty` so the new tags surface in AllSongs without a full rescan. Filename is preserved (no `[audio]` suffix added) so track-numbered ordering survives. **Bot-detection bypass**: yt-dlp accepts `--cookies-from-browser` via the optional `[replacer] cookies_browser` setting (e.g. `chrome`, `firefox`, `edge`); empty = off. Set this when YouTube returns "Sign in to confirm you're not a bot" or `HTTP 403 Forbidden`. The flag is wired through **both** call paths — `replacer/youtube.rs::search_ytdlp` (ytsearch backend) and `replacer/download.rs::download_audio_mp3` — so the search worker hits the same auth wall as downloads. The deprecated `--no-call-home` flag has been removed from both. **Don't reintroduce a `backup/` step** — the source root *is* the backup; adding a third copy is wasted disk + complicates the failure model. **Bootstrap path**: a dedicated **Missing** page (top-bar nav, `Screen::Missing`, `screens/missing.rs`) drives `replacer/sync.rs` to direct-copy any source files whose mirrored `dest_path` doesn't exist yet — `std::fs::copy` + `create_dir_all`, no YouTube. Use it to populate a fresh destination from the catalog before kicking off searches. The list is global (no folder/query scoping); refreshes each touched destination folder on completion.

6. **Player UI must virtualize, cache, AND paginate.** The dataset has 2,457 rows; rendering and re-sorting them every frame caused several-hundred-ms lag. Layers: (a) `ScrollArea::show_rows` for the AllSongs / Search row lists virtualizes the visible rows; (b) `Arc<Vec<Song>>` caches in `ui::App` — `cached_library_view` keyed by `(library_version, sort, query)`, `cached_replacer_view` keyed by `(source_version, folder_filters, select_none, query)` (note: keyed off the **source** library since the Replacer enumerates the catalog at `music_original/`, not the destination), `cached_folders: Option<(u64, Arc<Vec<PathBuf>>)>` keyed by library_version, `cached_source_folders: Option<(u64, Arc<Vec<PathBuf>>)>` keyed by source_version, `cached_duplicates: Option<(u64, usize, Arc<Vec<DuplicateGroup>>)>` keyed by `(library_version, fingerprint_progress)` — the second component bumps as background fingerprinting workers fill `app.fingerprints`, so the Duplicates screen live-updates as new fingerprints arrive without polling, and `cached_missing: Option<MissingView>` keyed by `(source_version, dest_version, source_root, dest_root)` so the Missing page doesn't `Path::exists()` every source file each frame — eliminate per-frame Vec clones / filter / sort / `Library::folders()` walks / duplicate-detection passes (`Library::version()` is bumped on scan / refresh / remove on each library independently); (c) **pagination** on AllSongs at `ui::app::PAGE_SIZE = 50` rows per page, with `app.songs_page` clamped to the valid range each frame. **The Replacer screen does NOT show a per-song list at all** — it computes a single `ScopeStats` (counts: total / idle / pending / done-with-results / no-results / failed for searches; pending / done / failed / ready-to-start for downloads) inside one nested `WORKER.read_states(|s| dl.read_states(|d| ...))` and renders only the action buttons + two horizontal lines of colored stat labels. Don't add per-song UI back to Replacer; the row rendering itself was the lag. (d) `SearchWorker::read_states` / `DownloadWorker::read_states` are callback-style APIs that hold the read lock briefly without cloning the full state map — use them instead of `snapshot()` in hot paths since `SongState::Done` carries a `Vec<ScoredResult>` whose deep-clone is what made `snapshot()` expensive once searches accumulated. (e) Conditional repaints (`ctx.request_repaint_after` only fires while audio is playing) round it out. The Replacer's folder filter is **multi-select with explicit none state**: `app.replacer_folder_filters: Vec<PathBuf>` + `app.replacer_select_none: bool`. Three states: `select_none=false, filters=[]` = all selected; `select_none=false, filters=[..]` = those folders; `select_none=true, filters=[]` = none selected. Popup has both **Reset to all** and **Deselect all** buttons; unticking the last folder auto-flips into the none state instead of collapsing back to all (which was the prior behavior's bug). When adding a new screen that iterates the library, follow the same pattern.

7. **CJK glyphs need an explicit fallback chain.** egui's bundled fonts are Latin-only and will render Chinese / Cyrillic filenames as tofu. `ui/fonts.rs` loads `C:\Windows\Fonts\msyh.ttc` (Microsoft YaHei) plus a few siblings as Proportional/Monospace fallbacks; called once from `App::new`. If you fork to another OS, extend the candidate list there.

8. **Replacer workers are parallel and pausable.** `SearchWorker` spawns **4** threads (`SEARCH_PARALLELISM`) sharing one MPMC `crossbeam_channel` of `SearchRequest`s; `DownloadWorker` spawns **3** (`DOWNLOAD_PARALLELISM`) sharing the `DownloadRequest` channel. Each thread polls `recv_timeout(150-200ms)` and bails to `thread::sleep` when an `AtomicBool` paused flag is set, so pause is non-destructive — queued items stay queued, in-flight items finish. The UI surfaces a **⏸ Pause / ▶ Resume** button per worker plus a live "(N queued)" counter (an `AtomicUsize` bumped by `enqueue` and decremented by the worker after `recv`). When changing parallelism: yt-dlp searches are network-bound and 4 is comfortable; downloads transcode through ffmpeg and 3 is the empirical sweet spot before disk/CPU thrash. Don't use `recv()` (blocking, no pause polling) or unbounded sleep loops.

9. **Audio-only enforcement is two-stage and intentional.** The user's library is curated audio; music videos must not slip in. The cleaner appends `audio` to the search query (skipped for `(Slowed)`/`(Nightcore)` etc. so remix tags stay clean), and `score_results` runs each candidate through `is_audio_candidate(title, channel)` which keeps Topic channels (`Foo - Topic`) and titles tagged `audio`/`(audio)`/`[audio]` while dropping `official music video`, `(video)`, `visualizer`, `lyric video`, `(live)`, `concert`, `music clip`. An empty result list after filtering is the correct answer when no audio version exists — don't add a "fall back to videos" path.

10. **The Player's "Start replace top match" button is a progressive batch.** It only renders when there's at least one `Pending` search OR at least one `Done`-with-results search; clicking it queues a `DownloadRequest` for every song in the **full filtered set** with state=Done, results.first()=Some, download_state=Idle. The set of (song_id, path, video_url) triples is computed by `ScopeStats` during the same `read_states` pass that produces the on-screen counts, so the button doesn't re-walk the worker maps. Re-clicking is the intended workflow: as more searches finish, click again to pick up the new ones. It respects the current multi-folder selection + query filter, so scoping searches+replaces to a subset of folders is just ticking them in the Folders popup.

11. **Don't call subprocess-spawning probes per frame; catch lofty panics at the boundary.** Two defensive patterns came out of separate Replacer-page lag and crash investigations and must stay in place: (a) `replacer/youtube.rs::ytdlp_available()` and `replacer/download.rs::ffmpeg_available()` are wrapped in `OnceCell` because the Replacer screen's status pill calls each ~3-4× per redraw; uncached, that's ~240 `CreateProcess` calls per second at 60fps and was the dominant lag source after the per-song list was removed. If you add another `Command::new(...).output()`-style availability probe that the UI consults, cache it the same way. (b) `data/tags.rs::read_song` wraps `Probe::open(path)?.read()` in `std::panic::catch_unwind(AssertUnwindSafe(...))`, `replacer/download_worker.rs` wraps each `do_one(req)` in the same, and `data/fingerprint.rs::compute_fingerprint` wraps the whole symphonia decode pass in the same; all three convert decoder/lofty panics into `Err`. lofty's ID3v1 parser slices the fixed 30-byte title field as UTF-8 without a `is_char_boundary` check, so any CJK/Cyrillic title that gets truncated mid-codepoint panics with `"byte index 30 is not a char boundary"` — without these wrappers, a single such file in the dataset kills a worker thread mid-batch and the next download never lands.

12. **Duplicate detection is acoustic, intra-folder, and two-region.** `data/duplicates.rs` groups songs by `(fingerprint, duration_in_whole_seconds, parent_folder)` — all three must match for two files to land in the same group. The **parent_folder co-key** is intentional: cross-folder same-audio copies are intentional in this user's library (favorites mirrored from origin folders) and flagging them produced 200+ noise groups. The fingerprint is a 256-bit acoustic hash (`Fingerprint = [u64; 4]`, **v9**) computed by `data/fingerprint.rs::compute_fingerprint`: decode **two 30-second regions** via `symphonia` (head at offset 0, mid at offset `file_duration / 3`), downmix each to mono, split each into 32 equal-length windows, and per window encode:
    - `centroid` (2 bits) — *hybrid*: high bit = fixed (`centroid > 0.011 = corpus median`), low bit = rank within the song (`centroid > song-median centroid`, with a 20% deadzone so stationary signals don't flip rank bits under encoder noise). The fixed bit preserves bass-vs-treble discrimination on synthetic stationary signals; the rank bit discriminates within the bass-heavy regime where every song's centroid is below the corpus cutoff (this was the v6→v7 false-positive pattern).
    - `rms` (2 bits) — fixed quartile cutoffs at `0.065 / 0.129 / 0.232` (corpus quartiles measured during the v8/v9 audit; constants live in `data/fingerprint.rs`).

    Each region: 32 windows × (2 + 2) bits = 128 bits → packed into 2 u64s. Two regions = 256 bits = `[u64; 4]`. **Why two regions, not one 30 s window:** v8 had only the head region and collapsed when many tracks shared an album intro (DJ tag + producer tag). Requiring head AND mid to match knocked the corpus's 2 v8 false positives down to 0. **Why hybrid centroid:** v6/v7 used purely fixed centroid cutoffs; on bass-heavy tracks every window's centroid falls into bin 0 and unrelated songs hash to `[0…0, ffff…f]`. **Why duration co-key:** absorbs residual hash collisions on similar-length tracks. **Why parent-folder co-key:** see above (intentional cross-folder copies). Filename and ID3 tags are not consulted. **Algorithm history (do not regress)**: v1 = rank-quartile of mean |amp| (envelope-shape-equivalent collapse). v2 = absolute log-amp binning (12 of 16 bins unused). v3 = 64-sample-block avg + DefaultHasher (low-passed to bass envelope). v4/v5 = `ZCR ⊕ peak-factor` 1-bit per window (timbre half collapsed on bass-heavy material). v6 = 32 windows × 2-bit fixed centroid + 2-bit RMS, 128 bits (still collapsed bass-heavy regime). v7 = 64 windows × 2-bit, 256 bits (bit budget wasn't the issue — same false-positive count as v6). v8 = added hybrid centroid (1 fixed + 1 rank with deadzone) — 2/229 raw-md5 false positives, 0 PCM-verified. v9 = added second region (mid offset) — 0/229 PCM-verified false positives in full-corpus audit. Unit tests in `fingerprint.rs` are regression guards for each historical failure; do not break them. Fingerprinting is **incremental** via `App::start_fingerprinting(force_full)`: spawns `available_parallelism()` workers that pull from a `crossbeam` channel and fill `app.fingerprints: Arc<RwLock<HashMap<i64, Fingerprint>>>`. With `force_full=false` (auto-trigger) it only queues songs whose id isn't already in the map — so deleting a single duplicate does NOT rerun the entire library. `force_full=true` (the **Recompute** button) clears the map. Auto-trigger condition: `!running && fp_count < song_count`. The cache key for `cached_duplicates` includes `fingerprint_progress` so the rebuild is incremental as new fingerprints land. The Duplicates page also has a **`✕ Delete N extras (keep highest name)`** button: per-group sort by filename DESC and delete all but the lexicographically-largest filename, two-click confirmation. **Do NOT route bulk delete through `handle_delete`** — `handle_delete` calls `renumber_folder` + `library.refresh_folder` after each deletion, which re-derives every song's id from its new (renumbered) path; the bulk loop's pre-collected ids would then resolve to nothing and the next file would silently survive on disk. The current implementation calls `deletion::delete_song(&library, id, false, _)` per item (renumber off), tracks affected folders, and runs a single `renumber_folder` + `refresh_folder` per folder at the end. Verification rule for future fingerprint changes: do **not** confirm a change by raw-mp3 byte md5; re-encoded copies of the same audio have different bytes, so only a PCM-quantized hash is a meaningful equality check. Algorithm history (v1–v9) lives in the doc comment on `data/fingerprint.rs::Fingerprint` and the regression unit tests are the live guards.

13. **Surface user-facing failures via the toast API, not just `tracing::warn!`.** `ui/toasts.rs` exposes `app.toast_info(msg)`, `app.toast_warn(msg)`, and `app.toast_error(msg)`; toasts render top-right with an auto-dismiss TTL (4s/6s/9s by level) plus a manual `✕`. The motivating bug: bulk-delete used to silently skip files when their ids became stale mid-loop and the user only saw "duplicates still showing" with no clue why. Anything that fails *because of user input* (a click that didn't take effect, a download that errored, a path the app can't read) needs a toast in addition to the log line — the log line is for debugging, the toast is for closing the user's loop. Don't toast for background noise (per-frame state churn, periodic poll failures); reserve them for things tied to a specific user action. The status bar at the bottom of the window covers a different axis: it surfaces *ongoing* async ops (library scan, source scan, fingerprint pass) via `App::has_active_async_op` / `draw_status_bar` — extend it when adding a new long-running background task.

14. **`song_row::draw_with_options` owns the entire row width and the optional ✕.** `truncate()` caps the *rendered text* but not the *allocated width* — in an unconstrained `horizontal`, a long-text label still consumes the full remaining width and any subsequent `with_layout(right_to_left)` cluster gets placed at width 0, overflowing past the visible right edge. The current implementation in `ui/components/song_row.rs` allocates a single `[row_w, row_h]` rect via `ui.allocate_exact_size(_, Sense::click())` *before* rendering anything, then renders into a `child_ui` placed at that rect. Two consequences: (a) the row's click sense covers the full row including gaps, so anywhere-on-row counts as a play click; (b) the row's allocated width is fixed at the parent's `available_width()`, which is what stops the ScrollArea from widening each frame and staircasing the right cluster off the right edge over successive rows. Inside the row: 28 px row number, an explicit ▶ play button, the title (`add_sized([title_w, h])`), and a `right_to_left` cluster containing optional ✕, duration, separator, truncated artist. `right_reserve = min(220 + remove_reserve, inner_w * 0.6)`; `remove_reserve = 28` when `RowOptions::show_remove` is true. **Don't reintroduce an outer `ui.horizontal { song_row::draw + small_button }` wrapper** in callers — the wrapper used to read `available_width` before the trailing button was placed and re-introduce the staircase plus push the ✕ off-screen. Use `RowOptions::show_remove` and `remove_clicked` in `RowAction` instead. Apply the same "allocate the click rect first, render into a child_ui second" pattern when adding any other interactive flex row.

15. **Global keyboard shortcuts gate on `ctx.wants_keyboard_input`.** Live in `App::handle_shortcuts`: `Space` = play/pause, `Ctrl+→` = next, `Ctrl+←` = previous. The `wants_keyboard_input` check is non-negotiable — without it, typing into the search bar or any text field would pause playback on every space and skip tracks on every arrow. When adding a new shortcut, keep that gate, and avoid plain arrow keys (egui ScrollAreas use them).

16. **Seek slider commits a *stashed* fraction, and the engine anchors position on seek.** Two coupled bugs used to silently break the timeline scrubber, both fixed in tandem: (a) `mini_player.rs` reset `let mut frac = state.progress()` every frame; on the `drag_stopped` frame the user's released value was clobbered back to the OLD progress before the seek call ran, so we always seeked to where we already were. The current `draw_seek_slider` helper stashes the in-flight value in `egui::Memory` keyed by `Id::new("seek_slider_pending")`, restores it as the slider's input on the next frame, and only clears the entry when committing on `drag_stopped() || lost_focus()` or `changed() && !dragged()` (the last branch covers a bare click on the track that egui doesn't classify as a drag). (b) Even after a successful demuxer seek, `engine/output.rs::clear()` zeroed `samples_played` and `played_duration()` reported `0` to the UI for several frames while the buffer refilled — so the slider visually snapped back to start the moment the user released. The fix added `position_offset_ms: AtomicU64` to `AudioOutput` and a `set_position_anchor_ms(target_ms)` method; `played_duration() = anchor_ms + samples_played / sample_rate`. The engine's `EngineCmd::SeekFraction` handler now calls `drain_buffer()` + `set_position_anchor_ms(target_ms)` instead of `clear()`; `clear()` itself still zeros both counters and is reserved for stop / new-track. The decoder thread's post-seek `output.lock().reset_position()` only zeros the local sample counter, not the anchor, so the UI sees the new position the very next position event. **Don't reinstate `out.clear()` in the seek path** and don't bypass the `draw_seek_slider` helper from new screens; route through it (it's `pub` in `ui/components/mini_player.rs` and reused from `now_playing.rs`).

17. **Playlist page downloads new content, not replacements.** `Screen::Playlist` / `screens/playlist.rs` + `replacer/playlist.rs`. `fetch_playlist` runs one `yt-dlp --flat-playlist --dump-single-json` call on a background thread (result handed back through `PlaylistUi.fetched: Arc<Mutex<Option<Result<Playlist>>>>`); `plan_entries` maps each non-null entry to `<NN> - <Title> - <Artist>.mp3` (NN = playlist position, zero-padded to the playlist length; `Artist - Title` video titles split on the first ` - `, otherwise title = video title and artist = channel minus ` - Topic`). Files go to `<dest_root>/<sanitized folder name>/`, default folder = playlist title. Downloads reuse the Replacer's `DownloadWorker` (`screens::replacer::download_worker` is `pub(crate)` for this) with `song_id = song_id_from_path(dest_path)` and `source_path = dest_path`, so the id matches what the library assigns after `refresh_folder`. Two things to keep: (a) in flat-playlist mode yt-dlp emits `null` for `channel`/`uploader`, so the serde fields use the `null_to_empty` deserializer — plain `#[serde(default)]` fails the whole parse; (b) the "already on disk" check reads the folder once into `PlaylistUi.on_disk`, keyed by `(folder, done_count)`, instead of `Path::exists()` per row per frame (constraint 6).

18. **Playlists hub = folders; order lives in the filename prefix.** `Screen::Playlists` / `screens/playlists.rs`. Adding songs: `replacer/link_list.rs::parse_lines` classifies pasted lines (YouTube host → `Link`, else `Title`), `replacer/resolve.rs::ResolveJob` resolves them on one background thread (`Link` → `fetch_playlist`, so a playlist URL expands; `Title` → `search_ytdlp` + `score_results` top hit, empty = failure, no video fallback), then the screen queues `DownloadRequest`s into the shared `DownloadWorker` named `<renumberer::next_index> - Title - Artist.mp3` and tracks them in `PlaylistsUi.queued_ids`; when none are `Pending` it runs `renumber_folder` once so failed downloads leave no gap. Both steps live in `poll_batch` (called every frame the screen is shown) and act on `PlaylistsUi.batch_folder`, captured on **Find & download** — never on the currently selected folder, otherwise switching playlists mid-batch would drop files into, or renumber, the wrong folder. Find and Apply order are disabled while a batch into that folder is in flight (filename collisions, yt-dlp temp files). `renumberer::is_audio_entry` skips dot-prefixed files for the same reason. Reordering mutates `PlaylistsUi.order` (working copy) and only **Apply order** calls `renumberer::plan_order` + `apply` (two-phase temp rename; unlisted files are appended, foreign paths are rejected). Don't write order anywhere else (no sidecar files) — the prefix is the single source of truth the Songs page, the renumberer, and external file managers all agree on. The order list is a **merge**, not a snapshot: `merge_order` drops vanished ids and appends new songs on every `library.version()` bump, so downloads and deletes appear immediately even with unapplied edits; `is_dirty` compares against a cached `(version, folder, ids)` triple. Before enqueueing, `flag_duplicates` compares each resolved `(title, artist)` case-insensitively against the folder's songs and the batch itself; any hit parks the batch in `PlaylistsUi.pending_batch` behind a modal (Skip / Download anyway / Cancel) — never auto-skip, the user decides. Row ✕ routes through `library::handle_delete` and is disabled while the order is dirty (the renumber would invalidate the working copy's ids) or a batch is in flight.

19. **Screenshots must never show personal paths.** `docs/screenshots/*.png` are regenerated only with `cargo run --release --example capture_screenshots` (from `player/`). The tool exposes the library through directory junctions under `C:\RecurateDemo\` and keeps its `settings.toml` there, so every path on screen reads `C:\RecurateDemo\Music\…`; `Capture::privacy_check` refuses to write any image while a library root or the config path is outside that folder. Never capture screenshots by hand (Snipping Tool etc.) for the docs, never point the tool at the real roots, and keep the guard when editing the tool. The README section "Updating screenshots" describes the workflow for humans.

20. **Every colour and font comes from `ui/theme.rs`; screens never hardcode a `Color32`.** The design tokens (the design spec kept locally under `docs/`, gitignored) map onto egui in `theme::apply`; `theme::ensure` runs every frame because eframe resets `Visuals` when the OS theme flips. `theme::pal(ctx)` hands the current `Palette` to any widget. Shared widgets (`ui/widgets.rs`: buttons, inputs, path field, cards, pills, sliders, toggle, modal) are the building blocks; new UI composes them. Disabled buttons go through `widgets::disabled_button`, not `ui.add_enabled`: egui fades disabled widgets towards a colour that is transparent for secondary buttons, which made them vanish. Fonts (Inter, Newsreader, JetBrains Mono, static instances cut from the variable fonts because egui cannot pick a weight) are bundled under `assets/fonts` with their OFL licences; Windows CJK and symbol fonts stay as fallbacks. The `[ui] theme` setting is `system | light | dark`. Settings → Library paths use `rfd` for the folder dialog, run on a worker thread so the window keeps painting; the result is polled each frame from `SettingsUi.picker`.

## Commands

### Player (Rust, exists)

Run from `player/`:

```bash
cargo check --all-targets                # Type-check lib + bin + tests
cargo test                               # 76 tests (lib + library_dedup + renumberer integration)
cargo test --test renumberer             # 9 renumberer integration tests
cargo test --test library_dedup          # 4 path-canonicalization integration tests
cargo run --release                      # Launch GUI; default scan root = $PWD/music

# Replacer API backend (optional — yt-dlp is the default and runs 4-wide)
export YOUTUBE_API_KEY='AIza…'           # Git Bash; set/$env: equivalents on cmd/PS
cargo run --release                      # Then pick "YouTube Data API v3" in Replacer screen
```

Default scan root resolves to `player/music/` when run from `player/`. Settings live at `%APPDATA%/Recurate/Recurate/config/settings.toml` (auto-created on first save; `RECURATE_CONFIG_DIR` overrides the folder, used by the screenshot tool). The API key resolution order is: `YOUTUBE_API_KEY` env var (highest precedence, never persisted) → `settings.toml`'s `[replacer] youtube_api_key` (UI-set in Settings → Replacer, password field). The same Settings → Replacer panel exposes `[replacer] cookies_browser` (free-text: `chrome` / `firefox` / `edge` / `brave` / …; empty = off) which yt-dlp reads via `--cookies-from-browser` to bypass YouTube's bot-detection wall. Close the named browser before retrying — Chrome/Edge lock their cookie DB while running. If a user pastes their key into the Settings field it persists locally to settings.toml. yt-dlp and ffmpeg must be on PATH for the download/replace flow; the Replacer screen's top status pill turns red/amber when either is missing.

### Replacer (Python, not started)

(Per `plan.md` — none of these exist yet.)

```bash
pip install -r requirements.txt          # google-api-python-client, yt-dlp
winget install Gyan.FFmpeg               # ffmpeg system dep (Windows host)

python main.py --only "Suno2"            # Smoke test on a tiny folder
                                         # (override SKIP_FOLDERS for testing)
python main.py --only "AK" --limit 100   # Calibration run — tune scoring
python main.py                           # Full run
```

`Ctrl+C` mid-run must persist state up to the last fully-completed file and exit cleanly (130). The next invocation resumes.

## Host environment

Windows 11, bash shell available. Watch out for: full-width Unicode in real filenames; `os.path` separators; ffmpeg/yt-dlp must be on PATH; YOUTUBE_API_KEY via `set` (cmd) / `$env:` (PowerShell) / `export` (bash).

---

## Behavioral guidelines

Behavioral guidelines to reduce common LLM coding mistakes. Merge with project-specific instructions as needed.

Tradeoff: These guidelines bias toward caution over speed. For trivial tasks, use judgment.

### 1. Think Before Coding
Don't assume. Don't hide confusion. Surface tradeoffs.

Before implementing:

- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

### 2. Simplicity First
Minimum code that solves the problem. Nothing speculative.

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

### 3. Surgical Changes
Touch only what you must. Clean up only your own mess.

When editing existing code:

- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:

- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

### 4. Goal-Driven Execution
Define success criteria. Loop until verified.

Transform tasks into verifiable goals:

- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:

```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

These guidelines are working if: fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.
