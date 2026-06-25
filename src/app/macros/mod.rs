//! Macro playback controller (app layer). Fires a macro's steps onto the
//! polyphonic voice pool (#164) at their offsets, enforces one-macro-at-a-time,
//! and cancels cleanly on re-fire / Stop All. The schedule and run bookkeeping
//! are pure ([`scheduler`]); this module owns the Iced `Task` timing and the
//! engine dispatch. The timeline UI is #168; slot triggers (#169) call
//! [`HonkHonk::play_macro`].

mod scheduler;

use std::sync::Arc;

use iced::Task;

pub(crate) use scheduler::MacroPlayback;
use scheduler::schedule;

use super::{HonkHonk, Message};
use crate::audio::effects::EffectSettings;
use crate::audio::{AudioCommand, CachedPcm, PlayMode};
use crate::state::Step;

/// Macro voices live in a disjoint id space (top bit set) so they never collide
/// with — or advance — the tile `play_generation` that drives now-playing UI
/// ownership (#149). A macro firing mid-tile-press must not break the tile's
/// playhead/highlight by making the tile's own decode/finish look stale.
const MACRO_VOICE_FLAG: u64 = 1 << 63;

/// Per-voice parameters for one macro-step dispatch. Bundled so the dispatch
/// helpers and the `MacroStepDecoded` message stay within the arg-count lint.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MacroVoice {
    pub(crate) voice_id: u64,
    pub(crate) sound_id: String,
    pub(crate) gain: f32,
    pub(crate) effects: EffectSettings,
}

impl HonkHonk {
    /// Starts macro `id` from the top. Cancels any current run first (so
    /// re-firing restarts, and firing a different macro switches), then spawns
    /// one delayed [`Message::MacroStepDue`] per step. A no-op for an unknown or
    /// empty macro.
    pub(crate) fn play_macro(&mut self, id: &str) -> Task<Message> {
        // Validate before cancelling: a no-op request (unknown or empty macro)
        // must not stop a macro that is currently running.
        let plan = {
            let Some(macro_def) = self.macros.get(id) else {
                return Task::none();
            };
            if macro_def.steps.is_empty() {
                return Task::none();
            }
            schedule(macro_def)
        };
        self.cancel_macro();
        let run_id = self.macro_run_id;
        self.macro_playback = Some(MacroPlayback::new(run_id, id.to_string(), plan.len()));
        let tasks = plan.into_iter().map(|scheduled| {
            let (delay, step) = (scheduled.delay, scheduled.step);
            Task::perform(async move { tokio::time::sleep(delay).await }, move |()| {
                Message::MacroStepDue { run_id, step }
            })
        });
        Task::batch(tasks)
    }

    /// Cancels the active run: stops its still-playing voices and bumps
    /// `macro_run_id` so any in-flight `MacroStepDue` / `MacroStepDecoded` for
    /// the old run is ignored on arrival. Idempotent.
    pub(crate) fn cancel_macro(&mut self) {
        if let Some(playback) = self.macro_playback.take() {
            if let Some(audio) = &self.audio {
                for &voice_id in playback.active_voices() {
                    audio.send(AudioCommand::StopVoice(voice_id));
                }
            }
        }
        self.macro_run_id = self.macro_run_id.wrapping_add(1);
    }

    /// A scheduled step's timer fired. Ignored if its run was cancelled
    /// (`run_id` no longer current). Otherwise dispatches the step — warm PCM
    /// fires synchronously; a miss decodes off-thread.
    pub(crate) fn on_macro_step_due(&mut self, run_id: u64, step_idx: usize) -> Task<Message> {
        if self.current_macro_run() != Some(run_id) {
            return Task::none();
        }
        let macro_id = self
            .macro_playback
            .as_ref()
            .map(|p| p.macro_id.clone())
            .unwrap_or_default();
        let step = self
            .macros
            .get(&macro_id)
            .and_then(|m| m.steps.get(step_idx))
            .cloned();
        match step {
            Some(step) => self.dispatch_macro_step(run_id, step),
            None => {
                self.resolve_macro_step_failed();
                Task::none()
            }
        }
    }

