//! PCM → WAV output (hound)

/// Write mono f32 samples (-1.0 to 1.0) to a WAV file.
pub fn write_wav(
    path: &std::path::Path,
    samples: &[f32],
    sample_rate: u32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &s in samples {
        let sample_i16 = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        writer.write_sample(sample_i16)?;
    }
    writer.finalize()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::write_wav;

    #[test]
    fn write_wav_roundtrip_channels_rate_samples() {
        let samples = vec![0.0f32, 0.5, -0.5, 0.0];
        let sample_rate = 44100u32;
        let dir = std::env::temp_dir().join("audio-spectrum-generator-test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("roundtrip.wav");

        write_wav(&path, &samples, sample_rate).unwrap();

        let reader = hound::WavReader::open(&path).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, sample_rate);
        let read_samples: Vec<i16> = reader.into_samples().filter_map(Result::ok).collect();
        assert_eq!(read_samples.len(), samples.len());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn write_wav_clamps_to_valid_range() {
        let samples = vec![1.5f32, -1.5]; // clamped to 1.0 and -1.0
        let sample_rate = 8000u32;
        let dir = std::env::temp_dir().join("audio-spectrum-generator-test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("clamp.wav");

        write_wav(&path, &samples, sample_rate).unwrap();

        let reader = hound::WavReader::open(&path).unwrap();
        let read_samples: Vec<i16> = reader.into_samples().filter_map(Result::ok).collect();
        assert_eq!(read_samples.len(), 2);
        assert_eq!(read_samples[0], 32767);
        // -1.0 * 32767.0 = -32767.0, truncates to i16::MIN+1 = -32767
        assert_eq!(read_samples[1], -32767);

        std::fs::remove_file(&path).ok();
    }
}
