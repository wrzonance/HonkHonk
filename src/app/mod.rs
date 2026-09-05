use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use iced::widget::{button, container, row, scrollable, space, text};
use iced::{Element, Length, Point, Subscription, Task, Theme};

#[cfg(test)]
use crate::audio::effects::EffectSlot;
use crate::audio::{AudioCommand, AudioEvent, AudioHandle, PlayMode};
use crate::shortcuts::ShortcutsStatus;
use crate::state::config::OverlapMode;
use crate::state::{AppConfig, LibraryScan, SlotMap, SoundEntry, SoundMetaStore};
use crate::tray::{TrayEvent, TrayHandle};
#[cfg(test)]
use crate::ui::effects_panel::PresetId;
use crate::ui::effects_panel::{self, EffectsUiState};
use crate::ui::effects_panel_view;
use crate::ui::list_controls::filter::FilterState;
use crate::ui::side_panel::{PanelAnim, PanelFlourish};
use crate::ui::sound_grid;
use crate::ui::theme::{self, Hh};
use crate::ui::{now_playing, slot_manager};
use notices::{Notice, NoticeQueue};

// Keep construction, routing, transitions and layout in cohesive child modules.
mod audio_events;
mod construction;
mod filtering;
mod header;
/// Settings → Shortcuts bindings list sort state (#199).
mod hotkeys;
mod library_actions;
mod library_scan;
mod lifecycle;
mod macros;
mod message;
#[cfg(test)]
mod notice_tests;
pub(crate) mod notices;
mod panels;
mod playback;
mod preferences;
mod recording;
mod settings;
mod slot_sort;
mod slots;
mod sorting;
mod sound_metadata;
mod subscriptions;
#[cfg(test)]
mod tests;
mod update;
mod view;
mod window_actions;

use library_actions::pick_directory;

/// Bridges `HotkeyRow` across the module-tree boundary between `crate::app`
/// (which owns the state it's built from) and `crate::ui::settings::hotkeys`
/// (a sibling tree that renders it) — see `hotkeys.rs`'s module doc.
pub(crate) use hotkeys::HotkeyRow;
pub use message::Message;
pub use settings::SettingsMessage;

/// Virtual category name used for the Favorites filtered tab.
pub const FAVORITES_TAB: &str = "\u{2605} Favorites";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Main,
    SlotManager,
    Settings,
}

pub use crate::settings::SettingCategory as SettingsSection;

/// Smallest window dimension treated as a real, usable size. Resize events
/// below it (some compositors emit 0-size on minimize) are not recorded, and
/// restored sizes are floored to it so a bad config cannot launch an
/// invisible window.
pub const MIN_WINDOW_DIMENSION: f32 = 200.0;

