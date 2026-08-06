use iced::keyboard::Key;
use iced::widget::Id;
use iced_test::simulator;

use crate::app::{HonkHonk, Message};
use crate::test_lock::gui_lock;

pub(super) struct GuiHarness {
    pub(super) app: HonkHonk,
}

impl GuiHarness {
    pub(super) fn new() -> Self {
        Self {
            app: HonkHonk::new_for_test(),
        }
    }

    pub(super) fn click(&mut self, label: &str) -> usize {
        let messages = {
            let _gui = gui_lock();
            let mut ui = simulator(self.app.view());
            ui.click(label)
                .unwrap_or_else(|error| panic!("clicking {label:?} failed: {error}"));
            ui.into_messages().collect::<Vec<_>>()
        };
        self.update(messages)
    }

    pub(super) fn tap_key(&mut self, target: Id, key: Key) -> usize {
        let messages = {
            let _gui = gui_lock();
            let mut ui = simulator(self.app.view());
            ui.click(target.clone())
                .unwrap_or_else(|error| panic!("focusing {target:?} failed: {error}"));
            ui.tap_key(key);
            ui.into_messages().collect::<Vec<_>>()
        };
        self.update(messages)
    }

    pub(super) fn typewrite(&mut self, target: Id, text: &str) -> usize {
        let messages = {
            let _gui = gui_lock();
            let mut ui = simulator(self.app.view());
            ui.click(target.clone())
                .unwrap_or_else(|error| panic!("focusing {target:?} failed: {error}"));
            ui.typewrite(text);
            ui.into_messages().collect::<Vec<_>>()
        };
        self.update(messages)
    }

    pub(super) fn find(&self, label: &str) -> bool {
        let _gui = gui_lock();
        let mut ui = simulator(self.app.view());
        ui.find(label).is_ok()
    }

    pub(super) fn find_id(&self, id: Id) -> bool {
        let _gui = gui_lock();
        let mut ui = simulator(self.app.view());
        ui.find(id).is_ok()
    }

    fn update(&mut self, messages: impl IntoIterator<Item = Message>) -> usize {
        messages
            .into_iter()
            .map(|message| self.app.update(message).units())
            .sum()
    }
}
