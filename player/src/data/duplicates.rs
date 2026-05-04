use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::data::fingerprint::Fingerprint;
use crate::data::normalize_path_key;
use crate::domain::Song;

#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    pub fingerprint: Fingerprint,
    pub duration_secs: u64,
    pub songs: Vec<Song>,
}

/// Group songs by (fingerprint, duration_secs, **parent folder**). Two files
/// are flagged as duplicates only when they sit in the same folder — cross-
/// folder same-audio copies are ignored because the user often keeps
/// intentional copies of a track in `Fav/` alongside its origin folder, and
/// flagging those produces noise.
pub fn find_duplicates(
    songs: &[Song],
    fingerprints: &HashMap<i64, Fingerprint>,
) -> Vec<DuplicateGroup> {
    let mut buckets: HashMap<(Fingerprint, u64, PathBuf), Vec<Song>> = HashMap::new();
    // Dedup by *normalized* path so the same physical file (Windows: case-insensitive,
    // slash-insensitive) cannot occupy two rows in the same group when the library
    // was populated through scan roots that spelled the same file two ways.
    let mut seen_paths: HashSet<String> = HashSet::new();
    for song in songs {
        if !seen_paths.insert(normalize_path_key(&song.path)) {
            continue;
        }
        if let Some(fp) = fingerprints.get(&song.id) {
            let folder = song
                .path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_default();
            let key = (*fp, song.duration.as_secs(), folder);
            buckets.entry(key).or_default().push(song.clone());
        }
    }
    let mut groups: Vec<DuplicateGroup> = buckets
        .into_iter()
        .filter(|(_, v)| v.len() >= 2)
        .map(|((fingerprint, duration_secs, _folder), mut songs)| {
            songs.sort_by(|a, b| a.path.cmp(&b.path));
            DuplicateGroup {
                fingerprint,
                duration_secs,
                songs,
            }
        })
        .collect();
    groups.sort_by(|a, b| {
        b.songs
            .len()
            .cmp(&a.songs.len())
            .then_with(|| a.fingerprint.cmp(&b.fingerprint))
    });
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    fn song(id: i64, path: &str, secs: u64) -> Song {
        Song {
            id,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            album_artist: String::new(),
            duration: Duration::from_secs(secs),
            year: None,
            genre: None,
            composer: None,
            track_no: None,
            path: PathBuf::from(path),
            has_embedded_art: false,
        }
    }

    #[test]
    fn groups_songs_with_matching_fingerprint_and_duration() {
        let songs = vec![
            song(1, "/m/A/01.mp3", 180),
            song(2, "/m/A/02.mp3", 180),
            song(3, "/m/A/03.mp3", 200),
        ];
        let mut fps: HashMap<i64, Fingerprint> = HashMap::new();
        fps.insert(1, [0xAAAA, 0xBBBB, 0, 0]);
        fps.insert(2, [0xAAAA, 0xBBBB, 0, 0]);
        fps.insert(3, [0xCCCC, 0xDDDD, 0, 0]);
        let groups = find_duplicates(&songs, &fps);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].songs.len(), 2);
    }

    #[test]
    fn same_fingerprint_different_duration_does_not_match() {
        let songs = vec![
            song(1, "/m/A/01.mp3", 180),
            song(2, "/m/B/01.mp3", 220),
        ];
        let mut fps: HashMap<i64, Fingerprint> = HashMap::new();
        fps.insert(1, [0x1234, 0x5678, 0, 0]);
        fps.insert(2, [0x1234, 0x5678, 0, 0]);
        let groups = find_duplicates(&songs, &fps);
        assert!(groups.is_empty(), "duration mismatch must split groups");
    }

    #[test]
    fn cross_folder_duplicates_are_not_grouped() {
        // Same audio in different folders is intentional (user keeps Fav/
        // copies alongside origin folders); only intra-folder duplicates
        // get flagged.
        let songs = vec![song(1, "/m/A/01.mp3", 200), song(2, "/m/B/02.mp3", 200)];
        let mut fps: HashMap<i64, Fingerprint> = HashMap::new();
        fps.insert(1, [0x1234, 0x5678, 0, 0]);
        fps.insert(2, [0x1234, 0x5678, 0, 0]);
        let groups = find_duplicates(&songs, &fps);
        assert!(
            groups.is_empty(),
            "cross-folder duplicates must not be grouped"
        );
    }

    #[test]
    fn intra_folder_duplicates_are_grouped() {
        let songs = vec![song(1, "/m/A/01.mp3", 200), song(2, "/m/A/02.mp3", 200)];
        let mut fps: HashMap<i64, Fingerprint> = HashMap::new();
        fps.insert(1, [0x1234, 0x5678, 0, 0]);
        fps.insert(2, [0x1234, 0x5678, 0, 0]);
        let groups = find_duplicates(&songs, &fps);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].songs.len(), 2);
    }

    #[test]
    fn songs_without_fingerprint_are_skipped() {
        let songs = vec![
            song(1, "/m/A/01.mp3", 180),
            song(2, "/m/A/02.mp3", 180),
        ];
        let fps: HashMap<i64, Fingerprint> = HashMap::new();
        assert!(find_duplicates(&songs, &fps).is_empty());
    }

    #[test]
    fn identical_path_strings_dedupe_to_one_row() {
        // Two Song records pointing at the same path-string (e.g. an overlapping
        // scan root walked the same file twice) must collapse to a single row,
        // not appear twice in a group.
        let songs = vec![
            song(1, "/m/A/01.mp3", 180),
            song(1, "/m/A/01.mp3", 180),
        ];
        let mut fps: HashMap<i64, Fingerprint> = HashMap::new();
        fps.insert(1, [0xAA, 0xBB, 0, 0]);
        let groups = find_duplicates(&songs, &fps);
        assert!(
            groups.is_empty(),
            "single physical file (one path) cannot be its own duplicate; got {groups:?}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_case_insensitive_paths_dedupe_to_one_row() {
        // The user's bug pattern: same file reached through two case spellings
        // (`C:\Music\AK\01.mp3` vs `c:\music\AK\01.mp3`) gets two different
        // `song.id`s (path-string hash) and two fingerprints (same audio →
        // same hash). Without case-insensitive dedup, the duplicates view shows
        // one source song as two rows in a group.
        let songs = vec![
            song(101, r"C:\Music\AK\01.mp3", 180),
            song(102, r"c:\music\ak\01.mp3", 180),
        ];
        let mut fps: HashMap<i64, Fingerprint> = HashMap::new();
        fps.insert(101, [0xAA, 0xBB, 0, 0]);
        fps.insert(102, [0xAA, 0xBB, 0, 0]);
        let groups = find_duplicates(&songs, &fps);
        assert!(
            groups.is_empty(),
            "same file under two case spellings must dedupe; got {groups:?}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_slash_insensitive_paths_dedupe_to_one_row() {
        // Same file with mixed slash conventions must also collapse.
        let songs = vec![
            song(201, r"C:\music\AK\01.mp3", 180),
            song(202, "C:/music/AK/01.mp3", 180),
        ];
        let mut fps: HashMap<i64, Fingerprint> = HashMap::new();
        fps.insert(201, [0x11, 0x22, 0, 0]);
        fps.insert(202, [0x11, 0x22, 0, 0]);
        let groups = find_duplicates(&songs, &fps);
        assert!(
            groups.is_empty(),
            "same file under two slash conventions must dedupe; got {groups:?}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn duplicate_paths_dont_inflate_legitimate_groups() {
        // Three rows in the same folder: A and B are real duplicates (different
        // filenames, same audio), and B is duplicated in the input by case.
        // The output must show one group of two rows (A + B), not three.
        let songs = vec![
            song(1, r"C:\m\A\01.mp3", 200),
            song(2, r"C:\m\A\02.mp3", 200),
            song(3, r"c:\M\A\02.mp3", 200), // same file as id=2 under different case
        ];
        let mut fps: HashMap<i64, Fingerprint> = HashMap::new();
        fps.insert(1, [0xAA, 0xBB, 0, 0]);
        fps.insert(2, [0xAA, 0xBB, 0, 0]);
        fps.insert(3, [0xAA, 0xBB, 0, 0]);
        let groups = find_duplicates(&songs, &fps);
        assert_eq!(groups.len(), 1, "exactly one duplicate group");
        assert_eq!(
            groups[0].songs.len(),
            2,
            "case-collapsed dup of B must not become a third row"
        );
    }
}
