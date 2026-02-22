# audio-spectrum-generator

A CLI tool that generates MP4 videos with a rounded audio spectrum at the bottom of the frame (16:9 recommended). Input is an MP3 file; the output includes the original audio.

## Requirements

- **Rust** (for building)
- **ffmpeg** (required at runtime; must be on your PATH)

## Build

```bash
cargo build --release
```

## Usage

```bash
# Minimal (default 1920x1080)
cargo run --release -- input.mp3 -o output.mp4

# Specify resolution
cargo run --release -- input.mp3 -o output.mp4 --width 1920 --height 1080
cargo run --release -- input.mp3 -o output.mp4 --resolution 1280x720

# Other options
cargo run --release -- input.mp3 -o output.mp4 --fps 30 --bars 128 --spectrum-height 200
```

### Options

| Option | Description | Default |
|--------|-------------|---------|
| `-o`, `--output` | Output MP4 path | (required) |
| `--resolution` | Resolution (e.g. `1280x720`) | - |
| `--width` | Video width (pixels) | 1920 |
| `--height` | Video height (pixels) | 1080 |
| `--fps` | Frame rate | 30 |
| `--bars` | Number of spectrum bars | 128 |
| `--spectrum-height` | Spectrum area height (pixels) | 200 |

16:9 aspect ratio is recommended (e.g. 1920x1080, 1280x720).

## License

See the license of each dependency. symphonia is MPL-2.0; rustfft, image, hound, and clap are MIT or Apache-2.0. ffmpeg is LGPL etc.; check license notices when distributing.
