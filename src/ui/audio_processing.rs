//! Shared controls for global dynamics and per-sound processing.
use crate::app::Message;
use crate::audio::processing::{DynamicsSettings, GlobalProcessing, OutputMode, SoundProcessing};
use iced::{
    Element,
    widget::{checkbox, column, pick_list, slider, text},
};

pub fn global(settings: GlobalProcessing) -> Element<'static, Message> {
    column![
        text("Playback processing").size(18),
        checkbox(settings.normalize).label("Normalize loudness (−18 LUFS)")
            .on_toggle(move |normalize| Message::GlobalProcessingChanged(GlobalProcessing { normalize, ..settings })),
        text("Normalization applies when a sound starts. Maximum boost: 12 dB."),
        dynamics(settings.dynamics).map(move |dynamics| Message::GlobalProcessingChanged(GlobalProcessing { dynamics, ..settings })),
        text("Global dynamics act after all sound voices and effects are mixed. Sample peaks are capped at 98% while the global compressor/limiter is enabled; it does not limit your microphone or other apps."),
    ].spacing(8).into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Normalization {
    Global,
    On,
    Off,
}

impl std::fmt::Display for Normalization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Global => "Use global normalization",
            Self::On => "Normalize this sound",
            Self::Off => "No normalization",
        })
    }
}

pub fn sound(settings: SoundProcessing, loading: bool) -> Element<'static, Message> {
    if loading {
        return text("Reading sound identity… Audio controls will appear when ready.").into();
    }
    let selected = match settings.normalize {
        None => Normalization::Global,
        Some(true) => Normalization::On,
        Some(false) => Normalization::Off,
    };
    column![
        text("Audio controls follow identical file contents, even after a move or rename."),
        pick_list([Normalization::Global, Normalization::On, Normalization::Off], Some(selected), move |n| {
            Message::SoundProcessingChanged(SoundProcessing { normalize: match n { Normalization::Global => None, Normalization::On => Some(true), Normalization::Off => Some(false) }, ..settings })
        }),
        pick_list([OutputMode::Preserve, OutputMode::Mono, OutputMode::Stereo], Some(settings.output), move |output| Message::SoundProcessingChanged(SoundProcessing { output, ..settings })),
        text(format!("Pan: {:.0}% (left − / right +)", settings.pan * 100.0)),
        slider(-1.0..=1.0, settings.pan, move |pan| Message::SoundProcessingChanged(SoundProcessing { pan, ..settings })).step(0.01_f32),
        dynamics(settings.dynamics).map(move |dynamics| Message::SoundProcessingChanged(SoundProcessing { dynamics, ..settings })),
        text("Changes apply on the next play. Nearly silent stereo channels are repaired automatically; active stereo imaging is preserved."),
    ].spacing(8).into()
}

fn dynamics(settings: DynamicsSettings) -> Element<'static, DynamicsSettings> {
    column![
        checkbox(settings.enabled)
            .label("Compressor / limiter")
            .on_toggle(move |enabled| DynamicsSettings {
                enabled,
                ..settings
            }),
        text(format!("Threshold: {:.0} dB", settings.threshold_db)),
        slider(-60.0..=0.0, settings.threshold_db, move |threshold_db| {
            DynamicsSettings {
                threshold_db,
                ..settings
            }
        })
        .step(1.0_f32),
        text(format!("Ratio: {:.1}:1", settings.ratio)),
        slider(1.0..=20.0, settings.ratio, move |ratio| DynamicsSettings {
            ratio,
            ..settings
        })
        .step(0.5_f32),
    ]
    .spacing(6)
    .into()
}
