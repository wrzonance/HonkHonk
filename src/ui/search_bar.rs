use iced::widget::{container, text_input};
use iced::{Border, Element, Length};

use crate::app::Message;
use crate::ui::theme::{self, Hh, Theme};

pub fn view_search_bar(query: &str) -> Element<'_, Message> {
    let t = Theme::Dark;

    let input = text_input("Find a sound\u{2026}", query)
        .on_input(Message::SearchChanged)
        .size(theme::font::BODY)
        .width(Length::Fixed(300.0))
        .style(move |_theme, status| {
            let border_color = match status {
                text_input::Status::Focused { .. } => t.accent(),
                _ => t.hairline(),
            };
            text_input::Style {
                background: theme::bg_color(t.panel()),
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: theme::radius::PILL,
                },
                icon: t.ink_dim(),
                placeholder: t.ink_faint(),
                value: t.ink(),
                selection: t.accent(),
            }
        });

    container(input).into()
}
