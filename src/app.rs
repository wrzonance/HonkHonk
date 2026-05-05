use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use iced::widget::{button, column, container, row, scrollable, space, text};
use iced::{Element, Length, Subscription, Task, Theme};

use crate::audio::{AudioCommand, AudioEvent, AudioHandle};
use crate::state::{AppConfig, SoundEntry};
use crate::tray::{TrayEvent, TrayHandle};
use crate::ui::sound_grid;
use crate::ui::theme::{self, Hh};

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
}

impl HonkHonk {
    pub fn new(
        mut tray: TrayHandle,
        audio: AudioHandle,
        sounds: Vec<SoundEntry>,
        config: AppConfig,
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
                    }
                    AudioEvent::Progress(_) => {
                        // TODO(Task 5): wire progress to UI progress bar
                    }
                    AudioEvent::Error(e) => {
                        eprintln!("honkhonk: audio error: {e}");
                    }
                }
                Task::none()
            }
            Message::PlaySound(sound_id) => {
                let sound = self.sounds.iter().find(|s| s.id == sound_id);
                let sound = match sound {
                    Some(s) => s,
                    None => return Task::none(),
                };

                let decoded = match crate::audio::decode(&sound.path) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("honkhonk: decode error: {e}");
                        return Task::none();
                    }
                };

                if let Some(ref audio) = self.audio {
                    audio.send(AudioCommand::Play {
                        sound_id,
                        samples: Arc::new(decoded.samples),
                        sample_rate: decoded.sample_rate,
                        channels: decoded.channels,
                    });
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
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let t = theme::Theme::Dark;
        let header = self.view_header(t);
        let chips = self.view_category_chips(t);
        let grid = sound_grid::view_grid(
            &self.sounds,
            self.playing.as_deref(),
            self.active_category.as_deref(),
        );

        let content =
            column![header, chips, scrollable(grid).height(Length::Fill)].spacing(theme::space::MD);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(theme::space::XL)
            .style(move |_theme| container::Style {
                background: Some(theme::bg_color(t.bg())),
                ..Default::default()
            })
            .into()
    }

    fn view_header(&self, t: theme::Theme) -> Element<'_, Message> {
        let title = text("HonkHonk").size(24).color(t.ink());

        let stop_btn = button(text("Stop All").size(14).color(t.ink()))
            .on_press(Message::StopAll)
            .style(move |_theme, _status| button::Style {
                background: Some(theme::bg_color(t.panel())),
                text_color: t.ink(),
                border: theme::tile_border(t.hairline(), 1.0),
                ..Default::default()
            });

        row![title, space::horizontal(), stop_btn]
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
        iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::TrayPoll)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        app.playing = Some("test-id".into());
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
        app.playing = Some("abc123".into());
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
}
