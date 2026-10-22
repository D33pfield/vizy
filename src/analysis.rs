//! Turns raw samples into what the renderers draw: per-channel loudness and
//! log-spaced spectrum bands, smoothed and with peak hold.
//!
//! Spectrum chain per channel: Hann window → FFT → log bands → treble tilt →
//! auto-gain → fast-attack/slow-decay smoothing → peak hold.
//!
//! Two FFT sizes run side by side: a short one that reacts quickly for most
//! bands, and a long one with finer frequency resolution for the bass, where
//! log-spaced bands get narrower than a short FFT's bins.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

use crate::audio::Shared;

/// 4096 samples at 48 kHz is ~85 ms, with ~11.7 Hz between FFT bins.
const FFT_SMALL: usize = 4096;
/// 8192 samples: ~170 ms, ~5.9 Hz bins. Used for bands narrower than
/// BASS_MIN_BINS bins of the small FFT.
const FFT_LARGE: usize = 8192;
const BASS_MIN_BINS: f32 = 2.0;
/// Loudness (RMS) is measured over this window of the most recent samples.
const RMS_WINDOW_SECONDS: f32 = 0.05;
/// No packets for this long means the output is silent.
const SILENCE_AFTER: Duration = Duration::from_millis(100);

/// Bottom of the level meter scale, in dBFS.
pub const LEVEL_FLOOR_DB: f32 = -60.0;
const FREQ_HI: f32 = 16_000.0;
/// Music has far less energy up high; tilt the spectrum so treble shows up.
/// The tilt pivots at TILT_PIVOT_HZ and only boosts: bass is never cut by it.
const TILT_DB_PER_OCTAVE: f32 = 4.5;
const TILT_PIVOT_HZ: f32 = 500.0;
/// How many dB the spectrum display spans, from full height down to nothing.
pub const SPECTRUM_RANGE_DB: f32 = 50.0;
/// Auto-gain never boosts past this, so quiet passages and noise stay small.
const AUTOGAIN_MIN_DB: f32 = -40.0;
const AUTOGAIN_FALL_DB_PER_SECOND: f32 = 3.0;
/// Room left above the loudest band so it doesn't sit pinned at the top.
const HEADROOM_DB: f32 = 3.0;

const LEVEL_FALL_PER_SECOND: f32 = 1.5;
const BAND_FALL_PER_SECOND: f32 = 1.2;
const PEAK_HOLD_SECONDS: f32 = 0.8;
const PEAK_FALL_PER_SECOND: f32 = 0.35;

/// A 0..1 value that jumps up instantly, falls back slowly, and keeps a
/// peak marker that holds for a moment before falling too.
#[derive(Clone, Default)]
pub struct Smoothed {
    pub value: f32,
    pub peak: f32,
    peak_age: f32,
}

impl Smoothed {
    fn update(&mut self, target: f32, fall_per_second: f32, dt: f32) {
        self.value = if target > self.value {
            target
        } else {
            (self.value - fall_per_second * dt).max(target)
        };

        if self.value >= self.peak {
            self.peak = self.value;
            self.peak_age = 0.0;
        } else {
            self.peak_age += dt;
            if self.peak_age > PEAK_HOLD_SECONDS {
                self.peak = (self.peak - PEAK_FALL_PER_SECOND * dt).max(self.value);
            }
        }
    }
}

#[derive(Default)]
pub struct Channel {
    /// Loudness, 0..1 on the level meter's dB scale.
    pub level: Smoothed,
    /// Instantaneous loudness in dBFS (-inf when silent).
    pub db: f32,
    /// Spectrum bands, low to high frequency, each 0..1.
    pub bands: Vec<Smoothed>,
}

/// Everything the analysis side hands to a renderer each frame.
pub struct Frame {
    /// False while the output device is sending nothing (silence).
    pub live: bool,
    pub left: Channel,
    pub right: Channel,
    /// Average of left and right bands.
    pub mono: Vec<Smoothed>,
    /// dB value that a full-height spectrum band corresponds to right now.
    pub gain_top_db: f32,
    pub freq_lo: f32,
    pub freq_hi: f32,
}

