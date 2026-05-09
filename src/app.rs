use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use iced::widget::{button, container, row, scrollable, space, text};
use iced::{Element, Length, Point, Subscription, Task, Theme};

use crate::audio::{AudioCommand, AudioEvent, AudioHandle};
use crate::shortcuts::ShortcutsStatus;
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
    sounds: Vec<SoundEntry>,
    playing: Option<String>,
    active_category: Option<String>,
    config: AppConfig,
    search_query: String,
    progress: f32,
    slots: SlotMap,
    shortcuts_status: ShortcutsStatus,
    context_menu: Option<String>,
    context_menu_pos: Option<Point>,
    cursor_pos: Point,
    window_size: (f32, f32),
    shortcuts_warning_dismissed: bool,
    view_mode: ViewMode,
    selected_slot: Option<u8>,
}

fn shortcuts_stream_sub() -> impl iced::futures::Stream<Item = Message> {
    use iced::futures::SinkExt;
    use iced::futures::StreamExt;
    iced::stream::channel(16, async |mut tx| {
        use crate::shortcuts::{portal, ShortcutEvent};
        let stream = portal::shortcut_stream(None);
        let mut stream = std::pin::pin!(stream);
        while let Some(ev) = stream.next().await {
            let msg = match ev {
                ShortcutEvent::Ready => Message::ShortcutsReady,
                ShortcutEvent::Activated(i) => Message::ShortcutActivated(i),
                ShortcutEvent::Bindings(_) => continue, // Task 2: parse and handle
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

impl HonkHonk {
    pub fn new(
        mut tray: TrayHandle,
        audio: AudioHandle,
        sounds: Vec<SoundEntry>,
        config: AppConfig,
        slots: SlotMap,
    ) -> Self {
        let rx = tray.take_rx();
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
            shortcuts_status: ShortcutsStatus::Initializing,
            context_menu: None,
            context_menu_pos: None,
            cursor_pos: Point::ORIGIN,
            window_size: (1280.0, 800.0),
            shortcuts_warning_dismissed: false,
            view_mode: ViewMode::default(),
            selected_slot: None,
        }
    }

    pub fn new_for_test() -> Self {
        let (_tx, rx) = std::sync::mpsc::channel();
        Self {
            visible: true,
            exit: false,
            tray_rx: Arc::new(Mutex::new(rx)),
            _tray: None,
            audio: None,
            sounds: Vec::new(),
            playing: None,
            active_category: None,
            config: AppConfig::default(),
            search_query: String::new(),
            progress: 0.0,
            slots: SlotMap::default(),
            shortcuts_status: ShortcutsStatus::Initializing,
            context_menu: None,
            context_menu_pos: None,
            cursor_pos: Point::ORIGIN,
            window_size: (1280.0, 800.0),
            shortcuts_warning_dismissed: false,
            view_mode: ViewMode::default(),
            selected_slot: None,
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
            Message::SelectSlot(idx) => {
                self.selected_slot = Some(idx);
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

        let search = search_bar::view_search_bar(&self.search_query);

        let stop_btn = button(text("Stop All").size(14).color(t.ink()))
            .on_press(Message::StopAll)
            .style(move |_theme, _status| button::Style {
                background: Some(theme::bg_color(t.panel())),
                text_color: t.ink(),
                border: theme::tile_border(t.hairline(), 1.0),
                ..Default::default()
            });

        row![title, slots_btn, space::horizontal(), search, stop_btn]
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
        Theme::Dark
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let shortcuts = Subscription::run(shortcuts_stream_sub);

        let tray_poll =
            iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::TrayPoll);

        let events = iced::event::listen_with(|event, _, _| match event {
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

        Subscription::batch([shortcuts, tray_poll, events])
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
        let t = theme::Theme::Dark;
        let header = self.view_header(t);
        let chips = self.view_category_chips(t);
        let filtered = self.filtered_sounds();
        let grid = sound_grid::view_grid(
            &filtered,
            self.playing.as_deref(),
            &self.slots,
            matches!(self.shortcuts_status, ShortcutsStatus::Active),
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
            let overlay =
                sound_grid::context_menu_overlay(found, &self.slots, t, pos, self.window_size);
            iced::widget::stack![base, overlay].into()
        } else {
            base.into()
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        match self.view_mode {
            ViewMode::Main => self.view_main(),
            ViewMode::SlotManager => {
                let t = theme::Theme::Dark;
                slot_manager::view_slot_manager(&self.slots, &self.sounds, self.selected_slot, t)
            }
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
}
