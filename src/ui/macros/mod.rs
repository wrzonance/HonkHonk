pub mod geometry;
pub(crate) mod timeline;
#[cfg(test)]
mod timeline_tests;

use crate::app::{Message, macro_editor::EditorState};
use iced::widget::{canvas, container, scrollable, text};
use iced::{Element, Length, Padding};

pub(crate) fn view_timeline(editor: &EditorState) -> Element<'_, Message> {
    let size = editor.timeline.size();
    let canvas = canvas(timeline::Timeline { editor })
        .width(size.width)
        .height(size.height);
    // Widget text is clipped to each bar; it never uses unbounded canvas text.
    let mut layers: Vec<Element<'_, Message>> = vec![canvas.into()];
    for (index, bar) in editor.timeline.bars.iter().enumerate() {
        let rect = editor.timeline.rectangle(bar);
        let label = text(format!("{}: {}", index + 1, bar.label))
            .size(12)
            .color(if bar.missing {
                iced::Color::WHITE
            } else {
                iced::Color::BLACK
            });
        let clipped = container(label)
            .width(rect.width)
            .height(rect.height)
            .clip(true)
            .padding(3);
        layers.push(
            container(clipped)
                .padding(Padding {
                    left: rect.x,
                    top: rect.y,
                    right: 0.0,
                    bottom: 0.0,
                })
                .into(),
        );
    }
    scrollable(
        iced::widget::Stack::with_children(layers)
            .width(size.width)
            .height(size.height),
    )
    .direction(scrollable::Direction::Both {
        vertical: scrollable::Scrollbar::new(),
        horizontal: scrollable::Scrollbar::new(),
    })
    .height(Length::Fill)
    .into()
}
