use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender};
use parking_lot::RwLock;

use crate::replacer::scoring::{score_results, ScoredResult};
use crate::replacer::title_cleaner::{clean_title, CleanedTitle};
use crate::replacer::youtube::search_ytdlp;
use crate::replacer::youtube_api::search_api;
use crate::replacer::SearchBackend;

#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub song_id: i64,
    pub filename_stem: String,
    pub expected_duration_secs: Option<u64>,
    pub max_results: usize,
    pub backend: SearchBackend,
    pub api_key: Option<String>,
    pub cookies_browser: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SearchResponse {
    pub song_id: i64,
    pub cleaned: CleanedTitle,
    pub result: Result<Vec<ScoredResult>, String>,
}

#[derive(Debug, Clone)]
pub enum SongState {
    Idle,
    Pending,
    Done {
        cleaned: CleanedTitle,
        results: Vec<ScoredResult>,
    },
    Failed {
        cleaned: CleanedTitle,
        error: String,
    },
}

const SEARCH_PARALLELISM: usize = 4;

pub struct SearchWorker {
    tx: Sender<SearchRequest>,
    states: Arc<RwLock<HashMap<i64, SongState>>>,
    paused: Arc<AtomicBool>,
    queue_len: Arc<std::sync::atomic::AtomicUsize>,
}

impl SearchWorker {
    pub fn start() -> Self {
        let (tx, rx) = unbounded::<SearchRequest>();
        let (rtx, rrx) = unbounded::<SearchResponse>();

        let states: Arc<RwLock<HashMap<i64, SongState>>> = Arc::new(RwLock::new(HashMap::new()));
        let paused = Arc::new(AtomicBool::new(false));
        let queue_len = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        for i in 0..SEARCH_PARALLELISM {
            let rx_w = rx.clone();
            let tx_w = rtx.clone();
            let paused_w = paused.clone();
            let queue_w = queue_len.clone();
            thread::Builder::new()
                .name(format!("replacer-search-{i}"))
                .spawn(move || run_searches(rx_w, tx_w, paused_w, queue_w))
                .expect("spawn replacer-search");
        }
        drop(rx);
        drop(rtx);

        let states_collector = states.clone();
        thread::Builder::new()
            .name("replacer-collect".into())
            .spawn(move || {
                while let Ok(resp) = rrx.recv() {
                    let mut s = states_collector.write();
                    let new_state = match resp.result {
                        Ok(results) => SongState::Done {
                            cleaned: resp.cleaned,
                            results,
                        },
                        Err(e) => SongState::Failed {
                            cleaned: resp.cleaned,
                            error: e,
                        },
                    };
                    s.insert(resp.song_id, new_state);
                }
            })
            .expect("spawn replacer-collect");

        Self {
            tx,
            states,
            paused,
            queue_len,
        }
    }

    pub fn enqueue(&self, req: SearchRequest) {
        self.states
            .write()
            .insert(req.song_id, SongState::Pending);
        self.queue_len
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let _ = self.tx.send(req);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn set_paused(&self, p: bool) {
        self.paused.store(p, Ordering::Relaxed);
    }

    pub fn pending_count(&self) -> usize {
        self.queue_len
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn state_of(&self, song_id: i64) -> SongState {
        self.states
            .read()
            .get(&song_id)
            .cloned()
            .unwrap_or(SongState::Idle)
    }

    pub fn snapshot(&self) -> HashMap<i64, SongState> {
        self.states.read().clone()
    }

    pub fn read_states<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&HashMap<i64, SongState>) -> R,
    {
        f(&self.states.read())
    }

    pub fn clear(&self) {
        self.states.write().clear();
    }
}

fn run_searches(
    rx: Receiver<SearchRequest>,
    tx: Sender<SearchResponse>,
    paused: Arc<AtomicBool>,
    queue_len: Arc<std::sync::atomic::AtomicUsize>,
) {
    loop {
        if paused.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(150));
            continue;
        }
        let req = match rx.recv_timeout(Duration::from_millis(150)) {
            Ok(r) => r,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        queue_len.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        let cleaned = clean_title(&req.filename_stem);
        let response = if cleaned.query.is_empty() {
            SearchResponse {
                song_id: req.song_id,
                cleaned,
                result: Err("empty query".to_string()),
            }
        } else {
            let search_query = if cleaned.is_remix {
                cleaned.query.clone()
            } else {
                format!("{} audio", cleaned.query)
            };
            let videos = match req.backend {
                SearchBackend::Ytdlp => search_ytdlp(
                    &search_query,
                    req.max_results,
                    req.cookies_browser.as_deref(),
                ),
                SearchBackend::Api => match req.api_key.as_deref() {
                    Some(key) if !key.is_empty() => search_api(&search_query, req.max_results, key),
                    _ => Err(anyhow::anyhow!(
                        "API backend selected but YOUTUBE_API_KEY is empty"
                    )),
                },
            };
            let result = videos
                .map(|v| score_results(&cleaned, req.expected_duration_secs, &v))
                .map_err(|e| format!("{e:#}"));
            SearchResponse {
                song_id: req.song_id,
                cleaned,
                result,
            }
        };
        let _ = tx.send(response);
    }
}
