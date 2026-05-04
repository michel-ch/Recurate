//! Regression tests for the duplicates-page integration bugs.
//!
//! These exercise `Library::scan` and `Library::refresh_folder` at the
//! filesystem boundary to confirm that the same physical file cannot appear
//! twice in the library — the precondition for the user-reported bug
//! "same source song appears multiple times across different rows with
//! different displayed names" on the Duplicates page.
//!
//! The tests need a real, decodable mp3 because `tags::read_song` rejects
//! files lofty cannot probe. We seed each tempdir by copying the first .mp3
//! found anywhere under `player/music/`. If no such file exists (e.g. on a
//! fresh checkout without the user's dataset), the tests skip cleanly.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use recurate::data::Library;
use tempfile::tempdir;
use walkdir::WalkDir;

fn find_seed_mp3() -> Option<PathBuf> {
    let here = std::env::current_dir().ok()?;
    let candidates = [here.join("music"), here.parent()?.join("music")];
    for root in candidates.iter() {
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file()
                && entry.path().extension().and_then(|s| s.to_str()) == Some("mp3")
            {
                return Some(entry.path().to_path_buf());
            }
        }
    }
    None
}

fn seed_two_mp3s(dest_dir: &Path) -> bool {
    let Some(seed) = find_seed_mp3() else {
        return false;
    };
    fs::copy(&seed, dest_dir.join("01.mp3")).expect("copy 01");
    fs::copy(&seed, dest_dir.join("02.mp3")).expect("copy 02");
    true
}

#[test]
fn scan_with_overlapping_roots_does_not_double_count_files() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let sub = root.join("AK");
    fs::create_dir(&sub).unwrap();
    if !seed_two_mp3s(&sub) {
        eprintln!("skip: no seed mp3 found under ./music");
        return;
    }

    let library = Arc::new(Library::new());
    // Pass two roots where the second is a *subpath* of the first — walkdir
    // walks both, but Library::scan must dedupe by path.
    let roots: Vec<PathBuf> = vec![root.to_path_buf(), sub.clone()];
    library.scan(&roots).unwrap();

    assert_eq!(
        library.song_count(),
        2,
        "overlapping scan roots must not double-count files"
    );
}

#[cfg(windows)]
#[test]
fn scan_with_case_variant_roots_does_not_double_count_files() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let sub = root.join("AK");
    fs::create_dir(&sub).unwrap();
    if !seed_two_mp3s(&sub) {
        eprintln!("skip: no seed mp3 found under ./music");
        return;
    }

    // Case-mutate the leaf segment of the root: walkdir reports entries with
    // the literal case of the supplied root, producing different path strings
    // for the same physical files. Without normalization, song_id_from_path
    // would yield distinct ids for the same file.
    let root_upper = PathBuf::from(root.to_string_lossy().to_uppercase());
    if !root_upper.exists() {
        eprintln!("skip: tempdir not case-insensitive (per-folder NTFS flag)");
        return;
    }

    let library = Arc::new(Library::new());
    library
        .scan(&[root.to_path_buf(), root_upper])
        .unwrap();

    assert_eq!(
        library.song_count(),
        2,
        "case-variant scan roots must not double-count the same physical files"
    );
}

#[test]
fn refresh_folder_does_not_double_add_files() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let sub = root.join("AK");
    fs::create_dir(&sub).unwrap();
    if !seed_two_mp3s(&sub) {
        eprintln!("skip: no seed mp3 found under ./music");
        return;
    }

    let library = Arc::new(Library::new());
    library.scan(&[root.to_path_buf()]).unwrap();
    assert_eq!(library.song_count(), 2);

    library.refresh_folder(&sub).unwrap();
    assert_eq!(library.song_count(), 2, "refresh must not duplicate songs");
}

#[cfg(windows)]
#[test]
fn refresh_folder_with_case_variant_path_does_not_double_add() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let sub = root.join("AK");
    fs::create_dir(&sub).unwrap();
    if !seed_two_mp3s(&sub) {
        eprintln!("skip: no seed mp3 found under ./music");
        return;
    }

    let library = Arc::new(Library::new());
    library.scan(&[root.to_path_buf()]).unwrap();
    assert_eq!(library.song_count(), 2);

    // Pass a case-mutated copy of the same folder. Without case-insensitive
    // folder matching in `retain()`, the prior 2 songs would remain alongside
    // 2 freshly-scanned (different-case) Songs => 4 entries.
    let sub_mut = PathBuf::from(sub.to_string_lossy().to_uppercase());
    if !sub_mut.exists() {
        eprintln!("skip: tempdir not case-insensitive");
        return;
    }
    library.refresh_folder(&sub_mut).unwrap();
    assert_eq!(
        library.song_count(),
        2,
        "case-variant refresh must not duplicate songs"
    );
}
