use iced::widget::{button, container, row, text, text_input};
use iced::{Alignment, Border, Element, Length, Padding};

use crate::app::Message;
use crate::ui::theme::{self, Hh, Theme};

pub fn view_search_bar(query: &str) -> Element<'_, Message> {
    let t = Theme::Dark;

    // Reserve right space for the clear button so typed text doesn't run under it.
    let padding = if query.is_empty() {
        Padding::from(5.0)
    } else {
        Padding {
            top: 5.0,
            right: 30.0,
            bottom: 5.0,
            left: 10.0,
        }
    };

    let input: Element<'_, Message> = text_input("Find a sound to honk\u{2026}", query)
        .on_input(Message::SearchChanged)
        .size(theme::font::BODY)
        .width(Length::Fixed(300.0))
        .padding(padding)
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
        })
        .into();

    // Always use stack so the widget tree shape is stable across all query states.
    // Changing from container → stack on first keystroke caused Iced to reset
    // text_input focus. An empty row as the second layer has no hit area or cost.
    let overlay: Element<'_, Message> = if query.is_empty() {
        row![].into()
    } else {
        // Clear button — floats over the right edge of the input via stack.
        let clear_btn = button(text("\u{2715}").size(theme::font::BODY).color(t.ink_dim()))
            .on_press(Message::SearchChanged(String::new()))
            .padding(Padding {
                top: 4.0,
                right: 10.0,
                bottom: 4.0,
                left: 4.0,
            })
            .style(move |_t, status| button::Style {
                text_color: match status {
                    button::Status::Hovered | button::Status::Pressed => t.ink(),
                    _ => t.ink_dim(),
                },
                background: None,
                ..Default::default()
            });

        container(clear_btn)
            .width(Length::Fixed(300.0))
            .align_x(Alignment::End)
            .align_y(Alignment::Center)
            .into()
    };

    iced::widget::stack![input, overlay].into()
}
