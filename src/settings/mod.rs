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
    fn audio_category_has_no_phase2_entries() {
        let count = SETTINGS_REGISTRY
            .iter()
            .filter(|d| matches!(d.category, SettingCategory::Audio))
            .count();
        assert_eq!(count, 0, "No audio settings wired in Phase 2 shell");
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
}
