use iced::Element;
use iced::widget::{row, slider, text};

use crate::app::Message;
use crate::ui::theme::{self, Hh, Theme};

pub fn view_volume(volume: f32) -> Element<'static, Message> {
    let t = Theme::Dark;
    let pct = format!("{}%", (volume * 100.0).round() as u32);

    let vol_slider = slider(0.0..=1.0, volume, Message::VolumeChanged)
        .on_release(Message::VolumeSaveRequested)
        .step(0.01)
        .width(140.0);

    let label = text(pct)
        .size(theme::font::LABEL)
        .color(t.ink_dim())
        .width(32.0)
        .align_x(iced::alignment::Horizontal::Right);

    row![vol_slider, label]
        .spacing(theme::space::SM)
        .align_y(iced::Alignment::Center)
        .into()
}
