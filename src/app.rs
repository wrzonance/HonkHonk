use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use iced::widget::{button, container, row, scrollable, space, text};
use iced::{Element, Length, Point, Subscription, Task, Theme};

use crate::audio::{AudioCommand, AudioEvent, AudioHandle};
use crate::shortcuts::ShortcutsStatus;
use crate::state::config::Density;
use crate::state::{AppConfig, SlotMap, SoundEntry};
use crate::tray::{TrayEvent, TrayHandle};
use crate::ui::sound_grid;
use crate::ui::theme::{self, Hh};
use crate::ui::{now_playing, search_bar, slot_manager};

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
    // Shortcut capture
    StartCapture(u8),
    CancelCapture,
    /// Raw key press — only processed during capture mode.
    KeyPressed {
        key: iced::keyboard::Key,
        modifiers: iced::keyboard::Modifiers,
    },
    // Portal handle + rebind results
    /// Carries the command sender from the portal stream.
    /// Two `ShortcutHandle` messages are never meaningfully equal — treated as always-unequal.
    ShortcutHandle(crate::shortcuts::PortalCmdSender),
    RebindResult {
        changed_idx: u8,
        bindings: Vec<(u8, String)>,
    },
    ShortcutsChangedExternal(Vec<(u8, String)>),
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
    portal_cmd_tx: Option<tokio::sync::mpsc::Sender<crate::shortcuts::PortalCommand>>,
    pub(crate) capturing_slot: Option<u8>,
    pub(crate) bind_feedback: [crate::shortcuts::BindFeedback; 20],
    /// Snapshot of desired_triggers at startup — passed to the portal subscription once.
    /// Never updated after init so the subscription ID stays stable.
    initial_desired_for_sub: std::sync::Arc<[Option<String>; 20]>,
}

