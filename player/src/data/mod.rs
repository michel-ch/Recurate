pub mod duplicates;
pub mod fingerprint;
pub mod scanner;
pub mod tags;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::Result;
use parking_lot::RwLock;

use crate::domain::Song;

/// Normalize a path into a string suitable for **dedup-key comparison only**.
///
/// On Windows the filesystem is case-insensitive by default, and a single physical
/// file can appear under multiple spellings (`C:\Music\AK\foo.mp3` vs
/// `c:\music\AK\foo.mp3`) when the user edits scan roots. Without normalization,
/// the same file would land in the library twice with different `Song.id`s
/// (`song_id_from_path` hashes the raw string), and the duplicates view would show
/// the same source song as two rows in the same group. Lowercasing + slash
/// normalization collapses those forms.
///
/// The normalized string is *only* used as a HashSet key for dedup; the original
/// `path` is preserved on `Song` so display and disk operations stay byte-exact.
pub fn normalize_path_key(path: &Path) -> String {
    let raw = path.to_string_lossy();
    if cfg!(windows) {
        raw.replace('/', "\\").to_lowercase()
    } else {
        raw.into_owned()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryStatus {
    Idle,
    Scanning,
    Ready,
}

pub struct Library {
    songs: RwLock<Vec<Song>>,
    status: RwLock<LibraryStatus>,
    version: AtomicU64,
}

impl Library {
    pub fn new() -> Self {
        Self {
            songs: RwLock::new(Vec::new()),
            status: RwLock::new(LibraryStatus::Idle),
            version: AtomicU64::new(0),
        }
    }

    pub fn status(&self) -> LibraryStatus {
        *self.status.read()
    }

    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Relaxed)
    }

    fn bump(&self) {
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    pub fn songs_snapshot(&self) -> Vec<Song> {
        self.songs.read().clone()
    }

    pub fn read_songs<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[Song]) -> R,
    {
        f(self.songs.read().as_slice())
    }

    pub fn song_count(&self) -> usize {
        self.songs.read().len()
    }

    pub fn folders(&self) -> Vec<PathBuf> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for song in self.songs.read().iter() {
            if let Some(folder) = song.folder() {
                if seen.insert(folder.to_path_buf()) {
                    out.push(folder.to_path_buf());
                }
            }
        }
        out.sort();
        out
    }

    pub fn songs_in_folder(&self, folder: &Path) -> Vec<Song> {
        self.songs
            .read()
            .iter()
            .filter(|s| s.folder() == Some(folder))
            .cloned()
            .collect()
    }

    pub fn scan(self: &Arc<Self>, roots: &[PathBuf]) -> Result<()> {
        *self.status.write() = LibraryStatus::Scanning;

        let mut all = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for root in roots {
            if !root.exists() {
                tracing::warn!("scan root does not exist: {}", root.display());
                continue;
            }
            let scanned = scanner::scan_dir(root)?;
            let count = scanned.len();
            for song in scanned {
                if seen.insert(normalize_path_key(&song.path)) {
                    all.push(song);
                }
            }
            tracing::info!("scanned {} files in {}", count, root.display());
        }

        *self.songs.write() = all;
        *self.status.write() = LibraryStatus::Ready;
        self.bump();
        Ok(())
    }

    pub fn refresh_folder(&self, folder: &Path) -> Result<()> {
        let new = scanner::scan_dir(folder)?;
        let folder_key = normalize_path_key(folder);
        let mut songs = self.songs.write();
        // Case-insensitive folder match on Windows: an earlier scan may have stored
        // the path with different casing/slashes than `folder` here, and a strict
        // `==` would leave the prior song behind alongside the freshly-scanned one.
        songs.retain(|s| match s.folder() {
            Some(f) => normalize_path_key(f) != folder_key,
            None => true,
        });
        let mut seen: HashSet<String> = songs
            .iter()
            .map(|s| normalize_path_key(&s.path))
            .collect();
        for song in new {
            if seen.insert(normalize_path_key(&song.path)) {
                songs.push(song);
            }
        }
        drop(songs);
        self.bump();
        Ok(())
    }

    pub fn remove_song(&self, song_id: i64) {
        self.songs.write().retain(|s| s.id != song_id);
        self.bump();
    }
}

impl Default for Library {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn path_key_collapses_case_and_slashes_on_windows() {
        let a = normalize_path_key(Path::new(r"C:\Music\AK\01.mp3"));
        let b = normalize_path_key(Path::new(r"c:\music\ak\01.mp3"));
        let c = normalize_path_key(Path::new("C:/Music/AK/01.mp3"));
        let d = normalize_path_key(Path::new("c:/music/ak/01.mp3"));
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_eq!(a, d);
    }

    #[cfg(not(windows))]
    #[test]
    fn path_key_is_byte_exact_on_unix() {
        // Unix is case-sensitive; do not collapse.
        let a = normalize_path_key(Path::new("/Music/AK/01.mp3"));
        let b = normalize_path_key(Path::new("/music/AK/01.mp3"));
        assert_ne!(a, b);
    }

    #[test]
    fn path_key_stable_for_same_input() {
        let p = Path::new("/m/A/01.mp3");
        assert_eq!(normalize_path_key(p), normalize_path_key(p));
    }
}
