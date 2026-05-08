use iced::widget::{button, column, container, row, scrollable, text, Column, Row, Space};
use iced::{Element, Length};

use crate::app::Message;
use crate::state::{SlotMap, SoundEntry};
use crate::ui::theme::{self, Hh, Theme, Tone};

fn fmt_duration(ms: Option<u64>) -> String {
    ms.map(|ms| format!("{}:{:02}", ms / 60000, (ms % 60000) / 1000))
        .unwrap_or_else(|| "—".into())
}

fn tone_for(sound: &SoundEntry) -> Tone {
    let idx = sound
        .id
        .get(..8)
        .and_then(|s| u64::from_str_radix(s, 16).ok())
        .unwrap_or(0) as usize;
    Tone::from_index(idx)
}

pub fn view_slot_manager<'a>(
    slots: &'a SlotMap,
    sounds: &'a [SoundEntry],
    selected_slot: Option<u8>,
    t: Theme,
) -> Element<'a, Message> {
    let bound_count = (0u8..20).filter(|&i| slots.get(i).is_some()).count();
    let header = slot_header(bound_count, t);
    let divider = container(Space::new())
        .width(1)
        .height(Length::Fill)
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(t.hairline())),
            ..Default::default()
        });
    let grid = slot_grid(slots, sounds, selected_slot, t);
    let side = sidebar(slots, sounds, selected_slot, t);
    let body = row![grid, divider, side].height(Length::Fill);
    container(column![header, body].height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(t.bg())),
            ..Default::default()
        })
        .into()
}

fn slot_header<'a>(bound_count: usize, t: Theme) -> Element<'a, Message> {
    let back_btn = button(
        row![
            text("←").size(14).color(t.ink()),
            text("Back to sounds").size(13).color(t.ink()),
        ]
        .spacing(theme::space::XS)
        .align_y(iced::Alignment::Center),
    )
    .on_press(Message::ShowMain)
    .style(move |_t, _s| button::Style {
        background: Some(theme::bg_color(t.panel())),
        text_color: t.ink(),
        border: theme::tile_border(t.hairline(), 1.0),
        ..Default::default()
    });

    let title = text("Slots").size(22).color(t.ink());
    let sep = text("·").size(14).color(t.ink_dim());
    let stats = text(format!("{bound_count} bound"))
        .size(12)
        .color(t.ink_dim());

    container(
        row![back_btn, title, sep, stats]
            .spacing(theme::space::MD)
            .align_y(iced::Alignment::Center),
    )
    .padding([theme::space::MD, theme::space::LG])
    .style(move |_t| container::Style {
        border: iced::Border {
            color: t.hairline(),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn slot_grid<'a>(
    slots: &'a SlotMap,
    sounds: &'a [SoundEntry],
    selected_slot: Option<u8>,
    t: Theme,
) -> Element<'a, Message> {
    let rows: Vec<Element<'_, Message>> = (0u8..4)
        .map(|row_idx| {
            let tiles: Vec<Element<'_, Message>> = (0u8..5)
                .map(|col_idx| {
                    let idx = row_idx * 5 + col_idx;
                    let sound = slots
                        .get(idx)
                        .and_then(|p| sounds.iter().find(|s| &s.path == p));
                    slot_tile(idx, sound, selected_slot == Some(idx), t)
                })
                .collect();
            Row::with_children(tiles).spacing(theme::space::MD).into()
        })
        .collect();

    scrollable(
        container(Column::with_children(rows).spacing(theme::space::MD))
            .padding(theme::space::LG)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn slot_tile<'a>(
    idx: u8,
    sound: Option<&'a SoundEntry>,
    selected: bool,
    t: Theme,
) -> Element<'a, Message> {
    match sound {
        Some(s) => bound_tile(idx, s, selected, t),
        None => empty_tile(idx, selected, t),
    }
}

fn bound_tile<'a>(
    idx: u8,
    sound: &'a SoundEntry,
    selected: bool,
    t: Theme,
) -> Element<'a, Message> {
    let tone = tone_for(sound);
    let bg = tone.tile_tint(t.is_dark());
    let border = if selected {
        iced::Border {
            color: t.ink(),
            width: 2.5,
            radius: 18.0.into(),
        }
    } else {
        iced::Border {
            color: t.hairline(),
            width: 1.0,
            radius: 18.0.into(),
        }
    };
    let circle = container(Space::new())
        .width(40)
        .height(40)
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(tone.highlight(t.is_dark()))),
            border: iced::Border {
                radius: 20.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });
    button(
        column![
            text(format!("#{:02}", idx + 1))
                .size(10)
                .color(t.ink_faint()),
            circle,
            text(sound.name.clone()).size(11).color(t.ink()),
            text("no hotkey").size(10).color(t.ink_faint()),
        ]
        .spacing(4)
        .align_x(iced::Alignment::Center)
        .padding(theme::space::SM),
    )
    .on_press(Message::SelectSlot(idx))
    .width(Length::Fill)
    .height(138)
    .style(move |_t, _s| button::Style {
        background: Some(theme::bg_color(bg)),
        text_color: t.ink(),
        border,
        ..Default::default()
    })
    .into()
}

