//! Audio events and their state transitions.

use super::*;

/// First-run notice text for the persistent virtual mic (issue #49). When the
/// per-user conf.d was written the device persists across restarts; otherwise
/// it only lasts the session (until reboot) via the lingering node.
fn source_first_run_notice(confd_written: bool) -> String {
    if confd_written {
        "Created HonkHonk Mic virtual device. It will persist after restart. \
Select 'HonkHonk Mic' as your input in Discord/OBS."
            .to_string()
    } else {
        "HonkHonk Mic created for this session. \
Select 'HonkHonk Mic' as your input in Discord/OBS."
            .to_string()
    }
}

impl HonkHonk {
    pub(super) fn handle_audio_event(&mut self, event: AudioEvent) -> Task<Message> {
        match event {
            AudioEvent::MicrophoneFeedbackMuted => {
                self.config.mic_passthrough = false;
                self.mixer.mic_cooldown = Some(Instant::now() + Duration::from_secs(2));
                self.persist_config();
            }
            AudioEvent::Stream(event) => self.mixer_stream(event),
            AudioEvent::FeedbackDetected { suspected_source } => {
                self.mixer
                    .feedback(suspected_source.as_ref(), Instant::now());
                self.feedback_notice(suspected_source)
            }
            AudioEvent::RouteRejected { node_id, reason } => {
                self.mixer.rejected(node_id, reason, Instant::now());
                self.notices.push(
                    Notice::warning("Route blocked", format!("Source {node_id}: {reason}")),
                    Instant::now(),
                );
            }
            AudioEvent::RoutingChanged { node_id, enabled } => {
                self.mixer.routed(node_id, enabled);
                if enabled {
                    self.restore_source_level(node_id);
                }
            }
            AudioEvent::Ready => self.audio_ready(),
            AudioEvent::PlaybackStarted {
                sound_id,
                generation,
            } => self.note_playback_started(sound_id, generation),
            AudioEvent::PlaybackFinished {
                voice_id,
                sound_id,
                generation,
            } => self.note_playback_finished(voice_id, sound_id, generation),
            AudioEvent::Progress(p) => {
                // Raw 10 Hz anchor, retained for diagnostics/tests. The
                // smooth playhead is wall-clock driven (`Message::Frame`),
                // NOT this sample: re-anchoring a sample measured ~100 ms
                // ago to the current instant snapped the line backward
                // every drain (left/right jitter, #138).
                self.progress = p;
            }
            AudioEvent::Error(e) => self.audio_error(e),
            AudioEvent::SourceFirstRun { confd_written } => self.source_first_run(confd_written),
            AudioEvent::OutputDevicesChanged(devices) => self.update_output_devices(devices),
            AudioEvent::InputDevicesChanged(devices) => self.update_input_devices(devices),
            AudioEvent::EffectsLatencyChanged(_latency) => {
                // Reserved for Phase 4B: update UI latency indicator.
            }
        }
        Task::none()
    }

    fn feedback_notice(&mut self, source: Option<crate::audio::FeedbackSource>) {
        let body = match source {
            Some(source) => format!(
                "Feedback detected — {} was muted. Check your audio routing.",
                source.name
            ),
            None => {
                "Feedback detected. Check your audio routing; no routed source could be identified."
                    .into()
            }
        };
        self.notices
            .push(Notice::warning("Feedback detected", body), Instant::now());
    }

    fn audio_ready(&self) {
        self.send_audio_commands([AudioCommand::Router(
            crate::audio::RouterCommand::SetSafeMode(self.config.mixer_safe_mode),
        )]);
        self.send_audio_commands([AudioCommand::SetDynamics(self.config.processing.dynamics)]);
        tracing::info!("audio engine ready");
        if let Some(ref audio) = self.audio {
            audio.send(AudioCommand::SetVolume(self.config.volume));
        }
    }

    fn audio_error(&mut self, e: crate::audio::EngineErrorEvent) {
        tracing::error!(error = %e, "audio error");
        self.notices
            .push(Notice::error("Audio error", e.to_string()), Instant::now());
    }

    fn source_first_run(&mut self, confd_written: bool) {
        let body = source_first_run_notice(confd_written);
        tracing::info!(notice = %body, "source first-run notice");
        self.notices
            .push(Notice::info("HonkHonk Mic created", body), Instant::now());
    }

    fn note_playback_started(&mut self, sound_id: String, generation: u64) {
        // Warm plays and successful cold decodes claim
        // `playing` before the engine's Started event. When the
        // UI is idle, only the current generation may claim it:
        // a late superseded concurrent voice (older generation)
        // finishing after a newer short sound already ended
        // would otherwise re-highlight its tile and then leave
        // it stuck when the stale Finished is ignored
        // (#149/#152/#164).
        let confirms_current = self.playing.as_deref() == Some(sound_id.as_str());
        let claims_idle = self.playing.is_none() && generation == self.play_generation;
        if confirms_current || claims_idle {
            self.playing = Some(sound_id);
        }
    }

    fn note_playback_finished(&mut self, voice_id: u64, sound_id: String, generation: u64) {
        // Clear only when this Finished is for the sound we are
        // showing AND belongs to the current play. The sound_id
        // check keeps a Finished for an already-replaced sound
        // from blanking a newer press (#111); the generation
        // check additionally ignores the stale Finished emitted
        // for a same-sound voice that was superseded by an
        // immediate re-press, so its fresh playhead survives
        // (#149).
        if self.playing.as_deref() == Some(sound_id.as_str()) && generation == self.play_generation
        {
            self.clear_playback_state();
        }
        // A macro voice ending advances its run's completion
        // bookkeeping; a non-macro voice is ignored (#166).
        self.note_macro_voice_finished(voice_id);
    }

    fn update_output_devices(&mut self, devices: Vec<(String, String)>) {
        if let Some(ref target) = self.config.monitor_device.clone() {
            let was_visible = self.monitor_devices.iter().any(|(n, _)| n == target);
            let still_visible = devices.iter().any(|(n, _)| n == target);
            if was_visible && !still_visible {
                let config = AppConfig {
                    monitor_device: None,
                    ..self.config.clone()
                };
                self.config = config;
                self.persist_config();
                if let Some(ref audio) = self.audio {
                    audio.send(AudioCommand::SetMonitorDevice(None));
                }
            }
        }
        self.monitor_devices = devices;
    }

    fn update_input_devices(&mut self, devices: Vec<(String, String)>) {
        if let Some(ref target) = self.config.input_device.clone() {
            let was_visible = self.input_devices.iter().any(|(n, _)| n == target);
            let still_visible = devices.iter().any(|(n, _)| n == target);
            if was_visible && !still_visible {
                let config = AppConfig {
                    input_device: None,
                    ..self.config.clone()
                };
                self.config = config;
                self.persist_config();
                if let Some(ref audio) = self.audio {
                    audio.send(AudioCommand::SetInputDevice(None));
                }
            }
        }
        self.input_devices = devices;
    }
}
