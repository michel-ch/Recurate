use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::data::tags::split_prefix;

#[derive(Debug, Clone)]
pub struct RenamePair {
    pub from: PathBuf,
    pub to: PathBuf,
    pub old_index: i32,
    pub new_index: i32,
}

#[derive(Debug, Default)]
pub struct RenumberPlan {
    pub pairs: Vec<RenamePair>,
    pub skipped_no_prefix: usize,
    pub total_audio: usize,
}

impl RenumberPlan {
    pub fn is_noop(&self) -> bool {
        self.pairs.iter().all(|p| p.from == p.to)
    }

    pub fn changes(&self) -> usize {
        self.pairs.iter().filter(|p| p.from != p.to).count()
    }
}

const SUPPORTED: &[&str] = &["mp3", "flac", "m4a", "ogg", "wav", "aac", "opus"];

/// True for a regular audio file that should take part in numbering.
/// Dot-prefixed names are skipped: yt-dlp writes its in-flight transcode as
/// `.<stem>.dl.mp3` in the same folder, and renaming that mid-download would
/// break the final atomic rename.
fn is_audio_entry(p: &Path) -> bool {
    if !p.is_file() {
        return false;
    }
    if p.file_name().and_then(|s| s.to_str()).map_or(true, |n| n.starts_with('.')) {
        return false;
    }
    p.extension()
        .and_then(|s| s.to_str())
        .map(|e| SUPPORTED.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn analyze(folder: &Path, threshold: f32) -> Result<RenumberPlan> {
    let mut entries: Vec<PathBuf> = Vec::new();
    for e in std::fs::read_dir(folder).with_context(|| format!("read_dir {}", folder.display()))? {
        let e = match e {
            Ok(v) => v,
            Err(_) => continue,
        };
        let p = e.path();
        if is_audio_entry(&p) {
            entries.push(p);
        }
    }

    let total_audio = entries.len();

    // Threshold gate: fraction of files that already carry a digit prefix.
    // A folder under threshold (e.g. Suno/, Long/) probably doesn't follow
    // the convention and stays untouched; once we cross threshold, EVERY
    // audio file in the folder gets a number, including unprefixed ones,
    // so a single song without `NN - ` doesn't break the contiguous
    // sequence the user expects.
    let mut prefix_count = 0usize;
    let mut max_pad = 1usize;
    for p in &entries {
        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
            if let Some((digits, _)) = split_prefix(stem) {
                if digits.parse::<i32>().is_ok() {
                    prefix_count += 1;
                    max_pad = max_pad.max(digits.len());
                }
            }
        }
    }

    let skipped_no_prefix = total_audio.saturating_sub(prefix_count);
    if total_audio == 0 || (prefix_count as f32) / (total_audio as f32) < threshold {
        return Ok(RenumberPlan {
            pairs: Vec::new(),
            skipped_no_prefix,
            total_audio,
        });
    }

    // Sort by current full filename. Digit-prefixed files lead the order
    // (ASCII '0'..'9' < 'A'..'z'), unprefixed files trail alphabetically —
    // matching what the user already sees in the file manager, so the
    // resulting `01..N` mapping is intuitive even for folders with mixed
    // prefixed and unprefixed entries.
    let mut all: Vec<(PathBuf, String, String, String, i32)> =
        Vec::with_capacity(total_audio);
    for p in &entries {
        let stem = match p.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => continue,
        };
        let ext = p
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let filename = p
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let (rest, old_idx) = match split_prefix(stem) {
            Some((digits, rest)) => (
                rest.to_string(),
                digits.parse::<i32>().unwrap_or(0),
            ),
            None => (stem.to_string(), 0),
        };
        all.push((p.clone(), rest, ext, filename, old_idx));
    }
    all.sort_by(|a, b| a.3.cmp(&b.3));

    let pad_width = compute_pad_width(all.len()).max(max_pad);
    let mut pairs = Vec::with_capacity(all.len());
    for (i, (path, rest, ext, _filename, old_idx)) in all.iter().enumerate() {
        let new_idx = (i + 1) as i32;
        let new_name = format!(
            "{:0width$} - {}.{}",
            new_idx,
            rest,
            ext,
            width = pad_width
        );
        let to = folder.join(new_name);
        pairs.push(RenamePair {
            from: path.clone(),
            to,
            old_index: *old_idx,
            new_index: new_idx,
        });
    }

    Ok(RenumberPlan {
        pairs,
        skipped_no_prefix,
        total_audio,
    })
}