fn empty_tile<'a>(idx: u8, selected: bool, t: Theme) -> Element<'a, Message> {
    let border = if selected {
        iced::Border {
            color: t.ink(),
            width: 2.5,
            radius: 18.0.into(),
        }
    } else {
        iced::Border {
            color: t.hairline2(),
            width: 2.0,
            radius: 18.0.into(),
        }
    };
    button(
        column![
            text(format!("#{:02}", idx + 1))
                .size(10)
                .color(t.ink_faint()),
            text("+").size(22).color(t.ink_faint()),
            text("EMPTY").size(10).color(t.ink_faint()),
        ]
        .spacing(6)
        .align_x(iced::Alignment::Center)
        .padding(theme::space::SM),
    )
    .on_press(Message::SelectSlot(idx))
    .width(Length::Fill)
    .height(138)
    .style(move |_t, _s| button::Style {
        background: Some(theme::bg_color(t.panel())),
        text_color: t.ink_faint(),
        border,
        ..Default::default()
    })
    .into()
}

fn sound_header<'a>(sound: &'a SoundEntry, t: Theme) -> Element<'a, Message> {
    let tone = tone_for(sound);
    let circle = container(Space::new())
        .width(56)
        .height(56)
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(tone.highlight(t.is_dark()))),
            border: iced::Border {
                radius: 28.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });
    let info = column![
        text(sound.name.clone()).size(17).color(t.ink()),
        text(format!(
            "{} · {}",
            sound.category,
            fmt_duration(sound.duration_ms)
        ))
        .size(11)
        .color(t.ink_dim()),
    ]
    .spacing(2);
    row![circle, info]
        .spacing(theme::space::MD)
        .align_y(iced::Alignment::Center)
        .into()
}

fn sidebar_bound_hotkey<'a>(t: Theme) -> Element<'a, Message> {
    container(text("—").size(13).color(t.ink()))
        .padding([theme::space::SM, theme::space::MD])
        .width(Length::Fill)
        .style(move |_t| container::Style {
            border: iced::Border {
                color: t.accent(),
                width: 1.5,
                radius: 10.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn sidebar_bound_portal<'a>(t: Theme) -> Element<'a, Message> {
    let dot = container(Space::new())
        .width(8)
        .height(8)
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(t.good())),
            border: iced::Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });
    container(
        row![
            dot,
            text("Registered via xdg-desktop-portal")
                .size(11)
                .color(t.ink_dim())
        ]
        .spacing(theme::space::SM)
        .align_y(iced::Alignment::Center),
    )
    .padding([theme::space::SM, theme::space::MD])
    .style(move |_t| container::Style {
        background: Some(theme::bg_color(t.bg())),
        border: theme::tile_border(t.hairline(), 1.0),
        ..Default::default()
    })
    .into()
}

fn sidebar_bound<'a>(idx: u8, sound: &'a SoundEntry, t: Theme) -> Element<'a, Message> {
    let slot_label = text(format!("SLOT #{:02}", idx + 1))
        .size(10)
        .color(t.ink_dim());
    let hk_display = sidebar_bound_hotkey(t);
    let portal = sidebar_bound_portal(t);
    let unbind = button(
        text("Unbind")
            .size(12)
            .color(iced::Color::from_rgb(0.86, 0.15, 0.15)),
    )
    .on_press(Message::ClearSlot(idx))
    .width(Length::Fill)
    .style(move |_t, _s| button::Style {
        background: None,
        text_color: iced::Color::from_rgb(0.86, 0.15, 0.15),
        border: iced::Border {
            color: iced::Color::from_rgba(0.86, 0.15, 0.15, 0.4),
            width: 1.0,
            radius: 10.0.into(),
        },
        ..Default::default()
    });
    column![
        slot_label,
        sound_header(sound, t),
        text("GLOBAL HOTKEY").size(11).color(t.ink_dim()),
        hk_display,
        text("PORTAL STATUS").size(11).color(t.ink_dim()),
        portal,
        unbind,
    ]
    .spacing(theme::space::MD)
    .into()
}

fn sidebar_empty<'a>(idx: u8, t: Theme) -> Element<'a, Message> {
    let slot_label = text(format!("SLOT #{:02}", idx + 1))
        .size(10)
        .color(t.ink_dim());
    let placeholder = container(
        column![
            text("🪿").size(32),
            text("Slot is empty").size(13).color(t.ink()),
            text("Assign via right-click on any sound tile")
                .size(11)
                .color(t.ink_dim()),
        ]
        .spacing(theme::space::SM)
        .align_x(iced::Alignment::Center)
        .padding(theme::space::LG),
    )
    .width(Length::Fill)
    .style(move |_t| container::Style {
        background: Some(theme::bg_color(t.bg())),
        border: iced::Border {
            color: t.hairline2(),
            width: 2.0,
            radius: 14.0.into(),
        },
        ..Default::default()
    });
    column![slot_label, placeholder]
        .spacing(theme::space::MD)
        .into()
}

fn sidebar<'a>(
    slots: &'a SlotMap,
    sounds: &'a [SoundEntry],
    selected_slot: Option<u8>,
    t: Theme,
) -> Element<'a, Message> {
    let inner: Element<'_, Message> = match selected_slot {
        None => text("Select a slot to inspect it")
            .size(13)
            .color(t.ink_faint())
            .into(),
        Some(idx) => {
            let sound = slots
                .get(idx)
                .and_then(|p| sounds.iter().find(|s| &s.path == p));
            match sound {
                Some(s) => sidebar_bound(idx, s, t),
                None => sidebar_empty(idx, t),
            }
        }
    };
    container(inner)
        .width(320)
        .height(Length::Fill)
        .padding(theme::space::LG)
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(t.panel())),
            ..Default::default()
        })
        .into()
}
