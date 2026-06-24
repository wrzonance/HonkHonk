use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use iced::widget::{button, container, row, scrollable, space, text};
use iced::{Element, Length, Point, Subscription, Task, Theme};

use crate::audio::effects::EffectSlot;
use crate::audio::{AudioCommand, AudioEvent, AudioHandle};
use crate::shortcuts::ShortcutsStatus;
use crate::state::config::Density;
use crate::state::{AppConfig, SlotMap, SoundEntry, SoundMeta, SoundMetaStore};
use crate::tray::{TrayEvent, TrayHandle};
use crate::ui::effects_panel::{self, EffectsUiState, PresetId};
use crate::ui::effects_panel_view;
use crate::ui::side_panel::PanelAnim;
use crate::ui::sound_grid;
use crate::ui::theme::{self, Hh};
use crate::ui::{now_playing, search_bar, slot_manager};

/// Play-dispatch coordination (`request_play` / `handle_decoded` /
/// `start_playback`), extracted to keep this file from growing (#151).
mod playback;

/// Virtual category name used for the Favorites filtered tab.
pub const FAVORITES_TAB: &str = "\u{2605} Favorites";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Main,
    SlotManager,
    Settings,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SettingsSection {
    #[default]
    Audio,
    Library,
    Hotkeys,
    Appearance,
    About,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    ToggleVisibility,
    Quit,
    TrayEvent(TrayEvent),
    TrayPoll,
    AudioEvent(AudioEvent),
    PlaySound(String),
    StopAll,
    SelectCategory(Option<String>),
    SearchChanged(String),
    EscapePressed,
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
    // Settings navigation
    ShowSettings,
    ShowSettingsSection(SettingsSection),
    // Library management
    RescanLibrary,
    AddSoundDirectory,
    SoundDirectoryPickResult(Option<std::path::PathBuf>),
    RemoveSoundDirectory(std::path::PathBuf),
    // Appearance
    ThemeChanged(theme::Theme),
    DensityChanged(Density),
    RendererChanged(crate::state::Renderer),
    // Audio
    MicPassthroughChanged(bool),
    MicPassthroughLevelChanged(f32),
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
    SoundEditorVolumeChanged(String, f32),
    SaveSoundMeta(String),
    /// A background decode completed for play generation `generation`. Applied
    /// only if still the current generation (#149/#151).
    Decoded {
        generation: u64,
        id: String,
        result: Result<crate::audio::CachedPcm, String>,
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
    search_query: String,
    // True after SearchChanged fires; first Escape consumes it as a blur,
    // second Escape clears the query. Resets when SearchChanged fires again.
    search_had_focus: bool,
    progress: f32,
    slots: SlotMap,
    pub(crate) slot_triggers: [Option<String>; 20],
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
    pub(crate) settings_section: SettingsSection,
    pub monitor_devices: Vec<(String, String)>,
    pub input_devices: Vec<(String, String)>,
    shortcut_config: crate::shortcuts::config_ui::ShortcutConfigService,
    /// One-time notice surfaced on first run when the persistent virtual mic was
    /// created programmatically (issue #49). `None` until `SourceFirstRun` fires.
    source_notice: Option<String>,
    /// Per-sound metadata: favorites, per-sound volume, display names.
    pub(crate) sound_meta: SoundMetaStore,
    /// When `false`, `sound_meta.save()` is skipped (used in tests to avoid
    /// writing to the developer's real XDG config dir during `cargo test`).
    persist_sound_meta: bool,
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
    /// Eased panel progress (0=closed..1=open) fed to the view; refreshed each
    /// frame by `effects_panel.tick`.
    panel_progress: f32,
    /// Persistent now-playing waveform cache owner (#131). App holds it but all
    /// cache-lifecycle logic lives in `ui::now_playing::NowPlaying`.
    now_playing: crate::ui::now_playing::NowPlaying,
    /// Predict-and-correct clock driving the smooth playhead; `Some` while a
    /// sound plays. Authoritative anchor is the 10 Hz `AudioEvent::Progress`.
    playhead: Option<crate::ui::playhead::PlayheadClock>,
    /// Frame-interpolated playhead position fed to the now-playing view and the
    /// waveform cache-sync. Distinct from `progress` (the raw 10 Hz anchor).
    display_progress: f32,
    /// Monotonic counter bumped on every play dispatch and on StopAll. Stamped
    /// onto the `Play` command and echoed back on `PlaybackFinished` to tell a
    /// genuine end from the stale `Finished` of a re-pressed voice (#149), and
    /// onto each off-thread decode so a `Message::Decoded` whose generation no
    /// longer matches (a superseded press, or a StopAll mid-decode) is dropped
    /// rather than (re)started (#151).
    play_generation: u64,
    /// Hot-path caches: byte-capped decoded-PCM LRU + waveform envelope map
    /// (#151).
    audio_store: crate::audio::AudioStore,
}

fn shortcuts_stream_sub(
    window_id: Option<ashpd::WindowIdentifier>,
) -> impl iced::futures::Stream<Item = Message> {
    use iced::futures::SinkExt;
    use iced::futures::StreamExt;
    iced::stream::channel(16, async move |mut tx| {
        use crate::shortcuts::{portal, ShortcutEvent};
        let stream = portal::shortcut_stream(window_id);
        let mut stream = std::pin::pin!(stream);
        while let Some(ev) = stream.next().await {
            let msg = match ev {
                ShortcutEvent::Ready => Message::ShortcutsReady,
                ShortcutEvent::Handle(sender) => {
                    Message::ShortcutHandle(crate::shortcuts::PortalCmdSender(sender))
                }
                ShortcutEvent::ConfigureAvailable(v) => Message::ShortcutsConfigureAvailable(v),
                ShortcutEvent::Activated(i) => Message::ShortcutActivated(i),
                ShortcutEvent::Bindings(b) => Message::ShortcutBindingsUpdated(b),
                ShortcutEvent::Changed(b) => Message::ShortcutBindingsUpdated(b),
                ShortcutEvent::Failed(r) => Message::ShortcutsUnavailable(r),
            };
            if tx.send(msg).await.is_err() {
                break;
            }
        }
        // Stream ended unexpectedly (portal crashed mid-session). Notify the UI
        // so the unavailability banner appears, then park to keep the subscription alive.
        let _ = tx
            .send(Message::ShortcutsUnavailable(
                "portal connection lost".into(),
            ))
            .await;
        iced::futures::future::pending::<()>().await;
    })
}

/// Zero-arg wrapper for `Subscription::run` (which requires a fn pointer, not a closure).
fn shortcuts_stream_sub_none() -> impl iced::futures::Stream<Item = Message> {
    shortcuts_stream_sub(None)
}

/// Builder for the one-shot duration scan subscription.
///
/// Returns a `BoxStream` (concrete type) so it can be used as `fn(&D) -> S`
/// with `Subscription::run_with`, which requires a concrete `S: Stream`.
fn duration_scan_builder(
    pairs: &std::sync::Arc<Vec<(String, std::path::PathBuf)>>,
) -> iced::futures::stream::BoxStream<'static, Message> {
    let pairs = std::sync::Arc::clone(pairs);
    Box::pin(iced::stream::channel(1, async move |mut tx| {
        use iced::futures::SinkExt;
        let owned = (*pairs).clone();
        let map =
            tokio::task::spawn_blocking(move || crate::state::library::probe_durations(owned))
                .await
                .unwrap_or_default();
        let _ = tx.send(Message::DurationsLoaded(map)).await;
        iced::futures::future::pending::<()>().await;
    }))
}

async fn pick_directory() -> anyhow::Result<Option<std::path::PathBuf>> {
    use anyhow::Context;
    use ashpd::desktop::file_chooser::SelectedFiles;

    let request = SelectedFiles::open_file()
        .title("Select Sound Folder")
        .directory(true)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!(e))
        .context("file chooser portal send failed")?;

    let files = match request.response() {
        Ok(f) => f,
        Err(ashpd::Error::Response(_)) => return Ok(None), // user cancelled
        Err(e) => return Err(anyhow::anyhow!(e).context("file chooser response failed")),
    };

    let uri = match files.uris().first() {
        Some(u) => u.clone(),
        None => return Ok(None),
    };

    let url = url::Url::parse(uri.as_str()).with_context(|| format!("parsing file URI: {uri}"))?;

    url.to_file_path()
        .map(Some)
        .map_err(|_| anyhow::anyhow!("URI is not a file:// path: {uri}"))
}

/// First-run notice text for the persistent virtual mic (issue #49). When the
/// per-user conf.d was written the device persists across restarts; otherwise
/// it only lasts the session (until reboot) via the lingering node.
fn source_first_run_notice(confd_written: bool) -> String {
    if confd_written {
        "Created HonkHonk Mic virtual device. It will persist after restart. \
Select 'HonkHonk Mic' as your input in Discord/OBS."
            .to_string()
    } else {
        "HonkHonk Mic created for this session. \
Select 'HonkHonk Mic' as your input in Discord/OBS."
            .to_string()
    }
}

impl HonkHonk {
    pub fn new(
        mut tray: TrayHandle,
        audio: AudioHandle,
        sounds: Vec<SoundEntry>,
        config: AppConfig,
        slots: SlotMap,
    ) -> Self {
        let rx = tray.take_rx();
        let duration_scan_pairs = std::sync::Arc::new(
            sounds
                .iter()
                .map(|s| (s.id.clone(), s.path.clone()))
                .collect::<Vec<_>>(),
        );
        Self {
            visible: true,
            exit: false,
            tray_rx: Arc::new(Mutex::new(rx)),
            _tray: Some(tray),
            audio: Some(audio),
            sounds,
            playing: None,
            active_category: None,
            config,
            search_query: String::new(),
            search_had_focus: false,
            progress: 0.0,
            slots,
            slot_triggers: std::array::from_fn(|_| None),
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
            settings_section: SettingsSection::default(),
            monitor_devices: Vec::new(),
            input_devices: Vec::new(),
            shortcut_config: crate::shortcuts::config_ui::ShortcutConfigService::new(),
            source_notice: None,
            sound_meta: SoundMetaStore::load(),
            persist_sound_meta: true,
            editor_sound_id: None,
            editor_draft_name: String::new(),
            editor_draft_volume: 1.0,
            effects_ui: EffectsUiState::default(),
            effects_panel: PanelAnim::default(),
            panel_progress: 0.0,
            now_playing: crate::ui::now_playing::NowPlaying::default(),
            playhead: None,
            display_progress: 0.0,
            play_generation: 0,
            audio_store: crate::audio::AudioStore::new(crate::audio::DEFAULT_PCM_CAP_BYTES),
        }
    }

    pub fn new_for_test() -> Self {
        let (_tx, rx) = std::sync::mpsc::channel();
        let config = AppConfig::default();
        Self {
            visible: true,
            exit: false,
            tray_rx: Arc::new(Mutex::new(rx)),
            _tray: None,
            audio: None,
            sounds: Vec::new(),
            playing: None,
            active_category: None,
            config,
            search_query: String::new(),
            search_had_focus: false,
            progress: 0.0,
            slots: SlotMap::default(),
            slot_triggers: std::array::from_fn(|_| None),
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
            settings_section: SettingsSection::default(),
            monitor_devices: Vec::new(),
            input_devices: Vec::new(),
            shortcut_config: crate::shortcuts::config_ui::ShortcutConfigService::new(),
            source_notice: None,
            sound_meta: SoundMetaStore::default(),
            persist_sound_meta: false,
            editor_sound_id: None,
            editor_draft_name: String::new(),
            editor_draft_volume: 1.0,
            effects_ui: EffectsUiState::default(),
            effects_panel: PanelAnim::default(),
            panel_progress: 0.0,
            now_playing: crate::ui::now_playing::NowPlaying::default(),
            playhead: None,
            display_progress: 0.0,
            play_generation: 0,
            audio_store: crate::audio::AudioStore::new(crate::audio::DEFAULT_PCM_CAP_BYTES),
        }
    }

