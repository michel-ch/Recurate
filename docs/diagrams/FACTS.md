# Recurate — verified facts for diagrams

Only names in this file may appear in diagrams. Do not invent nodes, actors, routes, states or edges.
Source: `player/src/` (Rust 2021, crate `recurate`, binary `recurate.exe`).

## Stack (detected)
- Language/runtime: Rust, single desktop process. No server, no network API of its own, no database.
- UI: `eframe` + `egui` 0.28 (immediate mode). Fonts: `ui/fonts.rs` loads Windows CJK fallbacks.
- Audio: `cpal` (output), `symphonia` (decode mp3/aac/flac/ogg/wav), `rubato` (resample), `biquad` (EQ), `ringbuf`.
- Tags: `lofty`. Filesystem walk: `walkdir`.
- Persistence: `settings.toml` via `toml` + `directories` at `%APPDATA%/MusicSuite/Player/settings.toml`. Library state is the filesystem (mp3 files, `NN - Title - Artist.mp3` prefix = order). No DB, no cache server.
- Concurrency: `std::thread`, `crossbeam_channel`, `parking_lot::RwLock`, atomics.
- External processes: `yt-dlp` (search `ytsearch`, `--flat-playlist --dump-single-json`, download), `ffmpeg` (transcode to mp3). Both must be on PATH.
- External HTTP (optional backend): YouTube Data API v3 via `ureq` — `https://www.googleapis.com/youtube/v3/search` and `/youtube/v3/videos`. Auth = API key (`YOUTUBE_API_KEY` env var or `[replacer] youtube_api_key` in settings.toml). There is NO user login / session / JWT anywhere.
- No Docker, no CI config, no cloud. Launch: `start.bat` or `cargo run --release`. Tests: 68 (`cargo test`).

