use biquad::{Biquad, Coefficients, DirectForm1, ToHertz, Type, Q_BUTTERWORTH_F32};

pub const BAND_FREQS_HZ: [f32; 10] = [
    31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

pub struct Equalizer {
    bands: Vec<DirectForm1<f32>>,
    sample_rate: f32,
    enabled: bool,
}

impl Equalizer {
    pub fn new(sample_rate: u32) -> Self {
        let mut me = Self {
            bands: Vec::new(),
            sample_rate: sample_rate as f32,
            enabled: false,
        };
        me.rebuild(&[0.0; 10]);
        me
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn rebuild(&mut self, gains_db: &[f32; 10]) {
        self.bands.clear();
        for (i, &freq) in BAND_FREQS_HZ.iter().enumerate() {
            let gain_db = gains_db[i];
            let coeffs = Coefficients::<f32>::from_params(
                Type::PeakingEQ(gain_db),
                self.sample_rate.hz(),
                freq.hz(),
                Q_BUTTERWORTH_F32,
            )
            .unwrap_or_else(|_| {
                Coefficients::<f32>::from_params(
                    Type::AllPass,
                    self.sample_rate.hz(),
                    freq.hz(),
                    Q_BUTTERWORTH_F32,
                )
                .unwrap()
            });
            self.bands.push(DirectForm1::<f32>::new(coeffs));
        }
    }

    pub fn process_inplace(&mut self, samples: &mut [f32]) {
        if !self.enabled || self.bands.is_empty() {
            return;
        }
        for s in samples.iter_mut() {
            let mut v = *s;
            for band in self.bands.iter_mut() {
                v = band.run(v);
            }
            *s = v;
        }
    }
}