fn compute_pad_width(n: usize) -> usize {
    if n >= 100 {
        3
    } else if n >= 10 {
        2
    } else {
        1
    }
}

pub fn apply(plan: &RenumberPlan) -> Result<usize> {
    if plan.is_noop() {
        return Ok(0);
    }

    let unique_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let mut temps: Vec<(PathBuf, PathBuf)> = Vec::with_capacity(plan.pairs.len());
    for (i, pair) in plan.pairs.iter().enumerate() {
        if pair.from == pair.to {
            continue;
        }
        let parent = pair.from.parent().ok_or_else(|| anyhow!("no parent"))?;
        let stem = pair
            .from
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("track");
        let ext = pair.from.extension().and_then(|s| s.to_str()).unwrap_or("");
        let temp = parent.join(format!(".tmp_renumber_{unique_id}_{i}_{stem}.{ext}"));
        std::fs::rename(&pair.from, &temp)
            .with_context(|| format!("temp rename {} -> {}", pair.from.display(), temp.display()))?;
        temps.push((temp, pair.to.clone()));
    }

    let mut applied = 0;
    for (temp, final_path) in &temps {
        std::fs::rename(temp, final_path)
            .with_context(|| format!("final rename {} -> {}", temp.display(), final_path.display()))?;
        applied += 1;
    }
    Ok(applied)
}

pub fn renumber_folder(folder: &Path, threshold: f32) -> Result<usize> {
    let plan = analyze(folder, threshold)?;
    apply(&plan)
}

/// Build a plan that numbers `ordered` 1..N in the given order. Every path
/// must be an existing file directly inside `folder`; files in the folder
/// that are *not* listed are appended after the listed ones in their
/// current filename order so nothing silently loses its prefix.
pub fn plan_order(folder: &Path, ordered: &[PathBuf]) -> Result<RenumberPlan> {
    let mut on_disk: Vec<PathBuf> = Vec::new();
    for e in std::fs::read_dir(folder).with_context(|| format!("read_dir {}", folder.display()))? {
        let p = match e {
            Ok(v) => v.path(),
            Err(_) => continue,
        };
        if is_audio_entry(&p) {
            on_disk.push(p);
        }
    }
    on_disk.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    let mut sequence: Vec<PathBuf> = Vec::with_capacity(on_disk.len());
    for p in ordered {
        if !on_disk.iter().any(|d| d == p) {
            return Err(anyhow!("{} is not an audio file in {}", p.display(), folder.display()));
        }
        if !sequence.contains(p) {
            sequence.push(p.clone());
        }
    }
    for p in on_disk {
        if !sequence.contains(&p) {
            sequence.push(p);
        }
    }

    let pad_width = compute_pad_width(sequence.len()).max(2);
    let mut pairs = Vec::with_capacity(sequence.len());
    for (i, path) in sequence.iter().enumerate() {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("track");
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("mp3");
        let (rest, old_idx) = match split_prefix(stem) {
            Some((digits, rest)) => (rest.to_string(), digits.parse::<i32>().unwrap_or(0)),
            None => (stem.to_string(), 0),
        };
        let new_idx = (i + 1) as i32;
        pairs.push(RenamePair {
            from: path.clone(),
            to: folder.join(format!("{new_idx:0pad_width$} - {rest}.{ext}")),
            old_index: old_idx,
            new_index: new_idx,
        });
    }
    Ok(RenumberPlan {
        pairs,
        skipped_no_prefix: 0,
        total_audio: sequence.len(),
    })
}

/// `(next track number, pad width)` for appending a new file to `folder`.
/// Pad is at least 2 so a fresh playlist starts at `01`.
pub fn next_index(folder: &Path) -> (usize, usize) {
    let count = std::fs::read_dir(folder)
        .map(|rd| {
            rd.flatten().filter(|e| is_audio_entry(&e.path())).count()
        })
        .unwrap_or(0);
    (count + 1, compute_pad_width(count + 1).max(2))
}
