mod state;
pub use state::PlaybackState;

use std::cell::RefCell;
use std::rc::Rc;

use pipewire as pw;
use pw::spa;
use pw::spa::pod::Pod;

use super::error::AudioError;
use super::voices::{MixScratch, MixTarget, VoicePool};

const FRAME_SIZE: usize = std::mem::size_of::<f32>();

/// Holds a PipeWire stream and its listener together.
///
/// Both must be kept alive for the stream callbacks to fire. Dropping this
/// struct disconnects the stream and unregisters the listener.
pub struct PlaybackStream {
    _stream: pw::stream::StreamRc,
    _listener: pw::stream::StreamListener<()>,
}

// Raw SPA channel position constants (from spa/param/audio/raw.h).
// FL = Front Left (3), FR = Front Right (4).
const SPA_AUDIO_CHANNEL_FL: u32 = 3;
const SPA_AUDIO_CHANNEL_FR: u32 = 4;

fn build_audio_params(rate: u32, channels: u32) -> Vec<u8> {
    let mut audio_info = spa::param::audio::AudioInfoRaw::new();
    audio_info.set_format(spa::param::audio::AudioFormat::F32LE);
    audio_info.set_rate(rate);
    audio_info.set_channels(channels);

    let mut position = [0u32; spa::param::audio::MAX_CHANNELS];
    if channels >= 1 {
        position[0] = SPA_AUDIO_CHANNEL_FL;
    }
    if channels >= 2 {
        position[1] = SPA_AUDIO_CHANNEL_FR;
    }
    audio_info.set_position(position);

    pw::spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &pw::spa::pod::Value::Object(pw::spa::pod::Object {
            type_: pw::spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
            id: pw::spa::param::ParamType::EnumFormat.as_raw(),
            properties: audio_info.into(),
        }),
    )
    .expect("pod serialization cannot fail for valid AudioInfoRaw")
    .0
    .into_inner()
}

fn make_mix_process_closure(
    voices: Rc<RefCell<VoicePool>>,
    target: MixTarget,
    sample_rate: u32,
    channels: u16,
) -> impl FnMut(&pw::stream::Stream, &mut ()) + 'static {
    let mut scratch = MixScratch::default();
    move |stream, _| {
        if let Some(mut buffer) = stream.dequeue_buffer() {
            let datas = buffer.datas_mut();
            if let Some(data) = datas.first_mut() {
                let total_bytes = if let Some(slice) = data.data() {
                    let float_slice = cast_bytes_to_f32_mut(slice);
                    voices
                        .borrow_mut()
                        .mix(target, float_slice, &mut scratch, sample_rate);
                    slice.len()
                } else {
                    0
                };

                if total_bytes > 0 {
                    let chunk = data.chunk_mut();
                    *chunk.offset_mut() = 0;
                    *chunk.stride_mut() = (FRAME_SIZE * channels as usize) as i32;
                    *chunk.size_mut() = total_bytes as u32;
                }
            }
        }
    }
}

pub fn create_sink_mix_stream(
    core: pw::core::CoreRc,
    voices: Rc<RefCell<VoicePool>>,
    target_name: &str,
    sample_rate: u32,
    channels: u16,
) -> Result<PlaybackStream, AudioError> {
    let stream = pw::stream::StreamRc::new(
        core,
        "honkhonk-to-sink",
        pw::properties::properties! {
            *pw::keys::MEDIA_TYPE => "Audio",
            *pw::keys::MEDIA_ROLE => "Music",
            *pw::keys::MEDIA_CATEGORY => "Playback",
            "target.object" => target_name,
            *pw::keys::NODE_DONT_RECONNECT => "true",
            *pw::keys::AUDIO_CHANNELS => channels.to_string(),
        },
    )
    .map_err(|e| AudioError::StreamCreation(format!("sink stream: {e}")))?;

    let listener = stream
        .add_local_listener_with_user_data(())
        .process(make_mix_process_closure(
            voices,
            MixTarget::Sink,
            sample_rate,
            channels,
        ))
        .register()
        .map_err(|e| AudioError::StreamCreation(format!("sink listener: {e}")))?;

    let params_bytes = build_audio_params(sample_rate, channels as u32);
    let pod = Pod::from_bytes(&params_bytes)
        .ok_or_else(|| AudioError::StreamCreation("invalid audio params pod".into()))?;
    let mut params = [pod];

    stream
        .connect(
            spa::utils::Direction::Output,
            None,
            pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
            &mut params,
        )
        .map_err(|e| AudioError::StreamCreation(format!("sink connect: {e}")))?;

    Ok(PlaybackStream {
        _stream: stream,
        _listener: listener,
    })
}

