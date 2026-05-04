use std::process::Command;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use once_cell::sync::OnceCell;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct YtVideo {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub uploader: String,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub view_count: Option<u64>,
}

impl YtVideo {
    pub fn channel_or_uploader(&self) -> &str {
        if !self.channel.is_empty() {
            &self.channel
        } else {
            &self.uploader
        }
    }

    pub fn duration_seconds(&self) -> Option<u64> {
        self.duration.map(|d| d.round() as u64)
    }

    pub fn watch_url(&self) -> String {
        if !self.url.is_empty() {
            self.url.clone()
        } else {
            format!("https://www.youtube.com/watch?v={}", self.id)
        }
    }
}

pub fn search_ytdlp(
    query: &str,
    max_results: usize,
    cookies_browser: Option<&str>,
) -> Result<Vec<YtVideo>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let n = max_results.clamp(1, 25);
    let term = format!("ytsearch{n}:{query}");

    let mut cmd = Command::new("yt-dlp");
    cmd.args([
        "--dump-json",
        "--flat-playlist",
        "--no-warnings",
        "--skip-download",
        "--socket-timeout",
        "15",
    ]);
    if let Some(browser) = cookies_browser.map(str::trim).filter(|s| !s.is_empty()) {
        cmd.args(["--cookies-from-browser", browser]);
    }
    cmd.arg(&term);

    let output = cmd
        .output()
        .context("yt-dlp not found on PATH (install via `winget install yt-dlp`)")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("yt-dlp exited {}: {}", output.status, stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::with_capacity(n);
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<YtVideo>(line) {
            Ok(v) => results.push(v),
            Err(e) => tracing::debug!("skip non-video json line: {e}"),
        }
    }
    Ok(results)
}

pub fn ytdlp_available() -> bool {
    static CACHE: OnceCell<bool> = OnceCell::new();
    *CACHE.get_or_init(|| {
        Command::new("yt-dlp")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

pub fn format_duration(d: Option<u64>) -> String {
    match d {
        Some(s) => {
            let dur = Duration::from_secs(s);
            let total = dur.as_secs();
            format!("{}:{:02}", total / 60, total % 60)
        }
        None => "—".to_string(),
    }
}
