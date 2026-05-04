use crate::tray::TrayEvent;
use iced::widget::{center, text};
use iced::{Element, Subscription, Task, Theme};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    ToggleVisibility,
    Quit,
    TrayEvent(TrayEvent),
    TrayPoll,
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
}

impl HonkHonk {
    pub fn new(tray_rx: Receiver<TrayEvent>) -> Self {
        Self {
            visible: true,
            exit: false,
            tray_rx: Arc::new(Mutex::new(tray_rx)),
        }
    }

    pub fn new_for_test() -> Self {
        let (_tx, rx) = std::sync::mpsc::channel();
        Self {
            visible: true,
            exit: false,
            tray_rx: Arc::new(Mutex::new(rx)),
        }
    }

    pub fn should_exit(&self) -> bool {
        self.exit
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ToggleVisibility => {
                self.visible = !self.visible;
                Task::none()
            }
            Message::Quit => {
                self.exit = true;
                iced::exit()
            }
            Message::TrayEvent(event) => {
                let msg = Message::from_tray_event(event);
                self.update(msg)
            }
            Message::TrayPoll => {
                let event = self.tray_rx.lock().ok().and_then(|rx| rx.try_recv().ok());

                match event {
                    Some(e) => {
                        let msg = Message::from_tray_event(e);
                        self.update(msg)
                    }
                    None => Task::none(),
                }
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        center(text("HonkHonk").size(32)).into()
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    pub fn subscription(&self) -> Subscription<Message> {
        // TODO: replace polling with async stream subscription for zero-idle-cost tray events
        iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::TrayPoll)
    }
}