fn open_monitor_stream(
    core: pw::core::CoreRc,
    target: Option<&str>,
    channels: u16,
) -> Result<pw::stream::StreamRc, AudioError> {
    let result = if let Some(target_name) = target {
        pw::stream::StreamRc::new(
            core,
            "honkhonk-monitor",
            pw::properties::properties! {
                *pw::keys::MEDIA_TYPE => "Audio",
                *pw::keys::MEDIA_ROLE => "Music",
                *pw::keys::MEDIA_CATEGORY => "Playback",
                *pw::keys::AUDIO_CHANNELS => channels.to_string(),
                "target.object" => target_name,
                *pw::keys::NODE_DONT_RECONNECT => "true",
            },
        )
    } else {
        pw::stream::StreamRc::new(
            core,
            "honkhonk-monitor",
            pw::properties::properties! {
                *pw::keys::MEDIA_TYPE => "Audio",
                *pw::keys::MEDIA_ROLE => "Music",
                *pw::keys::MEDIA_CATEGORY => "Playback",
                *pw::keys::AUDIO_CHANNELS => channels.to_string(),
            },
        )
    };
    result.map_err(|e| AudioError::StreamCreation(format!("monitor stream: {e}")))
}

/// Create a PipeWire output stream for local monitoring.
///
/// When `target` is `None`, PipeWire autoconnects to the default output.
/// When `target` is `Some(node_name)`, the stream is pinned to that device
/// and `NODE_DONT_RECONNECT` prevents PipeWire from overriding the target.
pub fn create_monitor_stream(
    core: pw::core::CoreRc,
    voices: Rc<RefCell<VoicePool>>,
    sample_rate: u32,
    channels: u16,
    target: Option<&str>,
) -> Result<PlaybackStream, AudioError> {
    let stream = open_monitor_stream(core, target, channels)?;

    let listener = stream
        .add_local_listener_with_user_data(())
        .process(make_mix_process_closure(
            voices,
            MixTarget::Monitor,
            sample_rate,
            channels,
        ))
        .register()
        .map_err(|e| AudioError::StreamCreation(format!("monitor listener: {e}")))?;

    let params_bytes = build_audio_params(sample_rate, channels as u32);
    let pod = Pod::from_bytes(&params_bytes)
        .ok_or_else(|| AudioError::StreamCreation("invalid audio params pod".into()))?;
    let mut params = [pod];

    stream
        .connect(
            spa::utils::Direction::Output,
            None,
            pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
            &mut params,
        )
        .map_err(|e| AudioError::StreamCreation(format!("monitor connect: {e}")))?;

    Ok(PlaybackStream {
        _stream: stream,
        _listener: listener,
    })
}

/// Reinterpret a mutable byte slice as a mutable f32 slice.
///
/// # Safety
/// PipeWire MAP_BUFFERS guarantees the buffer is aligned to at least 4 bytes,
/// and F32LE has no invalid bit patterns, so the transmute is sound.
fn cast_bytes_to_f32_mut(bytes: &mut [u8]) -> &mut [f32] {
    let len = bytes.len() / FRAME_SIZE;
    let ptr = bytes.as_mut_ptr() as *mut f32;
    // SAFETY: see doc comment above.
    unsafe { std::slice::from_raw_parts_mut(ptr, len) }
}
