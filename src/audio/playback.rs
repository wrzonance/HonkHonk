use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use pipewire as pw;
use pw::spa;
use pw::spa::pod::Pod;

use super::error::AudioError;

const FRAME_SIZE: usize = std::mem::size_of::<f32>();

/// Upper bound on the per-sound volume multiplier (boost), mirroring the
/// per-sound volume domain in `state::sound_meta` (0.0..=2.0). The pre-#151
/// path scaled samples by this factor uncapped, so a user's above-unity boost
/// must survive here too — clamping to 1.0 would silently quiet boosted sounds.
const MAX_PER_SOUND_GAIN: f32 = 2.0;

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

fn make_process_closure(
    state: Rc<RefCell<PlaybackState>>,
    channels: u16,
) -> impl FnMut(&pw::stream::Stream, &mut ()) + 'static {
    move |stream, _| {
        if let Some(mut buffer) = stream.dequeue_buffer() {
            let datas = buffer.datas_mut();
            if let Some(data) = datas.first_mut() {
                // Obtain byte slice, fill it, then record its length — all
                // before releasing the borrow on `data` so we can call
                // `chunk_mut()` separately (they both take `&mut self`).
                let total_bytes = if let Some(slice) = data.data() {
                    let float_slice = cast_bytes_to_f32_mut(slice);
                    let mut ps = state.borrow_mut();
                    let wrote = ps.fill_buffer(float_slice);
                    for s in float_slice[wrote..].iter_mut() {
                        *s = 0.0;
                    }
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

pub fn create_sink_stream(
    core: pw::core::CoreRc,
    state: Rc<RefCell<PlaybackState>>,
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
        .process(make_process_closure(state, channels))
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
    state: Rc<RefCell<PlaybackState>>,
    sample_rate: u32,
    channels: u16,
    target: Option<&str>,
) -> Result<PlaybackStream, AudioError> {
    let stream = open_monitor_stream(core, target, channels)?;

    let listener = stream
        .add_local_listener_with_user_data(())
        .process(make_process_closure(state, channels))
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

pub struct PlaybackState {
    sound_id: Option<String>,
    samples: Option<Arc<Vec<f32>>>,
    cursor: usize,
    volume: f32,
    gain: f32,
    sample_rate: u32,
    channels: u16,
    active: bool,
}

impl PlaybackState {
    pub fn new() -> Self {
        Self {
            sound_id: None,
            samples: None,
            cursor: 0,
            volume: 1.0,
            gain: 1.0,
            sample_rate: 48000,
            channels: 2,
            active: false,
        }
    }

    pub fn with_volume(volume: f32) -> Self {
        Self {
            volume: volume.clamp(0.0, 1.0),
            ..Self::new()
        }
    }

    // Six args (one over the `too-many-arguments-threshold = 5`) is the
    // canonical playback descriptor: identity, buffer, format (rate + channels),
    // and the per-sound gain. They are not separable into a meaningful sub-struct
    // here, and the engine + app both call `start` positionally with exactly
    // these (#151).
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &mut self,
        sound_id: String,
        samples: Arc<Vec<f32>>,
        sample_rate: u32,
        channels: u16,
        gain: f32,
    ) {
        self.sound_id = Some(sound_id);
        self.samples = Some(samples);
        self.cursor = 0;
        self.gain = gain.clamp(0.0, MAX_PER_SOUND_GAIN);
        self.sample_rate = sample_rate;
        self.channels = channels;
        self.active = true;
    }

    pub fn stop(&mut self) {
        self.sound_id = None;
        self.samples = None;
        self.cursor = 0;
        self.active = false;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn sound_id(&self) -> Option<&str> {
        self.sound_id.as_deref()
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
    }

    pub fn progress(&self) -> f32 {
        match &self.samples {
            Some(s) if !s.is_empty() => self.cursor as f32 / s.len() as f32,
            _ => 0.0,
        }
    }

    pub fn fill_buffer(&mut self, buf: &mut [f32]) -> usize {
        let samples = match &self.samples {
            Some(s) if self.active => s,
            _ => return 0,
        };

        let remaining = samples.len().saturating_sub(self.cursor);
        let to_write = buf.len().min(remaining);

        if to_write == 0 {
            self.active = false;
            return 0;
        }

        let src = &samples[self.cursor..self.cursor + to_write];
        let g = self.volume * self.gain;
        for (dst, &sample) in buf[..to_write].iter_mut().zip(src.iter()) {
            *dst = sample * g;
        }

        self.cursor += to_write;

        if self.cursor >= samples.len() {
            self.active = false;
        }

        to_write
    }
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_at_start_is_zero() {
        let samples = Arc::new(vec![0.0_f32; 20]);
        let mut state = PlaybackState::new();
        state.start("test".into(), samples, 48000, 2, 1.0);
        assert_eq!(state.progress(), 0.0);
    }

    #[test]
    fn progress_at_midpoint() {
        let samples = Arc::new(vec![0.0_f32; 20]);
        let mut state = PlaybackState::new();
        state.start("test".into(), samples, 48000, 2, 1.0);
        let mut buf = vec![0.0_f32; 10];
        state.fill_buffer(&mut buf);
        let p = state.progress();
        assert!((p - 0.5).abs() < f32::EPSILON, "expected ~0.5, got {p}");
    }

    #[test]
    fn progress_at_end_is_one() {
        let samples = Arc::new(vec![0.0_f32; 20]);
        let mut state = PlaybackState::new();
        state.start("test".into(), samples, 48000, 2, 1.0);
        let mut buf = vec![0.0_f32; 20];
        state.fill_buffer(&mut buf);
        assert_eq!(state.progress(), 1.0);
    }

    #[test]
    fn progress_with_no_samples_is_zero() {
        let state = PlaybackState::new();
        assert_eq!(state.progress(), 0.0);
    }

    #[test]
    fn with_volume_sets_initial_volume() {
        let state = PlaybackState::with_volume(0.42);
        assert!((state.volume() - 0.42).abs() < f32::EPSILON);
        assert!(!state.is_active());
    }

    #[test]
    fn with_volume_clamps_above_one() {
        let state = PlaybackState::with_volume(1.5);
        assert!((state.volume() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn with_volume_clamps_below_zero() {
        let state = PlaybackState::with_volume(-0.3);
        assert!((state.volume() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn fill_buffer_respects_initial_volume() {
        let samples = Arc::new(vec![1.0_f32; 100]);
        let mut state = PlaybackState::with_volume(0.5);
        state.start("test".into(), samples, 48000, 1, 1.0);

        let mut buf = vec![0.0_f32; 10];
        let wrote = state.fill_buffer(&mut buf);

        assert_eq!(wrote, 10);
        for &s in &buf[..wrote] {
            assert!(
                (s - 0.5).abs() < f32::EPSILON,
                "expected 0.5 (1.0 * 0.5 volume), got {s}"
            );
        }
    }

    #[test]
    fn fill_buffer_multiplies_master_and_per_sound_gain() {
        // master 0.5 (with_volume) * per-sound gain 0.5 = 0.25 effective.
        let samples = Arc::new(vec![1.0_f32; 100]);
        let mut state = PlaybackState::with_volume(0.5);
        state.start("test".into(), samples, 48_000, 1, 0.5);

        let mut buf = vec![0.0_f32; 10];
        let wrote = state.fill_buffer(&mut buf);

        assert_eq!(wrote, 10);
        for &s in &buf[..wrote] {
            assert!((s - 0.25).abs() < f32::EPSILON, "expected 0.25, got {s}");
        }
    }

    #[test]
    fn fill_buffer_preserves_per_sound_boost_above_unity() {
        // The per-sound volume slider ranges to 2.0; a boost above unity must
        // survive (the pre-#151 path scaled samples uncapped). master 1.0 *
        // gain 2.0 = 2.0, so a 0.4 sample becomes 0.8.
        let samples = Arc::new(vec![0.4_f32; 100]);
        let mut state = PlaybackState::with_volume(1.0);
        state.start("test".into(), samples, 48_000, 1, 2.0);

        let mut buf = vec![0.0_f32; 10];
        let wrote = state.fill_buffer(&mut buf);

        assert_eq!(wrote, 10);
        for &s in &buf[..wrote] {
            assert!(
                (s - 0.8).abs() < f32::EPSILON,
                "expected 0.8 boost, got {s}"
            );
        }
    }
}