impl Frame {
    /// Where `hz` falls across the spectrum, 0 (left edge) to 1 (right edge).
    pub fn freq_position(&self, hz: f32) -> f32 {
        (hz / self.freq_lo).ln() / (self.freq_hi / self.freq_lo).ln()
    }
}

pub fn level_to_meter(db: f32) -> f32 {
    ((db - LEVEL_FLOOR_DB) / -LEVEL_FLOOR_DB).clamp(0.0, 1.0)
}

/// One FFT size with its window and output magnitudes.
struct Spectrum {
    size: usize,
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    /// Scale that makes a full-scale sine read 1.0 (0 dB).
    scale: f32,
    buf: Vec<Complex<f32>>,
    mags: Vec<f32>,
}

impl Spectrum {
    fn new(size: usize, planner: &mut FftPlanner<f32>) -> Self {
        let window: Vec<f32> = (0..size)
            .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / size as f32).cos())
            .collect();
        Self {
            size,
            fft: planner.plan_fft_forward(size),
            scale: 2.0 / window.iter().sum::<f32>(),
            window,
            buf: vec![Complex::default(); size],
            mags: vec![0.0; size / 2 + 1],
        }
    }

    /// FFT the last `size` samples of `samples`.
    fn compute(&mut self, samples: &[f32]) {
        let samples = &samples[samples.len() - self.size..];
        for ((c, &x), &w) in self.buf.iter_mut().zip(samples).zip(&self.window) {
            *c = Complex::new(x * w, 0.0);
        }
        self.fft.process(&mut self.buf);
        for (m, c) in self.mags.iter_mut().zip(&self.buf) {
            *m = c.norm() * self.scale;
        }
    }

    /// Strongest magnitude between two frequencies.
    fn band_mag(&self, lo_hz: f32, hi_hz: f32, sample_rate: f32) -> f32 {
        let bin_hz = sample_rate / self.size as f32;
        let (lo, hi) = (lo_hz / bin_hz, hi_hz / bin_hz);
        let last = self.mags.len() - 1;
        if hi - lo < 1.0 {
            // Narrower than one bin: interpolate between neighbouring bins.
            let pos = ((lo + hi) / 2.0).min(last as f32);
            let k = pos.floor() as usize;
            let t = pos - k as f32;
            self.mags[k] * (1.0 - t) + self.mags[(k + 1).min(last)] * t
        } else {
            let (k0, k1) = (lo.ceil() as usize, (hi.floor() as usize).min(last));
            self.mags[k0..=k1].iter().copied().fold(0.0, f32::max)
        }
    }
}

pub struct Analyzer {
    sample_rate: f32,
    small: Spectrum,
    large: Spectrum,
    /// Latest FFT_LARGE samples for left and right.
    samples: [Vec<f32>; 2],
    /// Per-channel band levels in dB, before auto-gain.
    band_db: [Vec<f32>; 2],
    /// Band edge frequencies (one more than the band count).
    edges: Vec<f32>,
    gain_ref_db: f32,
    pub frame: Frame,
}

impl Analyzer {
    pub fn new(sample_rate: u32, bands: usize, freq_lo: f32) -> Self {
        let sr = sample_rate as f32;
        let mut planner = FftPlanner::new();
        let mut analyzer = Self {
            sample_rate: sr,
            small: Spectrum::new(FFT_SMALL, &mut planner),
            large: Spectrum::new(FFT_LARGE, &mut planner),
            samples: [vec![0.0; FFT_LARGE], vec![0.0; FFT_LARGE]],
            band_db: [Vec::new(), Vec::new()],
            edges: Vec::new(),
            gain_ref_db: AUTOGAIN_MIN_DB,
            frame: Frame {
                live: false,
                left: Channel::default(),
                right: Channel::default(),
                mono: Vec::new(),
                gain_top_db: AUTOGAIN_MIN_DB + HEADROOM_DB,
                freq_lo: 0.0,
                freq_hi: FREQ_HI.min(sr * 0.45),
            },
        };
        analyzer.configure(bands, freq_lo);
        analyzer
    }

