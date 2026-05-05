use iced::widget::{button, column, container, row, text};
use iced::{Border, Element, Length};

use crate::app::Message;
use crate::state::SoundEntry;
use crate::ui::theme::{self, Hh, Theme, Tone};

const COLUMNS: usize = 5;
const TILE_HEIGHT: f32 = 140.0;

pub fn view_grid<'a>(
    sounds: &[&'a SoundEntry],
    playing: Option<&str>,
) -> Element<'a, Message> {
    let theme = Theme::Dark;

    if sounds.is_empty() {
        return container(
            text("No sounds found. Add audio files to your sound directory.")
                .size(16)
                .color(theme.ink_dim()),
        )
        .width(Length::Fill)
        .padding(theme::space::XXL)
        .into();
    }

    let rows: Vec<Element<'a, Message>> = sounds
        .chunks(COLUMNS)
        .map(|chunk| {
            let tiles: Vec<Element<'a, Message>> = chunk
                .iter()
                .map(|sound| {
                    let is_playing = playing == Some(sound.id.as_str());
                    let tone_idx = u64::from_str_radix(&sound.id[..8], 16).unwrap_or(0) as usize;
                    tile_view(sound, is_playing, Tone::from_index(tone_idx), theme)
                })
                .collect();

            let r = tiles
                .into_iter()
                .fold(row![].spacing(theme::space::LG), |r, t| r.push(t));

            r.into()
        })
        .collect();

    let grid = rows
        .into_iter()
        .fold(column![].spacing(theme::space::LG), |c, r| c.push(r));

    grid.width(Length::Fill).into()
}

fn tile_view<'a>(
    sound: &'a SoundEntry,
    is_playing: bool,
    tone: Tone,
    theme: Theme,
) -> Element<'a, Message> {
    let duration_str = match sound.duration_ms {
        Some(ms) => {
            let secs = ms / 1000;
            format!("{}:{:02}", secs / 60, secs % 60)
        }
        None => "\u{2014}".into(),
    };

    let category_text = text(sound.category.clone()).size(11).color(theme.ink_dim());
    let name_text = text(sound.name.clone()).size(15).color(theme.ink());
    let duration_text = text(duration_str).size(11).color(theme.ink_faint());

    let content = column![category_text, name_text, duration_text]
        .spacing(theme::space::SM)
        .padding(theme::space::LG);

    let bg = tone.tile_tint(theme.is_dark());
    let border_color = if is_playing {
        theme.accent()
    } else {
        theme.hairline()
    };
    let border_width = if is_playing { 2.5 } else { 1.0 };

    button(content)
        .on_press(Message::PlaySound(sound.id.clone()))
        .width(Length::Fill)
        .height(TILE_HEIGHT)
        .style(move |_theme, status| {
            let bg_final = match status {
                button::Status::Hovered | button::Status::Pressed => lighten(bg, 0.03),
                _ => bg,
            };
            button::Style {
                background: Some(theme::bg_color(bg_final)),
                text_color: theme.ink(),
                border: Border {
                    color: border_color,
                    width: border_width,
                    radius: theme::radius::TILE,
                },
                ..Default::default()
            }
        })
        .into()
}

fn lighten(c: iced::Color, amount: f32) -> iced::Color {
    iced::Color {
        r: (c.r + amount).min(1.0),
        g: (c.g + amount).min(1.0),
        b: (c.b + amount).min(1.0),
        a: c.a,
    }
}