    pub fn should_exit(&self) -> bool {
        self.exit
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
        &self.search_query
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

    /// First-run persistent-mic notice text, if one was surfaced this session
    /// (issue #49). Returns `None` until a `SourceFirstRun` event fires. A UI
    /// banner can render this; rendering is intentionally out of scope here.
    pub fn source_notice(&self) -> Option<&str> {
        self.source_notice.as_deref()
    }

    pub fn sound_meta(&self) -> &SoundMetaStore {
        &self.sound_meta
    }

    pub fn editor_sound_id(&self) -> Option<&str> {
        self.editor_sound_id.as_deref()
    }

    pub fn filtered_sounds(&self) -> Vec<&SoundEntry> {
        let query = self.search_query.to_lowercase();
        self.sounds
            .iter()
            .filter(|s| match self.active_category.as_deref() {
                Some(cat) if cat == FAVORITES_TAB => self.sound_meta.is_favorite(&s.id),
                Some(cat) => s.category == cat,
                None => true,
            })
            .filter(|s| {
                if query.is_empty() {
                    return true;
                }
                // Also match against the display-name override so sounds
                // renamed by the user remain discoverable by their visible label.
                let display_name_matches = self
                    .sound_meta
                    .get_ref(&s.id)
                    .and_then(|m| m.display_name.as_deref())
                    .is_some_and(|name| name.to_lowercase().contains(&query));
                s.name.to_lowercase().contains(&query) || display_name_matches
            })
            .collect()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let task = match message {
            Message::ToggleVisibility => {
                self.visible = !self.visible;
                Task::none()
            }
            Message::Quit => {
                if let Some(ref audio) = self.audio {
                    audio.shutdown();
                }
                self.exit = true;
                iced::exit()
            }
            Message::TrayEvent(event) => {
                let msg = Message::from_tray_event(event);
                self.update(msg)
            }
            Message::TrayPoll => {
                while gtk::events_pending() {
                    gtk::main_iteration_do(false);
                }

                let event = self.tray_rx.lock().ok().and_then(|rx| rx.try_recv().ok());
                if let Some(e) = event {
                    let msg = Message::from_tray_event(e);
                    return self.update(msg);
                }

                self.drain_audio_events()
            }
            Message::AudioEvent(event) => {
                match event {
                    AudioEvent::Ready => {
                        eprintln!("honkhonk: audio engine ready");
                        if let Some(ref audio) = self.audio {
                            audio.send(AudioCommand::SetVolume(self.config.volume));
                        }
                    }
                    AudioEvent::PlaybackStarted { sound_id } => {
                        // Every play path sets `playing` optimistically at
                        // dispatch, so a Started for a *different* sound can
                        // only be a stale event from an older press still in
                        // the queue — don't let it steal the highlight (#111).
                        if self.playing.is_none()
                            || self.playing.as_deref() == Some(sound_id.as_str())
                        {
                            self.playing = Some(sound_id);
                        }
                    }
                    AudioEvent::PlaybackFinished {
                        sound_id,
                        generation,
                    } => {
                        // Clear only when this Finished is for the sound we are
                        // showing AND belongs to the current play. The sound_id
                        // check keeps a Finished for an already-replaced sound
                        // from blanking a newer press (#111); the generation
                        // check additionally ignores the stale Finished emitted
                        // for a same-sound voice that was superseded by an
                        // immediate re-press, so its fresh playhead survives
                        // (#149).
                        if self.playing.as_deref() == Some(sound_id.as_str())
                            && generation == self.play_generation
                        {
                            self.clear_playback_state();
                        }
                    }
                    AudioEvent::Progress(p) => {
                        // Raw 10 Hz anchor, retained for diagnostics/tests. The
                        // smooth playhead is wall-clock driven (`Message::Frame`),
                        // NOT this sample: re-anchoring a sample measured ~100 ms
                        // ago to the current instant snapped the line backward
                        // every drain (left/right jitter, #138).
                        self.progress = p;
                    }
                    AudioEvent::Error(e) => {
                        eprintln!("honkhonk: audio error: {e}");
                    }
                    AudioEvent::SourceFirstRun { confd_written } => {
                        let notice = source_first_run_notice(confd_written);
                        eprintln!("honkhonk: {notice}");
                        self.source_notice = Some(notice);
                    }
                    AudioEvent::OutputDevicesChanged(devices) => {
                        if let Some(ref target) = self.config.monitor_device.clone() {
                            let was_visible = self.monitor_devices.iter().any(|(n, _)| n == target);
                            let still_visible = devices.iter().any(|(n, _)| n == target);
                            if was_visible && !still_visible {
                                let config = AppConfig {
                                    monitor_device: None,
                                    ..self.config.clone()
                                };
                                if let Err(e) = config.save() {
                                    eprintln!("honkhonk: failed to save config: {e}");
                                }
                                self.config = config;
                                if let Some(ref audio) = self.audio {
                                    audio.send(AudioCommand::SetMonitorDevice(None));
                                }
                            }
                        }
                        self.monitor_devices = devices;
                    }
                    AudioEvent::InputDevicesChanged(devices) => {
                        if let Some(ref target) = self.config.input_device.clone() {
                            let was_visible = self.input_devices.iter().any(|(n, _)| n == target);
                            let still_visible = devices.iter().any(|(n, _)| n == target);
                            if was_visible && !still_visible {
                                let config = AppConfig {
                                    input_device: None,
                                    ..self.config.clone()
                                };
                                if let Err(e) = config.save() {
                                    eprintln!("honkhonk: failed to save config: {e}");
                                }
                                self.config = config;
                                if let Some(ref audio) = self.audio {
                                    audio.send(AudioCommand::SetInputDevice(None));
                                }
                            }
                        }
                        self.input_devices = devices;
                    }
                    AudioEvent::EffectsLatencyChanged(_latency) => {
                        // Reserved for Phase 4B: update UI latency indicator.
                    }
                }
                Task::none()
            }
            Message::PlaySound(sound_id) => {
                if let Some(sound) = self.sounds.iter().find(|s| s.id == sound_id).cloned() {
                    self.request_play(&sound, false)
                } else {
                    Task::none()
                }
            }
            Message::StopAll => {
                if let Some(ref audio) = self.audio {
                    audio.send(AudioCommand::Stop);
                }
                self.clear_playback_state();
                // Invalidate any decode still in flight for a just-pressed sound
                // so a cold-cache play cancelled mid-decode is not resurrected
                // when its `Message::Decoded` lands (#151). The genuine-end and
                // decode-error teardowns have no in-flight decode for the current
                // generation, so only the explicit Stop bumps here.
                self.play_generation = self.play_generation.wrapping_add(1);
                Task::none()
            }
            Message::SelectCategory(cat) => {
                self.active_category = cat;
                Task::none()
            }
            Message::EscapePressed => {
                if self.context_menu.is_some() {
                    // Context menu takes priority — close it, leave search state intact.
                    self.context_menu = None;
                    self.context_menu_pos = None;
                } else if self.editor_sound_id.is_some() {
                    // Editor overlay takes next priority — discard draft and close.
                    self.editor_sound_id = None;
                    self.editor_draft_name = String::new();
                    self.editor_draft_volume = 1.0;
                } else if self.effects_panel.is_visible() {
                    // Drawer absorbs Escape whenever it is on screen — including
                    // mid-close — so a second Escape never falls through to clear
                    // the search query. `close` is a no-op if already closing.
                    let now = Instant::now();
                    self.effects_panel.close(now);
                    self.panel_progress = self.effects_panel.progress(now);
                } else if self.search_had_focus {
                    // First Esc: treat as blur — Iced already handled unfocus.
                    self.search_had_focus = false;
                } else if !self.search_query.is_empty() {
                    // Second Esc (or Esc when unfocused): clear query.
                    self.search_query = String::new();
                }
                Task::none()
            }
            Message::SearchChanged(query) => {
                self.search_had_focus = true;
                self.search_query = query;
                Task::none()
            }
            Message::VolumeChanged(v) => {
                self.config.volume = v.clamp(0.0, 1.0);
                if let Some(ref audio) = self.audio {
                    audio.send(AudioCommand::SetVolume(self.config.volume));
                }
                Task::none()
            }
            Message::VolumeSaveRequested => {
                if let Err(e) = self.config.save() {
                    eprintln!("honkhonk: config save error: {e}");
                }
                Task::none()
            }
            Message::ShortcutsReady => {
                self.shortcuts_status = ShortcutsStatus::Active;
                Task::none()
            }
            Message::ShortcutsUnavailable(reason) => {
                self.shortcuts_status = ShortcutsStatus::Unavailable(reason);
                Task::none()
            }
            Message::DismissShortcutsWarning => {
                self.shortcuts_warning_dismissed = true;
                Task::none()
            }
            Message::ShortcutActivated(idx) => {
                if let Some(path) = self.slots.get(idx).cloned() {
                    if let Some(sound) = self.sounds.iter().find(|s| s.path == path).cloned() {
                        return self.request_play(&sound, true);
                    } else {
                        // Path no longer in library (file deleted/moved) — clear stale slot
                        eprintln!(
                            "honkhonk: slot {} points to missing file {:?}, clearing",
                            idx + 1,
                            path
                        );
                        self.slots.clear(idx);
                        if let Err(e) = self.slots.save() {
                            eprintln!("honkhonk: slots save error: {e}");
                        }
                    }
                }
                Task::none()
            }
            Message::ShortcutBindingsUpdated(bindings) => {
                for (idx, trigger) in bindings {
                    if let Some(slot) = self.slot_triggers.get_mut(idx as usize) {
                        *slot = Some(trigger);
                    }
                }
                Task::none()
            }
            Message::DurationsLoaded(map) => {
                self.sounds =
                    crate::state::library::apply_durations(std::mem::take(&mut self.sounds), &map);
                self.durations_loaded = true;
                Task::none()
            }
            Message::AssignSlot(idx, path) => {
                self.slots.set(idx, path);
                if let Err(e) = self.slots.save() {
                    eprintln!("honkhonk: slots save error: {e}");
                }
                Task::none()
            }
            Message::ClearSlot(idx) => {
                self.slots.clear(idx);
                if let Err(e) = self.slots.save() {
                    eprintln!("honkhonk: slots save error: {e}");
                }
                Task::none()
            }
            Message::OpenContextMenu(sound_id) => {
                self.context_menu = Some(sound_id);
                self.context_menu_pos = Some(self.cursor_pos);
                Task::none()
            }
            Message::CloseContextMenu => {
                self.context_menu = None;
                self.context_menu_pos = None;
                Task::none()
            }
            Message::CursorMoved(pos) => {
                self.cursor_pos = pos;
                Task::none()
            }
            Message::WindowResized(w, h) => {
                self.window_size = (w, h);
                Task::none()
            }
            Message::Frame(now) => {
                if let Some(ref clock) = self.playhead {
                    self.display_progress = clock.display(now);
                }
                self.panel_progress = self.effects_panel.tick(now);
                Task::none()
            }
            Message::ShowSlots => {
                self.view_mode = ViewMode::SlotManager;
                self.selected_slot = None;
                Task::none()
            }
            Message::ShowMain => {
                self.view_mode = ViewMode::Main;
                self.selected_slot = None;
                Task::none()
            }
            Message::ShowSettings => {
                self.view_mode = ViewMode::Settings;
                self.settings_section = SettingsSection::Audio;
                Task::none()
            }
            Message::ShowSettingsSection(section) => {
                self.settings_section = section;
                Task::none()
            }
            Message::SelectSlot(idx) => {
                self.selected_slot = Some(idx);
                Task::none()
            }
            Message::RescanLibrary => {
                let new_sounds = match crate::state::Library::scan(&self.config.sound_directories) {
                    Ok(sounds) => sounds,
                    Err(e) => {
                        eprintln!(
                            "honkhonk: library rescan failed for {:?}: {e}",
                            self.config.sound_directories
                        );
                        return Task::none();
                    }
                };
                let pairs: Vec<(String, std::path::PathBuf)> = new_sounds
                    .iter()
                    .map(|s| (s.id.clone(), s.path.clone()))
                    .collect();
                self.sounds = new_sounds;
                self.duration_scan_pairs = std::sync::Arc::new(pairs);
                self.durations_loaded = false;
                Task::none()
            }
            Message::AddSoundDirectory => Task::perform(
                async {
                    match pick_directory().await {
                        Ok(opt) => opt,
                        Err(e) => {
                            eprintln!("honkhonk: directory picker error: {e:#}");
                            None
                        }
                    }
                },
                Message::SoundDirectoryPickResult,
            ),
            Message::SoundDirectoryPickResult(Some(path)) => {
                if !self.config.sound_directories.contains(&path) {
                    self.config.sound_directories.push(path);
                    if let Err(e) = self.config.save() {
                        eprintln!("honkhonk: config save error: {e}");
                    }
                    self.update(Message::RescanLibrary)
                } else {
                    Task::none()
                }
            }
            Message::SoundDirectoryPickResult(None) => Task::none(),
            Message::RemoveSoundDirectory(path) => {
                self.config.sound_directories.retain(|p| p != &path);
                if let Err(e) = self.config.save() {
                    eprintln!("honkhonk: config save error: {e}");
                }
                self.update(Message::RescanLibrary)
            }
            Message::ThemeChanged(t) => {
                if self.config.theme != t {
                    self.config = AppConfig {
                        theme: t,
                        ..self.config.clone()
                    };
                    if let Err(e) = self.config.save() {
                        eprintln!("honkhonk: config save error: {e}");
                    }
                }
                Task::none()
            }
            Message::DensityChanged(d) => {
                if self.config.density != d {
                    self.config = AppConfig {
                        density: d,
                        ..self.config.clone()
                    };
                    if let Err(e) = self.config.save() {
                        eprintln!("honkhonk: config save error: {e}");
                    }
                }
                Task::none()
            }
            Message::RendererChanged(r) => {
                if self.config.renderer != r {
                    self.config = AppConfig {
                        renderer: r,
                        ..self.config.clone()
                    };
                    if let Err(e) = self.config.save() {
                        eprintln!("honkhonk: config save error: {e}");
                    }
                }
                Task::none()
            }
            Message::MicPassthroughChanged(v) => {
                let config = AppConfig {
                    mic_passthrough: v,
                    ..self.config.clone()
                };
                if let Err(e) = config.save() {
                    eprintln!("honkhonk: failed to save config: {e}");
                }
                self.config = config;
                if let Some(ref audio) = self.audio {
                    audio.send(AudioCommand::SetMicPassthrough(v));
                }
                Task::none()
            }
            Message::MicPassthroughLevelChanged(v) => {
                let config = AppConfig {
                    mic_passthrough_level: v.clamp(0.0, 1.0),
                    ..self.config.clone()
                };
                if let Err(e) = config.save() {
                    eprintln!("honkhonk: failed to save config: {e}");
                }
                self.config = config;
                if let Some(ref audio) = self.audio {
                    audio.send(AudioCommand::SetMicPassthroughLevel(
                        self.config.mic_passthrough_level,
                    ));
                }
                Task::none()
            }
            Message::MonitorDeviceChanged(target) => {
                if self.config.monitor_device == target {
                    return Task::none();
                }
                let config = AppConfig {
                    monitor_device: target.clone(),
                    ..self.config.clone()
                };
                if let Err(e) = config.save() {
                    eprintln!("honkhonk: failed to save config: {e}");
                }
                self.config = config;
                if let Some(ref audio) = self.audio {
                    audio.send(AudioCommand::SetMonitorDevice(target));
                }
                Task::none()
            }
            Message::InputDeviceChanged(target) => {
                if self.config.input_device == target {
                    return Task::none();
                }
                let config = AppConfig {
                    input_device: target.clone(),
                    ..self.config.clone()
                };
                if let Err(e) = config.save() {
                    eprintln!("honkhonk: failed to save config: {e}");
                }
                self.config = config;
                if let Some(ref audio) = self.audio {
                    audio.send(AudioCommand::SetInputDevice(target));
                }
                Task::none()
            }
            Message::SelectEffectPreset(preset) => {
                let cmds = effects_panel::select_preset(&mut self.effects_ui, preset);
                self.send_audio_commands(cmds);
                Task::none()
            }
            Message::SetEffectBypassUi(bypass) => {
                let cmd = effects_panel::set_chain_bypass(&mut self.effects_ui, bypass);
                self.send_audio_commands([cmd]);
                Task::none()
            }
            Message::SetWetDryMix(mix) => {
                let cmd = effects_panel::set_wet_dry(&mut self.effects_ui, mix);
                self.send_audio_commands([cmd]);
                Task::none()
            }
            Message::SetEffectParamUi { slot, param, value } => {
                let cmds = effects_panel::edit_param(&mut self.effects_ui, slot, param, value);
                self.send_audio_commands(cmds);
                Task::none()
            }
            Message::ToggleEffectsPanel => {
                let now = Instant::now();
                self.effects_panel.toggle(now);
                self.panel_progress = self.effects_panel.progress(now);
                Task::none()
            }
            Message::CloseEffectsPanel => {
                let now = Instant::now();
                self.effects_panel.close(now);
                self.panel_progress = self.effects_panel.progress(now);
                Task::none()
            }
            Message::ShortcutHandle(crate::shortcuts::PortalCmdSender(sender)) => {
                self.shortcut_config.set_portal_sender(sender);
                Task::none()
            }
            Message::ShortcutsConfigureAvailable(available) => {
                self.shortcut_config.set_portal_v2_available(available);
                Task::none()
            }
            Message::OpenShortcutConfig => {
                self.shortcut_config.open();
                Task::none()
            }
            Message::ToggleFavorite(sound_id) => {
                let is_favorite = self.sound_meta.toggle_favorite(&sound_id);
                if self.persist_sound_meta {
                    if let Err(e) = self.sound_meta.save() {
                        eprintln!("honkhonk: sound meta save error: {e}");
                    }
                }
                // If the user just unstarred the last favorite while on the
                // Favorites tab, the chip disappears from the header. Reset to
                // "All" so the list doesn't show empty under an invisible filter.
                if !is_favorite
                    && self.active_category.as_deref() == Some(FAVORITES_TAB)
                    && !self
                        .sounds
                        .iter()
                        .any(|s| self.sound_meta.is_favorite(&s.id))
                {
                    self.active_category = None;
                }
                Task::none()
            }
            Message::OpenSoundEditor(sound_id) => {
                let meta = self.sound_meta.get(&sound_id);
                let name_override = meta.display_name.clone().unwrap_or_default();
                let vol = meta.volume;
                // Clear the context menu so the editor overlay surfaces immediately.
                self.context_menu = None;
                self.context_menu_pos = None;
                self.editor_sound_id = Some(sound_id);
                self.editor_draft_name = name_override;
                self.editor_draft_volume = vol;
                Task::none()
            }
            Message::CloseSoundEditor => {
                self.editor_sound_id = None;
                self.editor_draft_name = String::new();
                self.editor_draft_volume = 1.0;
                Task::none()
            }
            Message::SoundEditorNameChanged(name) => {
                self.editor_draft_name = name;
                Task::none()
            }
            Message::SoundEditorVolumeChanged(_sound_id, v) => {
                self.editor_draft_volume = v.clamp(0.0, 2.0);
                Task::none()
            }
            Message::SaveSoundMeta(sound_id) => {
                let display_name = if self.editor_draft_name.trim().is_empty() {
                    None
                } else {
                    Some(self.editor_draft_name.trim().to_owned())
                };
                let meta = SoundMeta {
                    favorite: self.sound_meta.get(&sound_id).favorite,
                    volume: self.editor_draft_volume,
                    display_name,
                };
                self.sound_meta.set(sound_id, meta);
                if self.persist_sound_meta {
                    if let Err(e) = self.sound_meta.save() {
                        eprintln!("honkhonk: sound meta save error: {e}");
                    }
                }
                self.editor_sound_id = None;
                self.editor_draft_name = String::new();
                self.editor_draft_volume = 1.0;
                Task::none()
            }
            Message::Decoded {
                generation,
                id,
                result,
            } => self.handle_decoded(generation, id, result),
        };
        // Keep the now-playing waveform cache in step with playback state.
        // Single delegating call — all lifecycle logic lives in NowPlaying.
        self.now_playing
            .sync(self.playing.as_deref(), self.display_progress);
        task
    }

    /// Process every audio event queued since the last poll tick.
    ///
    /// The engine emits ~10 Progress events/sec while playing plus two events
    /// per Play (Finished for the replaced sound + Started), while this poll
    /// runs at 10 Hz. Draining one event per tick (the old behavior) therefore
    /// could never catch up after a burst of button presses, leaving the UI
    /// seconds behind the audio (#111).
    fn drain_audio_events(&mut self) -> Task<Message> {
        let mut tasks = Vec::new();
        loop {
            let event = match self.audio {
                Some(ref audio) => audio.try_recv(),
                None => None,
            };
            let Some(event) = event else { break };
            tasks.push(self.update(Message::AudioEvent(event)));
        }
        Task::batch(tasks)
    }

    /// Clears all now-playing state together so the highlight, raw progress
    /// anchor, playhead clock, and smooth display position never drift apart.
    /// The single teardown path for StopAll, the genuine PlaybackFinished end,
    /// and a failed decode.
    fn clear_playback_state(&mut self) {
        self.playing = None;
        self.progress = 0.0;
        self.playhead = None;
        self.display_progress = 0.0;
    }

    fn view_header(&self, t: theme::Theme) -> Element<'_, Message> {
        let title = text("HonkHonk").size(24).color(t.ink());

        let slots_btn = button(text("Slots").size(14).color(t.ink()))
            .on_press(Message::ShowSlots)
            .style(move |_theme, _status| button::Style {
                background: Some(theme::bg_color(t.panel())),
                text_color: t.ink(),
                border: theme::tile_border(t.hairline(), 1.0),
                ..Default::default()
            });

        let settings_btn = button(text("Settings").size(14).color(t.ink()))
            .on_press(Message::ShowSettings)
            .style(move |_theme, _status| button::Style {
                background: Some(theme::bg_color(t.panel())),
                text_color: t.ink(),
                border: theme::tile_border(t.hairline(), 1.0),
                ..Default::default()
            });

        let search = search_bar::view_search_bar(&self.search_query);

        let stop_btn = button(text("Stop All").size(14).color(t.ink()))
            .on_press(Message::StopAll)
            .style(move |_theme, _status| button::Style {
                background: Some(theme::bg_color(t.panel())),
                text_color: t.ink(),
                border: theme::tile_border(t.hairline(), 1.0),
                ..Default::default()
            });

        row![
            title,
            slots_btn,
            settings_btn,
            space::horizontal(),
            search,
            stop_btn
        ]
        .spacing(theme::space::LG)
        .align_y(iced::Alignment::Center)
        .into()
    }

