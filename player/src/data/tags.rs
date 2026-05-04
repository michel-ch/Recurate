use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::Accessor;

use crate::data::scanner::song_id_from_path;
use crate::domain::Song;

pub fn read_song(path: &Path) -> Result<Song> {
    let id = song_id_from_path(path);

    let tagged = match catch_unwind(AssertUnwindSafe(|| Probe::open(path)?.read())) {
        Ok(r) => r?,
        Err(_) => {
            return Err(anyhow!(
                "lofty panicked reading {} (likely malformed ID3v1 tag with non-ASCII title \
                 truncated mid-codepoint)",
                path.display()
            ))
        }
    };
    let properties = tagged.properties();
    let duration = properties.duration();

    let primary = tagged.primary_tag().or_else(|| tagged.first_tag());

    let mut title = String::new();
    let mut artist = String::new();
    let mut album = String::new();
    let mut album_artist = String::new();
    let mut year: Option<i32> = None;
    let mut genre: Option<String> = None;
    let mut composer: Option<String> = None;
    let mut track_no: Option<i32> = None;
    let mut has_embedded_art = false;

    if let Some(tag) = primary {
        title = tag.title().unwrap_or_default().to_string();
        artist = tag.artist().unwrap_or_default().to_string();
        album = tag.album().unwrap_or_default().to_string();
        album_artist = tag.get_string(&lofty::tag::ItemKey::AlbumArtist).unwrap_or("").to_string();
        year = tag.year().map(|y| y as i32);
        genre = tag.genre().map(|g| g.to_string());
        composer = tag.get_string(&lofty::tag::ItemKey::Composer).map(|s| s.to_string());
        track_no = tag.track().map(|t| t as i32);
        has_embedded_art = !tag.pictures().is_empty();
    }

    if title.is_empty() {
        title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();
    }
    if artist.is_empty() {
        artist = "Unknown Artist".to_string();
    }
    if album.is_empty() {
        album = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown Album")
            .to_string();
    }
    if album_artist.is_empty() {
        album_artist = artist.clone();
    }

    if track_no.is_none() {
        track_no = parse_track_prefix(path);
    }

    Ok(Song {
        id,
        title,
        artist,
        album,
        album_artist,
        duration,
        year,
        genre,
        composer,
        track_no,
        path: path.to_path_buf(),
        has_embedded_art,
    })
}

pub fn parse_track_prefix(path: &Path) -> Option<i32> {
    let name = path.file_stem()?.to_str()?;
    let (digits, _rest) = split_prefix(name)?;
    digits.parse::<i32>().ok()
}

pub fn split_prefix(name: &str) -> Option<(&str, &str)> {
    let trimmed = name.trim_start();
    let mut end = 0;
    for (i, c) in trimmed.char_indices() {
        if c.is_ascii_digit() {
            end = i + c.len_utf8();
        } else {
            break;
        }
    }
    if end == 0 {
        return None;
    }
    let after = &trimmed[end..];
    let rest = after.trim_start();
    let rest = rest.strip_prefix('-')?.trim_start();
    if rest.is_empty() {
        return None;
    }
    Some((&trimmed[..end], rest))
}

pub fn extract_embedded_art(path: &Path) -> Result<Vec<u8>> {
    let tagged = Probe::open(path)?.read()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag()).ok_or_else(|| anyhow!("no tag"))?;
    let pic = tag.pictures().first().ok_or_else(|| anyhow!("no embedded art"))?;
    Ok(pic.data().to_vec())
}

#[allow(dead_code)]
pub fn placeholder_song(path: &Path) -> Song {
    let title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();
    Song {
        id: song_id_from_path(path),
        title,
        artist: "Unknown Artist".into(),
        album: "Unknown Album".into(),
        album_artist: "Unknown Artist".into(),
        duration: Duration::from_secs(0),
        year: None,
        genre: None,
        composer: None,
        track_no: parse_track_prefix(path),
        path: path.to_path_buf(),
        has_embedded_art: false,
    }
}
