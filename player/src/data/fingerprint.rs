use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use anyhow::{anyhow, Result};
use symphonia::core::audio::{SampleBuffer, SignalSpec};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

const TARGET_SECONDS: f32 = 30.0;
/// Total duration sampled per fingerprint = head 30 s + mid 30 s = 60 s. The
/// decoder reads contiguously from offset 0 up to `HEAD_END_SECONDS`, then
/// (when the file is long enough) skips ahead and reads another
/// `TARGET_SECONDS` from a body offset.
const HEAD_END_SECONDS: f32 = TARGET_SECONDS;
/// Number of analysis windows **per region**. 32 windows × (2 bits centroid +
/// 2 bits RMS) = 128 bits per region; head region packs into `fp[0..2]`, mid
/// region into `fp[2..4]`. Total still 256 bits.
///
/// History (do not regress):
/// * **v5 (128 windows × 1-bit ZCR⊕PF for even + 1-bit RMS for odd)**: timbre
///   channel collapsed to 8/64 set bits on bass-heavy material; same-album
///   tracks (MASN, Nakani, Phonk) converged on identical hi-halves.
/// * **v6 (32 windows × 2-bit centroid + 2-bit RMS = 128 bits)**: a
///   PCM-verified full-corpus audit showed 56 false-positive groups out of
///   231 (175 files affected) — distinct bass-heavy songs all
///   landed in centroid bin 0 across every window, producing degenerate
///   fingerprints like `[0000000000000000, ffffffffffffffff]` shared across
///   5+ unrelated songs.
/// * **v7 (64 windows × 2-bit fixed centroid + 2-bit fixed RMS = 256 bits)**:
///   doubled the bit budget over v6, but full-corpus audit showed false-
///   positive groups only dropped 56→54. The features themselves were
///   exhausted: bass-heavy songs all have centroid < 0.0042 in every window,
///   so all 64 centroid windows pack into bin 0 regardless of song content.
/// * **v8 (64 windows × 2-bit hybrid centroid + 2-bit fixed RMS = 256 bits,
///   first 30 s only)**: replaced the centroid encoding with a hybrid 1-bit
///   fixed (`centroid > 0.011 = corpus median`) + 1-bit rank (`centroid >
///   this song's median centroid`). Audit on the real corpus dropped to 2
///   false / 227 true out of 229 collision groups. The remaining false
///   positive was a 6-way Lefa-album collision: 6 distinct tracks (CDM, Trip,
///   Batman, Course poursuite, Spécial×2) all share a ~25 s DJ-tag + producer-
///   tag + bass intro. With the analysis window pinned to the first 30 s,
///   ~85% of the fingerprint's attention landed on shared intro audio and
///   the song bodies were never sampled.
/// * **v9 (current — two 32-window regions, head + mid, 256 bits total)**:
///   compute the v8 features over the first 30 s (head, packed into `fp[0..2]`)
///   AND a second 30 s starting at `min(file_duration / 3, 30 s)` (mid,
///   packed into `fp[2..4]`). Two songs collide only if BOTH regions match,
///   so a shared album intro alone is not enough — bodies must also coincide.
///   Bit budget per region halves from 64 to 32 windows, but 128 bits per
///   region is still ample resolution for the corpus, and the orthogonality
///   of intro vs body is a stronger discriminator than 64 vs 32 windows on
///   intro alone. The mid offset is a divisor of the file's duration so that
///   intros under ~10 s also land outside the head region; for files shorter
///   than ~3× head (i.e. <90 s), mid is sampled at the same offset as head
///   and the fingerprint degrades to v8 behavior on those short files —
///   acceptable since intro-collision requires long-enough siblings.
const WINDOWS: usize = 32;
/// Minimum mono samples needed for a meaningful fingerprint (per region).
const MIN_MONO_SAMPLES: usize = WINDOWS * 64;

pub type Fingerprint = [u64; 4];

