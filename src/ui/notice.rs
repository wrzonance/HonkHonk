use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Column, Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Background, Border, Color, Element, Length};

use crate::app::Message;
use crate::app::notices::{NoticeLevel, NoticeQueue, QueuedNotice};
use crate::ui::theme::{self, Hh, Theme};

const NOTICE_MAX_WIDTH: f32 = 360.0;
/// Cap the toast stack's on-screen height; beyond this it scrolls so every
/// notice (including capped persistent errors) stays reachable and dismissable.
const NOTICE_MAX_HEIGHT: f32 = 420.0;

pub fn view_notice_layer<'a>(notices: &'a NoticeQueue, t: Theme) -> Option<Element<'a, Message>> {
    if notices.is_empty() {
        return None;
    }

    let stack = notices
        .iter()
        .fold(Column::new().spacing(theme::space::SM), |col, notice| {
            col.push(view_notice(notice, t))
        });

    // Scroll within a bounded height so a stack taller than the window never
    // pushes notices (and their close buttons) off-screen.
    let scroller = scrollable(stack.width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Shrink);

    Some(
        container(
            container(scroller)
                .width(Length::Fill)
                .max_width(NOTICE_MAX_WIDTH)
                .max_height(NOTICE_MAX_HEIGHT),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(theme::space::LG)
        .align_x(Horizontal::Right)
        .align_y(Vertical::Top)
        .into(),
    )
}

fn view_notice<'a>(queued: &'a QueuedNotice, t: Theme) -> Element<'a, Message> {
    let style = level_style(queued.notice.level, t);
    let label = text(level_label(queued.notice.level))
        .size(theme::font::LABEL)
        .color(style.accent);
    let title = text(queued.notice.title.as_str())
        .size(theme::font::BODY)
        .color(t.ink());
    let body = text(queued.notice.body.as_str())
        .size(theme::font::BODY)
        .color(t.ink_dim())
        .width(Length::Fill);
    let close = button(text("×").size(theme::font::BODY).color(t.ink_dim()))
        .on_press(Message::DismissNotice(queued.id))
        .style(move |_theme, _status| button::Style {
            background: None,
            text_color: t.ink_dim(),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: theme::radius::SM,
            },
            ..Default::default()
        });

    let header = row![label, title, Space::new().width(Length::Fill), close]
        .spacing(theme::space::SM)
        .align_y(Alignment::Center);
    let content = column![header, body]
        .spacing(theme::space::XS)
        .width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .padding([theme::space::SM, theme::space::MD])
        .style(move |_| container::Style {
            background: Some(Background::Color(style.bg)),
            border: Border {
                color: style.accent,
                width: 1.0,
                radius: theme::radius::SM,
            },
            ..Default::default()
        })
        .into()
}

fn level_label(level: NoticeLevel) -> &'static str {
    match level {
        NoticeLevel::Info => "INFO",
        NoticeLevel::Warning => "WARN",
        NoticeLevel::Error => "ERROR",
    }
}

#[derive(Clone, Copy)]
struct LevelStyle {
    accent: Color,
    bg: Color,
}

fn level_style(level: NoticeLevel, t: Theme) -> LevelStyle {
    let accent = match level {
        NoticeLevel::Info => Color::from_rgb(0.25, 0.55, 0.95),
        NoticeLevel::Warning => Color::from_rgb(0.9, 0.58, 0.15),
        NoticeLevel::Error => Color::from_rgb(0.9, 0.22, 0.18),
    };
    let bg = if t.is_dark() {
        Color::from_rgba(
            (t.panel().r + accent.r * 0.28).min(1.0),
            (t.panel().g + accent.g * 0.28).min(1.0),
            (t.panel().b + accent.b * 0.28).min(1.0),
            0.96,
        )
    } else {
        Color::from_rgba(
            (1.0 - (1.0 - accent.r) * 0.12).min(1.0),
            (1.0 - (1.0 - accent.g) * 0.12).min(1.0),
            (1.0 - (1.0 - accent.b) * 0.12).min(1.0),
            0.98,
        )
    };
    LevelStyle { accent, bg }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    use crate::app::notices::{Notice, NoticeQueue};
    use crate::ui::theme::Theme;

    #[test]
    fn notice_layer_is_absent_for_empty_queue() {
        let queue = NoticeQueue::new();

        assert!(view_notice_layer(&queue, Theme::Dark).is_none());
    }

    #[test]
    fn notice_layer_builds_for_queued_notice() {
        let mut queue = NoticeQueue::new();
        queue.push(
            Notice::info("Saved", "Configuration updated"),
            Instant::now(),
        );

        assert!(view_notice_layer(&queue, Theme::Dark).is_some());
    }
}