fn shortcuts_stream_sub(
    window_id: Option<ashpd::WindowIdentifier>,
    initial_desired: [Option<String>; 20],
) -> impl iced::futures::Stream<Item = Message> {
    use iced::futures::SinkExt;
    use iced::futures::StreamExt;
    iced::stream::channel(16, async move |mut tx| {
        use crate::shortcuts::{portal, ShortcutEvent};
        let stream = portal::shortcut_stream(window_id, initial_desired);
        let mut stream = std::pin::pin!(stream);
        while let Some(ev) = stream.next().await {
            let msg = match ev {
                ShortcutEvent::Ready => Message::ShortcutsReady,
                ShortcutEvent::Handle(sender) => {
                    Message::ShortcutHandle(crate::shortcuts::PortalCmdSender(sender))
                }
                ShortcutEvent::Activated(i) => Message::ShortcutActivated(i),
                ShortcutEvent::Bindings(b) => Message::ShortcutBindingsUpdated(b),
                ShortcutEvent::RebindResult { changed_idx, bindings } => {
                    Message::RebindResult { changed_idx, bindings }
                }
                ShortcutEvent::Changed(b) => Message::ShortcutsChangedExternal(b),
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

/// Wrapper for `Subscription::run_with` — takes the initial desired triggers Arc.
/// The Arc value is set once at startup and never changes, keeping the subscription ID stable.
fn shortcuts_stream_with_initial(
    initial: &std::sync::Arc<[Option<String>; 20]>,
) -> impl iced::futures::Stream<Item = Message> {
    let initial = (**initial).clone();
    shortcuts_stream_sub(None, initial)
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
        let initial_desired = config.desired_triggers.clone();
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
            portal_cmd_tx: None,
            capturing_slot: None,
            bind_feedback: std::array::from_fn(|_| crate::shortcuts::BindFeedback::Unset),
            initial_desired_for_sub: std::sync::Arc::new(initial_desired),
        }
    }

    pub fn new_for_test() -> Self {
        let (_tx, rx) = std::sync::mpsc::channel();
        let config = AppConfig::default();
        let initial_desired = config.desired_triggers.clone();
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
            portal_cmd_tx: None,
            capturing_slot: None,
            bind_feedback: std::array::from_fn(|_| crate::shortcuts::BindFeedback::Unset),
            initial_desired_for_sub: std::sync::Arc::new(initial_desired),
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

    pub fn filtered_sounds(&self) -> Vec<&SoundEntry> {
        let query = self.search_query.to_lowercase();
        self.sounds
            .iter()
            .filter(|s| match &self.active_category {
                Some(cat) => s.category == *cat,
                None => true,
            })
            .filter(|s| query.is_empty() || s.name.to_lowercase().contains(&query))
            .collect()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
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

                if let Some(ref audio) = self.audio {
                    if let Some(event) = audio.try_recv() {
                        return self.update(Message::AudioEvent(event));
                    }
                }

                Task::none()
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
                        self.playing = Some(sound_id);
                    }
                    AudioEvent::PlaybackFinished { .. } => {
                        self.playing = None;
                        self.progress = 0.0;
                    }
                    AudioEvent::Progress(p) => {
                        self.progress = p;
                    }
                    AudioEvent::Error(e) => {
                        eprintln!("honkhonk: audio error: {e}");
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
                }
                Task::none()
            }
            Message::PlaySound(sound_id) => {
                if let Some(sound) = self.sounds.iter().find(|s| s.id == sound_id) {
                    self.play_sound_entry(sound, false);
                }
                Task::none()
            }
            Message::StopAll => {
                if let Some(ref audio) = self.audio {
                    audio.send(AudioCommand::Stop);
                }
                self.playing = None;
                Task::none()
            }
            Message::SelectCategory(cat) => {
                self.active_category = cat;
                Task::none()
            }
            Message::SearchChanged(query) => {
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
                    if let Some(sound) = self.sounds.iter().find(|s| s.path == path) {
                        self.play_sound_entry(sound, true);
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
                self.capturing_slot = None;
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
            Message::ShortcutHandle(crate::shortcuts::PortalCmdSender(sender)) => {
                self.portal_cmd_tx = Some(sender);
                Task::none()
            }
            Message::StartCapture(idx) => {
                if idx >= 20 {
                    return Task::none();
                }
                // Only allow capture on bound slots
                if self.slots.get(idx).is_some() {
                    self.capturing_slot = Some(idx);
                    self.bind_feedback[idx as usize] = crate::shortcuts::BindFeedback::Unset;
                }
                Task::none()
            }
            Message::CancelCapture => {
                self.capturing_slot = None;
                Task::none()
            }
            Message::KeyPressed { key, modifiers } => {
                use crate::shortcuts::capture::format_combo;

                let Some(idx) = self.capturing_slot else {
                    return Task::none();
                };

                if let Some(combo) = format_combo(modifiers, &key) {
                    self.capturing_slot = None;
                    self.config.desired_triggers[idx as usize] = Some(combo.clone());
                    if let Err(e) = self.config.save() {
                        eprintln!("honkhonk: config save: {e}");
                    }
                    if let Some(tx) = &self.portal_cmd_tx {
                        if let Err(e) = tx.try_send(
                            crate::shortcuts::PortalCommand::RebindSlot { idx, trigger: combo },
                        ) {
                            eprintln!("honkhonk: rebind command dropped: {e}");
                        }
                    }
                }
                // Bare key without modifier (or Escape, handled by CloseContextMenu): keep capture open
                Task::none()
            }
            Message::RebindResult { changed_idx, bindings } => {
                if changed_idx >= 20 {
                    return Task::none();
                }
                // Only reset and repopulate slot_triggers when the portal returned bindings.
                // An empty response means the rebind failed — leave existing triggers intact.
                if !bindings.is_empty() {
                    self.slot_triggers = std::array::from_fn(|_| None);
                    for (idx, trigger) in &bindings {
                        if let Some(slot) = self.slot_triggers.get_mut(*idx as usize) {
                            *slot = Some(trigger.clone());
                        }
                    }
                }
                // Determine feedback for the specifically changed slot
                let requested = self.config.desired_triggers[changed_idx as usize].as_deref();
                let granted = self.slot_triggers[changed_idx as usize].as_deref();
                self.bind_feedback[changed_idx as usize] = match (requested, granted) {
                    (Some(req), Some(got)) if req == got => {
                        crate::shortcuts::BindFeedback::Saved
                    }
                    (Some(_), _) => crate::shortcuts::BindFeedback::NotSaved,
                    _ => crate::shortcuts::BindFeedback::Unset,
                };
                Task::none()
            }
            Message::ShortcutsChangedExternal(bindings) => {
                // Full reset+repopulate so removed external shortcuts are cleared, not left stale.
                self.slot_triggers = std::array::from_fn(|_| None);
                for (idx, trigger) in bindings {
                    if let Some(slot) = self.slot_triggers.get_mut(idx as usize) {
                        *slot = Some(trigger);
                    }
                }
                Task::none()
            }
        }
    }

    fn play_sound_entry(&self, sound: &SoundEntry, stop_before: bool) {
        let decoded = match crate::audio::decode(&sound.path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("honkhonk: decode error: {e}");
                return;
            }
        };
        if let Some(ref audio) = self.audio {
            if stop_before {
                audio.send(AudioCommand::Stop);
            }
            audio.send(AudioCommand::Play {
                sound_id: sound.id.clone(),
                samples: Arc::new(decoded.samples),
                sample_rate: decoded.sample_rate,
                channels: decoded.channels,
            });
        }
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

        let chips: Vec<Element<'_, Message>> = std::iter::once(all_chip)
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
        let shortcuts = Subscription::run_with(
            std::sync::Arc::clone(&self.initial_desired_for_sub),
            shortcuts_stream_with_initial,
        );

        let tray_poll =
            iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::TrayPoll);

        let events = iced::event::listen_with(|event, _, _window_id| match event {
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                ..
            }) => Some(Message::CloseContextMenu),
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

        // Keyboard capture subscription — active only during capture mode.
        // Escape is explicitly filtered out: the always-on `events` subscription already maps
        // Escape → CloseContextMenu, which also clears capturing_slot. Emitting KeyPressed for
        // Escape here would cause double-dispatch.
        if self.capturing_slot.is_some() {
            let capture = iced::event::listen_with(|event, _, _| match event {
                iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                    ..
                }) => None,
                iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    ..
                }) => Some(Message::KeyPressed { key, modifiers }),
                _ => None,
            });
            subs.push(capture);
        }

        if !self.durations_loaded {
            subs.push(Subscription::run_with(
                std::sync::Arc::clone(&self.duration_scan_pairs),
                duration_scan_builder,
            ));
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
            },
        );

        let now_playing = now_playing::view_now_playing(
            self.playing.as_deref(),
            &self.sounds,
            self.progress,
            self.config.volume,
        );

        let mut items: Vec<Element<'_, Message>> = Vec::new();
        if let Some(banner) = self.view_shortcuts_banner(t) {
            items.push(banner);
        }
        items.push(header);
        items.push(chips);
        items.push(scrollable(grid).height(Length::Fill).into());
        items.push(now_playing);

        let content = iced::widget::Column::with_children(items).spacing(theme::space::MD);

        let base = container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(theme::space::XL)
            .style(move |_theme| container::Style {
                background: Some(theme::bg_color(t.bg())),
                ..Default::default()
            });

        // Overlay context menu at window level so cursor coords map exactly.
        if let (Some(ref sound_id), Some(pos)) = (&self.context_menu, self.context_menu_pos) {
            let found = self.sounds.iter().find(|s| s.id == *sound_id);
            let overlay = sound_grid::context_menu_overlay(
                found,
                sound_grid::SlotCtx {
                    slots: &self.slots,
                    triggers: &self.slot_triggers,
                },
                t,
                pos,
                self.window_size,
            );
            iced::widget::stack![base, overlay].into()
        } else {
            base.into()
        }
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
                        capturing_slot: self.capturing_slot,
                        bind_feedback: &self.bind_feedback,
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
    fn stop_all_clears_playing() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
            sound_id: "test-id".into(),
        }));
        let _ = app.update(Message::StopAll);
        assert!(app.playing().is_none());
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
        }));
        assert!(app.playing().is_none());
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
    fn start_capture_sets_capturing_slot_for_bound_slot() {
        let mut app = HonkHonk::new_for_test();
        let path = std::path::PathBuf::from("/tmp/test.wav");
        let _ = app.update(Message::AssignSlot(0, path));
        assert!(app.capturing_slot.is_none());
        let _ = app.update(Message::StartCapture(0));
        assert_eq!(app.capturing_slot, Some(0));
    }

    #[test]
    fn start_capture_ignored_for_empty_slot() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::StartCapture(3));
        assert!(app.capturing_slot.is_none());
    }

    #[test]
    fn cancel_capture_clears_capturing_slot() {
        let mut app = HonkHonk::new_for_test();
        let path = std::path::PathBuf::from("/tmp/test.wav");
        let _ = app.update(Message::AssignSlot(0, path));
        let _ = app.update(Message::StartCapture(0));
        assert!(app.capturing_slot.is_some());
        let _ = app.update(Message::CancelCapture);
        assert!(app.capturing_slot.is_none());
    }

    #[test]
    fn close_context_menu_cancels_capture() {
        // Escape flows through CloseContextMenu (the always-on `events` sub), not KeyPressed.
        // CloseContextMenu must clear capturing_slot to prevent double-dispatch.
        let mut app = HonkHonk::new_for_test();
        let path = std::path::PathBuf::from("/tmp/test.wav");
        let _ = app.update(Message::AssignSlot(0, path));
        let _ = app.update(Message::StartCapture(0));
        assert_eq!(app.capturing_slot, Some(0));
        let _ = app.update(Message::CloseContextMenu);
        assert!(app.capturing_slot.is_none());
    }

    #[test]
    fn key_pressed_bare_letter_does_not_snap() {
        let mut app = HonkHonk::new_for_test();
        let path = std::path::PathBuf::from("/tmp/test.wav");
        let _ = app.update(Message::AssignSlot(0, path));
        let _ = app.update(Message::StartCapture(0));
        let _ = app.update(Message::KeyPressed {
            key: iced::keyboard::Key::Character("a".into()),
            modifiers: iced::keyboard::Modifiers::empty(),
        });
        // Capture still active — bare key rejected
        assert_eq!(app.capturing_slot, Some(0));
    }

    #[test]
    fn rebind_result_sets_saved_feedback_when_trigger_matches() {
        let mut app = HonkHonk::new_for_test();
        app.config.desired_triggers[0] = Some("Meta+1".into());
        let _ = app.update(Message::RebindResult {
            changed_idx: 0,
            bindings: vec![(0, "Meta+1".into())],
        });
        assert_eq!(app.bind_feedback[0], crate::shortcuts::BindFeedback::Saved);
        assert_eq!(app.slot_triggers[0].as_deref(), Some("Meta+1"));
    }

    #[test]
    fn rebind_result_sets_not_saved_when_trigger_absent() {
        let mut app = HonkHonk::new_for_test();
        app.config.desired_triggers[0] = Some("Meta+1".into());
        let _ = app.update(Message::RebindResult {
            changed_idx: 0,
            bindings: vec![], // portal rejected it — absent from response
        });
        assert_eq!(app.bind_feedback[0], crate::shortcuts::BindFeedback::NotSaved);
    }

    #[test]
    fn shortcuts_changed_external_updates_slot_triggers() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::ShortcutsChangedExternal(vec![(2, "Ctrl+F3".into())]));
        assert_eq!(app.slot_triggers[2].as_deref(), Some("Ctrl+F3"));
    }
}
