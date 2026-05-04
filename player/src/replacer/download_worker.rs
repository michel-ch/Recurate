use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender};
use parking_lot::RwLock;

use crate::data::Library;
use crate::replacer::download::download_audio_mp3;

#[derive(Debug, Clone)]
pub struct DownloadRequest {
    pub song_id: i64,
    pub source_path: PathBuf,
    pub dest_path: PathBuf,
    pub video_url: String,
    pub cookies_browser: Option<String>,
}

#[derive(Debug, Clone)]
pub enum DownloadState {
    Idle,
    Pending,
    Done,
    Failed { error: String },
}

const DOWNLOAD_PARALLELISM: usize = 3;

pub struct DownloadWorker {
    tx: Sender<DownloadRequest>,
    states: Arc<RwLock<HashMap<i64, DownloadState>>>,
    paused: Arc<AtomicBool>,
    queue_len: Arc<AtomicUsize>,
}

impl DownloadWorker {
    pub fn start(library: Arc<Library>) -> Self {
        let (tx, rx) = unbounded::<DownloadRequest>();
        let states: Arc<RwLock<HashMap<i64, DownloadState>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let paused = Arc::new(AtomicBool::new(false));
        let queue_len = Arc::new(AtomicUsize::new(0));

        for i in 0..DOWNLOAD_PARALLELISM {
            let rx_w = rx.clone();
            let states_w = states.clone();
            let library_w = library.clone();
            let paused_w = paused.clone();
            let queue_w = queue_len.clone();
            thread::Builder::new()
                .name(format!("replacer-download-{i}"))
                .spawn(move || run_downloads(rx_w, states_w, library_w, paused_w, queue_w))
                .expect("spawn replacer-download");
        }
        drop(rx);

        Self {
            tx,
            states,
            paused,
            queue_len,
        }
    }

    pub fn enqueue(&self, req: DownloadRequest) {
        self.states
            .write()
            .insert(req.song_id, DownloadState::Pending);
        self.queue_len.fetch_add(1, Ordering::Relaxed);
        let _ = self.tx.send(req);
    }

    pub fn state_of(&self, song_id: i64) -> DownloadState {
        self.states
            .read()
            .get(&song_id)
            .cloned()
            .unwrap_or(DownloadState::Idle)
    }

    pub fn read_states<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&HashMap<i64, DownloadState>) -> R,
    {
        f(&self.states.read())
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn set_paused(&self, p: bool) {
        self.paused.store(p, Ordering::Relaxed);
    }

    pub fn pending_count(&self) -> usize {
        self.queue_len.load(Ordering::Relaxed)
    }

    pub fn clear(&self) {
        self.states.write().clear();
    }
}

fn run_downloads(
    rx: Receiver<DownloadRequest>,
    states: Arc<RwLock<HashMap<i64, DownloadState>>>,
    library: Arc<Library>,
    paused: Arc<AtomicBool>,
    queue_len: Arc<AtomicUsize>,
) {
    loop {
        if paused.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(200));
            continue;
        }
        let req = match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(r) => r,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        queue_len.fetch_sub(1, Ordering::Relaxed);
        let req_for_panic = req.clone();
        let library_for_panic = library.clone();
        let result = catch_unwind(AssertUnwindSafe(|| {
            do_one(&req_for_panic)?;
            if let Some(folder) = req_for_panic.dest_path.parent() {
                if let Err(e) = library_for_panic.refresh_folder(folder) {
                    tracing::warn!("refresh after replace failed: {e:#}");
                }
            }
            Ok::<_, anyhow::Error>(())
        }));
        let new_state = match result {
            Ok(Ok(())) => DownloadState::Done,
            Ok(Err(e)) => DownloadState::Failed {
                error: format!("{e:#}"),
            },
            Err(_) => DownloadState::Failed {
                error: "download worker panicked (likely a malformed tag in the rescanned folder)"
                    .to_string(),
            },
        };
        states.write().insert(req.song_id, new_state);
    }
}

fn do_one(req: &DownloadRequest) -> anyhow::Result<()> {
    download_audio_mp3(
        &req.video_url,
        &req.dest_path,
        req.cookies_browser.as_deref(),
    )?;
    Ok(())
}
