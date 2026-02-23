mod config;
mod decode;
mod draw;
mod spectrum;
mod wav;

use std::io::Read;
use std::path::PathBuf;
use std::process::Stdio;

use clap::Parser;
use image::imageops::FilterType;
use indicatif::{ProgressBar, ProgressStyle};
use config::Config;
use decode::decode_mp3;
use draw::draw_spectrum_frame;
use spectrum::compute_all_spectrums;
use wav::write_wav;

#[derive(Parser, Debug)]
#[command(name = "audio-spectrum-generator")]
#[command(about = "Generate an audio spectrum video (MP4) from an MP3 file")]
struct Args {
    /// Input MP3 file
    input: PathBuf,

    /// Output MP4 file
    #[arg(short, long)]
    output: PathBuf,

    /// Resolution (e.g. 1920x1080). Overrides --width / --height when set
    #[arg(long, value_parser = parse_resolution)]
    resolution: Option<(u32, u32)>,

    /// Video width (pixels)
    #[arg(long, default_value_t = 1920)]
    width: u32,

    /// Video height (pixels)
    #[arg(long, default_value_t = 1080)]
    height: u32,

    /// Frame rate (fps)
    #[arg(long, default_value_t = 30)]
    fps: u32,

    /// Number of spectrum bars
    #[arg(long, default_value_t = 128)]
    bars: usize,

    /// Spectrum area height (pixels)
    #[arg(long, default_value_t = 200)]
    spectrum_height: u32,

    /// Bar color in hex RGB (e.g. 000000 or #ff6600). Default: black
    #[arg(long, default_value = "000000", value_parser = parse_hex_color)]
    bar_color: [u8; 4],

    /// Background color in hex RGB (e.g. ffffff or #1a1a2e). Default: white
    #[arg(long, default_value = "ffffff", value_parser = parse_hex_color)]
    bg_color: [u8; 4],

    /// Background image path (PNG/JPEG etc.). Resized to video size if needed. Overrides --bg-color when set
    #[arg(long)]
    bg_image: Option<PathBuf>,

    /// Distance from bottom of frame to the bottom edge of the spectrum band (pixels)
    #[arg(long, default_value_t = 0)]
    spectrum_y_from_bottom: u32,

    /// Horizontal width of the spectrum band (pixels). Centered. When not set, uses full frame width
    #[arg(long)]
    spectrum_width: Option<u32>,
}

fn parse_hex_color(s: &str) -> Result<[u8; 4], String> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 {
        return Err(format!("color must be 6 hex digits (e.g. ff6600), got {:?}", s));
    }
    let r = u8::from_str_radix(&s[0..2], 16).map_err(|_| format!("invalid hex in color: {:?}", s))?;
    let g = u8::from_str_radix(&s[2..4], 16).map_err(|_| format!("invalid hex in color: {:?}", s))?;
    let b = u8::from_str_radix(&s[4..6], 16).map_err(|_| format!("invalid hex in color: {:?}", s))?;
    Ok([r, g, b, 255])
}

