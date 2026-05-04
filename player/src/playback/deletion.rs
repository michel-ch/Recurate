use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};

use crate::data::Library;
use crate::renumberer;

pub struct DeletionResult {
    pub deleted_path: PathBuf,
    pub renumbered: usize,
}

pub fn delete_song(
    library: &Arc<Library>,
    song_id: i64,
    renumber: bool,
    threshold: f32,
) -> Result<DeletionResult> {
    let song = library
        .songs_snapshot()
        .into_iter()
        .find(|s| s.id == song_id)
        .ok_or_else(|| anyhow!("song not found in library: id={song_id}"))?;

    let path = song.path.clone();
    let folder = path.parent().map(|p| p.to_path_buf());

    std::fs::remove_file(&path).map_err(|e| anyhow!("remove_file {}: {e}", path.display()))?;
    library.remove_song(song_id);

    let mut renumbered = 0;
    if renumber {
        if let Some(folder) = folder.as_ref() {
            match renumberer::renumber_folder(folder, threshold) {
                Ok(n) => {
                    renumbered = n;
                    if n > 0 {
                        if let Err(e) = library.refresh_folder(folder) {
                            tracing::warn!("refresh_folder after renumber failed: {e:#}");
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("renumber failed for {}: {e:#}", folder.display());
                }
            }
        }
    }

    Ok(DeletionResult {
        deleted_path: path,
        renumbered,
    })
}