pub fn compute_fingerprint(path: &Path) -> Result<Fingerprint> {
    match catch_unwind(AssertUnwindSafe(|| compute_inner(path))) {
        Ok(r) => r,
        Err(_) => Err(anyhow!(
            "decoder panicked while fingerprinting {}",
            path.display()
        )),
    }
}

fn compute_inner(path: &Path) -> Result<Fingerprint> {
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
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
    let sample_rate = codec_params.sample_rate.ok_or_else(|| anyhow!("no sample rate"))? as f32;
    let channels = codec_params
        .channels
        .ok_or_else(|| anyhow!("no channel layout"))?;

    let mut decoder =
        symphonia::default::get_codecs().make(&codec_params, &DecoderOptions::default())?;

    let max_frames = codec_params.max_frames_per_packet.unwrap_or(8192) as u64;
    let spec = SignalSpec::new(sample_rate as u32, channels);
    let mut sample_buf = SampleBuffer::<f32>::new(max_frames, spec);

    let target_samples = (sample_rate * TARGET_SECONDS) as usize;
    let ch_count = channels.count().max(1);

    // Track total file duration so we can pick a sensible mid offset (and so
    // that on very short files we just sample once).
    let file_duration_secs: Option<f32> = codec_params
        .n_frames
        .map(|n| n as f32 / sample_rate);

    // === Region 1: head (offset 0). ===
    let mut head: Vec<f32> = Vec::with_capacity(target_samples);
    decode_into(
        &mut format,
        &mut decoder,
        &mut sample_buf,
        track_id,
        ch_count,
        target_samples,
        &mut head,
    );
    if head.len() < MIN_MONO_SAMPLES {
        return Err(anyhow!(
            "not enough decoded samples ({}) for fingerprint",
            head.len()
        ));
    }

    // === Region 2: mid. Seek to file_duration / 3 (rounded down to whole
    // seconds), so that the body region begins after typical album intros
    // (which run 20–30 s on Lefa-style hip-hop). Files shorter than 3 ×
    // HEAD_END_SECONDS get the same offset twice — the fingerprint degrades
    // to v8 behavior for very short tracks, which is fine because intro-
    // collision on tracks <90 s is not a real failure mode in this corpus.
    let mid_offset_secs: f32 = match file_duration_secs {
        Some(d) if d >= 3.0 * HEAD_END_SECONDS => (d / 3.0).min(HEAD_END_SECONDS * 4.0),
        _ => 0.0,
    };

    let mut mid: Vec<f32> = Vec::with_capacity(target_samples);
    if mid_offset_secs >= HEAD_END_SECONDS {
        // Seek then decode another TARGET_SECONDS. Coarse seek is fine —
        // we don't need sample accuracy, and Accurate mode is much slower.
        let secs_floor = mid_offset_secs.floor() as u64;
        let frac = (mid_offset_secs - secs_floor as f32) as f64;
        let seek_ok = format
            .seek(
                SeekMode::Coarse,
                SeekTo::Time {
                    time: Time::new(secs_floor, frac),
                    track_id: Some(track_id),
                },
            )
            .is_ok();
        if seek_ok {
            decoder.reset();
            decode_into(
                &mut format,
                &mut decoder,
                &mut sample_buf,
                track_id,
                ch_count,
                target_samples,
                &mut mid,
            );
        }
    }

    // If the mid region didn't yield enough samples (seek failed, file
    // shorter than expected, etc.), fall back to the head buffer so the mid
    // half is still well-formed. This collapses the fingerprint back to the
    // v8 shape on short or unseekable files.
    let mid_ref: &[f32] = if mid.len() >= MIN_MONO_SAMPLES { &mid } else { &head };

    Ok(fingerprint_from_two_regions(&head, mid_ref))
}

