use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use anyhow::Result;
use walkdir::WalkDir;

use crate::data::tags;
use crate::data::normalize_path_key;
use crate::domain::Song;

const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "flac", "m4a", "ogg", "wav", "aac", "opus"];

pub fn scan_dir(root: &Path) -> Result<Vec<Song>> {
    let mut songs = Vec::new();
    for entry in WalkDir::new(root).follow_links(false).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = match path.extension().and_then(|s| s.to_str()) {
            Some(e) => e.to_ascii_lowercase(),
            None => continue,
        };
        if !SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }
        match tags::read_song(path) {
            Ok(song) => songs.push(song),
            Err(e) => tracing::debug!("skip {}: {e}", path.display()),
        }
    }
    Ok(songs)
}

/// Stable id derived from the **normalized** path string. The same physical
/// file reached through different scan-root spellings (case, slashes on Windows)
/// MUST hash to the same id; otherwise `App::fingerprints` (HashMap<i64, _>)
/// holds two entries for one file and the duplicates view shows the same source
/// song as two rows in the same group. Defense-in-depth: `Library::scan` /
/// `Library::refresh_folder` already dedupe by `normalize_path_key`, but
/// stabilizing the id at its source means any future entry path that bypasses
/// that gate still self-collapses in the fingerprint map.
pub fn song_id_from_path(path: &Path) -> i64 {
    let mut h = DefaultHasher::new();
    normalize_path_key(path).hash(&mut h);
    h.finish() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn song_id_is_stable_for_same_path() {
        let p = Path::new("/m/A/01.mp3");
        assert_eq!(song_id_from_path(p), song_id_from_path(p));
    }

    #[cfg(windows)]
    #[test]
    fn song_id_collapses_case_variants_on_windows() {
        // The user's bug pattern: the same physical file spelled two ways
        // (e.g., scan root edited from `C:\Music` to `c:\music` and back)
        // must produce the SAME id. Otherwise `App::fingerprints` keys two
        // entries against the same audio and the duplicates view renders one
        // source song as two rows.
        let a = song_id_from_path(Path::new(r"C:\Music\AK\01.mp3"));
        let b = song_id_from_path(Path::new(r"c:\music\ak\01.mp3"));
        assert_eq!(a, b, "case variants of one path must hash to one id");
    }

    #[cfg(windows)]
    #[test]
    fn song_id_collapses_slash_variants_on_windows() {
        let a = song_id_from_path(Path::new(r"C:\music\AK\01.mp3"));
        let b = song_id_from_path(Path::new("C:/music/AK/01.mp3"));
        assert_eq!(a, b, "slash variants of one path must hash to one id");
    }

    #[cfg(not(windows))]
    #[test]
    fn song_id_is_byte_exact_on_unix() {
        // Unix is case-sensitive; two case spellings ARE two different files.
        let a = song_id_from_path(Path::new("/Music/AK/01.mp3"));
        let b = song_id_from_path(Path::new("/music/AK/01.mp3"));
        assert_ne!(a, b);
    }
}
