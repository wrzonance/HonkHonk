/// Central settings registry — pure metadata, zero coupling to app state.
/// To add a new setting: add to SettingId, add SettingDef to SETTINGS_REGISTRY,
/// add arms to get_setting_value and setting_message in src/ui/settings.rs,
/// add Message variant + update() handler in src/app.rs.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingId {
    // Library — wired Phase 2
    RescanLibrary,
    // Appearance — wired when backends land (issues #69, #70)
    Theme,
    Density,
    // Audio — wired when backends land (issues #71, #72)
    MicPassthrough,
    MicPassthroughLevel,
    OverlapMode,
    MonitorDevice,
    // App — wired when backend lands (issue #73)
    Renderer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingCategory {
    Audio,
    Library,
    Hotkeys,
    Appearance,
    About,
}

#[derive(Debug, Clone, Copy)]
pub enum ControlType {
    Toggle,
    Radio(&'static [&'static str]),
    Slider { min: f32, max: f32, step: f32 },
    Button,
    Select,
}

#[derive(Debug, Clone, Copy)]
pub enum SettingValue {
    Bool(bool),
    Index(usize),
    F32(f32),
    None,
}

pub struct SettingDef {
    pub id: SettingId,
    pub category: SettingCategory,
    pub label: &'static str,
    pub hint: &'static str,
    pub control: ControlType,
}

