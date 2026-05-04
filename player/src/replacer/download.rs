use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::Accessor;
use once_cell::sync::OnceCell;

use crate::data::tags::split_prefix;

pub fn ffmpeg_available() -> bool {
    static CACHE: OnceCell<bool> = OnceCell::new();
    *CACHE.get_or_init(|| {
        Command::new("ffmpeg")
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

pub fn download_audio_mp3(
    video_url: &str,
    target_path: &Path,
    cookies_browser: Option<&str>,
) -> Result<()> {
    let parent = target_path
        .parent()
        .ok_or_else(|| anyhow!("target path has no parent: {}", target_path.display()))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("create dir {}", parent.display()))?;

    let stem = target_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("bad target stem: {}", target_path.display()))?;
    let tmp_stem = format!(".{stem}.dl");
    let tmp_template = parent.join(format!("{tmp_stem}.%(ext)s"));
    let tmp_mp3 = parent.join(format!("{tmp_stem}.mp3"));

    let _ = std::fs::remove_file(&tmp_mp3);

    let mut cmd = Command::new("yt-dlp");
    cmd.args([
        "-x",
        "--audio-format",
        "mp3",
        "--audio-quality",
        "0",
        "--embed-metadata",
        "--embed-thumbnail",
        "--convert-thumbnails",
        "jpg",
        "--parse-metadata",
        "%(uploader|)s:^(?P<artist>.+?)(?: - Topic)?$",
        "--parse-metadata",
        "%(title)s:^(?P<artist>[^-]+?) - (?P<title>.+?)(?: \\(.+\\))?$",
        "--no-warnings",
        "--no-playlist",
        "--socket-timeout",
        "30",
    ]);
    if let Some(browser) = cookies_browser.map(str::trim).filter(|s| !s.is_empty()) {
        cmd.args(["--cookies-from-browser", browser]);
    }
    let output = cmd
        .arg("-o")
        .arg(&tmp_template)
        .arg(video_url)
        .output()
        .context("yt-dlp not found on PATH")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_file(&tmp_mp3);
        return Err(anyhow!(
            "yt-dlp exited {}: {}",
            output.status,
            stderr.trim()
        ));
    }

    if !tmp_mp3.exists() {
        return Err(anyhow!(
            "yt-dlp finished but {} not found (is ffmpeg on PATH?)",
            tmp_mp3.display()
        ));
    }

    if let Err(e) = override_tags_from_filename(&tmp_mp3, stem) {
        tracing::warn!("override tags failed for {}: {e:#}", tmp_mp3.display());
    }

    rename_replace(&tmp_mp3, target_path).with_context(|| {
        format!(
            "replace {} with {}",
            target_path.display(),
            tmp_mp3.display()
        )
    })?;

    Ok(())
}

fn rename_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            std::fs::copy(from, to)?;
            std::fs::remove_file(from)?;
            Ok(())
        }
    }
}

fn parse_filename_meta(stem: &str) -> Option<(Option<u32>, String, String)> {
    let stem = stem.trim();
    let (digits, rest) = split_prefix(stem)?;
    let track_no = digits.parse::<u32>().ok();
    let (title, artist) = rest.rsplit_once(" - ")?;
    let title = title.trim();
    let artist = artist.trim();
    if title.is_empty() || artist.is_empty() {
        return None;
    }
    Some((track_no, title.to_string(), artist.to_string()))
}

fn override_tags_from_filename(mp3_path: &Path, target_stem: &str) -> Result<()> {
    let Some((track_no, title, artist)) = parse_filename_meta(target_stem) else {
        return Ok(());
    };

    let mut tagged = Probe::open(mp3_path)
        .with_context(|| format!("probe {}", mp3_path.display()))?
        .read()
        .with_context(|| format!("read tags {}", mp3_path.display()))?;

    let has_primary = tagged.primary_tag().is_some();
    let tag = if has_primary {
        tagged.primary_tag_mut().expect("checked above")
    } else {
        tagged
            .first_tag_mut()
            .ok_or_else(|| anyhow!("no tag in {}", mp3_path.display()))?
    };

    tag.set_title(title);
    tag.set_artist(artist);
    if let Some(t) = track_no {
        tag.set_track(t);
    }

    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(mp3_path)
        .with_context(|| format!("open for write {}", mp3_path.display()))?;
    tagged
        .save_to(&mut file, WriteOptions::default())
        .with_context(|| format!("save tags {}", mp3_path.display()))?;

    Ok(())
}
