use super::*;

fn props(volume: f32, channels: Option<Vec<f32>>, muted: bool) -> Props {
    Props {
        volume: Some(volume),
        channels,
        muted: Some(muted),
    }
}
fn level(volume: f32, muted: bool) -> SourceLevel {
    SourceLevel { volume, muted }
}
fn observed(state: &mut NodeVolume, props: &Props) -> Option<StreamEvent> {
    state.observe(7, &props.encode().unwrap())
}

#[test]
fn teardown_restores_exact_original_props_even_before_own_echo() {
    let original = props(0.8, Some(vec![0.25, 0.125]), false);
    let mut state = NodeVolume::default();
    observed(&mut state, &original);
    state.set_level(level(0.5, true)).unwrap();
    assert_eq!(
        Props::decode(&state.release().unwrap().unwrap()),
        Some(original)
    );
    assert!(state.release().unwrap().is_none());
}

#[test]
fn channel_only_updates_and_slider_writes_share_effective_gain_and_balance() {
    let mut state = NodeVolume::default();
    observed(&mut state, &props(0.8, Some(vec![1.0, 0.5]), false));
    let event = observed(
        &mut state,
        &Props {
            channels: Some(vec![0.25, 0.125]),
            ..Default::default()
        },
    );
    assert_eq!(
        event,
        Some(StreamEvent::SourceLevelChanged {
            id: 7,
            volume: Some(0.2),
            muted: Some(false),
        })
    );
    let written = Props::decode(&state.set_level(level(0.5, false)).unwrap()).unwrap();
    assert_eq!(written.volume, Some(1.0));
    assert_eq!(written.channels, Some(vec![0.5, 0.25]));
}

#[test]
fn external_changes_supersede_restore_but_queued_own_echoes_do_not() {
    let original = props(0.4, None, false);
    let mut state = NodeVolume::default();
    observed(&mut state, &original);
    let first = state.set_level(level(0.5, false)).unwrap();
    state.set_level(level(0.6, true)).unwrap();
    state.observe(7, &first);
    assert_eq!(
        Props::decode(&state.release().unwrap().unwrap()),
        Some(original.clone())
    );
    observed(&mut state, &original);
    state.set_level(level(0.6, true)).unwrap();
    observed(&mut state, &props(0.3, None, false));
    assert!(state.release().unwrap().is_none());
}

#[test]
fn scalar_fallback_and_unknown_state_are_safe() {
    let mut state = NodeVolume::default();
    assert!(state.set_level(level(0.5, true)).is_err());
    observed(&mut state, &props(0.4, None, true));
    let written = Props::decode(&state.set_level(level(0.5, false)).unwrap()).unwrap();
    assert_eq!(written, props(0.5, None, false));
}

#[test]
fn malformed_channels_do_not_replace_known_state() {
    let mut state = NodeVolume::default();
    observed(&mut state, &props(1.0, Some(vec![0.4, 0.2]), false));
    for channels in [
        vec![],
        vec![f32::NAN],
        vec![f32::INFINITY],
        vec![-0.1],
        vec![1.0; 65],
    ] {
        assert!(
            observed(
                &mut state,
                &Props {
                    channels: Some(channels),
                    ..Default::default()
                }
            )
            .is_none()
        );
    }
    let written = Props::decode(&state.set_level(level(0.8, false)).unwrap()).unwrap();
    assert_eq!(written.channels, Some(vec![0.8, 0.4]));
}