## Modules (real paths)
- `main.rs` — loads Settings, creates two `Library` (destination `library`, read-only `source`), spawns scan threads, `Engine::start()`, `PlaybackController::new`, runs `ui::App` in eframe.
- `settings.rs` — `Settings { scan: ScanSettings{roots, source_root}, playback: PlaybackSettings{volume, shuffle, repeat, crossfade_ms}, equalizer: EqualizerSettings{enabled, bands[10], bass_boost}, renumber: RenumberSettings{enabled, threshold}, replacer: ReplacerSettings{youtube_api_key, cookies_browser} }`.
- `domain/model.rs` — `Song{id,title,artist,album,album_artist,duration,year,genre,composer,track_no,path,has_embedded_art}`, `PlaybackState`, `RepeatMode{Off,All,One}`, `SortOption`, `Screen` enum.
- `data/` — `Library` (`scan`, `refresh_folder`, `remove_song`, `version()`, `folders()`, `songs_in_folder`, status Idle/Scanning), `scanner.rs` (walkdir), `tags.rs` (`read_song` via lofty, panic-caught), `fingerprint.rs` (`compute_fingerprint` 256-bit acoustic hash v9), `duplicates.rs` (groups by fingerprint+duration+parent folder).
- `engine/` — `Engine` thread. `EngineCmd { Load{path,autoplay}, Play, Pause, Stop, SeekFraction(f32), SetVolume(f32), Shutdown }`. `EngineEvent { LoadStarted, LoadFailed, Started{duration}, Position{current_ms,duration_ms}, Paused, Resumed, EndOfTrack }`. `decoder.rs` (symphonia), `output.rs` (`AudioOutput` cpal stream + ring buffer + position anchor), `eq.rs` (biquad bands).
- `playback/` — `PlaybackController` (`play_songs`, `play_pause`, `next`, `previous`, `jump_to`, `seek_fraction`, `set_volume`, `set_shuffle`, `cycle_repeat`, `remove_from_queue`, `state_snapshot`, `queue_snapshot`), `queue.rs` (`Queue{items,current}`), `snapshot.rs`, `deletion.rs` (`delete_song` → fs remove, optional renumber).
- `renumberer.rs` — `analyze`, `apply` (two-phase temp rename), `renumber_folder`, `plan_order(folder, ordered)`, `next_index`, `is_audio_entry`.
- `replacer/` — `title_cleaner.rs` (clean filename stem → query, keeps Slowed/Reverb tags), `youtube.rs` (`search_ytdlp`, `ytdlp_available` cached), `youtube_api.rs` (`search_api` over ureq), `scoring.rs` (`score_results`, `is_audio_candidate` audio-only filter), `search_worker.rs` (`SearchWorker`, 4 threads, `SearchRequest{song_id,filename_stem,expected_duration_secs,max_results,backend,api_key,cookies_browser}`, `SongState { Idle, Pending, Done{cleaned,results}, Failed{cleaned,error} }`, pause flag, queue_len), `download.rs` (`download_audio_mp3` runs yt-dlp → `.<stem>.dl.mp3` temp → `override_tags_from_filename` via lofty → atomic rename to dest_path; `ffmpeg_available` cached), `download_worker.rs` (`DownloadWorker`, 3 threads, `DownloadRequest{song_id,source_path,dest_path,video_url,cookies_browser}`, `DownloadState { Idle, Pending, Done, Failed{error} }`, calls `Library::refresh_folder` after each success), `playlist.rs` (`fetch_playlist` yt-dlp flat playlist → `Playlist{title,entries}`, `plan_entries`, `sanitize_filename`, `split_artist_title`), `link_list.rs` (`parse_lines` → `LineItem::{Link,Title}`), `resolve.rs` (`ResolveJob` one thread: Link→`fetch_playlist`, Title→`search_ytdlp`+`score_results` → `Resolved{title,artist,video_url}` / `LineOutcome`), `sync.rs` (`missing_in_dest`, `copy_file`).
- `ui/` — `app.rs` (`App` struct: holds `library`, `source`, `playback`, `settings`, `screen`, cached views `cached_library_view`, `cached_replacer_view`, `cached_folders`, `cached_duplicates`, `cached_missing`, fingerprint state, `toasts`, `playlist: PlaylistUi`, `playlists: PlaylistsUi`; `update()` each frame: `rescan_if_paths_changed`, `handle_shortcuts` (Space, Ctrl+→, Ctrl+←, gated on `wants_keyboard_input`), draws `TopBottomPanel top_bar`, `CentralPanel` screen, `TopBottomPanel mini_player`, conditional `status_bar`), `toasts.rs` (`Toast`, `ToastLevel{Info,Warn,Error}`, `toast_info/warn/error`, TTL 4/6/9 s), `fonts.rs`.
- `ui/components/` — `top_bar.rs` (nav: Songs, Albums, Artists, Folders, Playlists, Queue, Replacer, Missing, Playlist, Duplicates, Settings; song count), `mini_player.rs` (⏮ ▶/⏸ ⏭, `draw_seek_slider`, volume, Now Playing button), `song_row.rs` (`draw`, `draw_with_options`, `RowOptions{show_remove}`, `RowAction{clicked,remove_clicked}`).
- `ui/screens/` — `library.rs` (`draw` AllSongs paginated 50/page with `draw_header`/`draw_sort_picker`, `draw_search`, `draw_albums`, `draw_album_detail`, `draw_artists`, `draw_artist_detail`, `draw_folders`, `handle_delete`), `now_playing.rs`, `queue.rs`, `replacer.rs` (`download_worker()` shared static, `ScopeStats`, folder multi-select popup, Search first N / Search all N / Start replace top match, pause/resume), `missing.rs` (`spawn_copy_all`, `spawn_renumber_all`), `duplicates.rs` (Recompute, bulk delete two-click), `playlist.rs` (`PlaylistUi{url,folder_name,plan,fetching,fetched,on_disk}`), `playlists.rs` (`PlaylistsUi{selected,new_name,paste_text,resolve,batch_folder,queued_ids,order,pending_batch,...}`; functions `draw_folder_list`, `draw_editor`, `draw_add_songs`, `poll_batch`, `enqueue_batch`, `draw_batch_status`, `draw_duplicate_prompt`, `draw_reorder`, `flag_duplicates`, `merge_order`, `apply_order`), `settings.rs` (`draw`, `draw_equalizer`).

## Screen enum (real variants) and navigation
`Library, AllSongs, AlbumsList, AlbumDetail(String), ArtistsList, ArtistDetail(String), Folders, Playlists, NowPlaying, Equalizer, Search, Queue, Replacer, Duplicates, Missing, Playlist, Settings`.
Top bar reaches: AllSongs, AlbumsList, ArtistsList, Folders, Playlists, Queue, Replacer, Missing, Playlist, Duplicates, Settings. AlbumDetail/ArtistDetail from their lists. NowPlaying from mini player button. Equalizer from Settings ("Open as screen"). Search via search field on library screens. Screens without bottom nav / mini player: NowPlaying, Settings, Replacer, Duplicates, Missing, Playlist (each has a "← Back" button restoring `previous_screen`). No auth gate anywhere.

## Key flows (verified)
### A. Play a song (in-process)
User clicks a `song_row` → `App` → `PlaybackController::play_songs(list, idx)` → `Queue` updated → `Engine` receives `EngineCmd::Load{path, autoplay:true}` over crossbeam channel → decoder thread (symphonia) → `AudioOutput` (cpal) → `EngineEvent::Started` / `Position` events back → `PlaybackController` state → `App` repaints mini player. `EndOfTrack` → controller picks next per shuffle/repeat.

