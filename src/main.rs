mod config;
mod decode;
mod draw;
mod spectrum;
mod wav;

use std::io::Read;
use std::path::PathBuf;
use std::process::Stdio;

use clap::Parser;
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
        ..Config::default()
    };

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

    let bar_color = [0u8, 0, 0, 255];
    let bg_color = [255u8, 255, 255, 255];
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
            &bar_heights,
            bar_color,
            bg_color,
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
