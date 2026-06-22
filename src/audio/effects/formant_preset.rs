//! Named presets for the [`FormantPitchEffect`](super::FormantPitchEffect).
//!
//! Each preset maps to an independent `(pitch_ratio, formant_ratio)` pair, both
//! linear multipliers (`1.0` = unchanged). Pitch moves harmonics; formant moves
//! the vocal-tract resonance envelope. Keeping them independent is what makes a
//! pitch change sound natural instead of "chipmunk"/"barrel".

/// A named, opinionated formant-pitch setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormantPreset {
    /// Formant shift only (pitch unchanged): eerie, non-human timbre.
    Alien,
    /// Pitch up + formants raised toward a higher-voiced character.
    GenderSwap,
    /// Pitch lowered with formants preserved — deep but natural, no barrel effect.
    NaturalDeep,
}

impl FormantPreset {
    /// Linear pitch multiplier (`1.0` = unchanged, `2.0` = +1 octave).
    pub fn pitch_ratio(self) -> f32 {
        match self {
            FormantPreset::Alien => 1.0,
            FormantPreset::GenderSwap => 1.5,
            FormantPreset::NaturalDeep => 0.75,
        }
    }

    /// Linear formant (envelope) multiplier (`1.0` = formants preserved).
    pub fn formant_ratio(self) -> f32 {
        match self {
            FormantPreset::Alien => 1.6,
            FormantPreset::GenderSwap => 1.25,
            FormantPreset::NaturalDeep => 1.0,
        }
    }

    /// Stable identifier string for config/UI.
    pub fn name(self) -> &'static str {
        match self {
            FormantPreset::Alien => "alien",
            FormantPreset::GenderSwap => "gender_swap",
            FormantPreset::NaturalDeep => "natural_deep",
        }
    }

    /// All presets, in display order.
    pub fn all() -> [FormantPreset; 3] {
        [
            FormantPreset::Alien,
            FormantPreset::GenderSwap,
            FormantPreset::NaturalDeep,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alien_shifts_formants_only() {
        assert_eq!(FormantPreset::Alien.pitch_ratio(), 1.0);
        assert!(FormantPreset::Alien.formant_ratio() > 1.0);
    }

    #[test]
    fn gender_swap_shifts_both() {
        assert!(FormantPreset::GenderSwap.pitch_ratio() > 1.0);
        assert!(FormantPreset::GenderSwap.formant_ratio() > 1.0);
    }

    #[test]
    fn natural_deep_lowers_pitch_preserves_formants() {
        assert!(FormantPreset::NaturalDeep.pitch_ratio() < 1.0);
        assert_eq!(FormantPreset::NaturalDeep.formant_ratio(), 1.0);
    }

    #[test]
    fn names_are_stable() {
        assert_eq!(FormantPreset::Alien.name(), "alien");
        assert_eq!(FormantPreset::GenderSwap.name(), "gender_swap");
        assert_eq!(FormantPreset::NaturalDeep.name(), "natural_deep");
    }

    #[test]
    fn all_returns_three_presets() {
        assert_eq!(FormantPreset::all().len(), 3);
    }
}