    /// A cold macro step's off-thread decode landed. Dropped if its run was
    /// cancelled. `Ok` caches the PCM and fires the voice; `Err` resolves the
    /// step as failed so the run can still complete.
    pub(crate) fn on_macro_step_decoded(
        &mut self,
        run_id: u64,
        voice: MacroVoice,
        result: Result<CachedPcm, String>,
    ) -> Task<Message> {
        if self.current_macro_run() != Some(run_id) {
            return Task::none();
        }
        match result {
            Ok(pcm) => {
                let pcm = Arc::new(pcm);
                self.audio_store
                    .insert_pcm(voice.sound_id.clone(), Arc::clone(&pcm));
                self.send_macro_play(&voice, &pcm);
                self.record_macro_dispatch(voice.voice_id);
            }
            Err(e) => {
                tracing::warn!(sound = %voice.sound_id, error = %e, "macro step decode failed");
                self.resolve_macro_step_failed();
            }
        }
        Task::none()
    }

    /// Drops a finished voice from the active run, clearing the run once every
    /// step has resolved and no voices remain. A voice the run never owned (a
    /// tile press) is ignored. Called for every `PlaybackFinished`.
    pub(crate) fn note_macro_voice_finished(&mut self, voice_id: u64) {
        if let Some(playback) = &mut self.macro_playback {
            if playback.on_voice_finished(voice_id) {
                self.macro_playback = None;
            }
        }
    }

    fn current_macro_run(&self) -> Option<u64> {
        self.macro_playback.as_ref().map(|p| p.run_id)
    }

    /// Next macro voice id, in the top-bit-flagged space disjoint from the tile
    /// `play_generation`.
    fn next_macro_voice_id(&mut self) -> u64 {
        self.macro_voice_seq = self.macro_voice_seq.wrapping_add(1);
        MACRO_VOICE_FLAG | self.macro_voice_seq
    }

    /// Resolves the step against the library (macros reference sounds by path),
    /// assigns a fresh voice/generation, and fires it — warm PCM now, else via
    /// an off-thread decode. A step whose sound is gone resolves as failed.
    fn dispatch_macro_step(&mut self, run_id: u64, step: Step) -> Task<Message> {
        let Some(entry) = self.sounds.iter().find(|s| s.path == step.sound) else {
            self.resolve_macro_step_failed();
            return Task::none();
        };
        let path = entry.path.clone();
        let sound_id = entry.id.clone();
        let voice = MacroVoice {
            voice_id: self.next_macro_voice_id(),
            sound_id,
            gain: step.gain,
            effects: step.effects,
        };

        if let Some(pcm) = self.audio_store.get_pcm(&voice.sound_id) {
            self.send_macro_play(&voice, &pcm);
            self.record_macro_dispatch(voice.voice_id);
            return Task::none();
        }

        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || crate::audio::decode(&path))
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|r| r.map_err(|e| e.to_string()))
                    .map(|d| CachedPcm {
                        samples: Arc::new(d.samples),
                        sample_rate: d.sample_rate,
                        channels: d.channels,
                        duration: d.duration,
                    })
            },
            move |result| Message::MacroStepDecoded {
                run_id,
                voice_id: voice.voice_id,
                sound_id: voice.sound_id.clone(),
                gain: voice.gain,
                effects: voice.effects,
                result,
            },
        )
    }

    /// Sends a concurrent `Play`. A macro voice's `generation` equals its
    /// top-bit-flagged `voice_id`, which can never equal the tile
    /// `play_generation`, so its `PlaybackStarted`/`Finished` never claim or
    /// clear the now-playing highlight (that stays with tile presses).
    fn send_macro_play(&self, voice: &MacroVoice, pcm: &Arc<CachedPcm>) {
        if let Some(audio) = &self.audio {
            audio.send(AudioCommand::Play {
                voice_id: voice.voice_id,
                sound_id: voice.sound_id.clone(),
                samples: Arc::clone(&pcm.samples),
                sample_rate: pcm.sample_rate,
                channels: pcm.channels,
                generation: voice.voice_id,
                gain: voice.gain,
                effects: voice.effects,
                mode: PlayMode::Concurrent,
            });
        }
    }

    fn record_macro_dispatch(&mut self, voice_id: u64) {
        if let Some(playback) = &mut self.macro_playback {
            playback.record_dispatch(voice_id);
        }
    }

    fn resolve_macro_step_failed(&mut self) {
        if let Some(playback) = &mut self.macro_playback {
            playback.record_failed_step();
            if playback.is_complete() {
                self.macro_playback = None;
            }
        }
    }
}

#[cfg(test)]
mod tests;
