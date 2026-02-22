//! MP3 → PCM decoding (symphonia)

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::default::get_codecs;
use symphonia::default::get_probe;

/// Decoded audio (mono PCM and sample rate).
pub struct DecodedAudio {
    /// Mono PCM samples (f32, -1.0 to 1.0).
    pub samples: Vec<f32>,
    /// Sample rate (Hz).
    pub sample_rate: u32,
}

/// Decode an MP3 file and return mono PCM.
/// For stereo, left and right are averaged to mono.
pub fn decode_mp3(path: &std::path::Path) -> Result<DecodedAudio, Box<dyn std::error::Error + Send + Sync>> {
    let src = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    let hint = symphonia::core::probe::Hint::new();
    let format_opts = FormatOptions::default();
    let metadata_opts = MetadataOptions::default();
    let probe = get_probe();

    let mut probe_result = probe
        .format(&hint, mss, &format_opts, &metadata_opts)
        .map_err(|e| format!("format probe error: {}", e))?;

    let track = probe_result
        .format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("no audio track found")?;

    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let mut decoder = get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|e| format!("decoder creation error: {}", e))?;

    let mut all_samples: Vec<f32> = Vec::new();
    let sample_rate = codec_params
        .sample_rate
        .ok_or("missing sample rate")? as u32;
    let channels = codec_params.channels.ok_or("missing channel count")?.count() as usize;

    loop {
        let packet = match probe_result.format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(e.into()),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let spec = *decoded.spec();
        let duration = decoded.frames();
        let mut sample_buffer = SampleBuffer::<f32>::new(
            symphonia::core::units::Duration::from(duration as u64),
            spec,
        );
        sample_buffer.copy_interleaved_ref(decoded);

        let slice = sample_buffer.samples();
        if channels == 1 {
            all_samples.extend_from_slice(slice);
        } else {
            for ch in slice.chunks(channels) {
                let sum: f32 = ch.iter().sum();
                all_samples.push(sum / channels as f32);
            }
        }
    }

    Ok(DecodedAudio {
        samples: all_samples,
        sample_rate,
    })
}
