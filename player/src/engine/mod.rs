pub mod decoder;
pub mod eq;
pub mod output;

use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver, Sender};
use parking_lot::Mutex;

use crate::engine::decoder::DecodeJob;
use crate::engine::output::{AudioOutput, OutputConfig};

#[derive(Debug, Clone)]
pub enum EngineCmd {
    Load { path: PathBuf, autoplay: bool },
    Play,
    Pause,
    Stop,
    SeekFraction(f32),
    SetVolume(f32),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    LoadStarted { path: PathBuf },
    LoadFailed { path: PathBuf, error: String },
    Started { duration: Duration },
    Position { current_ms: u64, duration_ms: u64 },
    Paused,
    Resumed,
    EndOfTrack,
}

pub struct Engine {
    cmd_tx: Sender<EngineCmd>,
    event_rx: Receiver<EngineEvent>,
}

impl Engine {
    pub fn start() -> Result<Self> {
        let (cmd_tx, cmd_rx) = unbounded::<EngineCmd>();
        let (event_tx, event_rx) = unbounded::<EngineEvent>();

        let output = AudioOutput::new()?;
        let output = Arc::new(Mutex::new(output));

        thread::Builder::new()
            .name("engine".into())
            .spawn(move || run_engine(cmd_rx, event_tx, output))?;

        Ok(Engine { cmd_tx, event_rx })
    }

    pub fn send(&self, cmd: EngineCmd) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn events(&self) -> &Receiver<EngineEvent> {
        &self.event_rx
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(EngineCmd::Shutdown);
    }
}

fn run_engine(cmd_rx: Receiver<EngineCmd>, event_tx: Sender<EngineEvent>, output: Arc<Mutex<AudioOutput>>) {
    let mut current_job: Option<DecodeJob> = None;
    let mut paused = false;

    loop {
        let recv_timeout = if current_job.is_some() && !paused {
            Duration::from_millis(20)
        } else {
            Duration::from_secs(60)
        };

        match cmd_rx.recv_timeout(recv_timeout) {
            Ok(EngineCmd::Load { path, autoplay }) => {
                let _ = event_tx.send(EngineEvent::LoadStarted { path: path.clone() });

                if let Some(prev) = current_job.take() {
                    prev.stop();
                }
                {
                    let out = output.lock();
                    out.clear();
                    out.drain_buffer();
                }

                let target_cfg: OutputConfig = output.lock().config();
                match decoder::start_decode(path.clone(), target_cfg, output.clone()) {
                    Ok(job) => {
                        let _ = event_tx.send(EngineEvent::Started { duration: job.duration });
                        current_job = Some(job);
                        paused = !autoplay;
                        if autoplay {
                            output.lock().play();
                        } else {
                            output.lock().pause();
                        }
                    }
                    Err(e) => {
                        let _ = event_tx.send(EngineEvent::LoadFailed { path, error: format!("{e:#}") });
                    }
                }
            }
            Ok(EngineCmd::Play) => {
                output.lock().play();
                paused = false;
                let _ = event_tx.send(EngineEvent::Resumed);
            }
            Ok(EngineCmd::Pause) => {
                output.lock().pause();
                paused = true;
                let _ = event_tx.send(EngineEvent::Paused);
            }
            Ok(EngineCmd::Stop) => {
                output.lock().clear();
                if let Some(job) = current_job.take() {
                    job.stop();
                }
                paused = true;
            }
            Ok(EngineCmd::SeekFraction(fraction)) => {
                if let Some(job) = current_job.as_mut() {
                    let target = job.duration.mul_f32(fraction.clamp(0.0, 1.0));
                    let target_ms = target.as_millis() as u64;
                    if let Err(e) = job.seek(target) {
                        tracing::warn!("seek failed: {e:#}");
                    }
                    let out = output.lock();
                    out.drain_buffer();
                    // Anchor the position counter to the seek target so
                    // `played_duration()` immediately reports the new position
                    // instead of dropping to 0 (which made the UI slider snap
                    // back to the start the moment the user released).
                    out.set_position_anchor_ms(target_ms);
                    if !paused {
                        out.play();
                    }
                }
            }
            Ok(EngineCmd::SetVolume(v)) => {
                output.lock().set_volume(v.clamp(0.0, 1.0));
            }
            Ok(EngineCmd::Shutdown) => break,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        }

        if let Some(job) = current_job.as_mut() {
            if !paused {
                let pos = output.lock().played_duration();
                let _ = event_tx.send(EngineEvent::Position {
                    current_ms: pos.as_millis() as u64,
                    duration_ms: job.duration.as_millis() as u64,
                });
                if job.is_finished() && output.lock().buffered_samples() == 0 {
                    let _ = event_tx.send(EngineEvent::EndOfTrack);
                    current_job = None;
                }
            }
        }
    }

    if let Some(job) = current_job.take() {
        job.stop();
    }
}
