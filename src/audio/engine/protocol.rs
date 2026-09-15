use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayMode {
    Concurrent,
    Interrupt,
}

#[derive(Debug, Clone)]
pub enum AudioCommand {
    Play {
        processing: crate::audio::processing::VoiceProcessing,
        voice_id: u64,
        sound_id: String,
        samples: Arc<Vec<f32>>,
        sample_rate: u32,
        channels: u16,
        /// Monotonic token identifying this specific play. Echoed back on the
        /// matching `PlaybackFinished` so the app can tell a genuine end from the
        /// stale `Finished` emitted for a voice that was immediately superseded by
        /// a re-press of the same sound (#149).
        generation: u64,
        /// Per-sound volume multiplier, applied alongside the master volume in
        /// `PlaybackState`. Lets the app send the canonical (pre-volume) PCM Arc
        /// without an O(n) copy per play (#151).
        gain: f32,
        effects: EffectSettings,
        mode: PlayMode,
    },
    StopVoice(u64),
    Stop,
    SetVolume(f32),
    SetDynamics(crate::audio::processing::DynamicsSettings),
    SetMicPassthrough(bool),
    SetMicPassthroughLevel(f32),
    SetMonitorDevice(Option<String>),
    /// Select the microphone (input) source to pass through. `None` = Auto
    /// (system default, excluding HonkHonk's own virtual source).
    SetInputDevice(Option<String>),
    Router(crate::audio::router::RouterCommand),
    Shutdown,
    /// Set bypass state for the effect at `index` in the mixer chain.
    SetEffectBypass {
        index: usize,
        bypass: bool,
    },
    /// Set a parameter on the effect at `index`.
    SetEffectParam {
        index: usize,
        param: String,
        value: f32,
    },
    /// Set the chain-level wet/dry mix.
    SetEffectWetDry(f32),
    /// Set chain-level bypass.
    SetEffectChainBypass(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AudioEvent {
    Ready,
    PlaybackStarted {
        sound_id: String,
        /// Echoes the `generation` of the `Play` this voice came from, mirroring
        /// `PlaybackFinished`. Lets the app ignore a late superseded voice's
        /// Started so it cannot re-highlight an old tile while the UI is idle
        /// (#149/#164).
        generation: u64,
    },
    PlaybackFinished {
        voice_id: u64,
        sound_id: String,
        /// Echoes the `generation` of the `Play` this voice came from, so a stale
        /// `Finished` for a superseded voice can be distinguished from a genuine
        /// end (#149).
        generation: u64,
    },
    Progress(f32),
    Error(EngineErrorEvent),
    OutputDevicesChanged(Vec<(String, String)>),
    /// The set of real microphone (input) sources changed; carries
    /// (node_name, display_name) for each, to populate the input-device picker.
    InputDevicesChanged(Vec<(String, String)>),
    /// Emitted once on a first run that created the source programmatically and
    /// wrote the per-user conf.d. The UI shows a one-time notice telling the
    /// user the "HonkHonk Mic" device now persists and to select it in
    /// Discord/OBS. Carries whether a new conf.d file was actually written.
    SourceFirstRun {
        confd_written: bool,
    },
    /// The effect chain's total latency changed (in samples).
    EffectsLatencyChanged(u32),
}
