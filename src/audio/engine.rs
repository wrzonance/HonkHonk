use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc;

use super::confd;
use super::effects::EffectSettings;
use super::error::{AudioError, EngineErrorEvent};
use super::handle::AudioHandle;
mod nodes;
mod playback_streams;
mod protocol;
mod runtime;
#[cfg(test)]
mod tests;
use super::playback;
use super::registry::{RegistryConfig, setup_registry_listener};
use super::router::{Router, RouterEvent};
use super::streams;
use super::voices::{FinishedVoice, VoicePool, VoiceSpec};
use nodes::*;
use playback_streams::{
    PlaybackStreams, active_format_conflict, ensure_playback_streams, monitor_enabled,
    rebuild_monitor_stream,
};
pub use protocol::{AudioCommand, AudioEvent, PlayMode};
use runtime::run_engine;

const SINK_NODE_NAME: &str = "honkhonk-mix";
const SINK_DESCRIPTION: &str = "HonkHonk Mix";
const SOURCE_NODE_NAME: &str = "honkhonk-mic";
const SOURCE_DESCRIPTION: &str = "HonkHonk Mic";

pub fn spawn(
    initial_passthrough: bool,
    initial_monitor_device: Option<String>,
    initial_input_device: Option<String>,
) -> Result<AudioHandle, AudioError> {
    let (cmd_tx, cmd_rx) = pipewire::channel::channel::<AudioCommand>();
    let (evt_tx, evt_rx) = mpsc::channel::<AudioEvent>();

    std::thread::Builder::new()
        .name("honkhonk-pw".into())
        .spawn(move || {
            // An explicitly chosen input device wins; otherwise fall back to the
            // system default source (the registry sanitizes out our own mic).
            let preferred_source = initial_input_device.or_else(query_default_source_name);
            if let Err(e) = run_engine(
                cmd_rx,
                evt_tx.clone(),
                preferred_source,
                initial_passthrough,
                initial_monitor_device,
            ) {
                let _ = evt_tx.send(AudioEvent::Error(EngineErrorEvent::EngineInitialization {
                    detail: e.to_string(),
                }));
            }
        })
        .map_err(AudioError::ThreadSpawn)?;

    Ok(AudioHandle::from_parts(cmd_tx, evt_rx))
}

