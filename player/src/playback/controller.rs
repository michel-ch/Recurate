use std::sync::Arc;
use std::thread;

use parking_lot::RwLock;

use crate::data::Library;
use crate::domain::{PlaybackState, RepeatMode, Song, SortOption};
use crate::engine::{Engine, EngineCmd, EngineEvent};
use crate::playback::queue::Queue;

pub struct PlaybackController {
    engine: Engine,
    library: Arc<Library>,
    queue: RwLock<Queue>,
    state: RwLock<PlaybackState>,
}

impl PlaybackController {
    pub fn new(engine: Engine, library: Arc<Library>) -> Arc<Self> {
        let me = Arc::new(Self {
            engine,
            library,
            queue: RwLock::new(Queue::default()),
            state: RwLock::new(PlaybackState::default()),
        });
        let listener = me.clone();
        let event_rx = listener.engine.events().clone();
        thread::Builder::new()
            .name("playback-events".into())
            .spawn(move || {
                while let Ok(evt) = event_rx.recv() {
                    listener.handle_event(evt);
                }
            })
            .ok();
        me
    }

    pub fn state_snapshot(&self) -> PlaybackState {
        self.state.read().clone()
    }

    pub fn queue_snapshot(&self) -> Queue {
        self.queue.read().clone()
    }

    pub fn play_songs(&self, mut songs: Vec<Song>, start_index: usize, sort: Option<SortOption>) {
        if let Some(opt) = sort {
            crate::domain::sort_songs(&mut songs, opt);
        }
        if songs.is_empty() {
            return;
        }
        let start = start_index.min(songs.len() - 1);
        {
            let mut q = self.queue.write();
            q.replace(songs, start);
        }
        self.start_current();
    }

    pub fn play_pause(&self) {
        let playing = self.state.read().is_playing;
        if playing {
            self.engine.send(EngineCmd::Pause);
            self.state.write().is_playing = false;
        } else if self.queue.read().current_song().is_some() {
            self.engine.send(EngineCmd::Play);
            self.state.write().is_playing = true;
        }
    }

    pub fn next(&self) {
        let next_song = {
            let mut q = self.queue.write();
            q.advance().cloned()
        };
        if let Some(s) = next_song {
            self.load_and_play(&s);
        } else {
            self.engine.send(EngineCmd::Stop);
            self.state.write().is_playing = false;
        }
    }

    pub fn previous(&self) {
        if self.state.read().current_position_ms > 3000 {
            self.seek_fraction(0.0);
            return;
        }
        let prev_song = {
            let mut q = self.queue.write();
            q.rewind().cloned()
        };
        if let Some(s) = prev_song {
            self.load_and_play(&s);
        }
    }

    pub fn jump_to(&self, index: usize) {
        let song = {
            let mut q = self.queue.write();
            q.jump_to(index).cloned()
        };
        if let Some(s) = song {
            self.load_and_play(&s);
        }
    }

    pub fn seek_fraction(&self, fraction: f32) {
        self.engine.send(EngineCmd::SeekFraction(fraction.clamp(0.0, 1.0)));
    }

    pub fn set_volume(&self, v: f32) {
        self.engine.send(EngineCmd::SetVolume(v.clamp(0.0, 1.0)));
    }

    pub fn set_shuffle(&self, on: bool) {
        self.queue.write().shuffle = on;
        self.state.write().shuffle_enabled = on;
    }

    pub fn cycle_repeat(&self) {
        let mut q = self.queue.write();
        q.repeat = q.repeat.next();
        let mode = q.repeat;
        drop(q);
        self.state.write().repeat_mode = mode;
    }

    pub fn set_repeat(&self, mode: RepeatMode) {
        self.queue.write().repeat = mode;
        self.state.write().repeat_mode = mode;
    }

    pub fn library(&self) -> &Arc<Library> {
        &self.library
    }

    pub fn remove_from_queue(&self, song_id: i64) {
        let was_current_id = self
            .queue
            .read()
            .current_song()
            .map(|s| s.id)
            .filter(|&id| id == song_id);
        {
            let mut q = self.queue.write();
            q.remove_song_id(song_id);
        }
        if was_current_id.is_some() {
            let resume_song = self.queue.read().current_song().cloned();
            match resume_song {
                Some(s) => self.load_and_play(&s),
                None => {
                    self.engine.send(EngineCmd::Stop);
                    self.state.write().is_playing = false;
                    self.state.write().current_song = None;
                }
            }
        }
    }

    fn start_current(&self) {
        let song = self.queue.read().current_song().cloned();
        if let Some(s) = song {
            self.load_and_play(&s);
        }
    }

    fn load_and_play(&self, song: &Song) {
        self.state.write().current_song = Some(song.clone());
        self.state.write().is_playing = true;
        self.engine.send(EngineCmd::Load {
            path: song.path.clone(),
            autoplay: true,
        });
    }

    fn handle_event(&self, evt: EngineEvent) {
        match evt {
            EngineEvent::LoadStarted { .. } => {}
            EngineEvent::LoadFailed { path, error } => {
                tracing::warn!("load failed {}: {error}", path.display());
                self.state.write().is_playing = false;
            }
            EngineEvent::Started { duration } => {
                let mut s = self.state.write();
                s.duration_ms = duration.as_millis() as u64;
                s.current_position_ms = 0;
                s.is_playing = true;
            }
            EngineEvent::Position { current_ms, duration_ms } => {
                let mut s = self.state.write();
                s.current_position_ms = current_ms;
                if duration_ms > 0 {
                    s.duration_ms = duration_ms;
                }
            }
            EngineEvent::Paused => {
                self.state.write().is_playing = false;
            }
            EngineEvent::Resumed => {
                self.state.write().is_playing = true;
            }
            EngineEvent::EndOfTrack => {
                self.next();
            }
        }
    }
}
