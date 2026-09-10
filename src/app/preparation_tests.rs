use super::*;

#[test]
fn preparation_progress_is_scoped_to_current_generation() {
    let mut app = HonkHonk::new_for_test();
    app.sounds.push(crate::state::SoundEntry {
        id: "missing".into(),
        name: "missing".into(),
        path: "/does/not/exist.wav".into(),
        format: crate::state::AudioFormat::Wav,
        duration_ms: None,
        modified_ms: None,
        category: "test".into(),
    });
    app.start_library_preparation();
    let generation = app.preparation.request.as_ref().unwrap().generation;
    app.library_preparation_item(
        generation.wrapping_sub(1),
        "missing".into(),
        Ok(test_prepared()),
    );
    assert_eq!(app.library_preparation_progress(), Some((0, 1)));
}

#[test]
fn prepared_pcm_and_waveform_share_the_existing_caches() {
    let mut app = HonkHonk::new_for_test();
    app.sounds.push(crate::state::SoundEntry {
        id: "ready".into(),
        name: "ready".into(),
        path: "/tmp/ready.wav".into(),
        format: crate::state::AudioFormat::Wav,
        duration_ms: None,
        modified_ms: None,
        category: "test".into(),
    });
    app.start_library_preparation();
    let generation = app.preparation.request.as_ref().unwrap().generation;
    app.library_preparation_item(generation, "ready".into(), Ok(test_prepared()));

    assert!(app.audio_store.get_pcm("ready").is_some());
    assert!(app.now_playing.envelope("ready").is_some());
    assert_eq!(app.library_preparation_progress(), Some((1, 1)));
}

#[test]
fn preparation_skips_warm_pcm_and_queues_only_new_sounds() {
    let mut app = HonkHonk::new_for_test();
    app.sounds.extend([
        crate::state::SoundEntry {
            id: "warm".into(),
            name: "warm".into(),
            path: "/tmp/warm.wav".into(),
            format: crate::state::AudioFormat::Wav,
            duration_ms: None,
            modified_ms: None,
            category: "test".into(),
        },
        crate::state::SoundEntry {
            id: "new".into(),
            name: "new".into(),
            path: "/tmp/new.wav".into(),
            format: crate::state::AudioFormat::Wav,
            duration_ms: None,
            modified_ms: None,
            category: "test".into(),
        },
    ]);
    app.audio_store
        .insert_pcm("warm".into(), Arc::clone(&test_prepared().pcm));

    app.start_library_preparation();

    let request = app.preparation.request.as_ref().unwrap();
    assert_eq!(request.files.len(), 1);
    assert_eq!(request.files[0].0, "new");
    assert_eq!(app.library_preparation_progress(), Some((0, 1)));
}

#[test]
fn preparation_has_no_request_when_every_pcm_is_warm() {
    let mut app = HonkHonk::new_for_test();
    app.sounds.push(crate::state::SoundEntry {
        id: "warm".into(),
        name: "warm".into(),
        path: "/tmp/warm.wav".into(),
        format: crate::state::AudioFormat::Wav,
        duration_ms: None,
        modified_ms: None,
        category: "test".into(),
    });
    app.audio_store
        .insert_pcm("warm".into(), Arc::clone(&test_prepared().pcm));

    app.start_library_preparation();

    assert!(app.preparation.request.is_none());
    assert_eq!(app.library_preparation_progress(), None);
}

#[test]
fn preparation_drops_pcm_when_library_path_changed() {
    let mut app = HonkHonk::new_for_test();
    app.sounds.push(crate::state::SoundEntry {
        id: "changed".into(),
        name: "changed".into(),
        path: "/tmp/new.wav".into(),
        format: crate::state::AudioFormat::Wav,
        duration_ms: None,
        modified_ms: None,
        category: "test".into(),
    });
    app.start_library_preparation();
    let generation = app.preparation.request.as_ref().unwrap().generation;
    let pcm = crate::audio::CachedPcm {
        analysis: Default::default(),
        samples: Arc::new(vec![0.0]),
        sample_rate: 8_000,
        channels: 1,
        duration: std::time::Duration::from_millis(1),
    };

    app.library_preparation_item(
        generation,
        "changed".into(),
        Ok(crate::audio::preparation::wrap_for_test_at(
            pcm,
            std::path::Path::new("/tmp/old.wav"),
        )),
    );

    assert!(app.audio_store.get_pcm("changed").is_none());
}

#[test]
fn mixed_results_continue_and_emit_one_end_summary() {
    let mut app = HonkHonk::new_for_test();
    app.sounds.push(crate::state::SoundEntry {
        id: "broken".into(),
        name: "broken".into(),
        path: "/tmp/broken.wav".into(),
        format: crate::state::AudioFormat::Wav,
        duration_ms: None,
        modified_ms: None,
        category: "test".into(),
    });
    app.start_library_preparation();
    let generation = app.preparation.request.as_ref().unwrap().generation;
    app.library_preparation_item(generation, "broken".into(), Err("corrupt".into()));
    app.library_preparation_finished(generation, vec![("broken".into(), "corrupt".into())]);

    assert!(app.preparation.request.is_none());
    assert_eq!(app.notices.len(), 1);
    assert!(app.notices.front().unwrap().notice.body.contains("corrupt"));
}

#[test]
fn stale_completion_after_rescan_to_empty_is_ignored() {
    let mut app = HonkHonk::new_for_test();
    app.sounds.push(crate::state::SoundEntry {
        id: "old".into(),
        name: "old".into(),
        path: "/tmp/old.wav".into(),
        format: crate::state::AudioFormat::Wav,
        duration_ms: None,
        modified_ms: None,
        category: "test".into(),
    });
    app.start_library_preparation();
    let generation = app.preparation.request.as_ref().unwrap().generation;
    app.sounds.clear();
    app.start_library_preparation();
    app.library_preparation_item(generation, "old".into(), Ok(test_prepared()));
    app.library_preparation_finished(generation, vec![("old".into(), "late".into())]);

    assert!(app.preparation.request.is_none());
    assert!(app.audio_store.get_pcm("old").is_none());
    assert!(app.notices.is_empty());
}

fn test_prepared() -> Arc<crate::audio::preparation::PreparedAudio> {
    let pcm = crate::audio::CachedPcm {
        analysis: Default::default(),
        samples: Arc::new(vec![0.0]),
        sample_rate: 8_000,
        channels: 1,
        duration: std::time::Duration::from_millis(1),
    };
    crate::audio::preparation::wrap_for_test(pcm)
}