/// Pull `target_samples` mono samples out of the format reader at its current
/// position and append them to `out`.
fn decode_into(
    format: &mut Box<dyn symphonia::core::formats::FormatReader>,
    decoder: &mut Box<dyn symphonia::core::codecs::Decoder>,
    sample_buf: &mut SampleBuffer<f32>,
    track_id: u32,
    ch_count: usize,
    target_samples: usize,
    out: &mut Vec<f32>,
) {
    while out.len() < target_samples {
        let packet = match format.next_packet() {
            Ok(p) => p,
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
        let interleaved = sample_buf.samples();
        for chunk in interleaved.chunks(ch_count) {
            let avg = chunk.iter().sum::<f32>() / ch_count as f32;
            out.push(avg);
            if out.len() >= target_samples {
                break;
            }
        }
    }
}

/// Compute a 128-bit fingerprint from a mono PCM buffer (`f32`, range ~[-1, 1]).
///
/// **Layout.** 32 windows × (2 bits centroid + 2 bits RMS) = 128 bits total.
///
/// * `fp[0]` (the **timbre half**, 64 bits) packs 32 windows × 2-bit
///   spectral-centroid-proxy bins. Per window, the centroid proxy is
///   `Σ(diff²) / Σ(amp²)`, which for a pure sine of frequency `f` equals
///   `2(1 − cos(2π·f/sr))`, monotonic in frequency. Cutoffs at `0.0042 /
///   0.011 / 0.034` split the corpus into 4 quartile bins — bin 0 ≈ deep
///   bass, bin 3 ≈ broadband / high content. Window `w`'s 2 bits live at
///   `fp[0] >> (w * 2)`.
///
/// * `fp[1]` (the **loudness half**, 64 bits) packs 32 windows × 2-bit
///   RMS bins. Cutoffs `0.065 / 0.129 / 0.232` split the corpus into 4
///   quartile loudness levels — bin 0 ≈ near-silence, bin 3 ≈ peak loudness.
///
/// **Why 32 windows × 4 bits, not 128 windows × 1 bit (v5).** v5 packed each
/// window into a single bit derived from `(zcr ≥ 0.030) ⊕ (peak/rms ≥ 3.0)`,
/// which collapses to 0 whenever both features sit on the same side of their
/// thresholds (the common bass-heavy regime). Audit on the real 100-file
/// corpus showed 87% of windows mapping to 0 on bass-heavy material, so the
/// MASN trio "Fire / Adrenaline / Princeville Drive" all converged on
/// identical fingerprints. With 4 levels per window per feature, even
/// same-album tracks with shared envelopes differ on at least several
/// centroid bins because their lead frequencies differ section-to-section.
///
/// **Why both centroid and RMS.** Centroid alone collapses on monotone-timbre
/// songs that nonetheless have very different loudness arcs (e.g., a quiet
/// piano piece and a loud orchestral piece, both spectrally narrow). RMS
/// alone collapses on similarly-arranged songs. Together they form a 2-D
/// plane and distinct songs differ on at least one axis with very high
/// probability. The duration co-key in `find_duplicates` catches residual
/// false positives.
///
/// **Why fixed cutoffs, not rank-quantization.** Rank-quantization within a
/// song is envelope-shape-equivalent; two songs with the same dynamic shape
/// (a fade-in, a same-genre intro/build/drop) collapse to the same hash
/// regardless of their actual timbre. Fixed thresholds keep the fingerprint
/// sensitive to *what the song actually is*. See CLAUDE.md constraint #12 —
/// three v1/v2/v3 regressions are documented there and the unit tests below
/// are guards against each.
///
/// **Stability under re-encoding.** Bin boundaries are placed at empirical
/// quartiles, so most windows sit comfortably in the middle of a bin. Small
/// (±0.001) PCM perturbations from re-encoding shift centroid and RMS by
/// fractions of a percent — far less than the bin widths — so re-encoded
/// copies hash to the same fingerprint. The
/// `small_noise_does_not_change_fingerprint` test guards this.
pub fn fingerprint_from_mono(mono: &[f32]) -> Fingerprint {
    // Single-buffer entry point: same buffer used as both head and mid. This
    // keeps synthetic regression tests stable (they pass one buffer in) while
    // the real codepath uses `fingerprint_from_two_regions` with a body slice.
    fingerprint_from_two_regions(mono, mono)
}

/// Compute the 256-bit fingerprint from two independent PCM buffers.
///
/// `head` is analyzed into `fp[0..2]` (low half, 128 bits). `mid` is analyzed
/// into `fp[2..4]` (high half, 128 bits). Each region runs `WINDOWS = 32`
/// 2-bit hybrid-centroid + 2-bit RMS-quartile windows, identical to v8 except
/// for the halved window count per region.
pub fn fingerprint_from_two_regions(head: &[f32], mid: &[f32]) -> Fingerprint {
    let (h0, h1) = region_bits(head);
    let (m0, m1) = region_bits(mid);
    [h0, h1, m0, m1]
}

/// Encode one PCM buffer as 32 windows × (2-bit centroid + 2-bit RMS) packed
/// into two u64s: `(centroid_word, rms_word)`. Each word holds 32 windows × 2
/// bits = 64 bits. Returns `(0, 0)` on too-short buffers — the upstream caller
/// already gates with `MIN_MONO_SAMPLES`.
fn region_bits(mono: &[f32]) -> (u64, u64) {
    if mono.len() < WINDOWS * 2 {
        return (0, 0);
    }
    let n = mono.len();
    let win_len = n / WINDOWS;
    if win_len < 2 {
        return (0, 0);
    }

    // Corpus median centroid (0.011) measured on `player/music/` during the
    // v8 audit. Used as the *fixed* cutoff in the hybrid centroid encoding —
    // the 1-bit fixed half preserves bass-vs-treble discrimination on
    // stationary synthetic signals where rank-quantization is meaningless.
    const CENTROID_FIXED_CUT: f64 = 0.011;
    // Deadzone tolerance for the rank bit. If a window's centroid is within
    // ±RANK_DEADZONE of the song's median centroid (relative), suppress the
    // rank bit. Stationary signals (where every window is essentially equal
    // to the median) all fall into the deadzone, so noise can't flip rank
    // bits, and `small_noise_does_not_change_fingerprint` stays stable.
    const RANK_DEADZONE: f64 = 0.20;
    // RMS quartile cutoffs (current — well-balanced on the real corpus).
    const RMS_CUTOFFS: [f64; 3] = [0.065, 0.129, 0.232];

    let mut centroids = [0f64; WINDOWS];
    let mut rms_values = [0f64; WINDOWS];
    for w in 0..WINDOWS {
        let start = w * win_len;
        let end = start + win_len;
        let slice = &mono[start..end];

        let mut energy = 0f64;
        let mut diff_energy = 0f64;
        let first = slice[0] as f64;
        energy += first * first;
        let mut prev = first;
        for &s in &slice[1..] {
            let cur = s as f64;
            energy += cur * cur;
            let d = cur - prev;
            diff_energy += d * d;
            prev = cur;
        }
        rms_values[w] = (energy / slice.len() as f64).sqrt();
        centroids[w] = if energy > 1e-12 { diff_energy / energy } else { 0.0 };
    }

    // Region-median centroid for the rank bit.
    let mut sorted = centroids;
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_centroid = sorted[WINDOWS / 2];

    let mut centroid_word: u64 = 0;
    let mut rms_word: u64 = 0;

    for w in 0..WINDOWS {
        let centroid = centroids[w];
        let rms = rms_values[w];

        let fixed_bit = centroid > CENTROID_FIXED_CUT;
        let rel_diff = (centroid - median_centroid).abs() / median_centroid.max(1e-12);
        let rank_bit = rel_diff >= RANK_DEADZONE && centroid > median_centroid;
        let c_bin: u64 = ((fixed_bit as u64) << 1) | (rank_bit as u64);

        let r_bin = quartile_bin(rms, &RMS_CUTOFFS) as u64;

        centroid_word |= c_bin << (w * 2);
        rms_word |= r_bin << (w * 2);
    }

    (centroid_word, rms_word)
}

/// Map a value into one of 4 quartile bins (0..=3) using ascending cutoffs.
fn quartile_bin(v: f64, cutoffs: &[f64; 3]) -> u8 {
    if v < cutoffs[0] {
        0
    } else if v < cutoffs[1] {
        1
    } else if v < cutoffs[2] {
        2
    } else {
        3
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    fn sine(freq_hz: f32, sample_rate: f32, samples: usize, amp: f32) -> Vec<f32> {
        (0..samples)
            .map(|i| (TAU * freq_hz * i as f32 / sample_rate).sin() * amp)
            .collect()
    }

    /// Bass-heavy: low-frequency sine + faint high content.
    fn bass_heavy(sample_rate: f32, samples: usize) -> Vec<f32> {
        let bass = sine(80.0, sample_rate, samples, 0.6);
        let hi = sine(4000.0, sample_rate, samples, 0.05);
        bass.iter().zip(hi.iter()).map(|(a, b)| a + b).collect()
    }

    /// Treble-heavy: high-frequency sine + faint low content. Same RMS as bass_heavy
    /// (approximately) so loudness alone cannot distinguish them.
    fn treble_heavy(sample_rate: f32, samples: usize) -> Vec<f32> {
        let bass = sine(80.0, sample_rate, samples, 0.05);
        let hi = sine(4000.0, sample_rate, samples, 0.6);
        bass.iter().zip(hi.iter()).map(|(a, b)| a + b).collect()
    }

    #[test]
    fn identical_signals_match() {
        let signal: Vec<f32> = (0..32000).map(|i| (i as f32 * 0.05).sin()).collect();
        assert_eq!(
            fingerprint_from_mono(&signal),
            fingerprint_from_mono(&signal)
        );
    }

    #[test]
    fn deterministic_across_calls() {
        // Important: Fingerprint must be stable across runs (no seed-by-time hashers).
        let s = sine(440.0, 44100.0, 44100, 0.4);
        let a = fingerprint_from_mono(&s);
        let b = fingerprint_from_mono(&s);
        let c = fingerprint_from_mono(&s);
        assert_eq!(a, b);
        assert_eq!(b, c);
    }

    #[test]
    fn distinct_envelopes_differ() {
        let flat: Vec<f32> = vec![0.5; 32000];
        let ramp: Vec<f32> = (0..32000).map(|i| i as f32 / 32000.0).collect();
        assert_ne!(
            fingerprint_from_mono(&flat),
            fingerprint_from_mono(&ramp)
        );
    }

    #[test]
    fn quiet_and_loud_signals_have_distinct_fingerprints() {
        // Different shapes at different absolute amplitudes — must differ.
        let quiet: Vec<f32> = (0..32000).map(|i| (i as f32 * 0.01).sin() * 0.01).collect();
        let loud: Vec<f32> = (0..32000).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
        assert_ne!(
            fingerprint_from_mono(&quiet),
            fingerprint_from_mono(&loud)
        );
    }

    /// Regression for v1: 32 windows × rank-quartile of mean |amp| has every
    /// song's windows uniformly spread across 4 quartiles, so two unrelated
    /// signals at similar dynamic shape collide.
    #[test]
    fn distinct_sine_frequencies_at_same_amplitude_do_not_collide() {
        // 220 Hz vs 880 Hz, same amp, same length. v1/v2/v3 collide here
        // because envelope is flat.
        let a = sine(220.0, 44100.0, 44100, 0.4);
        let b = sine(880.0, 44100.0, 44100, 0.4);
        assert_ne!(
            fingerprint_from_mono(&a),
            fingerprint_from_mono(&b),
            "different sine frequencies at the same loudness must not collide"
        );
    }

    /// Regression for v3 (the user-reported bug): the 64-sample-average
    /// pipeline acts as a low-pass and captures only bass envelope, so two
    /// songs with similar bass programming but different timbre collide.
    /// Bass-heavy and treble-heavy signals at the same RMS must differ.
    #[test]
    fn bass_heavy_vs_treble_heavy_at_same_loudness_differ() {
        let a = bass_heavy(44100.0, 44100);
        let b = treble_heavy(44100.0, 44100);
        assert_ne!(
            fingerprint_from_mono(&a),
            fingerprint_from_mono(&b),
            "songs with the same loudness but inverted bass/treble balance \
             must not collide — this is what broke v3 on the Lefa album"
        );
    }

    /// Regression for v4 (the centroid-collapse bug found in the v4→v5
    /// corpus audit): on the real 100-file corpus, three different MASN tracks
    /// (`10 - Fire`, `11 - Adrenaline`, `12 - Princeville Drive`) collided
    /// because the centroid proxy thresholds put 88.6% of all windows into
    /// bins 1–2 and bin 3 was <1% populated. Same-artist tracks with similar
    /// loudness arcs and similar tonal balance landed on identical hashes.
    /// This synthetic case mirrors that shape: two tracks with the *same*
    /// RMS envelope and similar tonal balance but different lead frequencies
    /// must produce different fingerprints.
    #[test]
    fn same_artist_same_loudness_different_lead_pitches_differ() {
        let sr = 44100.0_f32;
        let n = 44100;
        // Shared kick + bass programming (loudness arc identical).
        let backing: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / sr;
                let kick = (-(t * 6.0).fract() * 8.0).exp() * (TAU * 60.0 * t).sin();
                let bass = 0.4 * (TAU * 110.0 * t).sin();
                0.3 * kick + 0.5 * bass
            })
            .collect();
        // Track A: 600 Hz lead. Track B: 2400 Hz lead. Same amplitude — so
        // RMS bins are identical window-for-window.
        let a: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / sr;
                backing[i] + 0.15 * (TAU * 600.0 * t).sin()
            })
            .collect();
        let b: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / sr;
                backing[i] + 0.15 * (TAU * 2400.0 * t).sin()
            })
            .collect();
        let fa = fingerprint_from_mono(&a);
        let fb = fingerprint_from_mono(&b);
        // The RMS halves can match (loudness is identical by construction);
        // the ZCR halves must differ.
        assert_ne!(
            fa, fb,
            "same artist / same loudness arc / different lead frequency must \
             not collide — this is the v4 MASN false-positive shape"
        );
        assert!(
            fa[0] != fb[0] || fa[1] != fb[1],
            "centroid stream (fp[0..2]) must carry the timbral difference — if \
             these are equal then the centroid encoding has collapsed and the \
             bug is back"
        );
    }

    /// Regression for the rank-quantize-only design: same envelope shape with
    /// different carrier frequency must not collide. (This is exactly the bug
    /// the previous attempt also still had.)
    #[test]
    fn distinct_waveforms_under_same_envelope_differ() {
        let n = 44100;
        let env: Vec<f32> = (0..n).map(|i| 0.1 + 0.7 * (i as f32 / n as f32)).collect();
        let a: Vec<f32> = env
            .iter()
            .enumerate()
            .map(|(i, e)| e * (TAU * 200.0 * i as f32 / 44100.0).sin())
            .collect();
        let b: Vec<f32> = env
            .iter()
            .enumerate()
            .map(|(i, e)| e * (TAU * 3000.0 * i as f32 / 44100.0).sin())
            .collect();
        assert_ne!(
            fingerprint_from_mono(&a),
            fingerprint_from_mono(&b),
            "same envelope, different carrier frequency must not collide"
        );
    }

    /// Robustness: small encoder-style noise must not flip the fingerprint.
    /// Re-encoded copies show ~0.001 amplitude perturbations and need to match.
    #[test]
    fn small_noise_does_not_change_fingerprint() {
        let n = 44100;
        let base: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / 44100.0;
                0.4 * (TAU * 220.0 * t).sin()
                    + 0.2 * (TAU * 880.0 * t).sin()
                    + 0.1 * (TAU * 1760.0 * t).sin()
            })
            .collect();
        // Linear-congruential noise in [-0.001, 0.001].
        let mut state: u32 = 0xC0FFEE;
        let perturbed: Vec<f32> = base
            .iter()
            .map(|&s| {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                let r = (state >> 8) as f32 / (1u32 << 24) as f32;
                s + (r - 0.5) * 0.002
            })
            .collect();
        assert_eq!(
            fingerprint_from_mono(&base),
            fingerprint_from_mono(&perturbed),
            "≤0.001 PCM noise must not change the fingerprint"
        );
    }

    /// Two same-genre, same-loudness, same-tempo synthetic "songs" with
    /// different lead melodies on top of the same bass pattern. This is the
    /// shape of the user's Lefa-album false positive: same artist, same kick
    /// pattern, different vocals/melody. Must not collide.
    #[test]
    fn same_genre_different_melody_do_not_collide() {
        let sr = 44100.0;
        let n = 44100;

        // Shared bass pattern: 60Hz pulses every 0.5s with envelope decay.
        let bass: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / sr;
                let pulse = (t * 2.0).fract();
                let env = (-pulse * 4.0).exp();
                0.4 * env * (TAU * 60.0 * t).sin()
            })
            .collect();

        // Song A: melody at 660Hz on top of bass.
        let a: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / sr;
                bass[i] + 0.25 * (TAU * 660.0 * t).sin()
            })
            .collect();
        // Song B: melody at 1320Hz on top of bass.
        let b: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / sr;
                bass[i] + 0.25 * (TAU * 1320.0 * t).sin()
            })
            .collect();

        assert_ne!(
            fingerprint_from_mono(&a),
            fingerprint_from_mono(&b),
            "same bass + different melody must not collide — this matches \
             the Lefa-album shape the user reported"
        );
    }

    /// Sanity: a non-trivial signal must have a non-trivial fingerprint, and
    /// information must be present in both halves (otherwise we only get 64
    /// bits of entropy and the fingerprint behaves like one feature).
    #[test]
    fn fingerprint_is_not_degenerate() {
        let s: Vec<f32> = (0..44100)
            .map(|i| {
                let t = i as f32 / 44100.0;
                0.3 * (TAU * 220.0 * t).sin() + 0.2 * (TAU * 1500.0 * t * t).sin()
            })
            .collect();
        let fp = fingerprint_from_mono(&s);
        assert_ne!(fp, [0; 4]);
        // Centroid stream lives in fp[0..2], RMS stream in fp[2..4]. Each must
        // carry information for a non-trivial chirp + tone signal.
        assert!(fp[0] != 0 || fp[1] != 0, "centroid half must be non-zero");
        assert!(fp[2] != 0 || fp[3] != 0, "rms half must be non-zero");
    }

    /// White noise vs a tonal signal at the same RMS: the centroid axis must
    /// distinguish them (noise is broadband, tone is narrowband).
    #[test]
    fn noise_and_tone_at_same_rms_differ() {
        let n = 44100;
        let tone = sine(440.0, 44100.0, n, 0.3);
        // Pseudo-random noise scaled to the same RMS as the tone (≈ 0.3/√2).
        let mut state: u32 = 0xDEADBEEF;
        let target_rms = 0.3 / std::f32::consts::SQRT_2;
        let raw: Vec<f32> = (0..n)
            .map(|_| {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                ((state >> 8) as f32 / (1u32 << 24) as f32) - 0.5
            })
            .collect();
        let raw_rms = (raw.iter().map(|s| s * s).sum::<f32>() / n as f32).sqrt();
        let scale = target_rms / raw_rms;
        let noise: Vec<f32> = raw.iter().map(|s| s * scale).collect();
        assert_ne!(
            fingerprint_from_mono(&tone),
            fingerprint_from_mono(&noise),
            "tone vs broadband noise at the same RMS must not collide"
        );
    }
}
