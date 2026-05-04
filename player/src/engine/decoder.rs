use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use rubato::{FftFixedInOut, Resampler};
use symphonia::core::audio::{SampleBuffer, SignalSpec};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

use crate::engine::output::{AudioOutput, OutputConfig};

pub struct DecodeJob {
    pub duration: Duration,
    handle: Option<JoinHandle<()>>,
    stop_flag: Arc<AtomicBool>,
    seek_request: Arc<Mutex<Option<Duration>>>,
    finished: Arc<AtomicBool>,
}

impl DecodeJob {
    pub fn stop(mut self) {
        self.stop_flag.store(true, Ordering::Release);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }

    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }

    pub fn seek(&self, target: Duration) -> Result<()> {
        *self.seek_request.lock() = Some(target);
        Ok(())
    }
}

impl Drop for DecodeJob {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Release);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

pub fn start_decode(
    path: PathBuf,
    target_cfg: OutputConfig,
    output: Arc<Mutex<AudioOutput>>,
) -> Result<DecodeJob> {
    let file = std::fs::File::open(&path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions {
            enable_gapless: true,
            ..Default::default()
        },
        &MetadataOptions::default(),
    )?;
    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow!("no decodable track"))?;
    let track_id = track.id;
    let codec_params = track.codec_params.clone();

    let sample_rate_in = codec_params.sample_rate.ok_or_else(|| anyhow!("no sample rate"))?;
    let channels_in_layout = codec_params
        .channels
        .ok_or_else(|| anyhow!("no channel layout"))?;
    let channels_in = channels_in_layout.count();
    let n_frames = codec_params.n_frames.unwrap_or(0);
    let duration = if n_frames > 0 {
        Duration::from_secs_f64(n_frames as f64 / sample_rate_in as f64)
    } else {
        Duration::from_secs(0)
    };

    let mut decoder = symphonia::default::get_codecs().make(&codec_params, &DecoderOptions::default())?;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let seek_request: Arc<Mutex<Option<Duration>>> = Arc::new(Mutex::new(None));
    let finished = Arc::new(AtomicBool::new(false));

    let stop_th = stop_flag.clone();
    let seek_th = seek_request.clone();
    let fin_th = finished.clone();
    let target_th = target_cfg;

    let handle = thread::Builder::new()
        .name("decoder".into())
        .spawn(move || {
            let max_frames = codec_params.max_frames_per_packet.unwrap_or(8192) as u64;
            let spec = SignalSpec::new(sample_rate_in, channels_in_layout);
            let mut sample_buf = SampleBuffer::<f32>::new(max_frames, spec);

            let need_resample = sample_rate_in != target_th.sample_rate;
            let mut resampler: Option<FftFixedInOut<f32>> = if need_resample {
                FftFixedInOut::<f32>::new(
                    sample_rate_in as usize,
                    target_th.sample_rate as usize,
                    1024,
                    channels_in.max(1),
                )
                .ok()
            } else {
                None
            };

            let mut planar: Vec<Vec<f32>> = vec![Vec::with_capacity(8192); channels_in.max(1)];
            let mut interleaved_out = Vec::<f32>::with_capacity(8192);

            loop {
                if stop_th.load(Ordering::Acquire) {
                    break;
                }

                if let Some(target) = seek_th.lock().take() {
                    let _ = format.seek(
                        SeekMode::Coarse,
                        SeekTo::Time {
                            time: Time::new(
                                target.as_secs(),
                                target.subsec_nanos() as f64 / 1e9,
                            ),
                            track_id: Some(track_id),
                        },
                    );
                    decoder.reset();
                    output.lock().reset_position();
                    for v in planar.iter_mut() {
                        v.clear();
                    }
                }

                let packet = match format.next_packet() {
                    Ok(p) => p,
                    Err(symphonia::core::errors::Error::IoError(ref e))
                        if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                    {
                        break;
                    }
                    Err(_) => break,
                };
                if packet.track_id() != track_id {
                    continue;
                }

                let decoded = match decoder.decode(&packet) {
                    Ok(d) => d,
                    Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
                    Err(_) => break,
                };

                sample_buf.copy_interleaved_ref(decoded);
                let interleaved_in = sample_buf.samples();

                if !need_resample {
                    push_with_channel_map(
                        &output,
                        interleaved_in,
                        channels_in,
                        target_th.channels as usize,
                        &stop_th,
                    );
                    continue;
                }

                let frames = interleaved_in.len() / channels_in;
                for f in 0..frames {
                    for ch in 0..channels_in {
                        planar[ch].push(interleaved_in[f * channels_in + ch]);
                    }
                }

                let rs = resampler.as_mut().unwrap();
                let needed = rs.input_frames_next();
                while planar[0].len() >= needed && !stop_th.load(Ordering::Acquire) {
                    let mut chunk_vec: Vec<Vec<f32>> = Vec::with_capacity(channels_in);
                    for ch_buf in planar.iter_mut() {
                        let take: Vec<f32> = ch_buf.drain(..needed).collect();
                        chunk_vec.push(take);
                    }
                    let out_chunks = match rs.process(&chunk_vec, None) {
                        Ok(o) => o,
                        Err(e) => {
                            tracing::warn!("resample error: {e}");
                            break;
                        }
                    };

                    interleaved_out.clear();
                    let frames_out = out_chunks[0].len();
                    let target_ch = target_th.channels as usize;
                    for f in 0..frames_out {
                        for tch in 0..target_ch {
                            let src_ch = if tch < out_chunks.len() { tch } else { 0 };
                            interleaved_out.push(out_chunks[src_ch][f]);
                        }
                    }
                    push_or_block(&output, &interleaved_out, &stop_th);
                }
            }

            fin_th.store(true, Ordering::Release);
        })?;

    Ok(DecodeJob {
        duration,
        handle: Some(handle),
        stop_flag,
        seek_request,
        finished,
    })
}

fn push_or_block(output: &Arc<Mutex<AudioOutput>>, samples: &[f32], stop: &AtomicBool) {
    let mut written = 0;
    while written < samples.len() {
        if stop.load(Ordering::Acquire) {
            return;
        }
        let pushed = output.lock().push_samples(&samples[written..]);
        written += pushed;
        if pushed == 0 {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

fn push_with_channel_map(
    output: &Arc<Mutex<AudioOutput>>,
    interleaved: &[f32],
    src_channels: usize,
    dst_channels: usize,
    stop: &AtomicBool,
) {
    if src_channels == dst_channels || src_channels == 0 {
        push_or_block(output, interleaved, stop);
        return;
    }
    let frames = interleaved.len() / src_channels;
    let mut buf = Vec::with_capacity(frames * dst_channels);
    for f in 0..frames {
        for ch in 0..dst_channels {
            let src_ch = if ch < src_channels { ch } else { 0 };
            buf.push(interleaved[f * src_channels + src_ch]);
        }
    }
    push_or_block(output, &buf, stop);
}