pub struct HonkHonk {
    visible: bool,
    exit: bool,
    tray_rx: Arc<Mutex<Receiver<TrayEvent>>>,
    _tray: Option<TrayHandle>,
    audio: Option<AudioHandle>,
    pub(crate) sounds: Vec<SoundEntry>,
    playing: Option<String>,
    active_category: Option<String>,
    pub(crate) config: AppConfig,
    filter: FilterState,
    sound_sort: sorting::SoundSortState,
    filtered_sound_indices: Vec<usize>,
    sort_menu_anchor: Option<Point>,
    progress: f32,
    slots: SlotMap,
    pub(crate) slot_triggers: [Option<String>; 20],
    /// Settings → Shortcuts bindings list filter query (#199). Independent of
    /// the main grid's `filter` — each list-controls view owns its own state.
    #[allow(
        dead_code,
        reason = "read by hotkeys::hotkey_filter_query/hotkey_rows; wired into a view by a follow-up task in this issue's task chain"
    )]
    hotkey_filter: FilterState,
    /// Slot manager's own search-bar filter query (#198). Independent of the
    /// main grid's `filter` and Settings' `hotkey_filter` — each list-controls
    /// view owns its own state.
    slot_filter: FilterState,
    /// Settings → Shortcuts bindings list sort state (#199).
    #[allow(
        dead_code,
        reason = "read by hotkeys::hotkey_sort_state/hotkey_rows; wired into a view by a follow-up task in this issue's task chain"
    )]
    hotkey_sort: hotkeys::HotkeySortState,
    /// Slot manager's own sort state (#198). Independent of `hotkey_sort` and
    /// the tiles view's `sound_sort` — each list-controls view owns its own
    /// state. Unlike `hotkey_sort`, wired into a `Message`-driven mutator and
    /// view within this same issue's task chain, so it carries no
    /// `dead_code` allowance.
    slot_sort: slots::SlotSortState,
    pub(crate) shortcuts_status: ShortcutsStatus,
    context_menu: Option<String>,
    context_menu_pos: Option<Point>,
    cursor_pos: Point,
    window_size: (f32, f32),
    shortcuts_warning_dismissed: bool,
    durations_loaded: bool,
    duration_scan_pairs: std::sync::Arc<Vec<(String, std::path::PathBuf)>>,
    view_mode: ViewMode,
    selected_slot: Option<u8>,
    pub(crate) settings_ui: crate::settings::search::SettingsUiState,
    pub monitor_devices: Vec<(String, String)>,
    pub input_devices: Vec<(String, String)>,
    shortcut_config: crate::shortcuts::config_ui::ShortcutConfigService,
    /// User-visible in-window notices raised from app/audio events.
    notices: NoticeQueue,
    /// Per-sound metadata: favorites, per-sound volume, display names.
    pub(crate) sound_meta: SoundMetaStore,
    /// Master persistence switch. When `false`, every disk write —
    /// `config.save()`, `slots.save()`, `sound_meta.save()` — is skipped. Test
    /// fixtures (`new_for_test`) set this `false` so `cargo test` never
    /// overwrites the developer's real XDG config dir (config.json, slots.json,
    /// meta.json).
    persist: bool,
    /// Set when startup could not load the on-disk config (I/O or parse
    /// error): the in-memory state is then bare defaults, and a quit-time
    /// save would overwrite the user's repairable file with them.
    config_load_failed: bool,
    /// Sound ID currently open in the per-sound editor overlay.
    editor_sound_id: Option<String>,
    /// Draft display name held while the editor is open.
    editor_draft_name: String,
    /// Draft per-sound volume held while the editor is open.
    editor_draft_volume: f32,
    /// User-facing voice-effects state (preset, bypass, wet/dry, params).
    effects_ui: EffectsUiState,
    /// Open/close animation state for the effects side panel (#143). Logic lives
    /// in `ui::side_panel`.
    effects_panel: PanelAnim,
    /// Reusable panel open/close feather burst overlay (#144).
    panel_flourish: PanelFlourish,
    /// Eased panel progress (0=closed..1=open) fed to the view; refreshed each
    /// frame by `effects_panel.tick`.
    panel_progress: f32,
    /// Persistent now-playing playback UI owner (#142): playhead lifecycle,
    /// display progress, waveform cache key, and per-sound envelopes.
    now_playing: crate::ui::now_playing::NowPlaying,
    /// Monotonic counter bumped on every play dispatch. Stamped onto the `Play`
    /// command and echoed back on `PlaybackFinished` to tell a genuine end from
    /// the stale `Finished` of a re-pressed voice (#149), and onto each
    /// off-thread decode so the latest same-id cold repeat can claim ownership
    /// when the shared decode lands (#151/#152).
    play_generation: u64,
    /// Hot-path decoded-PCM cache (#151).
    audio_store: crate::audio::AudioStore,
    pending_play_ids: HashSet<u64>,
    pending_decodes: HashMap<String, playback::PendingDecode>,
    /// Persisted macro collection (#165).
    macros: crate::state::MacroStore,
    /// Active live macro capture, if recording is enabled (#167).
    recording: Option<recording::Recording>,
    /// Unsaved macro draft produced by `StopRecording`, ready for #168's editor
    /// buffer to adopt.
    macro_editor_draft: Option<crate::state::Macro>,
    /// Session-local counter used for draft names/ids. Unsaved drafts are not
    /// inserted into `MacroStore`; #168 owns keep/discard persistence.
    macro_draft_seq: u64,
    /// The single in-flight macro run, if any — `Some` enforces one macro at a
    /// time (#166). `None` when idle.
    macro_playback: Option<macros::MacroPlayback>,
    /// Monotonic run counter; a `MacroStepDue`/`MacroStepDecoded` for a run that
    /// is no longer current is ignored (re-fire / Stop All cancellation).
    macro_run_id: u64,
    /// Per-voice counter for macro steps. Combined with a top-bit flag into a
    /// voice-id space disjoint from the tile `play_generation`, so a macro firing
    /// mid-tile-press never advances (and corrupts) the tile's now-playing UI
    /// ownership (#166).
    macro_voice_seq: u64,
}

impl HonkHonk {
    pub fn should_exit(&self) -> bool {
        self.exit
    }

    /// Marks the on-disk config as having failed to load at startup, which
    /// disables the quit-time config save for the session: the in-memory
    /// defaults must not clobber the user's repairable file.
    pub fn mark_config_load_failed(&mut self) {
        self.config_load_failed = true;
    }

    /// The quit save is gated on a live audio engine so unit-test fixtures
    /// (`audio: None`) never write the user's real config file, and on the
    /// config having loaded cleanly at startup. The `persist` master switch
    /// applies on top, inside `persist_config`.
    fn should_persist_config_on_quit(&self) -> bool {
        self.audio.is_some() && !self.config_load_failed
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn playing(&self) -> Option<&str> {
        self.playing.as_deref()
    }

    pub fn active_category(&self) -> Option<&str> {
        self.active_category.as_deref()
    }

    /// Route effects commands to the audio thread, no-op when no engine is up.
    fn send_audio_commands(&self, cmds: impl IntoIterator<Item = AudioCommand>) {
        if let Some(ref audio) = self.audio {
            for cmd in cmds {
                audio.send(cmd);
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn effects_ui_preset(&self) -> PresetId {
        self.effects_ui.preset
    }

    #[cfg(test)]
    pub(crate) fn effects_ui_wet_dry(&self) -> f32 {
        self.effects_ui.wet_dry
    }

    #[cfg(test)]
    pub(crate) fn effects_ui_chain_bypass(&self) -> bool {
        self.effects_ui.chain_bypass
    }

    pub fn search_query(&self) -> &str {
        self.filter.query()
    }

    pub fn progress(&self) -> f32 {
        self.progress
    }

    pub fn shortcuts_status(&self) -> &ShortcutsStatus {
        &self.shortcuts_status
    }

    pub fn slots(&self) -> &SlotMap {
        &self.slots
    }

    pub fn slot_triggers(&self) -> &[Option<String>; 20] {
        &self.slot_triggers
    }

    pub fn context_menu(&self) -> Option<&str> {
        self.context_menu.as_deref()
    }

    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    pub fn selected_slot(&self) -> Option<u8> {
        self.selected_slot
    }

    pub fn shortcuts_warning_dismissed(&self) -> bool {
        self.shortcuts_warning_dismissed
    }

    pub(crate) fn notices(&self) -> &NoticeQueue {
        &self.notices
    }

    pub fn sound_meta(&self) -> &SoundMetaStore {
        &self.sound_meta
    }

    pub fn editor_sound_id(&self) -> Option<&str> {
        self.editor_sound_id.as_deref()
    }
}
