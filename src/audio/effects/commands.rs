/// Commands sent from the UI/main thread to the audio thread to control effects.
/// Variants correspond to operations in `EffectChain`.
#[derive(Debug, Clone, PartialEq)]
pub enum EffectsCommand {
    /// Set bypass state for effect at `index`.
    SetBypass { index: usize, bypass: bool },
    /// Set a named parameter on effect at `index`.
    SetParam {
        index: usize,
        param: String,
        value: f32,
    },
    /// Set the chain-level wet/dry mix. `0.0` = dry only, `1.0` = wet only.
    SetWetDry(f32),
    /// Bypass the entire chain (all effects bypassed simultaneously).
    SetChainBypass(bool),
}

/// Events emitted from the audio thread to the UI about effect state.
#[derive(Debug, Clone, PartialEq)]
pub enum EffectsEvent {
    /// The chain's total latency changed (in samples).
    LatencyChanged(u32),
    /// A parameter set operation was rejected (param name unknown).
    ParamRejected { index: usize, param: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_command_set_bypass_is_constructible() {
        let cmd = EffectsCommand::SetBypass {
            index: 0,
            bypass: true,
        };
        assert!(matches!(
            cmd,
            EffectsCommand::SetBypass {
                index: 0,
                bypass: true
            }
        ));
    }

    #[test]
    fn effects_command_set_wet_dry_is_constructible() {
        let cmd = EffectsCommand::SetWetDry(0.5);
        assert!(matches!(cmd, EffectsCommand::SetWetDry(_)));
    }

    #[test]
    fn effects_event_latency_changed_is_constructible() {
        let evt = EffectsEvent::LatencyChanged(512);
        assert_eq!(evt, EffectsEvent::LatencyChanged(512));
    }

    #[test]
    fn effects_event_param_rejected_is_constructible() {
        let evt = EffectsEvent::ParamRejected {
            index: 1,
            param: "gain".into(),
        };
        assert!(matches!(evt, EffectsEvent::ParamRejected { index: 1, .. }));
    }
}