fn query_default_source_name() -> Option<String> {
    let output = std::process::Command::new("pw-metadata")
        .args(["0", "default.audio.source"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .split("\"name\":\"")
        .nth(1)?
        .split('"')
        .next()
        .map(String::from)
}

struct EngineCtx {
    registry_sink_id: Rc<Cell<Option<u32>>>,
    core: pipewire::core::CoreRc,
    voices: Rc<RefCell<VoicePool>>,
    playback_streams: Rc<RefCell<PlaybackStreams>>,
    evt_tx: mpsc::Sender<AudioEvent>,
    engine_volume: Rc<Cell<f32>>,
    monitor_target: Rc<RefCell<Option<String>>>,
    mixer: Rc<RefCell<super::mixer::Mixer>>,
    router: Rc<RefCell<Router>>,
}

fn setup_completion_timer(
    pw_loop: &pipewire::loop_::Loop,
    voices_timer: Rc<RefCell<VoicePool>>,
    evt_tx_timer: mpsc::Sender<AudioEvent>,
) -> Result<pipewire::loop_::TimerSource<'_>, AudioError> {
    let timer = pw_loop.add_timer(move |_expirations| {
        let (finished, progress) = {
            let mut voices = voices_timer.borrow_mut();
            let progress = voices.progress();
            let finished = voices.drain_finished();
            (finished, progress)
        };

        if let Some(p) = progress {
            let _ = evt_tx_timer.send(AudioEvent::Progress(p));
        }

        send_finished_events(&evt_tx_timer, finished);
    });

    if let Err(e) = timer
        .update_timer(
            Some(std::time::Duration::from_millis(100)),
            Some(std::time::Duration::from_millis(100)),
        )
        .into_result()
    {
        return Err(AudioError::PipeWireInit(format!(
            "arm completion timer: {e}"
        )));
    }

    Ok(timer)
}

fn send_finished_events(evt_tx: &mpsc::Sender<AudioEvent>, voices: Vec<FinishedVoice>) {
    for voice in voices {
        let _ = evt_tx.send(AudioEvent::PlaybackFinished {
            voice_id: voice.voice_id,
            sound_id: voice.sound_id,
            generation: voice.generation,
        });
    }
}

/// Bootstrap the external-stream observer (issue #26).
///
/// Starts the `streams::start` watcher bound to the engine's PipeWire core.
/// Returns both the watcher (MUST be held to end-of-scope — dropping detaches
/// the registry listener) and the receiver for stream events, which the caller
/// attaches to the PipeWire main loop so the Router receives events on the
/// engine thread.
fn spawn_stream_watcher(
    core: &pipewire::core::CoreRc,
) -> Result<(streams::StreamWatcher, mpsc::Receiver<streams::StreamEvent>), AudioError> {
    let self_pid = std::process::id();
    let (stream_watcher, stream_rx) = streams::start(core, self_pid)?;
    Ok((stream_watcher, stream_rx))
}

/// Decoded PCM plus identity for a single play, bundled so `handle_play` stays
/// within the argument-count lint as fields accrete (e.g. `generation`, #149).
struct PlayRequest {
    processing: crate::audio::processing::VoiceProcessing,
    voice_id: u64,
    sound_id: String,
    samples: Arc<Vec<f32>>,
    sample_rate: u32,
    channels: u16,
    generation: u64,
    gain: f32,
    effects: EffectSettings,
    mode: PlayMode,
}

fn handle_play(ctx: &EngineCtx, req: PlayRequest) {
    if ctx.registry_sink_id.get().is_none() {
        let _ = ctx.evt_tx.send(AudioEvent::Error(
            EngineErrorEvent::VirtualSinkNotRegistered,
        ));
        finish_failed_play(ctx, req);
        return;
    }
    let req = prepare_channels(req);
    let output_channels =
        super::processing::ChannelLayout::new(req.channels, req.processing.sound).output_channels();
    let format_fallback = req.mode == PlayMode::Concurrent
        && active_format_conflict(ctx, req.sample_rate, output_channels);
    if req.mode == PlayMode::Interrupt || format_fallback {
        let finished = ctx.voices.borrow_mut().stop_all();
        send_finished_events(&ctx.evt_tx, finished);
    }
    if !ensure_playback_streams(ctx, req.sample_rate, output_channels) {
        finish_failed_play(ctx, req);
        return;
    }
    start_voice(ctx, req);
}

fn prepare_channels(mut req: PlayRequest) -> PlayRequest {
    req.processing.sound = req.processing.sound.sanitized();
    req
}

fn finish_failed_play(ctx: &EngineCtx, req: PlayRequest) {
    // Every Play yields a matching-generation Finished, even on failure.
    let _ = ctx.evt_tx.send(AudioEvent::PlaybackFinished {
        voice_id: req.voice_id,
        sound_id: req.sound_id,
        generation: req.generation,
    });
}

fn start_voice(ctx: &EngineCtx, req: PlayRequest) {
    let finished = ctx.voices.borrow_mut().push(VoiceSpec {
        processing: req.processing,
        id: req.voice_id,
        sound_id: req.sound_id.clone(),
        generation: req.generation,
        samples: req.samples,
        sample_rate: req.sample_rate,
        channels: req.channels,
        gain: req.gain,
        master_volume: ctx.engine_volume.get(),
        effects: req.effects,
        monitor_enabled: monitor_enabled(ctx),
    });
    send_finished_events(&ctx.evt_tx, finished);
    let _ = ctx.evt_tx.send(AudioEvent::PlaybackStarted {
        sound_id: req.sound_id,
        generation: req.generation,
    });
}
