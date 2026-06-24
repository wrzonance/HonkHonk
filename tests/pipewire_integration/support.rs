use std::sync::Arc;

use honkhonk::audio::effects::EffectSettings;
use honkhonk::audio::{AudioCommand, PlayMode};

pub(super) fn play_command(
    sound_id: &str,
    samples: Arc<Vec<f32>>,
    generation: u64,
) -> AudioCommand {
    play_command_with_mode(sound_id, samples, generation, PlayMode::Concurrent)
}

pub(super) fn play_command_with_mode(
    sound_id: &str,
    samples: Arc<Vec<f32>>,
    generation: u64,
    mode: PlayMode,
) -> AudioCommand {
    AudioCommand::Play {
        voice_id: generation,
        sound_id: sound_id.into(),
        samples,
        sample_rate: 48_000,
        channels: 2,
        generation,
        gain: 1.0,
        effects: EffectSettings::default(),
        mode,
    }
}

pub(super) fn play_command_with_format(
    sound_id: &str,
    samples: Arc<Vec<f32>>,
    sample_rate: u32,
    channels: u16,
    generation: u64,
) -> AudioCommand {
    AudioCommand::Play {
        voice_id: generation,
        sound_id: sound_id.into(),
        samples,
        sample_rate,
        channels,
        generation,
        gain: 1.0,
        effects: EffectSettings::default(),
        mode: PlayMode::Concurrent,
    }
}
