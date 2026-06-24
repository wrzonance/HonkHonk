use iced::widget::{button, column, container, mouse_area, row, text};
use iced::{mouse, Element, Length};

use crate::app::Message;
use crate::state::{SlotMap, SoundEntry, SoundMetaStore};
use crate::ui::sound_tile::{self, SoundTileData};
use crate::ui::theme::{self, Hh, Theme};

#[derive(Clone, Copy)]
pub struct SlotCtx<'a> {
    pub slots: &'a SlotMap,
    pub triggers: &'a [Option<String>; 20],
}

#[derive(Clone, Copy)]
pub struct GridCtx<'a> {
    pub slots: &'a SlotMap,
    pub triggers: &'a [Option<String>; 20],
    pub shortcuts_active: bool,
    pub columns: usize,
    pub sound_meta: &'a SoundMetaStore,
}

#[derive(Clone, Copy)]
struct TileCtx<'a> {
    slot_ctx: SlotCtx<'a>,
    shortcuts_active: bool,
    sound_meta: &'a SoundMetaStore,
}

pub fn view_grid<'a>(
    sounds: &[&'a SoundEntry],
    playing: Option<&str>,
    grid: GridCtx<'a>,
) -> Element<'a, Message> {
    let theme = Theme::Dark;

    if sounds.is_empty() {
        return container(
            text("No sounds found. Add audio files to your sound directory.")
                .size(theme::font::BODY)
                .color(theme.ink_dim()),
        )
        .width(Length::Fill)
        .padding(theme::space::XXL)
        .into();
    }

    let ctx = TileCtx {
        slot_ctx: SlotCtx {
            slots: grid.slots,
            triggers: grid.triggers,
        },
        shortcuts_active: grid.shortcuts_active,
        sound_meta: grid.sound_meta,
    };

    let rows: Vec<Element<'a, Message>> = sounds
        .chunks(grid.columns)
        .map(|chunk| {
            let tiles: Vec<Element<'a, Message>> = chunk
                .iter()
                .map(|sound| {
                    let is_playing = playing == Some(sound.id.as_str());
                    let tile = sound_tile::view(tile_data(sound, ctx), theme, is_playing);
                    mouse_area(tile)
                        .on_press(Message::PlaySound(sound.id.clone()))
                        .on_right_press(Message::OpenContextMenu(sound.id.clone()))
                        .interaction(mouse::Interaction::Pointer)
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

fn tile_data(sound: &SoundEntry, ctx: TileCtx<'_>) -> SoundTileData {
    let seed = sound_tile::seed_from_sound_id(&sound.id);
    let name = ctx
        .sound_meta
        .get_ref(&sound.id)
        .and_then(|m| m.display_name.as_deref())
        .unwrap_or(sound.name.as_str())
        .to_owned();

    SoundTileData {
        id: sound.id.clone(),
        name,
        category: sound.category.clone(),
        duration: crate::ui::fmt_duration(sound.duration_ms),
        hotkey: hotkey_for(sound, ctx),
        favorite: ctx.sound_meta.is_favorite(&sound.id),
        tone: sound_tile::tone_from_seed(seed),
        seed,
    }
}

fn hotkey_for(sound: &SoundEntry, ctx: TileCtx<'_>) -> Option<String> {
    if !ctx.shortcuts_active {
        return None;
    }

    ctx.slot_ctx.slots.slot_for(&sound.path).and_then(|idx| {
        ctx.slot_ctx
            .triggers
            .get(idx as usize)
            .and_then(|trigger| trigger.clone())
    })
}

// Width and estimated max height of the context menu popup.
const MENU_W: f32 = 200.0;
const MENU_H: f32 = 340.0;

pub fn context_menu_overlay<'a>(
    sound: Option<&'a SoundEntry>,
    slot_ctx: SlotCtx<'a>,
    theme: Theme,
    pos: iced::Point,
    window_size: (f32, f32),
) -> Element<'a, Message> {
    use iced::widget::Column;

    let sound_path = sound.map(|s| &s.path);
    let assigned_slot = sound_path.and_then(|p| slot_ctx.slots.slot_for(p));

    let slot_buttons: Vec<Element<'_, Message>> = (0u8..20)
        .map(|i| {
            let is_assigned = assigned_slot == Some(i);
            let trigger = slot_ctx.triggers.get(i as usize).and_then(|t| t.as_deref());
            let label = match (is_assigned, trigger) {
                (true, Some(t)) => format!("\u{2713} Slot {}  {}", i + 1, t),
                (true, None) => format!("\u{2713} Slot {}", i + 1),
                (false, Some(t)) => format!("  Slot {}  {}", i + 1, t),
                (false, None) => format!("  Slot {}", i + 1),
            };

            let msg = sound_path.map(|p| {
                if is_assigned {
                    Message::ClearSlot(i)
                } else {
                    Message::AssignSlot(i, p.clone())
                }
            });

            button(text(label).size(theme::font::BODY).color(theme.ink()))
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

    let edit_btn = sound.map(|s| {
        button(
            text("Edit sound…")
                .size(theme::font::BODY)
                .color(theme.ink()),
        )
        .on_press(Message::OpenSoundEditor(s.id.clone()))
        .width(Length::Fill)
        .style(move |_t, status| button::Style {
            background: Some(theme::bg_color(match status {
                button::Status::Hovered => theme.accent(),
                _ => theme.panel(),
            })),
            text_color: theme.ink(),
            ..Default::default()
        })
    });

    let mut menu_col = Column::new()
        .push(
            text(sound.map(|s| s.name.as_str()).unwrap_or(""))
                .size(theme::font::BODY)
                .color(theme.ink_dim()),
        )
        .spacing(theme::space::SM)
        .padding(theme::space::MD);

    if let Some(btn) = edit_btn {
        menu_col = menu_col.push(btn);
    }

    menu_col = menu_col.push(
        iced::widget::scrollable(
            Column::with_children(slot_buttons)
                .spacing(2)
                .width(Length::Fill),
        )
        .height(260),
    );

    let menu = container(menu_col)
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
