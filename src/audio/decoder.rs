use std::path::Path;
use std::time::Duration;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use super::{channel_repair::repair_dead_stereo_channel, error::AudioError};

pub struct DecodedAudio {
    pub repaired_channel: bool,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration: Duration,
}

pub fn decode(path: &Path) -> Result<DecodedAudio, AudioError> {
    decode_limited(path, usize::MAX)
}

pub fn decode_limited(path: &Path, max_samples: usize) -> Result<DecodedAudio, AudioError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        decode_limited_inner(path, max_samples)
    }))
    .unwrap_or(Err(AudioError::DecoderPanic))
}

fn decode_limited_inner(path: &Path, max_samples: usize) -> Result<DecodedAudio, AudioError> {
    let file = std::fs::File::open(path).map_err(AudioError::FileOpen)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(AudioError::UnsupportedFormat)?;

    let mut format = probed.format;

    let track = format.default_track().ok_or(AudioError::NoTrack)?;
    let track_id = track.id;

    // Seed rate / channels from the track header where present. Some containers
    // (notably AAC-in-MP4 / `.m4a`) omit `channels` — and rarely the rate — from
    // the header; those values live in the codec's AudioSpecificConfig and are
    // only resolved once the first frame is decoded. We fall back to the decoded
    // frame's spec below so such files are not rejected with
    // `MissingCodecParams` (#153).
    let header_rate = track.codec_params.sample_rate;
    let header_channels = track.codec_params.channels.map(|ch| ch.count() as u16);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(AudioError::DecoderInit)?;

    let decoded = decode_packets(&mut format, &mut decoder, track_id, max_samples)?;

    // Header value wins when valid; otherwise use the first decoded frame.
    let (sample_rate, channels) = metadata_from(header_rate, header_channels, &decoded)
        .ok_or(AudioError::MissingCodecParams)?;

    let mut samples = decoded.samples;
    let repaired_channel = repair_dead_stereo_channel(&mut samples, sample_rate, channels);

    let total_frames = samples.len() as u64 / channels as u64;
    let duration = Duration::from_secs_f64(total_frames as f64 / sample_rate as f64);

    Ok(DecodedAudio {
        repaired_channel,
        samples,
        sample_rate,
        channels,
        duration,
    })
}

fn metadata_from(
    header_rate: Option<u32>,
    header_channels: Option<u16>,
    decoded: &DecodedFrames,
) -> Option<(u32, u16)> {
    let sample_rate = header_rate
        .filter(|rate| *rate > 0)
        .or(decoded.sample_rate.filter(|rate| *rate > 0))?;
    let channels = header_channels
        .filter(|channels| *channels > 0)
        .or(decoded.channels.filter(|channels| *channels > 0))?;
    Some((sample_rate, channels))
}

fn record_frame_metadata(
    rate: u32,
    channel_count: usize,
    frames: usize,
    sample_rate: &mut Option<u32>,
    channels: &mut Option<u16>,
) -> Result<bool, AudioError> {
    if frames == 0 {
        return Ok(false);
    }
    if rate == 0 || channel_count == 0 {
        return Err(AudioError::MissingCodecParams);
    }
    sample_rate.get_or_insert(rate);
    channels.get_or_insert(channel_count as u16);
    Ok(true)
}

/// Decoded PCM plus the rate / channel count observed in the first decoded
/// frame's `SignalSpec`, used to backfill metadata the container header omitted
/// (#153). `sample_rate` / `channels` are `None` only when no frame decoded.
struct DecodedFrames {
    samples: Vec<f32>,
    sample_rate: Option<u32>,
    channels: Option<u16>,
}

fn decode_packets(
    format: &mut Box<dyn symphonia::core::formats::FormatReader>,
    decoder: &mut Box<dyn symphonia::core::codecs::Decoder>,
    track_id: u32,
    max_samples: usize,
) -> Result<DecodedFrames, AudioError> {
    let mut all_samples: Vec<f32> = Vec::new();
    let mut sample_buf: Option<SampleBuffer<f32>> = None;
    let mut sample_rate: Option<u32> = None;
    let mut channels: Option<u16> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(AudioError::Decode(e)),
        };

        if packet.track_id() != track_id {
            continue;
        }
        let decoded = decoder.decode(&packet).map_err(AudioError::Decode)?;
        let spec = *decoded.spec();
        let observed = record_frame_metadata(
            spec.rate,
            spec.channels.count(),
            decoded.frames(),
            &mut sample_rate,
            &mut channels,
        )?;
        if observed {
            append_decoded(
                decoded,
                spec,
                max_samples,
                &mut all_samples,
                &mut sample_buf,
            )?;
        }
    }

    Ok(DecodedFrames {
        samples: all_samples,
        sample_rate,
        channels,
    })
}

fn append_decoded(
    decoded: symphonia::core::audio::AudioBufferRef<'_>,
    spec: symphonia::core::audio::SignalSpec,
    max_samples: usize,
    all_samples: &mut Vec<f32>,
    sample_buf: &mut Option<SampleBuffer<f32>>,
) -> Result<(), AudioError> {
    let frames = decoded.frames();
    if frames == 0 {
        return Ok(());
    }
    let sample_count = frames
        .checked_mul(spec.channels.count())
        .ok_or(AudioError::SampleLimit)?;
    if sample_count > max_samples.saturating_sub(all_samples.len()) {
        return Err(AudioError::SampleLimit);
    }
    if sample_buf
        .as_ref()
        .is_none_or(|buffer| sample_count > buffer.capacity())
    {
        *sample_buf = Some(SampleBuffer::<f32>::new(frames as u64, spec));
    }
    let Some(buffer) = sample_buf.as_mut() else {
        return Err(AudioError::MissingCodecParams);
    };
    buffer.copy_interleaved_ref(decoded);
    all_samples.extend_from_slice(buffer.samples());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{DecodedFrames, metadata_from, record_frame_metadata};

    #[test]
    fn invalid_header_metadata_is_replaced_by_frame_metadata() {
        let decoded = DecodedFrames {
            samples: vec![0.0, 0.0],
            sample_rate: Some(48_000),
            channels: Some(2),
        };

        assert_eq!(metadata_from(Some(0), Some(0), &decoded), Some((48_000, 2)));
    }

    #[test]
    fn empty_frame_does_not_poison_metadata_for_following_audio() {
        let mut sample_rate = None;
        let mut channels = None;

        assert!(!record_frame_metadata(0, 0, 0, &mut sample_rate, &mut channels).unwrap());
        assert!(record_frame_metadata(48_000, 2, 1, &mut sample_rate, &mut channels).unwrap());
        assert_eq!(sample_rate, Some(48_000));
        assert_eq!(channels, Some(2));
    }

    #[test]
    fn nonempty_frame_without_metadata_is_rejected() {
        let mut sample_rate = None;
        let mut channels = None;

        assert!(record_frame_metadata(0, 2, 1, &mut sample_rate, &mut channels).is_err());
        assert!(record_frame_metadata(48_000, 0, 1, &mut sample_rate, &mut channels).is_err());
    }
}
