use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::data::tags::split_prefix;
use crate::domain::Song;

pub fn missing_in_dest(
    source_songs: &[Song],
    dest_songs: &[Song],
    source_root: &Path,
    dest_root: &Path,
) -> Vec<(PathBuf, PathBuf)> {
    let dest_present: HashSet<(PathBuf, String)> = dest_songs
        .iter()
        .filter_map(|s| {
            let rel_folder = s.path.parent()?.strip_prefix(dest_root).ok()?.to_path_buf();
            let stem = s.path.file_stem()?.to_str()?;
            Some((rel_folder, normalize_stem(stem)))
        })
        .collect();

    let mut out = Vec::new();
    for song in source_songs {
        let Ok(rel) = song.path.strip_prefix(source_root) else {
            continue;
        };
        let dest = dest_root.join(rel);
        if dest.exists() {
            continue;
        }
        let rel_folder = song
            .path
            .parent()
            .and_then(|p| p.strip_prefix(source_root).ok())
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        let stem = song
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let key = (rel_folder, normalize_stem(stem));
        if dest_present.contains(&key) {
            continue;
        }
        out.push((song.path.clone(), dest));
    }
    out
}

pub fn normalize_stem(stem: &str) -> String {
    let trimmed = stem.trim();
    let after_prefix = match split_prefix(trimmed) {
        Some((_digits, rest)) => rest,
        None => trimmed,
    };
    after_prefix.trim().to_lowercase()
}

pub fn copy_file(src: &Path, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir {}", parent.display()))?;
    }
    std::fs::copy(src, dest)
        .with_context(|| format!("copy {} → {}", src.display(), dest.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn song(path: &str) -> Song {
        Song {
            id: 0,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            album_artist: String::new(),
            duration: Duration::from_secs(0),
            year: None,
            genre: None,
            composer: None,
            track_no: None,
            path: PathBuf::from(path),
            has_embedded_art: false,
        }
    }

    #[test]
    fn track_prefix_difference_is_not_missing() {
        let src_root = PathBuf::from("/m_orig");
        let dst_root = PathBuf::from("/m");
        let src = vec![song("/m_orig/Kerosene/0033 - foo.mp3")];
        let dst = vec![song("/m/Kerosene/0023 - foo.mp3")];
        let missing = missing_in_dest(&src, &dst, &src_root, &dst_root);
        assert!(
            missing.is_empty(),
            "same content under different track prefix must not be flagged"
        );
    }

    #[test]
    fn truly_missing_file_is_flagged() {
        let src_root = PathBuf::from("/m_orig");
        let dst_root = PathBuf::from("/m");
        let src = vec![song("/m_orig/Kerosene/0033 - foo.mp3")];
        let dst: Vec<Song> = vec![];
        let missing = missing_in_dest(&src, &dst, &src_root, &dst_root);
        assert_eq!(missing.len(), 1);
    }

    #[test]
    fn different_folders_are_distinct() {
        let src_root = PathBuf::from("/m_orig");
        let dst_root = PathBuf::from("/m");
        let src = vec![song("/m_orig/A/01 - foo.mp3")];
        let dst = vec![song("/m/B/01 - foo.mp3")];
        let missing = missing_in_dest(&src, &dst, &src_root, &dst_root);
        assert_eq!(
            missing.len(),
            1,
            "same stem in different folders must not match"
        );
    }
}
