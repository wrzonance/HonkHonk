use std::path::Path;
use std::time::Duration;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use super::error::AudioError;

pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration: Duration,
}

pub fn decode(path: &Path) -> Result<DecodedAudio, AudioError> {
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

    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or(AudioError::MissingCodecParams)?;

    let channels = track
        .codec_params
        .channels
        .map(|ch| ch.count() as u16)
        .ok_or(AudioError::MissingCodecParams)?;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(AudioError::DecoderInit)?;

    let all_samples = decode_packets(&mut format, &mut decoder, track_id)?;

    let total_frames = all_samples.len() as u64 / channels as u64;
    let duration = Duration::from_secs_f64(total_frames as f64 / sample_rate as f64);

    Ok(DecodedAudio {
        samples: all_samples,
        sample_rate,
        channels,
        duration,
    })
}

fn decode_packets(
    format: &mut Box<dyn symphonia::core::formats::FormatReader>,
    decoder: &mut Box<dyn symphonia::core::codecs::Decoder>,
    track_id: u32,
) -> Result<Vec<f32>, AudioError> {
    let mut all_samples: Vec<f32> = Vec::new();
    let mut sample_buf: Option<SampleBuffer<f32>> = None;

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
        let capacity = decoded.capacity();

        if sample_buf
            .as_ref()
            .map_or(true, |b| capacity > b.capacity())
        {
            sample_buf = Some(SampleBuffer::<f32>::new(capacity as u64, spec));
        }
        let buf = sample_buf.as_mut().expect("buffer just initialized");

        buf.copy_interleaved_ref(decoded);
        all_samples.extend_from_slice(buf.samples());
    }

    Ok(all_samples)
}
