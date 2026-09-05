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
