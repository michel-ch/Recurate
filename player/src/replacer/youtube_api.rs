use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use crate::replacer::youtube::YtVideo;

const SEARCH_URL: &str = "https://www.googleapis.com/youtube/v3/search";
const VIDEOS_URL: &str = "https://www.googleapis.com/youtube/v3/videos";

#[derive(Debug, Deserialize)]
struct SearchResponse {
    items: Vec<SearchItem>,
}

#[derive(Debug, Deserialize)]
struct SearchItem {
    id: SearchId,
    snippet: SearchSnippet,
}

#[derive(Debug, Deserialize)]
struct SearchId {
    #[serde(rename = "videoId")]
    video_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchSnippet {
    title: String,
    #[serde(rename = "channelTitle")]
    channel_title: String,
}

#[derive(Debug, Deserialize)]
struct VideosResponse {
    items: Vec<VideoItem>,
}

#[derive(Debug, Deserialize)]
struct VideoItem {
    id: String,
    #[serde(rename = "contentDetails")]
    content_details: VideoContentDetails,
    statistics: Option<VideoStatistics>,
}

#[derive(Debug, Deserialize)]
struct VideoContentDetails {
    duration: String,
}

#[derive(Debug, Deserialize)]
struct VideoStatistics {
    #[serde(rename = "viewCount")]
    view_count: Option<String>,
}

pub fn search_api(query: &str, max_results: usize, api_key: &str) -> Result<Vec<YtVideo>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    if api_key.trim().is_empty() {
        return Err(anyhow!("YOUTUBE_API_KEY is not set"));
    }
    let n = max_results.clamp(1, 25);

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .build();

    let search_resp: SearchResponse = agent
        .get(SEARCH_URL)
        .query("part", "snippet")
        .query("type", "video")
        .query("maxResults", &n.to_string())
        .query("q", query)
        .query("key", api_key)
        .call()
        .map_err(api_error)
        .context("youtube search.list failed")?
        .into_json()
        .context("parse search.list response")?;

    let mut items: Vec<(String, String, String)> = Vec::with_capacity(n);
    for it in search_resp.items {
        if let Some(vid) = it.id.video_id {
            items.push((vid, it.snippet.title, it.snippet.channel_title));
        }
    }
    if items.is_empty() {
        return Ok(Vec::new());
    }

    let ids = items
        .iter()
        .map(|(id, _, _)| id.as_str())
        .collect::<Vec<_>>()
        .join(",");

    let videos_resp: VideosResponse = agent
        .get(VIDEOS_URL)
        .query("part", "contentDetails,statistics")
        .query("id", &ids)
        .query("key", api_key)
        .call()
        .map_err(api_error)
        .context("youtube videos.list failed")?
        .into_json()
        .context("parse videos.list response")?;

    let mut out = Vec::with_capacity(items.len());
    for (vid, title, channel) in items {
        let detail = videos_resp.items.iter().find(|v| v.id == vid);
        let duration_secs = detail.and_then(|d| parse_iso8601_duration(&d.content_details.duration));
        let view_count = detail
            .and_then(|d| d.statistics.as_ref())
            .and_then(|s| s.view_count.as_ref())
            .and_then(|v| v.parse::<u64>().ok());
        out.push(YtVideo {
            id: vid.clone(),
            title,
            channel,
            uploader: String::new(),
            duration: duration_secs.map(|s| s as f64),
            url: format!("https://www.youtube.com/watch?v={vid}"),
            view_count,
        });
    }
    Ok(out)
}

fn api_error(e: ureq::Error) -> anyhow::Error {
    match e {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            anyhow!("HTTP {code}: {}", body.trim())
        }
        ureq::Error::Transport(t) => anyhow!("transport: {t}"),
    }
}

fn parse_iso8601_duration(s: &str) -> Option<u64> {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'P') {
        return None;
    }
    let mut secs: u64 = 0;
    let mut in_time = false;
    let mut num: u64 = 0;
    let mut have_num = false;
    for &b in &bytes[1..] {
        match b {
            b'T' => in_time = true,
            b'0'..=b'9' => {
                num = num.saturating_mul(10).saturating_add((b - b'0') as u64);
                have_num = true;
            }
            b'H' if in_time => {
                secs = secs.saturating_add(num.saturating_mul(3600));
                num = 0;
                have_num = false;
            }
            b'M' if in_time => {
                secs = secs.saturating_add(num.saturating_mul(60));
                num = 0;
                have_num = false;
            }
            b'S' if in_time => {
                secs = secs.saturating_add(num);
                num = 0;
                have_num = false;
            }
            _ => {
                num = 0;
                have_num = false;
            }
        }
    }
    let _ = have_num;
    Some(secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_iso8601_durations() {
        assert_eq!(parse_iso8601_duration("PT3M45S"), Some(225));
        assert_eq!(parse_iso8601_duration("PT1H2M3S"), Some(3723));
        assert_eq!(parse_iso8601_duration("PT45S"), Some(45));
        assert_eq!(parse_iso8601_duration("PT1H"), Some(3600));
    }
}
