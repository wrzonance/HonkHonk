use iced::Element;
use iced::widget::{button, row, space, text};

use super::{HonkHonk, Message, SettingsMessage};
use crate::ui::list_controls::sort;
use crate::ui::search_bar;
use crate::ui::theme::{self, Hh};

impl HonkHonk {
    pub(super) fn view_header(&self, theme: theme::Theme) -> Element<'_, Message> {
        let title = text("HonkHonk").size(24).color(theme.ink());
        let slots = nav_button("Slots", Message::ShowSlots, theme);
        let macros = nav_button("Macros", Message::ShowMacros, theme);
        let settings = nav_button("Settings", SettingsMessage::Show.into(), theme);
        let search = search_bar::view_search_bar(self.filter.query(), Message::SearchChanged);
        let sort = sort::view_sort_chip(
            self.sound_sort,
            Message::ToggleSoundSortMenu,
            Message::ToggleSoundSortDirection,
            theme,
        );
        let list_controls = row![search, sort].spacing(theme::space::SM);
        let record = self.view_record_button(theme);
        let stop = nav_button("Stop All", Message::StopAll, theme);

        row![
            title,
            slots,
            macros,
            nav_button(
                "Import",
                Message::Import(super::import::ImportMessage::Open),
                theme
            ),
            settings,
            space::horizontal(),
            list_controls,
            record,
            stop
        ]
        .spacing(theme::space::SM)
        .align_y(iced::Alignment::Center)
        .into()
    }

    fn view_record_button(&self, theme: theme::Theme) -> Element<'_, Message> {
        let (label, message) = if self.is_recording() {
            ("\u{25a0} Stop", Message::StopRecording)
        } else {
            ("\u{25cf} Record", Message::StartRecording)
        };
        nav_button(label, message, theme)
    }
}

fn nav_button(label: &str, message: Message, theme: theme::Theme) -> Element<'_, Message> {
    button(text(label).size(14).color(theme.ink()))
        .on_press(message)
        .style(move |_theme, _status| button::Style {
            background: Some(theme::bg_color(theme.panel())),
            text_color: theme.ink(),
            border: theme::tile_border(theme.hairline(), 1.0),
            ..Default::default()
        })
        .into()
}
