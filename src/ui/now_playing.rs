use iced::widget::{container, row, space, text, Column, Space};
use iced::{Border, Element, Length};

use crate::app::Message;
use crate::state::SoundEntry;
use crate::ui::theme::{self, Hh, Theme};
use crate::ui::volume;

pub fn view_now_playing<'a>(
    playing: Option<&'a str>,
    sounds: &'a [SoundEntry],
    progress: f32,
    vol: f32,
) -> Element<'a, Message> {
    let t = Theme::Dark;

    let sound = match playing {
        Some(id) => sounds.iter().find(|s| s.id == id),
        None => None,
    };

    let sound = match sound {
        Some(s) => s,
        None => return Space::new().into(),
    };

    let placeholder = container(Space::new())
        .width(44.0)
        .height(44.0)
        .style(move |_theme| container::Style {
            background: Some(theme::bg_color(t.bg())),
            border: Border {
                color: t.hairline(),
                width: 1.0,
                radius: theme::radius::MD,
            },
            ..Default::default()
        });

    let name = text(sound.name.clone()).size(14).color(t.ink());
    let subtitle = text(format!("HONKING NOW \u{00b7} {}", sound.category))
        .size(10.5)
        .color(t.ink_dim());
    let info = Column::new()
        .push(name)
        .push(subtitle)
        .spacing(theme::space::XS);

    let progress_bar = view_progress_bar(progress, t);
    let vol_widget = volume::view_volume(vol);

    let content = row![
        placeholder,
        info,
        progress_bar,
        space::horizontal(),
        vol_widget,
    ]
    .spacing(theme::space::LG)
    .align_y(iced::Alignment::Center);

    container(content)
        .width(Length::Fill)
        .padding([theme::space::MD, theme::space::XL])
        .style(move |_theme| container::Style {
            background: Some(theme::bg_color(t.panel())),
            border: Border {
                color: t.hairline(),
                width: 1.0,
                radius: iced::border::Radius::default(),
            },
            ..Default::default()
        })
        .into()
}

fn view_progress_bar(progress: f32, t: Theme) -> Element<'static, Message> {
    let filled_width = (progress.clamp(0.0, 1.0) * 320.0).round();

    let filled = container(Space::new())
        .width(filled_width)
        .height(6.0)
        .style(move |_theme| container::Style {
            background: Some(theme::bg_color(t.accent())),
            border: Border {
                radius: theme::radius::SM,
                ..Default::default()
            },
            ..Default::default()
        });

    let track = container(filled)
        .width(320.0)
        .height(6.0)
        .style(move |_theme| container::Style {
            background: Some(theme::bg_color(t.bg())),
            border: Border {
                radius: theme::radius::SM,
                ..Default::default()
            },
            ..Default::default()
        });

    track.into()
}
