//! Observe the existing sink monitor. No samples are written into the routing graph.
use super::FeedbackObservation;
use crate::audio::voices::VoicePool;
use crate::audio::{AudioError, AudioEvent, EngineErrorEvent};
use pipewire::{self as pw, spa};
use std::{cell::RefCell, rc::Rc, sync::mpsc};

pub(crate) struct FeedbackMonitor {
    _stream: pw::stream::StreamRc,
    _listener: pw::stream::StreamListener<()>,
}

impl FeedbackMonitor {
    pub fn start(
        core: pw::core::CoreRc,
        observation: Rc<RefCell<FeedbackObservation>>,
        voices: Rc<RefCell<VoicePool>>,
        events: mpsc::Sender<AudioEvent>,
    ) -> Result<Self, AudioError> {
        let stream = pw::stream::StreamRc::new(
            core,
            "honkhonk-feedback-observer",
            pw::properties::properties! {
                "node.name" => "honkhonk-feedback-observer",
                "application.name" => "HonkHonk",
                "media.type" => "Audio",
                "media.category" => "Capture",
                "target.object" => "honkhonk-mix",
                "stream.capture.sink" => "true",
                "stream.monitor" => "true",
                "node.passive" => "true",
                "node.dont-reconnect" => "true",
                "node.dont-fallback" => "true",
            },
        )
        .map_err(|e| AudioError::StreamCreation(format!("feedback observer: {e}")))?;
        let listener = stream
            .add_local_listener_with_user_data(())
            .process(move |stream, _| {
                observe(stream, &mut observation.borrow_mut(), &voices.borrow())
            })
            .state_changed(move |_, _, _, state| {
                if let pw::stream::StreamState::Error(detail) = state {
                    let _ = events.send(AudioEvent::Error(EngineErrorEvent::FeedbackMonitor {
                        detail: detail.to_string(),
                    }));
                }
            })
            .register()
            .map_err(|e| AudioError::StreamCreation(format!("feedback listener: {e}")))?;
        let bytes = crate::audio::playback::build_audio_params(48_000, 2);
        let pod = spa::pod::Pod::from_bytes(&bytes)
            .ok_or_else(|| AudioError::StreamCreation("feedback format pod invalid".into()))?;
        stream
            .connect(
                spa::utils::Direction::Input,
                None,
                pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
                &mut [pod],
            )
            .map_err(|e| AudioError::StreamCreation(format!("feedback connect: {e}")))?;
        Ok(Self {
            _stream: stream,
            _listener: listener,
        })
    }
}

fn observe(stream: &pw::stream::Stream, observation: &mut FeedbackObservation, voices: &VoicePool) {
    let Some(mut buffer) = stream.dequeue_buffer() else {
        return;
    };
    let Some(data) = buffer.datas_mut().first_mut() else {
        return;
    };
    let offset = data.chunk().offset() as usize;
    let size = data.chunk().size() as usize;
    let Some(bytes) = data.data() else {
        return;
    };
    let Some(bytes) = bytes.get(offset..offset.saturating_add(size)) else {
        return;
    };
    let samples = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b));
    observation.observe(samples, voices);
}