fn parse_resolution(s: &str) -> Result<(u32, u32), String> {
    let parts: Vec<&str> = s.split('x').collect();
    if parts.len() != 2 {
        return Err("resolution must be WIDTHxHEIGHT (e.g. 1920x1080)".to_string());
    }
    let w: u32 = parts[0].trim().parse().map_err(|_| "invalid width")?;
    let h: u32 = parts[1].trim().parse().map_err(|_| "invalid height")?;
    if w == 0 || h == 0 {
        return Err("width and height must be positive".to_string());
    }
    Ok((w, h))
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();

    if std::process::Command::new("ffmpeg").arg("-version").output().is_err() {
        return Err("ffmpeg not found. Please install ffmpeg and add it to your PATH.".into());
    }

    let (width, height) = args.resolution.unwrap_or((args.width, args.height));
    let config = Config {
        width,
        height,
        fps: args.fps,
        bars: args.bars,
        spectrum_height: args.spectrum_height,
        spectrum_y_from_bottom: args.spectrum_y_from_bottom,
        spectrum_width: args.spectrum_width,
        bar_color: args.bar_color,
        bg_color: args.bg_color,
        ..Config::default()
    };

    let bg_image: Option<image::RgbaImage> = if let Some(ref path) = args.bg_image {
        let img = image::ImageReader::open(path)
            .map_err(|e| format!("failed to open background image {:?}: {}", path, e))?
            .decode()
            .map_err(|e| format!("failed to decode background image {:?}: {}", path, e))?;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        if w == width && h == height {
            Some(rgba)
        } else {
            Some(image::imageops::resize(&rgba, width, height, FilterType::Triangle))
        }
    } else {
        None
    };
    if args.bg_image.is_some() {
        println!("Using background image: {:?}", args.bg_image.as_ref().unwrap());
    }

    println!("Decoding MP3: {:?}", args.input);
    let decoded = decode_mp3(&args.input)?;
    println!(
        "Decoded {} samples at {} Hz",
        decoded.samples.len(),
        decoded.sample_rate
    );

    println!("Computing spectrum...");
    let (frame_spectrums, global_max) = compute_all_spectrums(
        &decoded.samples,
        decoded.sample_rate,
        config.fps,
        config.fft_size,
        config.overlap,
        config.bars,
    );
    let num_spectrum_frames = frame_spectrums.len();
    let duration_sec = decoded.samples.len() as f32 / decoded.sample_rate as f32;
    let total_frames = (duration_sec * config.fps as f32).ceil().max(1.0) as usize;
    println!(
        "Spectrum frames: {}, total video frames: {}",
        num_spectrum_frames, total_frames
    );

    let temp_dir = std::env::temp_dir().join("audio-spectrum-generator");
    std::fs::create_dir_all(&temp_dir)?;
    let frames_dir = temp_dir.join("frames");
    std::fs::create_dir_all(&frames_dir)?;
    let wav_path = temp_dir.join("audio.wav");

    let cleanup = || {
        let _ = std::fs::remove_dir_all(&frames_dir);
        let _ = std::fs::remove_file(&wav_path);
    };

    println!("Writing WAV: {:?}", wav_path);
    write_wav(&wav_path, &decoded.samples, decoded.sample_rate)?;

    let norm = if global_max > 0.0 { global_max } else { 1.0 };

    let default_heights = vec![0.0; config.bars];
    let pb_render = ProgressBar::new(total_frames as u64);
    pb_render.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} frames")
            .unwrap()
            .progress_chars("=>-"),
    );
    pb_render.set_message("Rendering frames");
    for frame_index in 0..total_frames {
        let spectrum_index = if num_spectrum_frames == 0 {
            0
        } else {
            (frame_index * num_spectrum_frames / total_frames.max(1)).min(num_spectrum_frames - 1)
        };
        let bar_heights: Vec<f32> = frame_spectrums
            .get(spectrum_index)
            .unwrap_or(&default_heights)
            .iter()
            .map(|&v| (v / norm).min(1.0))
            .collect();
        let img = draw_spectrum_frame(
            config.width,
            config.height,
            config.spectrum_height,
            config.spectrum_y_from_bottom,
            config.spectrum_width,
            &bar_heights,
            config.bar_color,
            config.bg_color,
            bg_image.as_ref(),
        );
        let path = frames_dir.join(format!("frame_{:06}.png", frame_index));
        img.save(&path)?;
        pb_render.inc(1);
    }
    pb_render.finish_with_message("Rendering done");

    let pb_ffmpeg = ProgressBar::new(total_frames as u64);
    pb_ffmpeg.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.green/black} {pos}/{len} encoding")
            .unwrap()
            .progress_chars("=>-"),
    );
    pb_ffmpeg.set_message("Encoding MP4 with ffmpeg");

    let mut child = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-framerate",
            &config.fps.to_string(),
            "-i",
            &format!("{}/frame_%06d.png", frames_dir.display()),
            "-i",
            wav_path.to_str().unwrap(),
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-shortest",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(args.output.as_os_str())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stderr = child.stderr.take().ok_or("failed to take ffmpeg stderr")?;
    let total = total_frames as u64;
    let pb = pb_ffmpeg.clone();
    let reader_handle = std::thread::spawn(move || {
        let mut buf = [0u8; 512];
        let mut tail = Vec::<u8>::new();
        let mut last_pos = 0u64;
        while let Ok(n) = stderr.read(&mut buf) {
            if n == 0 {
                break;
            }
            tail.extend_from_slice(&buf[..n]);
            if tail.len() > 4096 {
                tail.drain(..tail.len() - 1024);
            }
            let s = String::from_utf8_lossy(&tail);
            for (i, _) in s.match_indices("frame=") {
                let rest = &s[i + 6..];
                let num_str: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == ' ')
                    .filter(|c| *c != ' ')
                    .collect();
                if !num_str.is_empty() {
                    if let Ok(n) = num_str.parse::<u64>() {
                        let pos = n.min(total);
                        if pos > last_pos {
                            last_pos = pos;
                            pb.set_position(pos);
                        }
                    }
                }
            }
        }
    });

    let status = child.wait()?;
    reader_handle.join().ok();
    pb_ffmpeg.finish_with_message("Encoding done");

    cleanup();

    if !status.success() {
        return Err("ffmpeg failed (run without progress to see stderr)".into());
    }

    println!("Done: {:?}", args.output);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_hex_color, parse_resolution};

    #[test]
    fn parse_hex_color_with_hash() {
        let got = parse_hex_color("#ff6600").unwrap();
        assert_eq!(got, [255, 102, 0, 255]);
    }

    #[test]
    fn parse_hex_color_without_hash() {
        let got = parse_hex_color("000000").unwrap();
        assert_eq!(got, [0, 0, 0, 255]);
    }

    #[test]
    fn parse_hex_color_white() {
        let got = parse_hex_color("ffffff").unwrap();
        assert_eq!(got, [255, 255, 255, 255]);
    }

    #[test]
    fn parse_hex_color_too_short() {
        let err = parse_hex_color("ff00").unwrap_err();
        assert!(err.contains("6 hex digits"));
    }

    #[test]
    fn parse_hex_color_too_long() {
        let err = parse_hex_color("1234567").unwrap_err();
        assert!(err.contains("6 hex digits"));
    }

    #[test]
    fn parse_hex_color_invalid_char() {
        let err = parse_hex_color("ff00gg").unwrap_err();
        assert!(err.contains("invalid hex"));
    }

    #[test]
    fn parse_resolution_ok() {
        let got = parse_resolution("1920x1080").unwrap();
        assert_eq!(got, (1920, 1080));
    }

    #[test]
    fn parse_resolution_with_spaces() {
        let got = parse_resolution(" 640 x 480 ").unwrap();
        assert_eq!(got, (640, 480));
    }

    #[test]
    fn parse_resolution_zero_width() {
        let err = parse_resolution("0x1080").unwrap_err();
        assert!(err.contains("positive"));
    }

    #[test]
    fn parse_resolution_zero_height() {
        let err = parse_resolution("1920x0").unwrap_err();
        assert!(err.contains("positive"));
    }

    #[test]
    fn parse_resolution_invalid_format() {
        let err = parse_resolution("1920").unwrap_err();
        assert!(err.contains("WIDTHxHEIGHT"));
    }

    #[test]
    fn parse_resolution_invalid_number() {
        let err = parse_resolution("axb").unwrap_err();
        assert!(err.contains("invalid"));
    }
}
