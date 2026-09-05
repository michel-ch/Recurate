use std::fs;
use std::path::Path;

use recurate::renumberer::{analyze, apply, renumber_folder};
use tempfile::tempdir;

fn touch(dir: &Path, name: &str) {
    fs::write(dir.join(name), b"\xff\xfb\x90\x44test").expect("write file");
}

#[test]
fn renumber_after_delete_collapses_indices() {
    let dir = tempdir().unwrap();
    let p = dir.path();
    touch(p, "01 - one.mp3");
    touch(p, "02 - two.mp3");
    touch(p, "03 - three.mp3");
    touch(p, "04 - four.mp3");
    touch(p, "05 - five.mp3");

    fs::remove_file(p.join("03 - three.mp3")).unwrap();

    let n = renumber_folder(p, 0.5).unwrap();
    assert_eq!(n, 2, "should rename two files (4->3 and 5->4)");

    let mut names: Vec<String> = fs::read_dir(p)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "01 - one.mp3".to_string(),
            "02 - two.mp3".to_string(),
            "03 - four.mp3".to_string(),
            "04 - five.mp3".to_string(),
        ]
    );
}

#[test]
fn renumber_handles_swap_without_collision() {
    let dir = tempdir().unwrap();
    let p = dir.path();
    touch(p, "05 - alpha.mp3");
    touch(p, "02 - beta.mp3");

    let plan = analyze(p, 0.5).unwrap();
    let n = apply(&plan).unwrap();
    assert_eq!(n, 2, "both files should be renumbered to 01/02");

    let names: std::collections::HashSet<String> = fs::read_dir(p)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    assert!(names.contains("01 - beta.mp3"), "got: {names:?}");
    assert!(names.contains("02 - alpha.mp3"), "got: {names:?}");
}

#[test]
fn renumber_skips_when_below_threshold() {
    let dir = tempdir().unwrap();
    let p = dir.path();
    touch(p, "01 - one.mp3");
    touch(p, "untitled.mp3");
    touch(p, "another.mp3");
    touch(p, "more.mp3");

    let plan = analyze(p, 0.5).unwrap();
    assert!(plan.pairs.is_empty(), "below 50% threshold should skip");
}

#[test]
fn renumber_includes_unprefixed_when_majority_prefixed() {
    // Regression for "renumber not working when a file has no prefix": as long
    // as the folder crosses the threshold, every audio file in it should land
    // in the contiguous 01..N sequence — including ones that arrived without
    // a `NN - ` prefix (e.g. fresh download, rename slip). Sort is by current
    // filename, so digit-prefixed files lead and unprefixed files trail.
    let dir = tempdir().unwrap();
    let p = dir.path();
    touch(p, "01 - one.mp3");
    touch(p, "02 - two.mp3");
    touch(p, "04 - four.mp3");
    touch(p, "untitled.mp3");

    let n = renumber_folder(p, 0.5).unwrap();
    assert!(n >= 2, "expected at least 04→03 and untitled→04 renames; got {n}");

    let mut names: Vec<String> = fs::read_dir(p)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "01 - one.mp3".to_string(),
            "02 - two.mp3".to_string(),
            "03 - four.mp3".to_string(),
            "04 - untitled.mp3".to_string(),
        ],
        "unprefixed file must be slotted at the end of the sequence"
    );
}

#[test]
fn renumber_pad_width_scales() {
    let dir = tempdir().unwrap();
    let p = dir.path();
    for i in 1..=12 {
        touch(p, &format!("{:02} - song-{}.mp3", i, i));
    }
    fs::remove_file(p.join("05 - song-5.mp3")).unwrap();

    let n = renumber_folder(p, 0.5).unwrap();
    assert!(n >= 1);
    let names: Vec<String> = fs::read_dir(p)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    let max = names
        .iter()
        .filter(|s| s.ends_with(".mp3"))
        .filter_map(|s| s.split(' ').next())
        .filter_map(|d| d.parse::<i32>().ok())
        .max()
        .unwrap();
    assert_eq!(max, 11, "11 remaining files numbered 1..=11");
}

#[test]
fn plan_order_renames_to_requested_positions() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["01 - A - X.mp3", "02 - B - X.mp3", "03 - C - X.mp3"] {
        std::fs::write(dir.path().join(name), b"x").unwrap();
    }
    let ordered = vec![
        dir.path().join("03 - C - X.mp3"),
        dir.path().join("01 - A - X.mp3"),
        dir.path().join("02 - B - X.mp3"),
    ];
    let plan = recurate::renumberer::plan_order(dir.path(), &ordered).unwrap();
    assert_eq!(plan.changes(), 3);
    recurate::renumberer::apply(&plan).unwrap();
    let mut names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    assert_eq!(names, vec!["01 - C - X.mp3", "02 - A - X.mp3", "03 - B - X.mp3"]);
}

#[test]
fn plan_order_rejects_paths_outside_folder() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("01 - A - X.mp3"), b"x").unwrap();
    let bogus = vec![dir.path().join("nope.mp3")];
    assert!(recurate::renumberer::plan_order(dir.path(), &bogus).is_err());
}

#[test]
fn next_index_counts_audio_files_and_pad() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(recurate::renumberer::next_index(dir.path()), (1, 2));
    for i in 1..=9 {
        std::fs::write(dir.path().join(format!("0{i} - T - A.mp3")), b"x").unwrap();
    }
    assert_eq!(recurate::renumberer::next_index(dir.path()), (10, 2));
}
