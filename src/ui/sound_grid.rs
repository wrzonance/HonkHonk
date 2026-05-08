use iced::widget::{button, column, container, mouse_area, row, text};
use iced::{Border, Element, Length};

use crate::app::Message;
use crate::state::{SlotMap, SoundEntry};
use crate::ui::theme::{self, Hh, Theme, Tone};

#[derive(Clone, Copy)]
struct TileCtx<'a> {
    slots: &'a SlotMap,
    shortcuts_active: bool,
}

const COLUMNS: usize = 5;
const TILE_HEIGHT: f32 = 140.0;

pub fn view_grid<'a>(
    sounds: &[&'a SoundEntry],
    playing: Option<&str>,
    slots: &'a crate::state::SlotMap,
    shortcuts_active: bool,
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

    let ctx = TileCtx {
        slots,
        shortcuts_active,
    };

    let rows: Vec<Element<'a, Message>> = sounds
        .chunks(COLUMNS)
        .map(|chunk| {
            let tiles: Vec<Element<'a, Message>> = chunk
                .iter()
                .map(|sound| {
                    let is_playing = playing == Some(sound.id.as_str());
                    let tone_idx = sound
                        .id
                        .get(..8)
                        .and_then(|s| u64::from_str_radix(s, 16).ok())
                        .unwrap_or(0) as usize;
                    let tile = tile_view(sound, is_playing, Tone::from_index(tone_idx), theme, ctx);
                    mouse_area(tile)
                        .on_right_press(Message::OpenContextMenu(sound.id.clone()))
                        .into()
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
    ctx: TileCtx<'a>,
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

    let slot_badge: Option<Element<'_, Message>> = if ctx.shortcuts_active {
        ctx.slots.slot_for(&sound.path).map(|idx| {
            container(
                text(format!("F{}", idx + 1))
                    .size(10)
                    .font(iced::Font::MONOSPACE)
                    .color(theme.ink_dim()),
            )
            .padding([2, 6])
            .style(move |_t| container::Style {
                background: Some(theme::bg_color(theme.panel())),
                border: theme::tile_border(theme.hairline(), 1.0),
                ..Default::default()
            })
            .into()
        })
    } else {
        None
    };

    let mut col = column![category_text, name_text, duration_text].spacing(theme::space::SM);
    if let Some(badge) = slot_badge {
        col = col.push(badge);
    }
    let content = col.padding(theme::space::LG);

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

// Width and estimated max height of the context menu popup.
const MENU_W: f32 = 200.0;
const MENU_H: f32 = 340.0;

pub fn context_menu_overlay<'a>(
    sound: Option<&'a SoundEntry>,
    slots: &'a crate::state::SlotMap,
    theme: Theme,
    pos: iced::Point,
    window_size: (f32, f32),
) -> Element<'a, Message> {
    use iced::widget::Column;

    let sound_path = sound.map(|s| &s.path);
    let assigned_slot = sound_path.and_then(|p| slots.slot_for(p));

    let slot_buttons: Vec<Element<'_, Message>> = (0u8..20)
        .map(|i| {
            let is_assigned = assigned_slot == Some(i);
            let label = if is_assigned {
                format!("\u{2713} Slot {} (F{})", i + 1, i + 1)
            } else {
                format!("  Slot {} (F{})", i + 1, i + 1)
            };

            let msg = sound_path.map(|p| {
                if is_assigned {
                    Message::ClearSlot(i)
                } else {
                    Message::AssignSlot(i, p.clone())
                }
            });

            button(text(label).size(13).color(theme.ink()))
                .on_press_maybe(msg)
                .width(Length::Fill)
                .style(move |_t, status| button::Style {
                    background: Some(theme::bg_color(match status {
                        button::Status::Hovered => theme.accent(),
                        _ => theme.panel(),
                    })),
                    text_color: theme.ink(),
                    ..Default::default()
                })
                .into()
        })
        .collect();

    let menu = container(
        column![
            text(sound.map(|s| s.name.as_str()).unwrap_or(""))
                .size(13)
                .color(theme.ink_dim()),
            iced::widget::scrollable(
                Column::with_children(slot_buttons)
                    .spacing(2)
                    .width(Length::Fill)
            )
            .height(300),
        ]
        .spacing(theme::space::SM)
        .padding(theme::space::MD),
    )
    .width(MENU_W)
    .style(move |_t| container::Style {
        background: Some(theme::bg_color(theme.panel())),
        border: theme::tile_border(theme.hairline(), 1.0),
        ..Default::default()
    });

    // Clamp so menu stays inside window bounds.
    let (win_w, win_h) = window_size;
    let left = if pos.x + MENU_W > win_w {
        (pos.x - MENU_W).max(0.0)
    } else {
        pos.x
    };
    let top = if pos.y + MENU_H > win_h {
        (pos.y - MENU_H).max(0.0)
    } else {
        pos.y
    };

    let dismiss = mouse_area(
        container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(Message::CloseContextMenu)
    .on_right_press(Message::CloseContextMenu);

    container(iced::widget::stack![
        dismiss,
        container(menu)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(iced::Padding {
                top,
                left,
                right: 0.0,
                bottom: 0.0,
            }),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
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
