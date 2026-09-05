//! Download a whole YouTube playlist into a destination subfolder.
//!
//! `fetch_playlist` shells out to `yt-dlp --flat-playlist --dump-single-json`
//! (one process, no per-video metadata fetch) and `plan_entries` turns the
//! result into `<NN> - <Title> - <Artist>.mp3` file names — the library's
//! track-prefixed convention, so `download::override_tags_from_filename`,
//! the renumberer, and track-number sorting all apply to the new files
//! unchanged.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

/// yt-dlp emits `null` (not a missing key) for fields it couldn't resolve
/// in `--flat-playlist` mode, so plain `#[serde(default)]` isn't enough.
fn null_to_empty<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlaylistEntry {
    #[serde(default, deserialize_with = "null_to_empty")]
    pub id: String,
    #[serde(default, deserialize_with = "null_to_empty")]
    pub title: String,
    #[serde(default, deserialize_with = "null_to_empty")]
    pub uploader: String,
    #[serde(default, deserialize_with = "null_to_empty")]
    pub channel: String,
    #[serde(default, deserialize_with = "null_to_empty")]
    pub url: String,
    #[serde(default)]
    pub duration: Option<f64>,
}

impl PlaylistEntry {
    pub fn watch_url(&self) -> String {
        if !self.url.is_empty() {
            self.url.clone()
        } else {
            format!("https://www.youtube.com/watch?v={}", self.id)
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Playlist {
    #[serde(default, deserialize_with = "null_to_empty")]
    pub title: String,
    #[serde(default)]
    pub entries: Vec<Option<PlaylistEntry>>,
}

/// One planned download: where the file goes and what to fetch.
#[derive(Debug, Clone)]
pub struct PlannedTrack {
    pub index: usize,
    pub file_name: String,
    pub video_url: String,
    pub video_title: String,
}

pub fn fetch_playlist(url: &str, cookies_browser: Option<&str>) -> Result<Playlist> {
    let url = url.trim();
    if url.is_empty() {
        return Err(anyhow!("playlist URL is empty"));
    }
    let mut cmd = Command::new("yt-dlp");
    cmd.args([
        "--flat-playlist",
        "--dump-single-json",
        "--skip-download",
        "--no-warnings",
        "--yes-playlist",
        "--socket-timeout",
        "30",
    ]);
    if let Some(browser) = cookies_browser.map(str::trim).filter(|s| !s.is_empty()) {
        cmd.args(["--cookies-from-browser", browser]);
    }
    let output = cmd
        .arg(url)
        .output()
        .context("yt-dlp not found on PATH (install via `winget install yt-dlp`)")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("yt-dlp exited {}: {}", output.status, stderr.trim()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut playlist: Playlist =
        serde_json::from_str(stdout.trim()).context("parse yt-dlp playlist json")?;
    // A bare video URL yields a single-video object with no `entries`; treat
    // it as a one-item playlist so the screen still works.
    if playlist.entries.is_empty() {
        if let Ok(single) = serde_json::from_str::<PlaylistEntry>(stdout.trim()) {
            if !single.id.is_empty() {
                playlist.entries.push(Some(single));
            }
        }
    }
    Ok(playlist)
}

/// Build the per-track file plan. Unavailable entries (yt-dlp emits `null`
/// for deleted/private videos) are skipped but keep their position so the
/// numbering matches the playlist order on YouTube.
pub fn plan_entries(playlist: &Playlist) -> Vec<PlannedTrack> {
    let width = playlist.entries.len().to_string().len().max(2);
    playlist
        .entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| e.as_ref().map(|e| (i + 1, e)))
        .filter(|(_, e)| !e.id.is_empty() && !e.title.is_empty())
        .map(|(index, e)| {
            let (artist, title) = split_artist_title(&e.title, channel_or_uploader(e));
            let stem = format!("{index:0width$} - {title} - {artist}");
            PlannedTrack {
                index,
                file_name: format!("{}.mp3", sanitize_filename(&stem)),
                video_url: e.watch_url(),
                video_title: e.title.clone(),
            }
        })
        .collect()
}

pub fn dest_folder(dest_root: &Path, folder_name: &str) -> PathBuf {
    dest_root.join(sanitize_filename(folder_name.trim()))
}

fn channel_or_uploader(e: &PlaylistEntry) -> &str {
    if !e.channel.is_empty() {
        &e.channel
    } else {
        &e.uploader
    }
}

/// `"Artist - Title"` video titles split on the first ` - `; otherwise the
/// whole title is the title and the channel (minus yt-dlp's ` - Topic`
/// suffix) is the artist.
fn split_artist_title(video_title: &str, channel: &str) -> (String, String) {
    let t = video_title.trim();
    if let Some((a, b)) = t.split_once(" - ") {
        let a = a.trim();
        let b = b.trim();
        if !a.is_empty() && !b.is_empty() {
            return (a.to_string(), b.to_string());
        }
    }
    let artist = channel
        .trim()
        .strip_suffix(" - Topic")
        .unwrap_or(channel.trim())
        .trim();
    let artist = if artist.is_empty() { "Unknown" } else { artist };
    (artist.to_string(), t.to_string())
}

/// Replace Windows-illegal characters with the same full-width substitutes
/// yt-dlp uses, so playlist downloads look like the rest of the dataset.
pub fn sanitize_filename(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        out.push(match c {
            '"' => '＂',
            '|' => '｜',
            '?' => '？',
            '*' => '＊',
            ':' => '：',
            '<' => '＜',
            '>' => '＞',
            '/' => '⧸',
            '\\' => '⧹',
            c if c.is_control() => ' ',
            c => c,
        });
    }
    let trimmed = out.trim().trim_end_matches('.').to_string();
    if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(title: &str, channel: &str) -> Option<PlaylistEntry> {
        Some(PlaylistEntry {
            id: "abc".into(),
            title: title.into(),
            uploader: String::new(),
            channel: channel.into(),
            url: String::new(),
            duration: None,
        })
    }

    #[test]
    fn splits_artist_title_when_dash_present() {
        assert_eq!(
            split_artist_title("Powfu - death bed", "whatever"),
            ("Powfu".into(), "death bed".into())
        );
    }

    #[test]
    fn falls_back_to_topic_channel() {
        assert_eq!(
            split_artist_title("death bed", "Powfu - Topic"),
            ("Powfu".into(), "death bed".into())
        );
    }

    #[test]
    fn plan_numbers_by_playlist_position_and_skips_nulls() {
        let pl = Playlist {
            title: "Mix".into(),
            entries: vec![entry("A - One", "x"), None, entry("Three", "Y - Topic")],
        };
        let plan = plan_entries(&pl);
        assert_eq!(plan.len(), 2);
        assert_eq!(plan[0].file_name, "01 - One - A.mp3");
        assert_eq!(plan[1].index, 3);
        assert_eq!(plan[1].file_name, "03 - Three - Y.mp3");
    }

    #[test]
    fn sanitize_uses_fullwidth_substitutes() {
        assert_eq!(sanitize_filename("a/b: \"c\"?"), "a⧸b： ＂c＂？");
        assert_eq!(sanitize_filename("  ..."), "untitled");
    }

    #[test]
    fn parses_flat_playlist_json() {
        let json = r#"{"title":"P","entries":[{"id":"x","title":"T","channel":null,"uploader":null},null]}"#;
        let pl: Playlist = serde_json::from_str(json).unwrap();
        assert_eq!(pl.entries.len(), 2);
        assert!(pl.entries[1].is_none());
        assert_eq!(pl.entries[0].as_ref().unwrap().channel, "");
    }
}
