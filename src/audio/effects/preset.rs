//! Named pitch-shift presets for the [`PitchShiftEffect`](super::PitchShiftEffect).
//!
//! Each preset maps to a fixed semitone shift. Semitones are the source of truth;
//! the approximate pitch factors below are informational (`2^(semitones / 12)`).

/// A named, opinionated pitch-shift setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PitchPreset {
    /// Deep, ominous voice. -5 semitones (~0.75x).
    Deep,
    /// High, fast "chipmunk" voice. +7 semitones (~1.5x).
    Chipmunk,
    /// Light anonymizing shift down. -2 semitones (~0.89x).
    Anonymous,
}

impl PitchPreset {
    /// Semitone shift for this preset. Negative lowers pitch, positive raises it.
    pub fn semitones(self) -> f32 {
        match self {
            PitchPreset::Deep => -5.0,
            PitchPreset::Chipmunk => 7.0,
            PitchPreset::Anonymous => -2.0,
        }
    }

    /// Approximate linear pitch factor for this preset (`2^(semitones / 12)`).
    pub fn approx_factor(self) -> f32 {
        2.0_f32.powf(self.semitones() / 12.0)
    }

    /// Stable identifier string, e.g. for config serialization or UI labels.
    pub fn name(self) -> &'static str {
        match self {
            PitchPreset::Deep => "deep",
            PitchPreset::Chipmunk => "chipmunk",
            PitchPreset::Anonymous => "anonymous",
        }
    }

    /// All presets, in display order.
    pub fn all() -> [PitchPreset; 3] {
        [
            PitchPreset::Deep,
            PitchPreset::Chipmunk,
            PitchPreset::Anonymous,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_is_negative_five_semitones() {
        assert_eq!(PitchPreset::Deep.semitones(), -5.0);
        assert!((PitchPreset::Deep.approx_factor() - 0.7491).abs() < 0.01);
    }

    #[test]
    fn chipmunk_is_plus_seven_semitones() {
        assert_eq!(PitchPreset::Chipmunk.semitones(), 7.0);
        assert!((PitchPreset::Chipmunk.approx_factor() - 1.4983).abs() < 0.01);
    }

    #[test]
    fn anonymous_is_minus_two_semitones() {
        assert_eq!(PitchPreset::Anonymous.semitones(), -2.0);
        assert!((PitchPreset::Anonymous.approx_factor() - 0.8909).abs() < 0.01);
    }

    #[test]
    fn names_are_stable() {
        assert_eq!(PitchPreset::Deep.name(), "deep");
        assert_eq!(PitchPreset::Chipmunk.name(), "chipmunk");
        assert_eq!(PitchPreset::Anonymous.name(), "anonymous");
    }

    #[test]
    fn all_returns_three_presets() {
        assert_eq!(PitchPreset::all().len(), 3);
    }
}
