//! Configuration for resolution, fps, bar count, spectrum height, etc.

/// Application configuration.
#[derive(Clone, Debug)]
pub struct Config {
    /// Output video width (pixels).
    pub width: u32,
    /// Output video height (pixels).
    pub height: u32,
    /// Frame rate (fps).
    pub fps: u32,
    /// Number of spectrum bars.
    pub bars: usize,
    /// Spectrum area height (pixels).
    pub spectrum_height: u32,
    /// FFT window size (number of samples).
    pub fft_size: usize,
    /// Overlap ratio (0.0–1.0, e.g. 0.5 = 50%).
    pub overlap: f32,
    /// Bar color as RGBA (default: black).
    pub bar_color: [u8; 4],
    /// Background color as RGBA (default: white).
    pub bg_color: [u8; 4],
}

impl Default for Config {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 30,
            bars: 128,
            spectrum_height: 200,
            fft_size: 2048,
            overlap: 0.5,
            bar_color: [0, 0, 0, 255],
            bg_color: [255, 255, 255, 255],
        }
    }
}
