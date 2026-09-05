//! The Iced `Message` type — every event `HonkHonk::update()` can receive.
//!
//! Extracted out of `mod.rs` (rather than added to it) per CLAUDE.md's
//! "Known violation: `src/app.rs`/`mod.rs` — do NOT add to it, split first."
//! #199 needed five new variants for the Settings → Shortcuts filter/sort
//! chip; this move takes the file-size hit for those additions here instead
//! of compounding the already-frozen `mod.rs`, and gives future features the
//! same escape hatch rather than defaulting back to `mod.rs`.

use std::time::Instant;

use iced::Point;

use crate::audio::effects::EffectSlot;
use crate::audio::{AudioEvent, PlayMode};
use crate::state::config::{Density, OverlapMode};
use crate::tray::TrayEvent;
use crate::ui::effects_panel::PresetId;
use crate::ui::theme;

use super::SettingsMessage;
use super::notices::{Notice, NoticeId};

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    GlobalProcessingChanged(crate::audio::processing::GlobalProcessing),
    SoundProcessingChanged(crate::audio::processing::SoundProcessing),
    AudioFingerprintReady {
        id: String,
        generation: u64,
        result: Result<String, String>,
    },
    Import(super::import::ImportMessage),
    NoOp,
    ShowMacros,
    MacroEditor(super::macro_editor::EditorMessage),
    ToggleVisibility,
    Quit,
    TrayEvent(TrayEvent),
    TrayPoll,
    AudioEvent(AudioEvent),
    RaiseNotice(Notice),
    DismissNotice(NoticeId),
    NoticeTick(Instant),
    PlaySound(String),
    StopAll,
    StartRecording,
    StopRecording,
    /// Fire macro by id (slots call this in #169).
    PlayMacro(String),
    /// A scheduled macro step's timer elapsed; dispatch it if its run is current.
    MacroStepDue {
        run_id: u64,
        step: usize,
    },
    /// A cold macro step's off-thread decode finished.
    MacroStepDecoded {
        run_id: u64,
        voice_id: u64,
        sound_id: String,
        gain: f32,
        effects: crate::audio::effects::EffectSettings,
        result: Result<crate::audio::CachedPcm, String>,
    },
    SelectCategory(Option<String>),
    SearchChanged(String),
    ToggleSoundSortMenu,
    ToggleSoundSortDirection,
    SelectSoundSort(&'static str),
    ToggleSoundTagGrouping,
    DismissSoundSortMenu,
    // Settings → Shortcuts list controls (#199): filter query and sort chip,
    // independent of the tiles view's messages above.
    HotkeySearchChanged(String),
    ToggleHotkeySortMenu,
    ToggleHotkeySortDirection,
    SelectHotkeySort(&'static str),
    DismissHotkeySortMenu,
    // Slot manager list controls (#198): filter query and sort chip,
    // independent of the tiles view's and Shortcuts list's messages above.
    SlotSearchChanged(String),
    ToggleSlotSortMenu,
    ToggleSlotSortDirection,
    SelectSlotSort(&'static str),
    DismissSlotSortMenu,
    /// Seeds the active filter from an otherwise-unhandled printable keypress.
    TypeToFilter(String),
    /// Routes an uncaptured Escape through overlay and filter staging.
    EscapePressed,
    /// Routes a widget-captured Escape without clearing the filter query.
    CapturedEscapePressed,
    VolumeChanged(f32),
    VolumeSaveRequested,
    // Shortcut lifecycle
    ShortcutsReady,
    ShortcutsUnavailable(String),
    DismissShortcutsWarning,
    // Shortcut activation
    ShortcutActivated(u8),
    ShortcutBindingsUpdated(Vec<(u8, String)>),
    // Duration scanning
    DurationsLoaded(std::collections::HashMap<String, u64>),
    // Slot assignment
    AssignSlot(u8, std::path::PathBuf),
    /// Binds a shortcut slot to a macro id instead of a sound (#169).
    AssignMacroSlot(u8, String),
    ClearSlot(u8),
    // Context menu
    OpenContextMenu(String), // sound_id
    CloseContextMenu,
    // Window / cursor
    CursorMoved(Point),
    WindowResized(f32, f32),
    /// Per-frame redraw tick (vsync-paced via `window::frames()`), carrying the
    /// frame time. Only subscribed while a sound plays. Drives playhead interpolation.
    Frame(Instant),
    // Navigation
    ShowSlots,
    ShowMain,
    SelectSlot(u8),
    Settings(SettingsMessage),
    // Library management
    RescanLibrary,
    AddSoundDirectory,
    SoundDirectoryPickResult(Option<std::path::PathBuf>),
    RemoveSoundDirectory(std::path::PathBuf),
    // Appearance
    ThemeChanged(theme::Theme),
    DensityChanged(Density),
    PanelAnimationsChanged(bool),
    RendererChanged(crate::state::Renderer),
    // Audio
    MicPassthroughChanged(bool),
    MicPassthroughLevelChanged(f32),
    OverlapModeChanged(OverlapMode),
    MonitorDeviceChanged(Option<String>),
    InputDeviceChanged(Option<String>),
    // Voice effects
    SelectEffectPreset(PresetId),
    SetEffectBypassUi(bool),
    SetWetDryMix(f32),
    SetEffectParamUi {
        slot: EffectSlot,
        param: &'static str,
        value: f32,
    },
    /// Toggle the effects side panel open/closed (pull tab).
    ToggleEffectsPanel,
    /// Close the effects side panel (scrim / ✕ / Escape).
    CloseEffectsPanel,
    /// Carries the command sender from the portal stream.
    /// Two `ShortcutHandle` messages are never meaningfully equal — treated as always-unequal.
    ShortcutHandle(crate::shortcuts::PortalCmdSender),
    /// Opens the DE's native shortcut configuration dialog for this session.
    OpenShortcutConfig,
    /// Whether `configure_shortcuts()` (portal v2) is available on this DE/backend.
    ShortcutsConfigureAvailable(bool),
    // Per-sound metadata
    ToggleFavorite(String),
    OpenSoundEditor(String),
    CloseSoundEditor,
    SoundEditorNameChanged(String),
    SoundEditorTagsChanged(String),
    SoundEditorVolumeChanged(String, f32),
    SaveSoundMeta(String),
    /// A background decode completed for play generation `generation`. Applied
    /// only if still the current generation (#149/#151).
    Decoded {
        generation: u64,
        voice_id: u64,
        id: String,
        result: Result<crate::audio::CachedPcm, String>,
        gain: f32,
        effects: crate::audio::effects::EffectSettings,
        mode: PlayMode,
    },
}

impl Message {
    pub fn from_tray_event(event: TrayEvent) -> Self {
        match event {
            TrayEvent::ToggleVisibility => Message::ToggleVisibility,
            TrayEvent::Quit => Message::Quit,
        }
    }
}
