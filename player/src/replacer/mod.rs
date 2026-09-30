pub mod download;
pub mod download_worker;
pub mod link_list;
pub mod playlist;
pub mod resolve;
pub mod scoring;
pub mod search_worker;
pub mod sync;
pub mod title_cleaner;
pub mod youtube;
pub mod youtube_api;

use serde::{Deserialize, Serialize};

/// `Command` for the yt-dlp / ffmpeg console tools. The release build is a
/// GUI-subsystem app with no console, so Windows would open a new terminal
/// window for every spawn; `CREATE_NO_WINDOW` keeps them hidden (and the
/// ffmpeg that yt-dlp starts inherits the hidden console).
pub fn command(program: &str) -> std::process::Command {
    #[allow(unused_mut)]
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

pub use download::{download_audio_mp3, ffmpeg_available};
pub use download_worker::{DownloadRequest, DownloadState, DownloadWorker};
pub use scoring::{score_results, ScoredResult};
pub use search_worker::{SearchRequest, SearchResponse, SearchWorker, SongState};
pub use title_cleaner::{clean_title, CleanedTitle, Confidence};
pub use youtube::{search_ytdlp, YtVideo};
pub use youtube_api::search_api;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchBackend {
    Ytdlp,
    Api,
}

impl Default for SearchBackend {
    fn default() -> Self {
        SearchBackend::Ytdlp
    }
}

impl SearchBackend {
    pub fn label(self) -> &'static str {
        match self {
            SearchBackend::Ytdlp => "yt-dlp (no quota)",
            SearchBackend::Api => "YouTube Data API v3",
        }
    }
}
