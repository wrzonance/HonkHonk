use super::ImportError;
use crate::audio::DecodedAudio;
use anyhow::Context;
use std::io::Write;
use std::ops::Range;
use std::path::Path;

pub fn decode(path: &Path) -> Result<DecodedAudio, ImportError> {
    let metadata =
        std::fs::metadata(path).with_context(|| format!("reading {}", path.display()))?;
    if !metadata.is_file() || metadata.len() > 64 * 1024 * 1024 {
        return Err(
            anyhow::anyhow!("{} exceeds the 64 MiB import file limit", path.display()).into(),
        );
    }
    let audio = crate::audio::decode_limited(path, 12_000_000)
        .with_context(|| format!("decoding {}", path.display()))?;
    if audio.samples.is_empty() || !audio.samples.iter().all(|s| s.is_finite()) {
        return Err(
            anyhow::anyhow!("{} contains empty or non-finite audio", path.display()).into(),
        );
    }
    Ok(audio)
}

pub fn audible_range(audio: &DecodedAudio) -> Range<usize> {
    let channels = usize::from(audio.channels);
    let active = |frame: &[f32]| frame.iter().any(|sample| sample.abs() > 0.001);
    let start = audio.samples.chunks_exact(channels).position(active);
    let end = audio.samples.chunks_exact(channels).rposition(active);
    match (start, end) {
        (Some(start), Some(end)) => start * channels..(end + 1) * channels,
        _ => 0..audio.samples.len(),
    }
}

pub fn prepare(mut audio: DecodedAudio, normalize: bool, trim: bool) -> DecodedAudio {
    if trim {
        let range = audible_range(&audio);
        audio.samples = audio.samples[range].to_vec();
    }
    if normalize {
        let peak = audio.samples.iter().fold(0.0_f32, |p, s| p.max(s.abs()));
        if peak > 0.0 {
            for sample in &mut audio.samples {
                *sample = *sample / peak * 0.9;
            }
        }
    }
    audio.duration = std::time::Duration::from_secs_f64(
        audio.samples.len() as f64 / f64::from(audio.channels) / f64::from(audio.sample_rate),
    );
    audio
}

pub fn write_wav(audio: &DecodedAudio, output: &mut impl Write) -> std::io::Result<()> {
    let bytes = (audio.samples.len() * 4) as u32;
    output.write_all(b"RIFF")?;
    output.write_all(&(36 + bytes).to_le_bytes())?;
    output.write_all(b"WAVEfmt ")?;
    output.write_all(&16_u32.to_le_bytes())?;
    output.write_all(&3_u16.to_le_bytes())?;
    output.write_all(&audio.channels.to_le_bytes())?;
    output.write_all(&audio.sample_rate.to_le_bytes())?;
    output.write_all(&(audio.sample_rate * u32::from(audio.channels) * 4).to_le_bytes())?;
    output.write_all(&(audio.channels * 4).to_le_bytes())?;
    output.write_all(&32_u16.to_le_bytes())?;
    output.write_all(b"data")?;
    output.write_all(&bytes.to_le_bytes())?;
    for sample in &audio.samples {
        output.write_all(&sample.to_le_bytes())?;
    }
    Ok(())
}