    /// Re-split the spectrum into `n` log-spaced bands starting at `freq_lo`
    /// (resets band smoothing).
    pub fn configure(&mut self, n: usize, freq_lo: f32) {
        if n == self.frame.mono.len() && freq_lo == self.frame.freq_lo {
            return;
        }
        self.frame.freq_lo = freq_lo;
        let (lo, hi) = (self.frame.freq_lo, self.frame.freq_hi);
        self.edges = (0..=n).map(|i| lo * (hi / lo).powf(i as f32 / n as f32)).collect();
        self.frame.left.bands = vec![Smoothed::default(); n];
        self.frame.right.bands = vec![Smoothed::default(); n];
        self.frame.mono = vec![Smoothed::default(); n];
        self.band_db = [vec![0.0; n], vec![0.0; n]];
    }

    pub fn update(&mut self, shared: &Mutex<Shared>, dt: f32) {
        let rms_n = (self.sample_rate * RMS_WINDOW_SECONDS) as usize;
        let mut rms = [0.0f32; 2];
        let live = {
            let s = shared.lock().unwrap();
            for (i, src) in [&s.left, &s.right].into_iter().enumerate() {
                // Newest FFT_LARGE samples, zero-padded at the front if we have fewer.
                let dst = &mut self.samples[i];
                let take = src.len().min(FFT_LARGE);
                dst[..FFT_LARGE - take].fill(0.0);
                for (d, &x) in dst[FFT_LARGE - take..].iter_mut().zip(src.range(src.len() - take..)) {
                    *d = x;
                }
                let n = rms_n.min(take).max(1);
                rms[i] = (dst[FFT_LARGE - n..].iter().map(|x| x * x).sum::<f32>() / n as f32).sqrt();
            }
            s.last_packet.is_some_and(|t| t.elapsed() <= SILENCE_AFTER) && !s.left.is_empty()
        };
        self.frame.live = live;

        for (i, ch) in [&mut self.frame.left, &mut self.frame.right].into_iter().enumerate() {
            ch.db = if live { 20.0 * rms[i].max(1e-9).log10() } else { f32::NEG_INFINITY };
            ch.level.update(level_to_meter(ch.db), LEVEL_FALL_PER_SECOND, dt);
        }

        self.spectrum_db(0);
        self.spectrum_db(1);

        // Auto-gain follows the loudest band: up instantly, down slowly.
        if live {
            let loudest = self.band_db.iter().flatten().copied().fold(f32::NEG_INFINITY, f32::max);
            self.gain_ref_db = if loudest > self.gain_ref_db {
                loudest
            } else {
                self.gain_ref_db - AUTOGAIN_FALL_DB_PER_SECOND * dt
            };
            self.gain_ref_db = self.gain_ref_db.max(AUTOGAIN_MIN_DB);
        }
        let top = self.gain_ref_db + HEADROOM_DB;
        self.frame.gain_top_db = top;
        let norm = |db: f32| {
            if live {
                ((db - (top - SPECTRUM_RANGE_DB)) / SPECTRUM_RANGE_DB).clamp(0.0, 1.0)
            } else {
                0.0
            }
        };

        for b in 0..self.frame.mono.len() {
            let l = norm(self.band_db[0][b]);
            let r = norm(self.band_db[1][b]);
            self.frame.left.bands[b].update(l, BAND_FALL_PER_SECOND, dt);
            self.frame.right.bands[b].update(r, BAND_FALL_PER_SECOND, dt);
            self.frame.mono[b].update((l + r) / 2.0, BAND_FALL_PER_SECOND, dt);
        }
    }

    /// FFT one channel and fill `band_db[ch]`.
    fn spectrum_db(&mut self, ch: usize) {
        self.small.compute(&self.samples[ch]);
        self.large.compute(&self.samples[ch]);

        let small_bin_hz = self.sample_rate / FFT_SMALL as f32;
        for (b, out) in self.band_db[ch].iter_mut().enumerate() {
            let (lo, hi) = (self.edges[b], self.edges[b + 1]);
            let spectrum = if (hi - lo) / small_bin_hz < BASS_MIN_BINS { &self.large } else { &self.small };
            let mag = spectrum.band_mag(lo, hi, self.sample_rate);
            let center = (lo * hi).sqrt();
            let tilt = TILT_DB_PER_OCTAVE * (center / TILT_PIVOT_HZ).log2().max(0.0);
            *out = 20.0 * mag.max(1e-9).log10() + tilt;
        }
    }
}