    fn view_category_chips(&self, t: theme::Theme) -> Element<'_, Message> {
        use std::collections::BTreeSet;

        let categories: BTreeSet<&str> = self.sounds.iter().map(|s| s.category.as_str()).collect();

        let all_chip = self.category_chip("All", self.active_category.is_none(), None, t);

        let has_favorites = self
            .sounds
            .iter()
            .any(|s| self.sound_meta.is_favorite(&s.id));
        let fav_active = self.active_category.as_deref() == Some(FAVORITES_TAB);

        let chips: Vec<Element<'_, Message>> = std::iter::once(all_chip)
            .chain(has_favorites.then(|| {
                self.category_chip(FAVORITES_TAB, fav_active, Some(FAVORITES_TAB.to_owned()), t)
            }))
            .chain(categories.into_iter().map(|cat| {
                let is_active = self.active_category.as_deref() == Some(cat);
                self.category_chip(cat, is_active, Some(cat.to_owned()), t)
            }))
            .collect();

        let chip_row = chips
            .into_iter()
            .fold(row![].spacing(theme::space::SM), |r, chip| r.push(chip));

        scrollable(chip_row)
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new(),
            ))
            .into()
    }

    fn category_chip(
        &self,
        label: &str,
        active: bool,
        value: Option<String>,
        t: theme::Theme,
    ) -> Element<'_, Message> {
        let bg = if active { t.accent() } else { t.panel() };
        let text_color = if active {
            iced::Color::from_rgb(0.1, 0.07, 0.03)
        } else {
            t.ink()
        };

        button(text(label.to_owned()).size(13).color(text_color))
            .on_press(Message::SelectCategory(value))
            .padding([theme::space::XS, theme::space::MD])
            .style(move |_theme, _status| button::Style {
                background: Some(theme::bg_color(bg)),
                text_color,
                border: iced::Border {
                    color: iced::Color::TRANSPARENT,
                    width: 0.0,
                    radius: theme::radius::PILL,
                },
                ..Default::default()
            })
            .into()
    }

    pub fn theme(&self) -> Theme {
        match self.config.theme {
            theme::Theme::Light => Theme::Light,
            theme::Theme::Dark | theme::Theme::System => Theme::Dark,
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let shortcuts = Subscription::run(shortcuts_stream_sub_none);

        let tray_poll =
            iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::TrayPoll);

        let events = iced::event::listen_with(|event, _, _window_id| match event {
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                ..
            }) => Some(Message::EscapePressed),
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                Some(Message::CursorMoved(position))
            }
            iced::Event::Window(iced::window::Event::Opened { size, .. }) => {
                Some(Message::WindowResized(size.width, size.height))
            }
            iced::Event::Window(iced::window::Event::Resized(size)) => {
                Some(Message::WindowResized(size.width, size.height))
            }
            _ => None,
        });

        let mut subs = vec![shortcuts, tray_poll, events];

        if !self.durations_loaded {
            subs.push(Subscription::run_with(
                std::sync::Arc::clone(&self.duration_scan_pairs),
                duration_scan_builder,
            ));
        }

        // Vsync-paced playhead animation — subscribed ONLY while a sound plays so
        // an idle tray app never repaints. `window::frames()` yields one `Instant`
        // per refresh; subscriptions are re-evaluated each update, so this drops
        // out automatically when playback ends. No fps cap (let it fly at refresh).
        if self.playing.is_some() || self.effects_panel.is_animating() {
            subs.push(iced::window::frames().map(Message::Frame));
        }

        Subscription::batch(subs)
    }

    fn view_shortcuts_banner(&self, t: theme::Theme) -> Option<Element<'_, Message>> {
        let ShortcutsStatus::Unavailable(ref reason) = self.shortcuts_status else {
            return None;
        };
        if self.shortcuts_warning_dismissed {
            return None;
        }
        let banner = container(
            row![
                text(format!(
                    "Global shortcuts unavailable: {reason}. Check xdg-desktop-portal is running."
                ))
                .size(13)
                .color(iced::Color::from_rgb(0.6, 0.4, 0.0)),
                space::horizontal(),
                button(text("×").size(14))
                    .on_press(Message::DismissShortcutsWarning)
                    .style(move |_t, _s| button::Style {
                        background: None,
                        text_color: t.ink(),
                        ..Default::default()
                    }),
            ]
            .spacing(theme::space::MD)
            .align_y(iced::Alignment::Center),
        )
        .padding([theme::space::SM, theme::space::LG])
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(iced::Color::from_rgb(0.98, 0.92, 0.75))),
            border: theme::tile_border(iced::Color::from_rgb(0.9, 0.75, 0.3), 1.0),
            ..Default::default()
        });
        Some(banner.into())
    }

    fn view_main(&self) -> Element<'_, Message> {
        let t = self.config.theme;
        let header = self.view_header(t);
        let chips = self.view_category_chips(t);
        let filtered = self.filtered_sounds();
        let grid = sound_grid::view_grid(
            &filtered,
            self.playing.as_deref(),
            sound_grid::GridCtx {
                slots: &self.slots,
                triggers: &self.slot_triggers,
                shortcuts_active: matches!(self.shortcuts_status, ShortcutsStatus::Active),
                columns: self.config.density.columns(),
                sound_meta: &self.sound_meta,
            },
        );

        let playing_sound = self
            .playing
            .as_deref()
            .and_then(|id| self.sounds.iter().find(|s| s.id == id));
        let envelope = self
            .playing
            .as_deref()
            .and_then(|id| self.audio_store.envelope(id));
        let now_playing = now_playing::view_now_playing(
            &self.now_playing,
            playing_sound,
            self.display_progress,
            self.config.volume,
            envelope.as_deref(),
        );

        // The banner shares one stable column slot with the header: inserting
        // it as its own top-level slot would shift every later sibling during
        // tree diffing when it appears/dismisses, wiping the grid scrollable's
        // offset (#112).
        let mut top = iced::widget::Column::new().spacing(theme::space::MD);
        if let Some(banner) = self.view_shortcuts_banner(t) {
            top = top.push(banner);
        }
        let top = top.push(header);

        // Inset the grid from the overlay scrollbar (10px, drawn over content) so
        // the last tile column is never clipped by it.
        let grid_scroll = scrollable(container(grid).width(Length::Fill).padding(iced::Padding {
            top: 0.0,
            right: theme::space::LG,
            bottom: 0.0,
            left: 0.0,
        }))
        .height(Length::Fill);

        let items: Vec<Element<'_, Message>> =
            vec![top.into(), chips, grid_scroll.into(), now_playing];

        let content = iced::widget::Column::with_children(items).spacing(theme::space::MD);

        let base = container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            // Larger right padding reserves a clean gutter for the closed side-panel
            // handle (#143, ~28px) so it never overlaps the grid or its scrollbar.
            .padding(iced::Padding {
                top: theme::space::XL,
                right: theme::space::XL + theme::space::SM,
                bottom: theme::space::XL,
                left: theme::space::XL,
            })
            .style(move |_theme| container::Style {
                background: Some(theme::bg_color(t.bg())),
                ..Default::default()
            });

        // iced reconciles widget state positionally: flipping the root between
        // `container` (no overlay) and `stack![...]` (overlay open) discards
        // every descendant's state — including the grid scrollable's offset,
        // which made the list snap to the top on right-click (#112). Keep the
        // root a Stack with the base layout always at child 0; overlays only
        // append/remove child 1, so the base subtree (and its scroll position)
        // survives the diff.
        let mut layers: Vec<Element<'_, Message>> = vec![base.into()];

        // Effects side panel (#143): pull tab always visible; scrim + body slide
        // in when open. Pushed below the context-menu/editor modals so those stack
        // on top. All drawer assembly + logic lives in `ui` modules, not here.
        layers.push(effects_panel_view::effects_side_panel_layer(
            &self.effects_ui,
            self.panel_progress,
            t,
        ));

        // Overlay context menu at window level so cursor coords map exactly.
        if let (Some(ref sound_id), Some(pos)) = (&self.context_menu, self.context_menu_pos) {
            let found = self.sounds.iter().find(|s| s.id == *sound_id);
            layers.push(sound_grid::context_menu_overlay(
                found,
                sound_grid::SlotCtx {
                    slots: &self.slots,
                    triggers: &self.slot_triggers,
                },
                t,
                pos,
                self.window_size,
            ));
        } else if let Some(ref sound_id) = self.editor_sound_id {
            // Per-sound editor overlay
            if let Some(sound) = self.sounds.iter().find(|s| s.id == *sound_id) {
                layers.push(crate::ui::sound_editor::view_editor_overlay(
                    crate::ui::sound_editor::EditorCtx {
                        sound,
                        meta: self.sound_meta.get(sound_id),
                        draft_name: &self.editor_draft_name,
                        draft_volume: self.editor_draft_volume,
                    },
                    t,
                ));
            }
        }

        iced::widget::Stack::with_children(layers)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn view(&self) -> Element<'_, Message> {
        match self.view_mode {
            ViewMode::Main => self.view_main(),
            ViewMode::SlotManager => {
                let t = self.config.theme;
                slot_manager::view_slot_manager(
                    slot_manager::SlotManagerCtx {
                        slots: &self.slots,
                        slot_triggers: &self.slot_triggers,
                        sounds: &self.sounds,
                        selected_slot: self.selected_slot,
                        configure_available: self.shortcut_config.can_open(),
                    },
                    t,
                )
            }
            ViewMode::Settings => crate::ui::settings::view_settings(self, self.config.theme),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shortcuts::ShortcutsStatus;

    #[test]
    fn toggle_visibility_flips_state() {
        let mut app = HonkHonk::new_for_test();
        assert!(app.is_visible());
        let _ = app.update(Message::ToggleVisibility);
        assert!(!app.is_visible());
        let _ = app.update(Message::ToggleVisibility);
        assert!(app.is_visible());
    }

    #[test]
    fn quit_sets_exit_flag() {
        let mut app = HonkHonk::new_for_test();
        assert!(!app.should_exit());
        let _ = app.update(Message::Quit);
        assert!(app.should_exit());
    }

    #[test]
    fn select_category_updates_active_category() {
        let mut app = HonkHonk::new_for_test();
        assert!(app.active_category().is_none());
        let _ = app.update(Message::SelectCategory(Some("Memes".into())));
        assert_eq!(app.active_category(), Some("Memes"));
        let _ = app.update(Message::SelectCategory(None));
        assert!(app.active_category().is_none());
    }

    #[test]
    fn select_effect_preset_updates_ui_state() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::SelectEffectPreset(PresetId::Robot));
        assert_eq!(app.effects_ui_preset(), PresetId::Robot);
    }

    #[test]
    fn set_wet_dry_updates_ui_state() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::SetWetDryMix(0.4));
        assert!((app.effects_ui_wet_dry() - 0.4).abs() < 1e-6);
    }

    #[test]
    fn set_effect_bypass_updates_ui_state() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::SetEffectBypassUi(true));
        assert!(app.effects_ui_chain_bypass());
    }

    #[test]
    fn set_effect_param_switches_to_custom_preset() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::SelectEffectPreset(PresetId::Robot));
        let _ = app.update(Message::SetEffectParamUi {
            slot: EffectSlot::Pitch,
            param: "semitones",
            value: -2.0,
        });
        assert_eq!(app.effects_ui_preset(), PresetId::Custom);
    }

    #[test]
    fn stop_all_clears_playing() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
            sound_id: "test-id".into(),
        }));
        let _ = app.update(Message::StopAll);
        assert!(app.playing().is_none());
    }

    #[test]
    fn stop_all_clears_all_playback_state() {
        use std::time::{Duration, Instant};
        let mut app = HonkHonk::new_for_test();
        app.playing = Some("x".into());
        app.progress = 0.7;
        app.display_progress = 0.7;
        app.playhead = Some(crate::ui::playhead::PlayheadClock::new(
            Duration::from_secs(5),
            Instant::now(),
        ));
        let _ = app.update(Message::StopAll);
        assert!(app.playing.is_none());
        assert_eq!(app.progress, 0.0);
        assert_eq!(app.display_progress, 0.0);
        assert!(app.playhead.is_none());
    }

    #[test]
    fn source_first_run_written_sets_persistent_notice() {
        let mut app = HonkHonk::new_for_test();
        assert!(app.source_notice().is_none());
        let _ = app.update(Message::AudioEvent(AudioEvent::SourceFirstRun {
            confd_written: true,
        }));
        let notice = app.source_notice().expect("notice set");
        assert!(notice.contains("persist"));
        assert!(notice.contains("HonkHonk Mic"));
    }

    #[test]
    fn source_first_run_not_written_sets_session_notice() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::AudioEvent(AudioEvent::SourceFirstRun {
            confd_written: false,
        }));
        let notice = app.source_notice().expect("notice set");
        assert!(notice.contains("this session"));
    }

    #[test]
    fn audio_event_playback_started_sets_playing() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
            sound_id: "abc123".into(),
        }));
        assert_eq!(app.playing(), Some("abc123"));
    }

    #[test]
    fn audio_event_playback_finished_clears_playing() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
            sound_id: "abc123".into(),
        }));
        let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackFinished {
            sound_id: "abc123".into(),
            generation: 0,
        }));
        assert!(app.playing().is_none());
    }

    /// Smoke test for the overlay layering in `view_main`: the element tree
    /// must build in every overlay state. The structural invariant itself
    /// (stable Stack root preserving scrollable offsets, #112) lives in iced's
    /// private widget state and is covered by the manual test plan instead.
    #[test]
    fn view_builds_in_all_overlay_states() {
        let mut app = HonkHonk::new_for_test();
        app.sounds = vec![SoundEntry {
            id: "aaa".into(),
            name: "Goose Honk".into(),
            path: "/a.mp3".into(),
            format: crate::state::AudioFormat::Mp3,
            duration_ms: Some(1000),
            category: "Honk".into(),
        }];

        let _ = app.view(); // no overlay
        let _ = app.update(Message::OpenContextMenu("aaa".into()));
        let _ = app.view(); // context menu overlay
        let _ = app.update(Message::OpenSoundEditor("aaa".into()));
        let _ = app.view(); // editor overlay
    }

    /// Minimal 16-bit PCM mono WAV (4 samples) so tests can exercise the real
    /// decode path without fixture files.
    fn write_test_wav(path: &std::path::Path) {
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&44u32.to_le_bytes()); // riff chunk size
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
        bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
        bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
        bytes.extend_from_slice(&44100u32.to_le_bytes());
        bytes.extend_from_slice(&88200u32.to_le_bytes()); // byte rate
        bytes.extend_from_slice(&2u16.to_le_bytes()); // block align
        bytes.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&8u32.to_le_bytes());
        for s in [0i16, 8000, -8000, 0] {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
        std::fs::write(path, bytes).expect("write test wav");
    }

    #[test]
    fn stale_playback_started_does_not_overwrite_newer_playing() {
        let mut app = HonkHonk::new_for_test();
        // "newer" is highlighted (set optimistically at dispatch); a Started
        // for an older press still sitting in the queue must not steal the
        // highlight back (#111).
        let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
            sound_id: "newer".into(),
        }));
        let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
            sound_id: "older".into(),
        }));
        assert_eq!(app.playing(), Some("newer"));
    }

    #[test]
    fn stale_playback_finished_does_not_clear_newer_playing() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
            sound_id: "newer".into(),
        }));
        // A Finished event for an already-replaced sound must not blank the
        // highlight of the sound that superseded it (issue #111).
        let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackFinished {
            sound_id: "older".into(),
            generation: 0,
        }));
        assert_eq!(app.playing(), Some("newer"));
    }

    #[test]
    fn play_sound_sets_playing_immediately() {
        let mut app = HonkHonk::new_for_test();
        let (handle, _evt_tx) = crate::audio::test_handle();
        app.audio = Some(handle);

        let dir = tempfile::tempdir().expect("tempdir");
        let wav_path = dir.path().join("honk.wav");
        write_test_wav(&wav_path);
        app.sounds = vec![SoundEntry {
            id: "wav1".into(),
            name: "Honk".into(),
            path: wav_path,
            format: crate::state::AudioFormat::Wav,
            duration_ms: Some(100),
            category: "Test".into(),
        }];

        // The tile highlight must track the press itself, not wait for the
        // engine's PlaybackStarted to round-trip through the event queue
        // (issue #111).
        let _ = app.update(Message::PlaySound("wav1".into()));
        assert_eq!(app.playing(), Some("wav1"));
    }

    #[test]
    fn drain_audio_events_processes_entire_backlog() {
        let mut app = HonkHonk::new_for_test();
        let (handle, evt_tx) = crate::audio::test_handle();
        app.audio = Some(handle);

        // Simulate the queue state after spamming tiles: stale started/finished
        // pairs piled up behind progress events (issue #111). One drain call
        // must consume them all so the UI reflects the latest engine state.
        let events = [
            AudioEvent::PlaybackStarted {
                sound_id: "a".into(),
            },
            AudioEvent::PlaybackFinished {
                sound_id: "a".into(),
                generation: 0,
            },
            AudioEvent::PlaybackStarted {
                sound_id: "b".into(),
            },
            AudioEvent::Progress(0.25),
            AudioEvent::PlaybackFinished {
                sound_id: "b".into(),
                generation: 0,
            },
            AudioEvent::PlaybackStarted {
                sound_id: "c".into(),
            },
            AudioEvent::Progress(0.5),
        ];
        for e in events {
            evt_tx.send(e).expect("send event");
        }

        let _ = app.drain_audio_events();

        assert_eq!(app.playing(), Some("c"));
        assert!((app.progress() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn shortcut_activation_sets_playing_immediately() {
        let mut app = HonkHonk::new_for_test();
        let (handle, _evt_tx) = crate::audio::test_handle();
        app.audio = Some(handle);

        let dir = tempfile::tempdir().expect("tempdir");
        let wav_path = dir.path().join("honk.wav");
        write_test_wav(&wav_path);
        app.sounds = vec![SoundEntry {
            id: "wav1".into(),
            name: "Honk".into(),
            path: wav_path.clone(),
            format: crate::state::AudioFormat::Wav,
            duration_ms: Some(100),
            category: "Test".into(),
        }];
        app.slots.set(0, wav_path);

        let _ = app.update(Message::ShortcutActivated(0));
        assert_eq!(app.playing(), Some("wav1"));
    }

    #[test]
    fn play_sound_no_op_for_unknown_id() {
        let mut app = HonkHonk::new_for_test();
        // Should not panic, just return Task::none()
        let _ = app.update(Message::PlaySound("nonexistent-id".into()));
        assert!(app.playing().is_none());
    }

    #[test]
    fn from_tray_event_maps_correctly() {
        assert_eq!(
            Message::from_tray_event(TrayEvent::ToggleVisibility),
            Message::ToggleVisibility
        );
        assert_eq!(Message::from_tray_event(TrayEvent::Quit), Message::Quit);
    }

    #[test]
    fn search_changed_updates_query() {
        let mut app = HonkHonk::new_for_test();
        assert_eq!(app.search_query(), "");
        let _ = app.update(Message::SearchChanged("honk".into()));
        assert_eq!(app.search_query(), "honk");
    }

    #[test]
    fn volume_changed_updates_config() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::VolumeChanged(0.42));
        assert!((app.config.volume - 0.42).abs() < f32::EPSILON);
    }

    #[test]
    fn frame_message_advances_display_progress_while_playing() {
        use std::time::{Duration, Instant};
        let mut app = HonkHonk::new_for_test();
        let t0 = Instant::now();
        app.playhead = Some(crate::ui::playhead::PlayheadClock::new(
            Duration::from_secs(10),
            t0,
        ));
        let _ = app.update(Message::Frame(t0 + Duration::from_secs(5)));
        assert!(
            (app.display_progress - 0.5).abs() < 1e-3,
            "got {}",
            app.display_progress
        );
    }

    #[test]
    fn frame_message_is_noop_when_idle() {
        use std::time::{Duration, Instant};
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::Frame(Instant::now() + Duration::from_secs(1)));
        assert_eq!(app.display_progress, 0.0);
    }

    #[test]
    fn progress_event_does_not_drive_display_progress() {
        // The smooth playhead is wall-clock driven (`Message::Frame`), NOT the
        // raw 10 Hz `Progress` anchor — re-anchoring to stale samples caused the
        // left/right jitter (#138). A Progress event updates the raw anchor but
        // must leave the smooth `display_progress` untouched.
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::AudioEvent(AudioEvent::Progress(0.65)));
        assert!((app.progress() - 0.65).abs() < f32::EPSILON);
        assert_eq!(app.display_progress, 0.0);
    }

    #[test]
    fn playback_finished_clears_playhead_and_display_progress() {
        use std::time::{Duration, Instant};
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
            sound_id: "test".into(),
        }));
        app.playhead = Some(crate::ui::playhead::PlayheadClock::new(
            Duration::from_secs(5),
            Instant::now(),
        ));
        let _ = app.update(Message::AudioEvent(AudioEvent::Progress(0.8)));
        let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackFinished {
            sound_id: "test".into(),
            generation: 0,
        }));
        assert!(app.playhead.is_none());
        assert_eq!(app.display_progress, 0.0);
    }

    #[test]
    fn re_pressing_same_sound_keeps_playhead_alive() {
        // Re-pressing the SAME tile while it is still playing must re-trigger the
        // playhead. The engine replaces the active voice and emits a
        // `PlaybackFinished` for the *displaced* voice carrying the SAME
        // `sound_id`; the app must not mistake that stale event for a genuine end
        // and tear down the freshly-created playhead, freezing it at 0 (#149).
        let mut app = HonkHonk::new_for_test();
        let (handle, _evt_tx) = crate::audio::test_handle();
        app.audio = Some(handle);

        let dir = tempfile::tempdir().expect("tempdir");
        let wav_path = dir.path().join("honk.wav");
        write_test_wav(&wav_path);
        app.sounds = vec![SoundEntry {
            id: "wav1".into(),
            name: "Honk".into(),
            path: wav_path,
            format: crate::state::AudioFormat::Wav,
            duration_ms: Some(100),
            category: "Test".into(),
        }];

        // First press → decode dispatched; the playhead is created when its
        // `Decoded` (generation 1) lands. Decode is off-thread now, so we feed
        // the matching `Decoded` directly.
        let sound = app.sounds[0].clone();
        let _ = app.request_play(&sound, false);
        let decoded = crate::audio::decode(&sound.path).expect("decode test wav");
        let to_pcm = |d: &crate::audio::DecodedAudio| crate::audio::CachedPcm {
            samples: std::sync::Arc::new(d.samples.clone()),
            sample_rate: d.sample_rate,
            channels: d.channels,
            duration: d.duration,
        };
        let _ = app.update(Message::Decoded {
            generation: app.play_generation,
            id: "wav1".into(),
            result: Ok(to_pcm(&decoded)),
        });
        // Second press re-triggers the same sound. The PCM is now cached, so
        // `request_play` fires synchronously and creates a fresh playhead
        // (generation 2) without another decode.
        let _ = app.request_play(&sound, false);
        assert!(
            app.playhead.is_some(),
            "re-press should create a fresh playhead"
        );

        // The displaced first voice's Finished (older generation) arrives on the
        // next drain — it must NOT clear the re-triggered playhead.
        let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackFinished {
            sound_id: "wav1".into(),
            generation: 1,
        }));
        assert!(
            app.playhead.is_some(),
            "stale displaced Finished must not clear the re-triggered playhead"
        );
        assert_eq!(app.playing(), Some("wav1"));

        // The genuine end of the current voice (matching generation) still clears.
        let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackFinished {
            sound_id: "wav1".into(),
            generation: 2,
        }));
        assert!(app.playhead.is_none(), "genuine end clears the playhead");
        assert_eq!(app.playing(), None);
    }

    #[test]
    fn progress_event_updates_progress() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::AudioEvent(AudioEvent::Progress(0.65)));
        assert!((app.progress() - 0.65).abs() < f32::EPSILON);
    }

    #[test]
    fn playback_finished_resets_progress() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
            sound_id: "test".into(),
        }));
        let _ = app.update(Message::AudioEvent(AudioEvent::Progress(0.8)));
        let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackFinished {
            sound_id: "test".into(),
            generation: 0,
        }));
        assert!((app.progress() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn search_filters_sounds() {
        let mut app = HonkHonk::new_for_test();
        app.sounds = vec![
            SoundEntry {
                id: "aaa".into(),
                name: "Goose Honk".into(),
                path: "/a.mp3".into(),
                format: crate::state::AudioFormat::Mp3,
                duration_ms: Some(1000),
                category: "Honk".into(),
            },
            SoundEntry {
                id: "bbb".into(),
                name: "Vine Boom".into(),
                path: "/b.mp3".into(),
                format: crate::state::AudioFormat::Mp3,
                duration_ms: Some(1000),
                category: "Memes".into(),
            },
        ];
        let _ = app.update(Message::SearchChanged("goose".into()));
        assert_eq!(app.filtered_sounds().len(), 1);
        assert_eq!(app.filtered_sounds()[0].id, "aaa");
    }

    #[test]
    fn search_is_case_insensitive() {
        let mut app = HonkHonk::new_for_test();
        app.sounds = vec![SoundEntry {
            id: "aaa".into(),
            name: "Goose Honk".into(),
            path: "/a.mp3".into(),
            format: crate::state::AudioFormat::Mp3,
            duration_ms: Some(1000),
            category: "Honk".into(),
        }];
        let _ = app.update(Message::SearchChanged("GOOSE".into()));
        assert_eq!(app.filtered_sounds().len(), 1);
    }

    #[test]
    fn search_and_category_filter_stack() {
        let mut app = HonkHonk::new_for_test();
        app.sounds = vec![
            SoundEntry {
                id: "aaa".into(),
                name: "Goose Honk".into(),
                path: "/a.mp3".into(),
                format: crate::state::AudioFormat::Mp3,
                duration_ms: Some(1000),
                category: "Honk".into(),
            },
            SoundEntry {
                id: "bbb".into(),
                name: "Goose Boom".into(),
                path: "/b.mp3".into(),
                format: crate::state::AudioFormat::Mp3,
                duration_ms: Some(1000),
                category: "Memes".into(),
            },
        ];
        let _ = app.update(Message::SelectCategory(Some("Honk".into())));
        let _ = app.update(Message::SearchChanged("goose".into()));
        assert_eq!(app.filtered_sounds().len(), 1);
        assert_eq!(app.filtered_sounds()[0].id, "aaa");
    }

    #[test]
    fn volume_changed_persists_in_config_across_sounds() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::VolumeChanged(0.15));
        assert!((app.config.volume - 0.15).abs() < f32::EPSILON);

        let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackFinished {
            sound_id: "old".into(),
            generation: 0,
        }));

        assert!(
            (app.config.volume - 0.15).abs() < f32::EPSILON,
            "config.volume should survive playback cycle"
        );
    }

    #[test]
    fn shortcuts_ready_sets_status_active() {
        let mut app = HonkHonk::new_for_test();
        assert_eq!(app.shortcuts_status(), &ShortcutsStatus::Initializing);
        let _ = app.update(Message::ShortcutsReady);
        assert_eq!(app.shortcuts_status(), &ShortcutsStatus::Active);
    }

    #[test]
    fn shortcuts_unavailable_sets_status() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::ShortcutsUnavailable("portal not found".into()));
        assert!(matches!(
            app.shortcuts_status(),
            ShortcutsStatus::Unavailable(_)
        ));
    }

    #[test]
    fn shortcuts_unavailable_contains_reason() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::ShortcutsUnavailable("no portal".into()));
        let ShortcutsStatus::Unavailable(reason) = app.shortcuts_status() else {
            panic!("expected Unavailable");
        };
        assert!(!reason.is_empty());
    }

    #[test]
    fn dismiss_warning_sets_flag() {
        let mut app = HonkHonk::new_for_test();
        assert!(!app.shortcuts_warning_dismissed());
        let _ = app.update(Message::DismissShortcutsWarning);
        assert!(app.shortcuts_warning_dismissed());
    }

    #[test]
    fn shortcut_activated_with_empty_slot_is_noop() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::ShortcutActivated(0));
        assert!(app.playing().is_none());
    }

    #[test]
    fn shortcut_activated_with_assigned_slot_does_not_panic() {
        let mut app = HonkHonk::new_for_test();
        let path = std::path::PathBuf::from("/sounds/honk.mp3");
        app.sounds = vec![SoundEntry {
            id: "honk-id".into(),
            name: "Honk".into(),
            path: path.clone(),
            format: crate::state::AudioFormat::Mp3,
            duration_ms: Some(500),
            category: "Honk".into(),
        }];
        let _ = app.update(Message::AssignSlot(0, path.clone()));
        // audio=None means no audio command is sent; slot must remain assigned after activation
        let _ = app.update(Message::ShortcutActivated(0));
        assert_eq!(app.slots().get(0), Some(&path));
    }

    #[test]
    fn assign_slot_updates_slot_map() {
        let mut app = HonkHonk::new_for_test();
        let path = std::path::PathBuf::from("/sounds/boom.mp3");
        let _ = app.update(Message::AssignSlot(3, path.clone()));
        assert_eq!(app.slots().get(3), Some(&path));
    }

    #[test]
    fn clear_slot_removes_assignment() {
        let mut app = HonkHonk::new_for_test();
        let path = std::path::PathBuf::from("/sounds/boom.mp3");
        let _ = app.update(Message::AssignSlot(3, path.clone()));
        let _ = app.update(Message::ClearSlot(3));
        assert!(app.slots().get(3).is_none());
    }

    #[test]
    fn open_context_menu_sets_sound_id() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::OpenContextMenu("some-id".into()));
        assert_eq!(app.context_menu(), Some("some-id"));
    }

    #[test]
    fn close_context_menu_clears_it() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::OpenContextMenu("some-id".into()));
        let _ = app.update(Message::CloseContextMenu);
        assert!(app.context_menu().is_none());
    }

    #[test]
    fn show_slots_sets_view_mode() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::ShowSlots);
        assert_eq!(app.view_mode(), ViewMode::SlotManager);
        assert!(app.selected_slot().is_none());
    }

    #[test]
    fn show_main_resets_view_mode() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::SelectSlot(3));
        let _ = app.update(Message::ShowSlots);
        let _ = app.update(Message::ShowMain);
        assert_eq!(app.view_mode(), ViewMode::Main);
        assert!(app.selected_slot().is_none());
    }

    #[test]
    fn select_slot_sets_selected() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::SelectSlot(3));
        assert_eq!(app.selected_slot(), Some(3));
    }

    #[test]
    fn clear_slot_keeps_selection_showing_empty_panel() {
        let mut app = HonkHonk::new_for_test();
        let path = std::path::PathBuf::from("/tmp/test.mp3");
        let _ = app.update(Message::AssignSlot(3, path.clone()));
        let _ = app.update(Message::SelectSlot(3));
        let _ = app.update(Message::ClearSlot(3));
        assert_eq!(app.selected_slot(), Some(3));
        assert!(app.slots().get(3).is_none());
    }

    #[test]
    fn shortcut_bindings_updated_stores_triggers() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::ShortcutBindingsUpdated(vec![
            (0, "Meta+1".into()),
            (4, "Ctrl+5".into()),
        ]));
        assert_eq!(app.slot_triggers()[0].as_deref(), Some("Meta+1"));
        assert_eq!(app.slot_triggers()[4].as_deref(), Some("Ctrl+5"));
        assert!(app.slot_triggers()[1].is_none());
    }

    #[test]
    fn shortcut_bindings_updated_ignores_out_of_range() {
        let mut app = HonkHonk::new_for_test();
        // slot index 20 is out of range — should not panic
        let _ = app.update(Message::ShortcutBindingsUpdated(vec![(20, "X".into())]));
    }

    #[test]
    fn durations_loaded_fills_matching_sound_entries() {
        let mut app = HonkHonk::new_for_test();
        app.sounds = vec![SoundEntry {
            id: "abc123".into(),
            name: "Honk".into(),
            path: "/tmp/honk.wav".into(),
            format: crate::state::AudioFormat::Wav,
            duration_ms: None,
            category: "Honk".into(),
        }];
        let map = std::collections::HashMap::from([("abc123".to_string(), 1500u64)]);
        let _ = app.update(Message::DurationsLoaded(map));
        assert_eq!(app.sounds[0].duration_ms, Some(1500));
        assert!(app.durations_loaded);
    }

    #[test]
    fn durations_loaded_ignores_unmatched_ids() {
        let mut app = HonkHonk::new_for_test();
        app.sounds = vec![SoundEntry {
            id: "abc123".into(),
            name: "Honk".into(),
            path: "/tmp/honk.wav".into(),
            format: crate::state::AudioFormat::Wav,
            duration_ms: None,
            category: "Honk".into(),
        }];
        let map = std::collections::HashMap::from([("no-match".to_string(), 999u64)]);
        let _ = app.update(Message::DurationsLoaded(map));
        assert_eq!(app.sounds[0].duration_ms, None);
    }

    #[test]
    fn show_settings_sets_view_mode() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::ShowSettings);
        assert!(matches!(app.view_mode, ViewMode::Settings));
    }

    #[test]
    fn show_settings_defaults_section_to_audio() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::ShowSettings);
        assert!(matches!(app.settings_section, SettingsSection::Audio));
    }

    #[test]
    fn show_settings_section_updates_active_section() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::ShowSettingsSection(SettingsSection::Library));
        assert!(matches!(app.settings_section, SettingsSection::Library));
    }

    #[test]
    fn show_main_from_settings_resets_view_mode() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::ShowSettings);
        let _ = app.update(Message::ShowMain);
        assert!(matches!(app.view_mode, ViewMode::Main));
    }

    #[test]
    fn rescan_library_resets_durations_loaded() {
        let mut app = HonkHonk::new_for_test();
        app.durations_loaded = true;
        let _ = app.update(Message::RescanLibrary);
        assert!(
            !app.durations_loaded,
            "RescanLibrary must reset durations_loaded"
        );
    }

    #[test]
    fn remove_sound_directory_removes_path() {
        let mut app = HonkHonk::new_for_test();
        let path = std::path::PathBuf::from("/tmp/hh_test_sounds");
        app.config.sound_directories.push(path.clone());
        let _ = app.update(Message::RemoveSoundDirectory(path.clone()));
        assert!(!app.config.sound_directories.contains(&path));
    }

    #[test]
    fn sound_directory_pick_some_appends_to_config() {
        let mut app = HonkHonk::new_for_test();
        let path = std::path::PathBuf::from("/tmp/hh_new_sounds");
        let before = app.config.sound_directories.len();
        let _ = app.update(Message::SoundDirectoryPickResult(Some(path.clone())));
        assert_eq!(app.config.sound_directories.len(), before + 1);
        assert!(app.config.sound_directories.contains(&path));
    }

    #[test]
    fn sound_directory_pick_none_is_noop() {
        let mut app = HonkHonk::new_for_test();
        let before = app.config.sound_directories.clone();
        let _ = app.update(Message::SoundDirectoryPickResult(None));
        assert_eq!(app.config.sound_directories, before);
    }

    #[test]
    fn theme_changed_updates_config() {
        let mut app = HonkHonk::new_for_test();
        assert_eq!(app.config.theme, crate::ui::theme::Theme::Dark);
        let _ = app.update(Message::ThemeChanged(crate::ui::theme::Theme::Light));
        assert_eq!(app.config.theme, crate::ui::theme::Theme::Light);
        let _ = app.update(Message::ThemeChanged(crate::ui::theme::Theme::System));
        assert_eq!(app.config.theme, crate::ui::theme::Theme::System);
    }

    #[test]
    fn density_changed_updates_config() {
        let mut app = HonkHonk::new_for_test();
        assert_eq!(app.config.density, crate::state::config::Density::Regular);
        let _ = app.update(Message::DensityChanged(
            crate::state::config::Density::Compact,
        ));
        assert_eq!(app.config.density, crate::state::config::Density::Compact);
        let _ = app.update(Message::DensityChanged(
            crate::state::config::Density::Comfy,
        ));
        assert_eq!(app.config.density, crate::state::config::Density::Comfy);
    }

    #[test]
    fn mic_passthrough_changed_message_carries_bool() {
        let msg = Message::MicPassthroughChanged(false);
        assert!(matches!(msg, Message::MicPassthroughChanged(false)));
    }

    #[test]
    fn mic_passthrough_level_changed_message_carries_f32() {
        let msg = Message::MicPassthroughLevelChanged(0.5);
        assert!(matches!(msg, Message::MicPassthroughLevelChanged(_)));
    }

    #[test]
    fn renderer_changed_dispatches_to_update() {
        use crate::state::Renderer;
        let mut app = HonkHonk::new_for_test();
        // default is Wgpu
        let _ = app.update(Message::RendererChanged(Renderer::TinySkia));
        assert_eq!(app.config.renderer, Renderer::TinySkia);
        let _ = app.update(Message::RendererChanged(Renderer::Wgpu));
        assert_eq!(app.config.renderer, Renderer::Wgpu);
    }

    #[test]
    fn renderer_changed_no_op_when_value_unchanged() {
        use crate::state::Renderer;
        let mut app = HonkHonk::new_for_test();
        // start: Wgpu (default). Send TinySkia, verify change.
        let _ = app.update(Message::RendererChanged(Renderer::TinySkia));
        assert_eq!(app.config.renderer, Renderer::TinySkia);
        // send TinySkia again — state must not corrupt
        let _ = app.update(Message::RendererChanged(Renderer::TinySkia));
        assert_eq!(app.config.renderer, Renderer::TinySkia);
    }

    #[test]
    fn monitor_device_changed_to_none_clears_config() {
        let mut app = HonkHonk::new_for_test();
        app.config = AppConfig {
            monitor_device: Some("alsa_output.pci-test".into()),
            ..AppConfig::default()
        };
        let _ = app.update(Message::MonitorDeviceChanged(None));
        assert!(app.config.monitor_device.is_none());
    }

    #[test]
    fn monitor_device_changed_to_some_sets_config() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::MonitorDeviceChanged(Some(
            "alsa_output.pci-test".into(),
        )));
        assert_eq!(
            app.config.monitor_device.as_deref(),
            Some("alsa_output.pci-test")
        );
    }

    #[test]
    fn monitor_device_changed_same_value_is_idempotent() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::MonitorDeviceChanged(None));
        let _ = app.update(Message::MonitorDeviceChanged(None));
        assert!(app.config.monitor_device.is_none());
    }

    #[test]
    fn audio_event_output_devices_changed_updates_monitor_devices() {
        let mut app = HonkHonk::new_for_test();
        let devices = vec![("alsa_output.pci".into(), "Built-in Audio".into())];
        let _ = app.update(Message::AudioEvent(AudioEvent::OutputDevicesChanged(
            devices.clone(),
        )));
        assert_eq!(app.monitor_devices, devices);
    }

    #[test]
    fn output_devices_changed_does_not_clear_device_before_it_is_first_seen() {
        // Startup race: saved device not yet enumerated — must NOT clear config
        let mut app = HonkHonk::new_for_test();
        app.config = AppConfig {
            monitor_device: Some("alsa_output.usb-headset".into()),
            ..AppConfig::default()
        };
        // monitor_devices is empty (startup) — first event only contains a different sink
        let _ = app.update(Message::AudioEvent(AudioEvent::OutputDevicesChanged(vec![
            ("alsa_output.hdmi".into(), "HDMI Audio".into()),
        ])));
        assert_eq!(
            app.config.monitor_device.as_deref(),
            Some("alsa_output.usb-headset"),
            "must not clear saved device before it has been enumerated"
        );
    }

    #[test]
    fn output_devices_changed_clears_device_after_it_disappears() {
        // Runtime removal: device was known, then removed — clear config
        let mut app = HonkHonk::new_for_test();
        app.config = AppConfig {
            monitor_device: Some("alsa_output.usb-headset".into()),
            ..AppConfig::default()
        };
        // First: device appears in list (now it's "seen")
        let _ = app.update(Message::AudioEvent(AudioEvent::OutputDevicesChanged(vec![
            ("alsa_output.usb-headset".into(), "USB Headset".into()),
        ])));
        assert_eq!(
            app.config.monitor_device.as_deref(),
            Some("alsa_output.usb-headset")
        );
        // Then: device disappears (unplugged)
        let _ = app.update(Message::AudioEvent(AudioEvent::OutputDevicesChanged(vec![
            ("alsa_output.pci".into(), "Built-in Audio".into()),
        ])));
        assert!(
            app.config.monitor_device.is_none(),
            "must clear config when device was visible and then removed"
        );
    }

    #[test]
    fn input_device_changed_to_none_clears_config() {
        let mut app = HonkHonk::new_for_test();
        app.config = AppConfig {
            input_device: Some("alsa_input.pci-test".into()),
            ..AppConfig::default()
        };
        let _ = app.update(Message::InputDeviceChanged(None));
        assert!(app.config.input_device.is_none());
    }

    #[test]
    fn input_device_changed_to_some_sets_config() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::InputDeviceChanged(Some(
            "alsa_input.usb-mic".into(),
        )));
        assert_eq!(
            app.config.input_device.as_deref(),
            Some("alsa_input.usb-mic")
        );
    }

    #[test]
    fn input_device_changed_same_value_is_idempotent() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::InputDeviceChanged(None));
        let _ = app.update(Message::InputDeviceChanged(None));
        assert!(app.config.input_device.is_none());
    }

    #[test]
    fn audio_event_input_devices_changed_updates_input_devices() {
        let mut app = HonkHonk::new_for_test();
        let devices = vec![("alsa_input.usb-mic".into(), "USB Microphone".into())];
        let _ = app.update(Message::AudioEvent(AudioEvent::InputDevicesChanged(
            devices.clone(),
        )));
        assert_eq!(app.input_devices, devices);
    }

    #[test]
    fn input_devices_changed_does_not_clear_device_before_it_is_first_seen() {
        // Startup race: saved mic not yet enumerated — must NOT clear config.
        let mut app = HonkHonk::new_for_test();
        app.config = AppConfig {
            input_device: Some("alsa_input.usb-mic".into()),
            ..AppConfig::default()
        };
        let _ = app.update(Message::AudioEvent(AudioEvent::InputDevicesChanged(vec![
            ("alsa_input.onboard".into(), "Onboard Mic".into()),
        ])));
        assert_eq!(
            app.config.input_device.as_deref(),
            Some("alsa_input.usb-mic"),
            "must not clear saved mic before it has been enumerated"
        );
    }

    #[test]
    fn input_devices_changed_clears_device_after_it_disappears() {
        // Runtime removal: mic was known, then unplugged — clear config.
        let mut app = HonkHonk::new_for_test();
        app.config = AppConfig {
            input_device: Some("alsa_input.usb-mic".into()),
            ..AppConfig::default()
        };
        let _ = app.update(Message::AudioEvent(AudioEvent::InputDevicesChanged(vec![
            ("alsa_input.usb-mic".into(), "USB Microphone".into()),
        ])));
        assert_eq!(
            app.config.input_device.as_deref(),
            Some("alsa_input.usb-mic")
        );
        let _ = app.update(Message::AudioEvent(AudioEvent::InputDevicesChanged(vec![
            ("alsa_input.onboard".into(), "Onboard Mic".into()),
        ])));
        assert!(
            app.config.input_device.is_none(),
            "must clear config when mic was visible and then removed"
        );
    }

    #[test]
    fn output_devices_changed_keeps_valid_monitor_device() {
        let mut app = HonkHonk::new_for_test();
        app.config = AppConfig {
            monitor_device: Some("alsa_output.pci".into()),
            ..AppConfig::default()
        };
        // Device appears, then stays in subsequent updates
        let _ = app.update(Message::AudioEvent(AudioEvent::OutputDevicesChanged(vec![
            ("alsa_output.pci".into(), "Built-in Audio".into()),
        ])));
        let _ = app.update(Message::AudioEvent(AudioEvent::OutputDevicesChanged(vec![
            ("alsa_output.pci".into(), "Built-in Audio".into()),
            ("alsa_output.usb".into(), "USB Headset".into()),
        ])));
        assert_eq!(
            app.config.monitor_device.as_deref(),
            Some("alsa_output.pci")
        );
    }

    #[test]
    fn audio_event_output_devices_changed_replaces_previous_list() {
        let mut app = HonkHonk::new_for_test();
        let first = vec![("alsa_output.pci".into(), "Built-in Audio".into())];
        let second = vec![
            ("alsa_output.pci".into(), "Built-in Audio".into()),
            ("alsa_output.usb".into(), "USB Headset".into()),
        ];
        let _ = app.update(Message::AudioEvent(AudioEvent::OutputDevicesChanged(first)));
        let _ = app.update(Message::AudioEvent(AudioEvent::OutputDevicesChanged(
            second.clone(),
        )));
        assert_eq!(app.monitor_devices, second);
    }

    #[test]
    fn open_shortcut_config_sends_command_when_handle_present() {
        use tokio::sync::mpsc;
        let mut app = HonkHonk::new_for_test();
        let (tx, mut rx) = mpsc::channel(8);
        app.shortcut_config.set_portal_sender(tx);
        app.shortcut_config.set_portal_v2_available(true);
        let _ = app.update(Message::OpenShortcutConfig);
        assert!(rx.try_recv().is_ok());
    }

    #[test]
    fn open_shortcut_config_is_noop_when_no_handle() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::OpenShortcutConfig);
    }

    #[test]
    fn escape_first_press_consumes_search_focus_flag_without_clearing_query() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::SearchChanged("honk".into()));
        assert!(app.search_had_focus);
        let _ = app.update(Message::EscapePressed);
        assert!(!app.search_had_focus);
        assert_eq!(app.search_query(), "honk");
    }

    #[test]
    fn escape_second_press_clears_query() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::SearchChanged("honk".into()));
        let _ = app.update(Message::EscapePressed); // consume focus flag
        let _ = app.update(Message::EscapePressed); // clear query
        assert_eq!(app.search_query(), "");
    }

    #[test]
    fn escape_closes_context_menu_without_consuming_search_focus() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::SearchChanged("honk".into()));
        let _ = app.update(Message::OpenContextMenu("test-id".into()));
        let _ = app.update(Message::EscapePressed);
        assert!(app.context_menu().is_none());
        assert!(app.search_had_focus); // not consumed — menu took priority
    }

    #[test]
    fn search_changed_sets_search_had_focus() {
        let mut app = HonkHonk::new_for_test();
        assert!(!app.search_had_focus);
        let _ = app.update(Message::SearchChanged("test".into()));
        assert!(app.search_had_focus);
    }

    // Per-sound metadata tests

    #[test]
    fn toggle_favorite_sets_and_clears_favorite() {
        let mut app = HonkHonk::new_for_test();
        assert!(!app.sound_meta.is_favorite("id1"));
        let _ = app.update(Message::ToggleFavorite("id1".into()));
        assert!(app.sound_meta.is_favorite("id1"));
        let _ = app.update(Message::ToggleFavorite("id1".into()));
        assert!(!app.sound_meta.is_favorite("id1"));
    }

    #[test]
    fn open_sound_editor_stores_sound_id_and_draft_state() {
        let mut app = HonkHonk::new_for_test();
        app.sounds = vec![SoundEntry {
            id: "abc".into(),
            name: "Honk".into(),
            path: "/a.mp3".into(),
            format: crate::state::AudioFormat::Mp3,
            duration_ms: None,
            category: "General".into(),
        }];
        let _ = app.update(Message::OpenSoundEditor("abc".into()));
        assert_eq!(app.editor_sound_id(), Some("abc"));
        // draft volume defaults to 1.0 when no meta saved
        let eps = f32::EPSILON;
        assert!((app.editor_draft_volume - 1.0).abs() < eps);
    }

    #[test]
    fn open_sound_editor_clears_context_menu() {
        // Regression: opening the editor must dismiss the context menu so the
        // editor overlay surfaces immediately (CodeRabbit review thread).
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::OpenContextMenu("some-id".into()));
        assert!(app.context_menu().is_some());
        let _ = app.update(Message::OpenSoundEditor("some-id".into()));
        assert!(app.context_menu().is_none());
        assert_eq!(app.editor_sound_id(), Some("some-id"));
    }

    #[test]
    fn escape_dismisses_editor_before_search_focus() {
        // Regression: Esc should close the editor overlay, not consume search focus.
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::SearchChanged("honk".into()));
        let _ = app.update(Message::OpenSoundEditor("abc".into()));
        assert!(app.editor_sound_id().is_some());
        let _ = app.update(Message::EscapePressed);
        assert!(app.editor_sound_id().is_none());
        // search_had_focus must NOT be consumed — editor took priority
        assert!(app.search_had_focus);
    }

    #[test]
    fn close_sound_editor_clears_editor_state() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::OpenSoundEditor("abc".into()));
        let _ = app.update(Message::CloseSoundEditor);
        assert!(app.editor_sound_id().is_none());
    }

    #[test]
    fn sound_editor_name_changed_updates_draft() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::SoundEditorNameChanged("New Name".into()));
        assert_eq!(app.editor_draft_name, "New Name");
    }

    #[test]
    fn sound_editor_volume_changed_updates_draft() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::SoundEditorVolumeChanged("id".into(), 1.5));
        let eps = 1e-5_f32;
        assert!((app.editor_draft_volume - 1.5).abs() < eps);
    }

    #[test]
    fn sound_editor_volume_changed_clamps_above_two() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::SoundEditorVolumeChanged("id".into(), 5.0));
        let eps = f32::EPSILON;
        assert!((app.editor_draft_volume - 2.0).abs() < eps);
    }

    #[test]
    fn save_sound_meta_persists_volume_and_name() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::SoundEditorNameChanged("Renamed".into()));
        let _ = app.update(Message::SoundEditorVolumeChanged("id1".into(), 1.25));
        let _ = app.update(Message::SaveSoundMeta("id1".into()));
        let meta = app.sound_meta.get("id1");
        assert_eq!(meta.display_name.as_deref(), Some("Renamed"));
        let eps = 1e-5_f32;
        assert!((meta.volume - 1.25).abs() < eps);
        assert!(
            app.editor_sound_id().is_none(),
            "editor must close after save"
        );
    }

    #[test]
    fn save_sound_meta_blank_name_stores_none() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::SoundEditorNameChanged("  ".into()));
        let _ = app.update(Message::SaveSoundMeta("id1".into()));
        assert!(app.sound_meta.get("id1").display_name.is_none());
    }

    #[test]
    fn save_sound_meta_preserves_existing_favorite_flag() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::ToggleFavorite("id1".into()));
        assert!(app.sound_meta.is_favorite("id1"));
        let _ = app.update(Message::SoundEditorVolumeChanged("id1".into(), 1.5));
        let _ = app.update(Message::SaveSoundMeta("id1".into()));
        // favorite must still be true after saving from editor
        assert!(app.sound_meta.is_favorite("id1"));
    }

    #[test]
    fn filtered_sounds_favorites_tab_shows_only_starred_sounds() {
        let mut app = HonkHonk::new_for_test();
        app.sounds = vec![
            SoundEntry {
                id: "fav".into(),
                name: "Favourite".into(),
                path: "/fav.mp3".into(),
                format: crate::state::AudioFormat::Mp3,
                duration_ms: None,
                category: "General".into(),
            },
            SoundEntry {
                id: "nonfav".into(),
                name: "Regular".into(),
                path: "/nonfav.mp3".into(),
                format: crate::state::AudioFormat::Mp3,
                duration_ms: None,
                category: "General".into(),
            },
        ];
        let _ = app.update(Message::ToggleFavorite("fav".into()));
        let _ = app.update(Message::SelectCategory(Some(FAVORITES_TAB.to_owned())));
        let filtered = app.filtered_sounds();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "fav");
    }

    #[test]
    fn filtered_sounds_all_tab_includes_all_when_no_favorites_selected() {
        let mut app = HonkHonk::new_for_test();
        app.sounds = vec![
            SoundEntry {
                id: "a".into(),
                name: "A".into(),
                path: "/a.mp3".into(),
                format: crate::state::AudioFormat::Mp3,
                duration_ms: None,
                category: "X".into(),
            },
            SoundEntry {
                id: "b".into(),
                name: "B".into(),
                path: "/b.mp3".into(),
                format: crate::state::AudioFormat::Mp3,
                duration_ms: None,
                category: "Y".into(),
            },
        ];
        let _ = app.update(Message::ToggleFavorite("a".into()));
        // Select All tab
        let _ = app.update(Message::SelectCategory(None));
        assert_eq!(app.filtered_sounds().len(), 2);
    }

    #[test]
    fn unstarring_last_favorite_while_on_favorites_tab_resets_to_all() {
        // Regression: removing the last favorite while on the Favorites tab
        // would leave active_category pointing to the now-invisible chip,
        // showing an empty list with no way to navigate back.
        let mut app = HonkHonk::new_for_test();
        app.sounds = vec![SoundEntry {
            id: "only".into(),
            name: "Only Fav".into(),
            path: "/only.mp3".into(),
            format: crate::state::AudioFormat::Mp3,
            duration_ms: None,
            category: "General".into(),
        }];
        let _ = app.update(Message::ToggleFavorite("only".into()));
        let _ = app.update(Message::SelectCategory(Some(FAVORITES_TAB.to_owned())));
        assert_eq!(app.active_category(), Some(FAVORITES_TAB));
        // Unstar the only favorite — must fall back to "All"
        let _ = app.update(Message::ToggleFavorite("only".into()));
        assert!(
            app.active_category().is_none(),
            "active_category must reset to All when last favorite is removed"
        );
    }

    #[test]
    fn search_matches_display_name_override() {
        // Regression: sounds renamed via the editor were invisible to search
        // because filtered_sounds() only matched SoundEntry.name, not the
        // display_name stored in SoundMetaStore.
        let mut app = HonkHonk::new_for_test();
        app.sounds = vec![SoundEntry {
            id: "id1".into(),
            name: "goose_honk_v2.wav".into(),
            path: "/id1.wav".into(),
            format: crate::state::AudioFormat::Wav,
            duration_ms: None,
            category: "Animals".into(),
        }];
        // Rename the sound via the editor workflow
        app.sound_meta
            .set_display_name("id1", Some("Angry Goose".to_owned()));
        // Searching for the display name override must find the sound
        let _ = app.update(Message::SearchChanged("angry".into()));
        assert_eq!(
            app.filtered_sounds().len(),
            1,
            "renamed sound must be discoverable by its display name"
        );
        // Searching for the original filename still works too
        let _ = app.update(Message::SearchChanged("goose_honk".into()));
        assert_eq!(app.filtered_sounds().len(), 1);
    }

    #[test]
    fn playing_a_sound_caches_its_waveform_envelope() {
        let mut app = HonkHonk::new_for_test();
        let (handle, _evt_tx) = crate::audio::test_handle();
        app.audio = Some(handle);

        let dir = tempfile::tempdir().expect("tempdir");
        let wav_path = dir.path().join("honk.wav");
        write_test_wav(&wav_path);
        app.sounds = vec![SoundEntry {
            id: "wav1".into(),
            name: "Honk".into(),
            path: wav_path,
            format: crate::state::AudioFormat::Wav,
            duration_ms: Some(100),
            category: "Test".into(),
        }];

        // Drive playback through the async path: `request_play` bumps the
        // generation and (on a cold cache) returns a decode `Task`; we feed the
        // matching `Decoded` directly, since the engine decode is off-thread now.
        let sound = app.sounds[0].clone();
        let _ = app.request_play(&sound, false);
        let decoded = crate::audio::decode(&sound.path).expect("decode test wav");
        let _ = app.update(Message::Decoded {
            generation: app.play_generation,
            id: "wav1".into(),
            result: Ok(crate::audio::CachedPcm {
                samples: std::sync::Arc::new(decoded.samples),
                sample_rate: decoded.sample_rate,
                channels: decoded.channels,
                duration: decoded.duration,
            }),
        });
        let env = app
            .audio_store
            .envelope("wav1")
            .expect("envelope should be cached after play");
        assert_eq!(
            env.bars(crate::ui::waveform::WAVEFORM_BARS).len(),
            crate::ui::waveform::WAVEFORM_BARS
        );
    }

    #[test]
    fn toggle_effects_panel_opens_then_closes() {
        let mut app = HonkHonk::new_for_test();
        assert!(!app.effects_panel.is_open());
        let _ = app.update(Message::ToggleEffectsPanel);
        assert!(app.effects_panel.is_open());
        assert!(app.effects_panel.is_animating());
        let _ = app.update(Message::ToggleEffectsPanel);
        assert!(!app.effects_panel.is_open());
    }

    #[test]
    fn close_effects_panel_closes_open_panel() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::ToggleEffectsPanel);
        assert!(app.effects_panel.is_open());
        let _ = app.update(Message::CloseEffectsPanel);
        assert!(!app.effects_panel.is_open());
    }

    #[test]
    fn escape_closes_open_effects_panel() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::ToggleEffectsPanel);
        assert!(app.effects_panel.is_open());
        let _ = app.update(Message::EscapePressed);
        assert!(!app.effects_panel.is_open());
    }

    #[test]
    fn frame_settles_panel_progress_after_slide() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::ToggleEffectsPanel); // opening
        let later = Instant::now() + crate::ui::side_panel::SLIDE_DURATION;
        let _ = app.update(Message::Frame(later));
        assert_eq!(app.panel_progress, 1.0);
        assert!(!app.effects_panel.is_animating());
    }

    #[test]
    fn escape_during_close_does_not_clear_search() {
        // Regression: while the drawer is mid-close, is_open() is false but the
        // panel is still on screen. Escape must be absorbed by the drawer, not
        // fall through and wipe the search query.
        let mut app = HonkHonk::new_for_test();
        app.search_query = "bark".to_owned();
        let _ = app.update(Message::ToggleEffectsPanel); // opening
        let open = Instant::now() + crate::ui::side_panel::SLIDE_DURATION;
        let _ = app.update(Message::Frame(open)); // settled open
        let _ = app.update(Message::ToggleEffectsPanel); // start closing
        assert!(!app.effects_panel.is_open());
        assert!(app.effects_panel.is_visible());
        let _ = app.update(Message::EscapePressed);
        assert_eq!(app.search_query, "bark");
    }

    #[test]
    fn stale_decoded_is_dropped() {
        // A Decoded carrying an older generation than the current play must not
        // start a playhead or change `playing` (a newer press superseded it, #149/#151).
        let mut app = HonkHonk::new_for_test();
        let (handle, _evt_tx) = crate::audio::test_handle();
        app.audio = Some(handle);
        app.play_generation = 5;
        app.playing = Some("newer".into());

        let pcm = std::sync::Arc::new(crate::audio::CachedPcm {
            samples: std::sync::Arc::new(vec![0.0_f32; 8]),
            sample_rate: 48_000,
            channels: 2,
            duration: std::time::Duration::from_secs(1),
        });
        let _ = app.update(Message::Decoded {
            generation: 4,
            id: "older".into(),
            result: Ok((*pcm).clone()),
        });

        assert!(
            app.playhead.is_none(),
            "stale decode must not start a playhead"
        );
        assert_eq!(app.playing(), Some("newer"));
    }

    #[test]
    fn current_decoded_starts_playhead_and_caches_pcm() {
        let mut app = HonkHonk::new_for_test();
        let (handle, _evt_tx) = crate::audio::test_handle();
        app.audio = Some(handle);
        app.play_generation = 2;
        app.playing = Some("snd".into());

        let pcm = crate::audio::CachedPcm {
            samples: std::sync::Arc::new(vec![0.25_f32; 64]),
            sample_rate: 48_000,
            channels: 2,
            duration: std::time::Duration::from_secs(3),
        };
        let _ = app.update(Message::Decoded {
            generation: 2,
            id: "snd".into(),
            result: Ok(pcm),
        });

        assert!(
            app.playhead.is_some(),
            "current decode must start the playhead"
        );
        assert!(
            app.audio_store.get_pcm("snd").is_some(),
            "decode result must be cached for instant re-fire"
        );
    }

    #[test]
    fn stopall_mid_decode_does_not_resurrect_playback() {
        // A cold-cache press dispatches an off-thread decode but sends no engine
        // Play yet. If the user hits StopAll before the decode lands, the stale
        // `Decoded` must be dropped — not resurrect the stopped sound (#151).
        let mut app = HonkHonk::new_for_test();
        let (handle, _evt_tx) = crate::audio::test_handle();
        app.audio = Some(handle);

        let dir = tempfile::tempdir().expect("tempdir");
        let wav_path = dir.path().join("honk.wav");
        write_test_wav(&wav_path);
        app.sounds = vec![SoundEntry {
            id: "wav1".into(),
            name: "Honk".into(),
            path: wav_path,
            format: crate::state::AudioFormat::Wav,
            duration_ms: Some(100),
            category: "Test".into(),
        }];

        // Cold press → generation bumped, decode Task in flight (ignored here).
        let sound = app.sounds[0].clone();
        let _ = app.request_play(&sound, false);
        let in_flight_gen = app.play_generation;
        assert_eq!(app.playing(), Some("wav1"));

        // StopAll tears down playback and must invalidate the in-flight decode.
        let _ = app.update(Message::StopAll);
        assert_eq!(app.playing(), None);

        // The decode lands carrying the now-stale generation.
        let _ = app.update(Message::Decoded {
            generation: in_flight_gen,
            id: "wav1".into(),
            result: Ok(crate::audio::CachedPcm {
                samples: std::sync::Arc::new(vec![0.0_f32; 8]),
                sample_rate: 48_000,
                channels: 2,
                duration: std::time::Duration::from_secs(1),
            }),
        });

        assert_eq!(app.playing(), None, "StopAll must win — no resurrection");
        assert!(
            app.playhead.is_none(),
            "no playhead after a stopped, stale decode"
        );
    }
}