pub static SETTINGS_REGISTRY: &[SettingDef] = &[
    SettingDef {
        id: SettingId::Theme,
        category: SettingCategory::Appearance,
        label: "Theme",
        hint: "Light, Dark, or follow your desktop environment.",
        control: ControlType::Radio(&["Light", "Dark", "System"]),
    },
    SettingDef {
        id: SettingId::Density,
        category: SettingCategory::Appearance,
        label: "Grid density",
        hint: "Number of tiles per row.",
        control: ControlType::Radio(&["Compact", "Regular", "Comfy"]),
    },
    SettingDef {
        id: SettingId::Renderer,
        category: SettingCategory::Appearance,
        label: "GPU acceleration",
        hint: "Disable for VMs or older hardware. Takes effect after restart.",
        control: ControlType::Toggle,
    },
    SettingDef {
        id: SettingId::MicPassthrough,
        category: SettingCategory::Audio,
        label: "Mic passthrough",
        hint: "Mix your real mic into the virtual mic.",
        control: ControlType::Toggle,
    },
    SettingDef {
        id: SettingId::MicPassthroughLevel,
        category: SettingCategory::Audio,
        label: "Passthrough level",
        hint: "Mic gain into virtual mic. Audio effect lands in issue #29.",
        control: ControlType::Slider {
            min: 0.0,
            max: 1.0,
            step: 0.01,
        },
    },
    SettingDef {
        id: SettingId::OverlapMode,
        category: SettingCategory::Audio,
        label: "Overlap mode",
        hint: "Whether tile presses layer sounds or interrupt the current voice.",
        control: ControlType::Radio(&["Concurrent", "Interrupt"]),
    },
    SettingDef {
        id: SettingId::RescanLibrary,
        category: SettingCategory::Library,
        label: "Scan now",
        hint: "Force a re-scan of all sound folders.",
        control: ControlType::Button,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rescan_library_entry_exists_in_registry() {
        let found = SETTINGS_REGISTRY
            .iter()
            .any(|d| matches!(d.id, SettingId::RescanLibrary));
        assert!(found, "RescanLibrary must be in SETTINGS_REGISTRY");
    }

    #[test]
    fn rescan_library_is_in_library_category() {
        let def = SETTINGS_REGISTRY
            .iter()
            .find(|d| matches!(d.id, SettingId::RescanLibrary))
            .expect("RescanLibrary must exist");
        assert!(matches!(def.category, SettingCategory::Library));
    }

    #[test]
    fn rescan_library_control_is_button() {
        let def = SETTINGS_REGISTRY
            .iter()
            .find(|d| matches!(d.id, SettingId::RescanLibrary))
            .expect("RescanLibrary must exist");
        assert!(matches!(def.control, ControlType::Button));
    }

    #[test]
    fn mic_passthrough_entry_exists_in_audio_category() {
        let def = SETTINGS_REGISTRY
            .iter()
            .find(|d| matches!(d.id, SettingId::MicPassthrough))
            .expect("MicPassthrough must be in SETTINGS_REGISTRY");
        assert!(matches!(def.category, SettingCategory::Audio));
    }

    #[test]
    fn mic_passthrough_control_is_toggle() {
        let def = SETTINGS_REGISTRY
            .iter()
            .find(|d| matches!(d.id, SettingId::MicPassthrough))
            .expect("MicPassthrough must be in SETTINGS_REGISTRY");
        assert!(matches!(def.control, ControlType::Toggle));
    }

    #[test]
    fn mic_passthrough_level_entry_exists_in_audio_category() {
        let def = SETTINGS_REGISTRY
            .iter()
            .find(|d| matches!(d.id, SettingId::MicPassthroughLevel))
            .expect("MicPassthroughLevel must be in SETTINGS_REGISTRY");
        assert!(matches!(def.category, SettingCategory::Audio));
    }

    #[test]
    fn mic_passthrough_level_control_is_slider() {
        let def = SETTINGS_REGISTRY
            .iter()
            .find(|d| matches!(d.id, SettingId::MicPassthroughLevel))
            .expect("MicPassthroughLevel must be in SETTINGS_REGISTRY");
        assert!(
            matches!(
                def.control,
                ControlType::Slider { min, max, step }
                    if (min - 0.0).abs() < f32::EPSILON
                        && (max - 1.0).abs() < f32::EPSILON
                        && (step - 0.01).abs() < f32::EPSILON
            ),
            "MicPassthroughLevel must be Slider(min=0.0, max=1.0, step=0.01)"
        );
    }

    #[test]
    fn audio_category_has_two_entries() {
        let count = SETTINGS_REGISTRY
            .iter()
            .filter(|d| matches!(d.category, SettingCategory::Audio))
            .count();
        assert_eq!(count, 3, "Audio section must include overlap mode");
    }

    #[test]
    fn theme_entry_exists_in_appearance_category() {
        let def = SETTINGS_REGISTRY
            .iter()
            .find(|d| matches!(d.id, SettingId::Theme))
            .expect("Theme must be in SETTINGS_REGISTRY");
        assert!(matches!(def.category, SettingCategory::Appearance));
        assert!(matches!(def.control, ControlType::Radio(_)));
    }

    #[test]
    fn density_entry_exists_in_appearance_category() {
        let def = SETTINGS_REGISTRY
            .iter()
            .find(|d| matches!(d.id, SettingId::Density))
            .expect("Density must be in SETTINGS_REGISTRY");
        assert!(matches!(def.category, SettingCategory::Appearance));
    }

    #[test]
    fn density_control_is_radio_with_three_options() {
        let def = SETTINGS_REGISTRY
            .iter()
            .find(|d| matches!(d.id, SettingId::Density))
            .expect("Density must be in SETTINGS_REGISTRY");
        assert!(
            matches!(def.control, ControlType::Radio(opts) if opts.len() == 3),
            "Density must be Radio with 3 options"
        );
    }

    #[test]
    fn renderer_entry_exists_in_appearance_category() {
        let def = SETTINGS_REGISTRY
            .iter()
            .find(|d| matches!(d.id, SettingId::Renderer))
            .expect("Renderer must be in SETTINGS_REGISTRY");
        assert!(matches!(def.category, SettingCategory::Appearance));
    }

    #[test]
    fn renderer_control_is_toggle() {
        let def = SETTINGS_REGISTRY
            .iter()
            .find(|d| matches!(d.id, SettingId::Renderer))
            .expect("Renderer must be in SETTINGS_REGISTRY");
        assert!(matches!(def.control, ControlType::Toggle));
    }

    #[test]
    fn appearance_category_has_three_entries() {
        let count = SETTINGS_REGISTRY
            .iter()
            .filter(|d| matches!(d.category, SettingCategory::Appearance))
            .count();
        assert_eq!(count, 3, "Appearance must have Theme + Density + Renderer");
    }
}
