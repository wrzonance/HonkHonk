use super::*;

#[test]
fn audio_command_set_mic_passthrough_is_constructible() {
    let _ = AudioCommand::SetMicPassthrough(true);
    let _ = AudioCommand::SetMicPassthrough(false);
}

#[test]
fn audio_command_set_mic_passthrough_level_is_constructible() {
    let _ = AudioCommand::SetMicPassthroughLevel(0.5);
}

#[test]
fn audio_command_set_monitor_device_none_is_constructible() {
    let _ = AudioCommand::SetMonitorDevice(None);
}

#[test]
fn audio_command_set_monitor_device_some_is_constructible() {
    let _ = AudioCommand::SetMonitorDevice(Some("alsa_output.pci-test".into()));
}

#[test]
fn audio_command_set_input_device_none_is_constructible() {
    let _ = AudioCommand::SetInputDevice(None);
}

#[test]
fn audio_command_set_input_device_some_is_constructible() {
    let _ = AudioCommand::SetInputDevice(Some("alsa_input.pci-test".into()));
}

#[test]
fn audio_command_polyphonic_play_is_constructible() {
    let _ = AudioCommand::Play {
        processing: Default::default(),
        voice_id: 42,
        sound_id: "test".into(),
        samples: Arc::new(vec![0.0_f32; 8]),
        sample_rate: 48_000,
        channels: 1,
        generation: 7,
        gain: 0.8,
        effects: crate::audio::effects::EffectSettings::default(),
        mode: PlayMode::Concurrent,
    };
}

#[test]
fn audio_command_stop_voice_is_constructible() {
    let _ = AudioCommand::StopVoice(42);
}

#[test]
fn playback_finished_carries_voice_id() {
    let event = AudioEvent::PlaybackFinished {
        voice_id: 42,
        sound_id: "test".into(),
        generation: 7,
    };
    assert!(matches!(
        event,
        AudioEvent::PlaybackFinished {
            voice_id: 42,
            generation: 7,
            ..
        }
    ));
}

#[test]
fn should_create_source_false_when_node_already_present() {
    assert!(!should_create_source(true));
}

#[test]
fn parse_source_present_detects_honkhonk_mic() {
    let dump = r#"
        id 42, type PipeWire:Interface:Node/3
            node.name = "honkhonk-mic"
            media.class = "Audio/Source/Virtual"
        "#;
    assert!(source_present_in_dump(dump));
}

#[test]
fn parse_source_present_detects_honkhonk_mic_pw_dump_json() {
    let dump = r#"
        {
          "props": {
            "node.name": "honkhonk-mic",
            "media.class": "Audio/Source/Virtual"
          }
        }
        "#;
    assert!(source_present_in_dump(dump));
}

#[test]
fn parse_source_present_false_when_absent() {
    let dump = r#"
        id 7, type PipeWire:Interface:Node/3
            node.name = "alsa_input.pci-0000"
        "#;
    assert!(!source_present_in_dump(dump));
}

#[test]
fn parse_source_present_false_on_empty() {
    assert!(!source_present_in_dump(""));
}

#[test]
fn parse_source_present_false_on_substring_node_name() {
    // A different node whose name merely contains our name must not match.
    let dump = r#"node.name = "honkhonk-mic-monitor""#;
    assert!(!source_present_in_dump(dump));
}

#[test]
fn should_create_source_true_when_node_absent() {
    assert!(should_create_source(false));
}

#[test]
fn audio_event_source_first_run_is_constructible() {
    let _ = AudioEvent::SourceFirstRun {
        confd_written: true,
    };
    let _ = AudioEvent::SourceFirstRun {
        confd_written: false,
    };
}

#[test]
fn audio_event_output_devices_changed_is_constructible() {
    let _ = AudioEvent::OutputDevicesChanged(vec![(
        "alsa_output.pci-test".into(),
        "Built-in Audio".into(),
    )]);
}

#[test]
fn audio_command_set_effect_bypass_is_constructible() {
    let _ = AudioCommand::SetEffectBypass {
        index: 0,
        bypass: true,
    };
}

#[test]
fn audio_command_set_effect_wet_dry_is_constructible() {
    let _ = AudioCommand::SetEffectWetDry(0.5);
}

#[test]
fn audio_event_effects_latency_changed_is_constructible() {
    let _ = AudioEvent::EffectsLatencyChanged(512);
}

#[test]
fn audio_command_router_variant_is_constructible() {
    use crate::audio::router::RouterCommand;
    let _ = AudioCommand::Router(RouterCommand::UnrouteAll);
    let _ = AudioCommand::Router(RouterCommand::RouteSource { source_node_id: 1 });
    let _ = AudioCommand::Router(RouterCommand::UnrouteSource { source_node_id: 1 });
}

#[test]
fn channel_preparation_retains_original_pcm_and_source_format() {
    let samples = Arc::new(vec![0.6, 0.2, 0.4, 0.0]);
    let request = PlayRequest {
        processing: crate::audio::processing::VoiceProcessing {
            sound: crate::audio::processing::SoundProcessing {
                output: crate::audio::processing::OutputMode::Mono,
                pan: f32::NAN,
                ..Default::default()
            },
            ..Default::default()
        },
        voice_id: 1,
        sound_id: "source".into(),
        samples: Arc::clone(&samples),
        sample_rate: 48_000,
        channels: 2,
        generation: 1,
        gain: 1.0,
        effects: Default::default(),
        mode: PlayMode::Concurrent,
    };
    let prepared = prepare_channels(request);
    assert_eq!(prepared.processing.sound.pan, 0.0);
    assert!(
        Arc::ptr_eq(&samples, &prepared.samples),
        "play command must not copy PCM"
    );
    assert_eq!(
        prepared.channels, 2,
        "retain source stride for streaming conversion"
    );
}
