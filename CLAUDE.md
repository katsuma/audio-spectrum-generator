# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build release binary
cargo build --release

# Run with input file
cargo run --release -- input.mp3 -o output.mp4

# Run with options
cargo run --release -- input.mp3 -o output.mp4 --width 1280 --height 720 --fps 30 --bars 128 --spectrum-height 200

# Check code
cargo check

# Run clippy lints
cargo clippy
```

**Runtime requirement:** `ffmpeg` must be available in PATH.

## Architecture

The pipeline is a linear data flow across five modules:

```
MP3 → decode.rs → spectrum.rs → draw.rs ──┐
                                           ├──→ ffmpeg (subprocess) → MP4
                         wav.rs ───────────┘
```

### Modules

- **`config.rs`** — `Config` struct passed to all stages. Contains `fft_size` (2048) and `overlap` (0.5) as compile-time constants alongside CLI-configurable fields.
- **`decode.rs`** — Uses `symphonia` to decode MP3 to mono f32 PCM samples. Stereo is downmixed by averaging channels.
- **`spectrum.rs`** — FFT pipeline: Hann window → `rustfft` → logarithmic bin aggregation → `log(1+x)` amplitude scaling. Computes all frames upfront and returns a global max for normalization.
- **`draw.rs`** — Renders each frame as a PNG using the `image` crate. Bars are drawn as rounded rectangles centered vertically in the spectrum area.
- **`wav.rs`** — Writes the decoded samples to a temporary WAV (mono, 16-bit PCM) for ffmpeg to use as audio input.
- **`main.rs`** — Orchestration: parses CLI args with `clap` derive macros, manages temp dirs, shows `indicatif` progress bars, spawns ffmpeg subprocess, parses its stderr for progress, then cleans up temp files.

### ffmpeg integration

ffmpeg is invoked as a subprocess from `main.rs`. It receives PNG frames via `-pattern_type glob` and the temp WAV for audio. Output is H.264 video with AAC audio. Progress is tracked by parsing `time=` tokens from ffmpeg's stderr.

### Key design choices

- Spectrum is computed for all frames before rendering begins, using a single global max for consistent normalization across the video.
- Frequency-to-bar mapping uses a logarithmic scale for perceptually even distribution.
- Bar corners are rounded via pixel-level `point_in_rounded_rect()` checks in `draw.rs`.
