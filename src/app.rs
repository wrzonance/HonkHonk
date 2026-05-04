use crate::tray::TrayEvent;

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    ToggleVisibility,
    Quit,
    TrayEvent(TrayEvent),
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
}

impl HonkHonk {
    pub fn new_for_test() -> Self {
        Self {
            visible: true,
            exit: false,
        }
    }

    pub fn should_exit(&self) -> bool {
        self.exit
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::ToggleVisibility => {
                self.visible = !self.visible;
                iced::Task::none()
            }
            Message::Quit => {
                self.exit = true;
                iced::Task::none()
            }
            Message::TrayEvent(event) => {
                let msg = Message::from_tray_event(event);
                self.update(msg)
            }
        }
    }
}