### B. Replace a rip (Replacer screen)
1. `SearchRequest` per song → `SearchWorker` (4 threads) → `title_cleaner` → `search_ytdlp` (yt-dlp `ytsearch10:"<query>"`) OR `search_api` (googleapis, API key) → `score_results` + `is_audio_candidate` → `SongState::Done{results}` (or `Failed`).
2. Click "Start replace top match" → `DownloadRequest{song_id, source_path, dest_path = dest_root/<mirrored relative path>, video_url = top result}` → `DownloadWorker` (3 threads) → `download_audio_mp3` runs yt-dlp (+ffmpeg, `--embed-metadata --embed-thumbnail`) to `.<stem>.dl.mp3` → `override_tags_from_filename` (lofty) → atomic rename over `dest_path` → `Library::refresh_folder` → `DownloadState::Done` → Replacer stats update. Failure → `DownloadState::Failed{error}` → toast.

### C. Add songs to a playlist (Playlists screen)
Paste text → `parse_lines` → `ResolveJob::start` (1 thread; Link→`fetch_playlist`, Title→`search_ytdlp`+`score_results`) → `poll_batch` each frame → `flag_duplicates` vs folder songs → if any: `pending_batch` + prompt window "Already in playlist" (Skip duplicates / Download anyway / Cancel) → `enqueue_batch` → `DownloadRequest`s named `<next_index> - Title - Artist.mp3` → shared `DownloadWorker` → files land → `Library::refresh_folder` → `merge_order` refreshes list → when no `Pending` left, `renumber_folder`.

### D. Reorder playlist
Drag / ▲▼ / First / Last / Move edits `PlaylistsUi.order` (working copy) → "Apply order" → `renumberer::plan_order` → `apply` (two-phase temp rename) → `Library::refresh_folder`.

### E. Delete a song
Row ✕ → `library::handle_delete` → `deletion::delete_song` (fs remove) → `renumber_folder` (if enabled) → `Library::refresh_folder` → `library.version()` bump → cached views rebuild.

### F. Missing / bootstrap
Missing screen → `cached_missing` from `sync::missing_in_dest(source, dest)` → "Copy all" → `spawn_copy_all` thread → `sync::copy_file` per file → `refresh_folder`.

### G. Duplicates
`App::start_fingerprinting` spawns N workers → `compute_fingerprint` per song → `fingerprints` map → `duplicates::find_groups` (cached by `(library_version, fingerprint_progress)`) → bulk delete = `delete_song` per extra + one `renumber_folder` per folder.

## State machines (verified)
- `SongState` (search): Idle → Pending (enqueue) → Done{results} | Failed{error}. "Clear results" → Idle.
- `DownloadState`: Idle → Pending (enqueue) → Done | Failed{error}. Re-queue from Failed → Pending.
- Worker pause: `paused: AtomicBool` — Pause keeps queued items queued, in-flight finish; Resume continues.
- `RepeatMode`: Off → All → One → Off (cycle_repeat).
- Playlists batch UI: Idle → Resolving (`ResolveJob.running`) → [duplicates found] PendingPrompt → Downloading (`queued_ids` with Pending) → Renumbered/Idle. Cancel from PendingPrompt → Idle.
- Library status: Idle | Scanning.

## Error handling (verified)
- Failures tied to a user action → `app.toast_error/warn/info` (top-right, TTL 4/6/9 s, manual ✕). Examples: download failed (yt-dlp/ffmpeg error string), fetch playlist failed, no audio match for a pasted title, bulk delete skipped stale id, path unreadable.
- Background failures → `tracing::warn!/error!` only (scan errors, per-frame churn).
- yt-dlp "Sign in to confirm you're not a bot" / HTTP 403 → user sets `cookies_browser` in Settings; hint shown in Replacer.
- API backend quota: `search.list` = 100 units; UI caps "Search first N" at 25 and warns above 99.
- lofty/symphonia panics are caught (`catch_unwind`) in `tags::read_song`, `download_worker::do_one`, `fingerprint::compute_fingerprint` and turned into `Err`.
- Tool missing: `ytdlp_available()` / `ffmpeg_available()` cached `OnceCell`; Replacer/Playlist status pill red/amber.

## Layers (for layer stack)
1. User input: egui window (mouse, keyboard shortcuts).
2. UI: `ui::App` frame loop, screens, components, toasts, cached views.
3. Coordination: `PlaybackController`, `SearchWorker`, `DownloadWorker`, `ResolveJob`, fingerprint workers, scan threads — all via crossbeam channels / `Arc<RwLock>`.
4. Core services: `Library`, `renumberer`, `replacer::{title_cleaner, scoring, download, playlist, sync}`, `data::{tags, fingerprint, duplicates}`, `Engine`.
5. External processes / network: `yt-dlp`, `ffmpeg`, YouTube Data API v3 (ureq).
6. Storage: filesystem (destination root `music/`, source root `music_original/`), `settings.toml`, cpal audio device.
