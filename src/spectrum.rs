//! FFT and bin computation (rustfft)

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

/// Per-frame spectrum amplitude (one f32 per bar).
/// Frequency uses a log scale; amplitude uses log(1+x) to expand dynamic range.
pub fn compute_spectrum_frame(
    samples: &[f32],
    sample_rate: u32,
    frame_index: u32,
    _fps: u32,
    fft_size: usize,
    overlap: f32,
    bars: usize,
) -> Vec<f32> {
    let hop = (fft_size as f32 * (1.0 - overlap)).max(1.0) as usize;
    let start = (frame_index as usize).saturating_mul(hop);
    if start + fft_size > samples.len() {
        return vec![0.0; bars];
    }

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_size);

    let mut buffer: Vec<Complex<f32>> = samples[start..start + fft_size]
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let w = hann_window(i, fft_size);
            Complex::new(s * w, 0.0)
        })
        .collect();

    fft.process(&mut buffer);

    let half = fft_size / 2 + 1;
    let magnitudes: Vec<f32> = buffer[..half]
        .iter()
        .map(|c| c.norm())
        .collect();

    // Aggregate bins to bars with log frequency scale; log(1+x) for amplitude makes the display more dynamic
    let raw = aggregate_bins_to_bars_log(sample_rate, fft_size, &magnitudes, bars);
    raw.into_iter()
        .map(|x| (1.0 + x).ln())
        .collect()
}

fn hann_window(i: usize, n: usize) -> f32 {
    let x = std::f32::consts::PI * (i as f32 + 1.0) / (n as f32 + 1.0);
    0.5 * (1.0 - x.cos())
}

/// Aggregate FFT bins to bars using a logarithmic frequency scale.
/// Gives a more perceptually even spread from low to high frequencies so the whole spectrum moves dynamically.
fn aggregate_bins_to_bars_log(
    sample_rate: u32,
    fft_size: usize,
    magnitudes: &[f32],
    bars: usize,
) -> Vec<f32> {
    if magnitudes.is_empty() || bars == 0 {
        return vec![0.0; bars];
    }
    let sr = sample_rate as f32;
    let f_min = sr / fft_size as f32;
    let f_max = sr * 0.5;
    let log_f_min = (f_min + 1.0).ln();
    let log_f_max = (f_max + 1.0).ln();
    let log_span = log_f_max - log_f_min;

    let mut result = vec![0.0f32; bars];
    for (bin_ix, &mag) in magnitudes.iter().enumerate().skip(1) {
        let f = bin_ix as f32 * sr / fft_size as f32;
        let log_f = (f + 1.0).ln();
        let t = ((log_f - log_f_min) / log_span).clamp(0.0, 1.0);
        let bar_ix = (t * bars as f32).min(bars as f32 - 1.0) as usize;
        if bar_ix < bars && mag > result[bar_ix] {
            result[bar_ix] = mag;
        }
    }
    result
}

/// Compute spectrum for all frames and return the global max for normalization.
/// Returns (frame_spectrums, global_max). Each frame has `bars` f32 values; normalization is done by the caller.
pub fn compute_all_spectrums(
    samples: &[f32],
    sample_rate: u32,
    fps: u32,
    fft_size: usize,
    overlap: f32,
    bars: usize,
) -> (Vec<Vec<f32>>, f32) {
    let hop = (fft_size as f32 * (1.0 - overlap)).max(1.0) as usize;
    let num_frames = samples.len().saturating_sub(fft_size).saturating_add(hop) / hop;
    let mut frame_spectrums = Vec::with_capacity(num_frames);
    let mut global_max = 0.0f32;

    for frame_index in 0..num_frames {
        let bar_values = compute_spectrum_frame(
            samples,
            sample_rate,
            frame_index as u32,
            fps,
            fft_size,
            overlap,
            bars,
        );
        let m = bar_values.iter().copied().fold(0.0f32, f32::max);
        if m > global_max {
            global_max = m;
        }
        frame_spectrums.push(bar_values);
    }

    (frame_spectrums, global_max)
}

#[cfg(test)]
mod tests {
    use super::{
        aggregate_bins_to_bars_log, compute_all_spectrums, compute_spectrum_frame, hann_window,
    };

    #[test]
    fn hann_window_range() {
        let n = 16;
        for i in 0..n {
            let w = hann_window(i, n);
            assert!(w >= 0.0 && w <= 1.0, "hann_window({}, {}) = {} out of [0,1]", i, n, w);
        }
    }

    #[test]
    fn hann_window_ends_non_zero() {
        let n = 8;
        let first = hann_window(0, n);
        let last = hann_window(n - 1, n);
        assert!(first > 0.0 && last > 0.0);
    }

    #[test]
    fn aggregate_bins_to_bars_log_empty_magnitudes() {
        let out = aggregate_bins_to_bars_log(44100, 2048, &[], 128);
        assert_eq!(out.len(), 128);
        assert!(out.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn aggregate_bins_to_bars_log_zero_bars() {
        let out = aggregate_bins_to_bars_log(44100, 2048, &[1.0, 2.0, 3.0], 0);
        assert!(out.is_empty());
    }

    #[test]
    fn aggregate_bins_to_bars_log_returns_bars_count() {
        let mut mags = vec![0.0f32; 1025]; // half of 2048 + 1
        mags[10] = 1.0;
        let out = aggregate_bins_to_bars_log(44100, 2048, &mags, 32);
        assert_eq!(out.len(), 32);
    }

    #[test]
    fn compute_spectrum_frame_insufficient_samples_returns_zeros() {
        let samples = vec![0.1f32; 100];
        let out = compute_spectrum_frame(&samples, 44100, 0, 30, 2048, 0.5, 64);
        assert_eq!(out.len(), 64);
        assert!(out.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn compute_spectrum_frame_enough_samples_returns_bars_len() {
        let samples: Vec<f32> = (0..4096).map(|i| 0.001 * (i as f32).sin()).collect();
        let out = compute_spectrum_frame(&samples, 44100, 0, 30, 2048, 0.5, 32);
        assert_eq!(out.len(), 32);
    }

    #[test]
    fn compute_all_spectrums_frame_count_and_global_max() {
        let samples: Vec<f32> = (0..8192).map(|i| 0.01 * (i as f32 * 0.1).sin()).collect();
        let (frames, global_max) =
            compute_all_spectrums(&samples, 44100, 30, 2048, 0.5, 16);
        let hop = (2048 as f32 * 0.5) as usize;
        let expected_frames = (8192usize.saturating_sub(2048).saturating_add(hop)) / hop;
        assert_eq!(frames.len(), expected_frames);
        for f in &frames {
            assert_eq!(f.len(), 16);
        }
        assert!(global_max >= 0.0);
        if global_max > 0.0 {
            assert!(global_max.is_finite());
        }
    }
}
