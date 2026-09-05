//! Production and persistence-disabled test construction.

use super::*;

impl HonkHonk {
    #[allow(
        clippy::too_many_lines,
        reason = "constructor lists every app state field explicitly to avoid hidden defaults during app split"
    )]
    pub fn new(
        mut tray: TrayHandle,
        audio: AudioHandle,
        scan: LibraryScan,
        config: AppConfig,
        slots: SlotMap,
    ) -> Self {
        let rx = tray.take_rx();
        let sound_meta = library_scan::load_sound_meta(&scan);
        let sound_sort = sorting::sound_sort_from_config(&config);
        let hotkey_sort = hotkeys::hotkey_sort_from_config(&config);
        let slot_sort = slots::slot_sort_from_config(&config);
        let sounds = scan.entries;
        let duration_scan_pairs = std::sync::Arc::new(
            sounds
                .iter()
                .map(|s| (s.id.clone(), s.path.clone()))
                .collect::<Vec<_>>(),
        );
        let mut app = Self {
            import: Default::default(),
            visible: true,
            exit: false,
            tray_rx: Arc::new(Mutex::new(rx)),
            _tray: Some(tray),
            audio: Some(audio),
            sounds,
            playing: None,
            active_category: None,
            config,
            filter: FilterState::default(),
            sound_sort,
            filtered_sound_indices: Vec::new(),
            sort_menu_anchor: None,
            progress: 0.0,
            slots,
            slot_triggers: std::array::from_fn(|_| None),
            hotkey_filter: FilterState::default(),
            slot_filter: FilterState::default(),
            hotkey_sort,
            slot_sort,
            shortcuts_status: ShortcutsStatus::Initializing,
            context_menu: None,
            context_menu_pos: None,
            cursor_pos: Point::ORIGIN,
            window_size: (1280.0, 800.0),
            shortcuts_warning_dismissed: false,
            durations_loaded: false,
            duration_scan_pairs,
            view_mode: ViewMode::default(),
            selected_slot: None,
            settings_ui: crate::settings::search::SettingsUiState::default(),
            monitor_devices: Vec::new(),
            input_devices: Vec::new(),
            shortcut_config: crate::shortcuts::config_ui::ShortcutConfigService::new(),
            notices: NoticeQueue::new(),
            sound_meta,
            persist: true,
            config_load_failed: false,
            editor_sound_id: None,
            editor_draft_name: String::new(),
            editor_draft_tags: String::new(),
            editor_draft_volume: 1.0,
            effects_ui: EffectsUiState::default(),
            effects_panel: PanelAnim::default(),
            panel_flourish: PanelFlourish::default(),
            panel_progress: 0.0,
            now_playing: crate::ui::now_playing::NowPlaying::default(),
            play_generation: 0,
            audio_store: crate::audio::AudioStore::new(crate::audio::DEFAULT_PCM_CAP_BYTES),
            pending_play_ids: HashSet::new(),
            pending_decodes: HashMap::new(),
            macros: crate::state::MacroStore::load(),
            macro_editor: Default::default(),
            recording: None,
            macro_editor_draft: None,
            macro_draft_seq: 0,
            macro_playback: None,
            macro_run_id: 0,
            macro_voice_seq: 0,
        };
        app.refresh_filtered_sounds();
        app
    }

    #[allow(
        clippy::too_many_lines,
        reason = "test constructor mirrors app state fields explicitly so tests do not depend on hidden defaults"
    )]
    pub fn new_for_test() -> Self {
        let (_tx, rx) = std::sync::mpsc::channel();
        let config = AppConfig::default();
        let sound_sort = sorting::sound_sort_from_config(&config);
        let hotkey_sort = hotkeys::hotkey_sort_from_config(&config);
        let slot_sort = slots::slot_sort_from_config(&config);
        let mut app = Self {
            visible: true,
            exit: false,
            tray_rx: Arc::new(Mutex::new(rx)),
            _tray: None,
            audio: None,
            sounds: Vec::new(),
            playing: None,
            active_category: None,
            config,
            filter: FilterState::default(),
            sound_sort,
            filtered_sound_indices: Vec::new(),
            sort_menu_anchor: None,
            progress: 0.0,
            slots: SlotMap::default(),
            slot_triggers: std::array::from_fn(|_| None),
            hotkey_filter: FilterState::default(),
            slot_filter: FilterState::default(),
            hotkey_sort,
            slot_sort,
            shortcuts_status: ShortcutsStatus::Initializing,
            context_menu: None,
            context_menu_pos: None,
            cursor_pos: Point::ORIGIN,
            window_size: (1280.0, 800.0),
            shortcuts_warning_dismissed: false,
            durations_loaded: false,
            duration_scan_pairs: std::sync::Arc::new(Vec::new()),
            view_mode: ViewMode::default(),
            selected_slot: None,
            settings_ui: crate::settings::search::SettingsUiState::default(),
            monitor_devices: Vec::new(),
            input_devices: Vec::new(),
            shortcut_config: crate::shortcuts::config_ui::ShortcutConfigService::new(),
            notices: NoticeQueue::new(),
            sound_meta: SoundMetaStore::default(),
            persist: false,
            config_load_failed: false,
            editor_sound_id: None,
            editor_draft_name: String::new(),
            editor_draft_tags: String::new(),
            editor_draft_volume: 1.0,
            effects_ui: EffectsUiState::default(),
            effects_panel: PanelAnim::default(),
            panel_flourish: PanelFlourish::default(),
            panel_progress: 0.0,
            now_playing: crate::ui::now_playing::NowPlaying::default(),
            play_generation: 0,
            audio_store: crate::audio::AudioStore::new(crate::audio::DEFAULT_PCM_CAP_BYTES),
            pending_play_ids: HashSet::new(),
            pending_decodes: HashMap::new(),
            macros: crate::state::MacroStore::default(),
            macro_editor: Default::default(),
            import: Default::default(),
            recording: None,
            macro_editor_draft: None,
            macro_draft_seq: 0,
            macro_playback: None,
            macro_run_id: 0,
            macro_voice_seq: 0,
        };
        app.refresh_filtered_sounds();
        app
    }
}
