use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub duration: Duration,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub track_no: Option<i32>,
    pub path: PathBuf,
    pub has_embedded_art: bool,
}

impl Song {
    pub fn formatted_duration(&self) -> String {
        let total = self.duration.as_secs();
        format!("{}:{:02}", total / 60, total % 60)
    }

    pub fn folder(&self) -> Option<&Path> {
        self.path.parent()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn next(self) -> Self {
        match self {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        }
    }
}

impl Default for RepeatMode {
    fn default() -> Self {
        RepeatMode::Off
    }
}

#[derive(Debug, Clone, Default)]
pub struct PlaybackState {
    pub current_song: Option<Song>,
    pub is_playing: bool,
    pub current_position_ms: u64,
    pub duration_ms: u64,
    pub shuffle_enabled: bool,
    pub repeat_mode: RepeatMode,
}

impl PlaybackState {
    pub fn progress(&self) -> f32 {
        if self.duration_ms == 0 {
            0.0
        } else {
            (self.current_position_ms as f32 / self.duration_ms as f32).clamp(0.0, 1.0)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortOption {
    TitleAsc,
    TitleDesc,
    ArtistAsc,
    ArtistDesc,
    AlbumAsc,
    AlbumDesc,
    DurationAsc,
    DurationDesc,
    TrackAsc,
    TrackDesc,
    FilenameAsc,
    FilenameDesc,
    Shuffle,
}

impl SortOption {
    pub fn label(self) -> &'static str {
        match self {
            SortOption::TitleAsc => "Title A→Z",
            SortOption::TitleDesc => "Title Z→A",
            SortOption::ArtistAsc => "Artist A→Z",
            SortOption::ArtistDesc => "Artist Z→A",
            SortOption::AlbumAsc => "Album A→Z",
            SortOption::AlbumDesc => "Album Z→A",
            SortOption::DurationAsc => "Duration ↑",
            SortOption::DurationDesc => "Duration ↓",
            SortOption::TrackAsc => "Track #",
            SortOption::TrackDesc => "Track # (desc)",
            SortOption::FilenameAsc => "Filename A→Z",
            SortOption::FilenameDesc => "Filename Z→A",
            SortOption::Shuffle => "Shuffle",
        }
    }

    pub const ALL: &'static [SortOption] = &[
        SortOption::TitleAsc,
        SortOption::TitleDesc,
        SortOption::ArtistAsc,
        SortOption::ArtistDesc,
        SortOption::AlbumAsc,
        SortOption::AlbumDesc,
        SortOption::DurationAsc,
        SortOption::DurationDesc,
        SortOption::TrackAsc,
        SortOption::TrackDesc,
        SortOption::FilenameAsc,
        SortOption::FilenameDesc,
        SortOption::Shuffle,
    ];
}

pub fn sort_songs(songs: &mut [Song], opt: SortOption) {
    match opt {
        SortOption::TitleAsc => songs.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase())),
        SortOption::TitleDesc => songs.sort_by(|a, b| b.title.to_lowercase().cmp(&a.title.to_lowercase())),
        SortOption::ArtistAsc => songs.sort_by(|a, b| a.artist.to_lowercase().cmp(&b.artist.to_lowercase())),
        SortOption::ArtistDesc => songs.sort_by(|a, b| b.artist.to_lowercase().cmp(&a.artist.to_lowercase())),
        SortOption::AlbumAsc => songs.sort_by(|a, b| a.album.to_lowercase().cmp(&b.album.to_lowercase())),
        SortOption::AlbumDesc => songs.sort_by(|a, b| b.album.to_lowercase().cmp(&a.album.to_lowercase())),
        SortOption::DurationAsc => songs.sort_by_key(|s| s.duration),
        SortOption::DurationDesc => {
            songs.sort_by_key(|s| s.duration);
            songs.reverse();
        }
        SortOption::TrackAsc => songs.sort_by_key(|s| s.track_no.unwrap_or(i32::MAX)),
        SortOption::TrackDesc => {
            songs.sort_by_key(|s| s.track_no.unwrap_or(i32::MIN));
            songs.reverse();
        }
        SortOption::FilenameAsc => songs.sort_by(|a, b| a.path.file_name().cmp(&b.path.file_name())),
        SortOption::FilenameDesc => songs.sort_by(|a, b| b.path.file_name().cmp(&a.path.file_name())),
        SortOption::Shuffle => {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            use std::time::SystemTime;
            let seed = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            songs.sort_by_cached_key(|s| {
                let mut h = DefaultHasher::new();
                s.id.hash(&mut h);
                seed.hash(&mut h);
                h.finish()
            });
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Screen {
    Library,
    AllSongs,
    AlbumsList,
    AlbumDetail(String),
    ArtistsList,
    ArtistDetail(String),
    Folders,
    Playlists,
    NowPlaying,
    Equalizer,
    Search,
    Queue,
    Replacer,
    Duplicates,
    Missing,
    Settings,
}

impl Screen {
    pub fn shows_bottom_nav(&self) -> bool {
        !matches!(
            self,
            Screen::NowPlaying
                | Screen::Settings
                | Screen::Replacer
                | Screen::Duplicates
                | Screen::Missing
        )
    }
}
