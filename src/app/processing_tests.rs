use super::*;
use crate::audio::processing::{SoundProcessing, VoiceProcessing};

fn fixture() -> (HonkHonk, SoundEntry, crate::audio::CachedPcm) {
    let mut app = HonkHonk::new_for_test();
    let (audio, _) = crate::audio::test_handle();
    app.audio = Some(audio);
    let sound = SoundEntry {
        id: "test".into(),
        name: "Test".into(),
        path: "/tmp/test.wav".into(),
        category: "Test".into(),
        duration_ms: None,
        format: crate::state::AudioFormat::Wav,
        modified_ms: None,
    };
    app.sounds.push(sound.clone());
    let pcm = crate::audio::CachedPcm {
        samples: Arc::new(vec![0.1; 100]),
        sample_rate: 48_000,
        channels: 1,
        duration: Duration::from_secs(1),
        analysis: crate::audio::processing::AudioAnalysis {
            fingerprint: "same bytes".into(),
            normalization_gain: 3.0,
            repaired_channel: true,
        },
    };
    app.sound_meta.bind_fingerprint("previous", "same bytes");
    app.sound_meta.set(
        "previous".into(),
        crate::state::SoundMeta {
            volume: 1.4,
            processing: SoundProcessing {
                pan: 0.5,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    (app, sound, pcm)
}

fn last_play(app: &HonkHonk) -> (f32, VoiceProcessing) {
    app.audio
        .as_ref()
        .unwrap()
        .sent_commands()
        .iter()
        .rev()
        .find_map(|cmd| match cmd {
            AudioCommand::Play {
                gain, processing, ..
            } => Some((*gain, *processing)),
            _ => None,
        })
        .unwrap()
}

#[test]
fn warm_tile_dispatch_uses_current_fingerprinted_processing() {
    let (mut app, sound, pcm) = fixture();
    app.audio_store.insert_pcm(sound.id.clone(), Arc::new(pcm));
    let _ = app.request_play(&sound, false);
    let (gain, processing) = last_play(&app);
    assert_eq!(gain, 1.4);
    assert_eq!(processing.normalization_gain, 3.0);
    assert_eq!(processing.sound.pan, 0.5);
    app.config.processing.normalize = false;
    let _ = app.request_play(&sound, false);
    assert_eq!(last_play(&app).1.normalization_gain, 1.0);
    assert!(
        app.notices
            .iter()
            .any(|n| n.notice.title == "Silent channel repaired")
    );
}

#[test]
fn cold_tile_uses_settings_at_landing_and_override_wins_over_global() {
    let (mut app, sound, pcm) = fixture();
    let _ = app.request_play(&sound, false);
    app.config.processing.normalize = false;
    let mut meta = app.sound_meta.get("previous");
    meta.processing.normalize = Some(true);
    app.sound_meta.set("previous".into(), meta);
    let dispatch = playback::PlaybackDispatch {
        generation: app.play_generation,
        voice_id: app.play_generation,
        gain: 1.0,
        effects: Default::default(),
        mode: PlayMode::Concurrent,
    };
    let _ = app.handle_decoded(sound.id.clone(), Ok(pcm), dispatch);
    assert_eq!(last_play(&app).0, 1.4);
    assert_eq!(last_play(&app).1.normalization_gain, 3.0);
    assert_eq!(last_play(&app).1.sound.normalize, Some(true));
}

#[test]
fn macro_warm_and_cold_paths_apply_content_controls_and_step_gain() {
    for warm in [false, true] {
        let (mut app, sound, pcm) = fixture();
        let id = app.macros.add("Test").id.clone();
        let mut step = crate::state::Step::new(sound.path.clone(), 0);
        step.gain = 0.5;
        app.macros.replace_steps(&id, vec![step]);
        if warm {
            app.audio_store
                .insert_pcm(sound.id.clone(), Arc::new(pcm.clone()));
        }
        let _ = app.play_macro(&id);
        let run_id = app.macro_run_id;
        let _ = app.on_macro_step_due(run_id, 0);
        if !warm {
            let voice = macros::MacroVoice {
                voice_id: 1 << 63,
                sound_id: sound.id.clone(),
                gain: 0.5,
                effects: Default::default(),
            };
            let _ = app.on_macro_step_decoded(run_id, voice, Ok(pcm));
        }
        let (gain, processing) = last_play(&app);
        assert_eq!(gain, 0.5);
        assert!((processing.normalization_gain - 4.2).abs() < 0.0001);
        assert_eq!(processing.sound.pan, 0.5);
    }
}

#[test]
fn stale_editor_identity_cannot_replace_a_newer_draft() {
    let (mut app, sound, _) = fixture();
    let _ = app.open_sound_editor(sound.id.clone());
    let old = app.processing_ui.generation;
    let _ = app.open_sound_editor(sound.id.clone());
    let current = app.processing_ui.generation;
    let _ = app.editor_fingerprint_ready(sound.id.clone(), old, Ok("same bytes".into()));
    assert!(app.processing_ui.loading);
    assert_eq!(app.processing_ui.draft.pan, 0.0);
    let _ = app.editor_fingerprint_ready(sound.id.clone(), current, Ok("same bytes".into()));
    assert!(!app.processing_ui.loading);
    assert_eq!(app.processing_ui.draft.pan, 0.5);
    let settings = SoundProcessing {
        pan: -0.3,
        ..Default::default()
    };
    let _ = app.update(Message::SoundProcessingChanged(settings));
    app.save_sound_metadata(sound.id.clone());
    assert_eq!(app.sound_meta.get("previous").processing.pan, -0.3);
}

#[test]
fn defaults_and_global_changes_reach_engine_and_persist_in_config() {
    let (mut app, _, _) = fixture();
    assert!(app.config.processing.normalize);
    assert!(app.config.processing.dynamics.enabled);
    let mut settings = app.config.processing;
    settings.dynamics.threshold_db = -24.0;
    settings.normalize = false;
    let _ = app.change_processing(settings);
    assert!(matches!(app.audio.as_ref().unwrap().sent_commands().last(),
        Some(AudioCommand::SetDynamics(dynamics)) if dynamics.threshold_db == -24.0));
    let json = serde_json::to_string(&app.config).unwrap();
    let loaded: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.processing, settings);
}

#[test]
fn import_preview_respects_global_normalization() {
    let (mut app, _, pcm) = fixture();
    app.import.open = true;
    let _ = app.update(Message::Import(import::ImportMessage::Previewed(
        0,
        0,
        Ok(pcm.clone()),
    )));
    assert_eq!(last_play(&app).1.normalization_gain, 3.0);
    app.config.processing.normalize = false;
    let _ = app.update(Message::Import(import::ImportMessage::Previewed(
        0,
        0,
        Ok(pcm),
    )));
    assert_eq!(last_play(&app).1.normalization_gain, 1.0);
}

#[test]
fn editor_identity_failure_reports_loading_failure_without_attempting_playback() {
    let (mut app, sound, _) = fixture();
    let _ = app.open_sound_editor(sound.id.clone());
    let generation = app.processing_ui.generation;
    let _ = app.editor_fingerprint_ready(sound.id, generation, Err("permission denied".into()));
    assert!(!app.processing_ui.loading);
    let notice = &app.notices.iter().last().unwrap().notice;
    assert_eq!(notice.title, "Sound identity could not load");
    assert!(notice.body.contains("permission denied"));
    assert!(notice.body.contains("reopen the editor"));
    assert!(
        !app.audio
            .as_ref()
            .unwrap()
            .sent_commands()
            .iter()
            .any(|cmd| matches!(cmd, AudioCommand::Play { .. }))
    );
}
