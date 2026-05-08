use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamConfig};
use parking_lot::Mutex;
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::HeapRb;

#[derive(Debug, Clone, Copy)]
pub struct OutputConfig {
    pub sample_rate: u32,
    pub channels: u16,
}

pub struct AudioOutput {
    stream: Stream,
    producer: Mutex<ringbuf::HeapProd<f32>>,
    config: OutputConfig,
    is_playing: Arc<AtomicBool>,
    /// Frames played since the last `reset_position` / seek anchor.
    samples_played: Arc<AtomicU64>,
    /// Anchor (in ms) added to the local samples_played counter when reporting
    /// `played_duration`. After a seek to T, the local counter restarts from 0
    /// and this is set to T, so the UI sees position == T immediately rather
    /// than snapping back to 0 while the decoder refills the buffer.
    position_offset_ms: Arc<AtomicU64>,
    volume: Arc<AtomicU32>,
    skip_samples: Arc<AtomicUsize>,
}

unsafe impl Send for AudioOutput {}

impl AudioOutput {
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("no default output device"))?;
        let supported = device
            .default_output_config()
            .map_err(|e| anyhow!("default output config: {e}"))?;

        let sample_rate = supported.sample_rate().0;
        let channels = supported.channels();
        let sample_format = supported.sample_format();
        let config = StreamConfig {
            channels,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };
        tracing::info!(
            "audio output: {} Hz, {} ch, fmt {:?}",
            sample_rate,
            channels,
            sample_format
        );

        let rb_capacity = (sample_rate as usize) * (channels as usize) * 2;
        let rb = HeapRb::<f32>::new(rb_capacity);
        let (producer, mut consumer) = rb.split();

        let is_playing = Arc::new(AtomicBool::new(false));
        let samples_played = Arc::new(AtomicU64::new(0));
        let position_offset_ms = Arc::new(AtomicU64::new(0));
        let volume = Arc::new(AtomicU32::new(1.0_f32.to_bits()));
        let skip_samples = Arc::new(AtomicUsize::new(0));

        let is_playing_cb = is_playing.clone();
        let samples_played_cb = samples_played.clone();
        let volume_cb = volume.clone();
        let skip_samples_cb = skip_samples.clone();
        let channels_cb = channels as usize;

        let stream = match sample_format {
            cpal::SampleFormat::F32 => device.build_output_stream(
                &config,
                move |out: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                    let vol = f32::from_bits(volume_cb.load(Ordering::Relaxed));
                    let playing = is_playing_cb.load(Ordering::Relaxed);

                    let skip = skip_samples_cb.load(Ordering::Acquire);
                    let mut start = 0usize;
                    if skip > 0 {
                        let to_skip = skip.min(out.len());
                        let popped = consumer.pop_slice(&mut out[..to_skip]);
                        skip_samples_cb.store(skip.saturating_sub(popped), Ordering::Release);
                        out[..popped].fill(0.0);
                        start = popped;
                    }

                    let mut filled = start;
                    if playing {
                        let popped = consumer.pop_slice(&mut out[start..]);
                        filled = start + popped;
                        for s in &mut out[start..filled] {
                            *s *= vol;
                        }
                    }
                    for s in &mut out[filled..] {
                        *s = 0.0;
                    }
                    let played = filled.saturating_sub(start);
                    let frames = (played / channels_cb.max(1)) as u64;
                    samples_played_cb.fetch_add(frames, Ordering::Relaxed);
                },
                |err| tracing::error!("cpal stream error: {err}"),
                None,
            )?,
            cpal::SampleFormat::I16 => {
                let channels_cb = channels_cb;
                device.build_output_stream(
                    &config,
                    move |out: &mut [i16], _info: &cpal::OutputCallbackInfo| {
                        let vol = f32::from_bits(volume_cb.load(Ordering::Relaxed));
                        let playing = is_playing_cb.load(Ordering::Relaxed);

                        let skip = skip_samples_cb.load(Ordering::Acquire);
                        if skip > 0 {
                            let to_skip = skip.min(out.len());
                            let mut tmp = vec![0.0f32; to_skip];
                            let popped = consumer.pop_slice(&mut tmp);
                            skip_samples_cb.store(skip.saturating_sub(popped), Ordering::Release);
                        }

                        let mut tmp = vec![0.0f32; out.len()];
                        let mut filled = 0;
                        if playing {
                            filled = consumer.pop_slice(&mut tmp);
                        }
                        for (dst, src) in out.iter_mut().zip(tmp.iter().take(filled)) {
                            let scaled = (src * vol * i16::MAX as f32)
                                .clamp(i16::MIN as f32, i16::MAX as f32);
                            *dst = scaled as i16;
                        }
                        for s in &mut out[filled..] {
                            *s = 0;
                        }
                        let frames = (filled / channels_cb.max(1)) as u64;
                        samples_played_cb.fetch_add(frames, Ordering::Relaxed);
                    },
                    |err| tracing::error!("cpal stream error: {err}"),
                    None,
                )?
            }
            cpal::SampleFormat::U16 => {
                let channels_cb = channels_cb;
                device.build_output_stream(
                    &config,
                    move |out: &mut [u16], _info: &cpal::OutputCallbackInfo| {
                        let vol = f32::from_bits(volume_cb.load(Ordering::Relaxed));
                        let playing = is_playing_cb.load(Ordering::Relaxed);

                        let skip = skip_samples_cb.load(Ordering::Acquire);
                        if skip > 0 {
                            let to_skip = skip.min(out.len());
                            let mut tmp = vec![0.0f32; to_skip];
                            let popped = consumer.pop_slice(&mut tmp);
                            skip_samples_cb.store(skip.saturating_sub(popped), Ordering::Release);
                        }

                        let mut tmp = vec![0.0f32; out.len()];
                        let mut filled = 0;
                        if playing {
                            filled = consumer.pop_slice(&mut tmp);
                        }
                        for (dst, src) in out.iter_mut().zip(tmp.iter().take(filled)) {
                            let v = ((src * vol + 1.0) * 0.5 * u16::MAX as f32)
                                .clamp(0.0, u16::MAX as f32);
                            *dst = v as u16;
                        }
                        for s in &mut out[filled..] {
                            *s = u16::MAX / 2;
                        }
                        let frames = (filled / channels_cb.max(1)) as u64;
                        samples_played_cb.fetch_add(frames, Ordering::Relaxed);
                    },
                    |err| tracing::error!("cpal stream error: {err}"),
                    None,
                )?
            }
            other => return Err(anyhow!("unsupported sample format: {other:?}")),
        };

        stream.play()?;

        Ok(Self {
            stream,
            producer: Mutex::new(producer),
            config: OutputConfig { sample_rate, channels },
            is_playing,
            samples_played,
            position_offset_ms,
            volume,
            skip_samples,
        })
    }

    pub fn config(&self) -> OutputConfig {
        self.config
    }

    pub fn play(&self) {
        self.is_playing.store(true, Ordering::Relaxed);
        let _ = self.stream.play();
    }

    pub fn pause(&self) {
        self.is_playing.store(false, Ordering::Relaxed);
    }

    pub fn clear(&self) {
        self.samples_played.store(0, Ordering::Relaxed);
        self.position_offset_ms.store(0, Ordering::Relaxed);
        self.is_playing.store(false, Ordering::Relaxed);
    }

    pub fn drain_buffer(&self) {
        let occupied = self.producer.lock().occupied_len();
        self.skip_samples.store(occupied, Ordering::Release);
    }

    pub fn set_volume(&self, v: f32) {
        self.volume.store(v.to_bits(), Ordering::Relaxed);
    }

    pub fn played_duration(&self) -> Duration {
        let frames = self.samples_played.load(Ordering::Relaxed);
        let local_ms = (frames * 1000) / self.config.sample_rate.max(1) as u64;
        let anchor_ms = self.position_offset_ms.load(Ordering::Relaxed);
        Duration::from_millis(anchor_ms + local_ms)
    }

    /// Anchor the reported playback position to `ms` and zero the local
    /// sample counter. Used by the engine's seek handler so the UI sees the
    /// new position immediately, not a momentary 0 while the decoder refills.
    pub fn set_position_anchor_ms(&self, ms: u64) {
        self.position_offset_ms.store(ms, Ordering::Relaxed);
        self.samples_played.store(0, Ordering::Relaxed);
    }

    pub fn buffered_samples(&self) -> usize {
        self.producer.lock().occupied_len()
    }

    pub fn push_samples(&self, samples: &[f32]) -> usize {
        self.producer.lock().push_slice(samples)
    }

    pub fn capacity_remaining(&self) -> usize {
        self.producer.lock().vacant_len()
    }

    pub fn reset_position(&self) {
        self.samples_played.store(0, Ordering::Relaxed);
    }
}
