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
