use iced::widget::{button, canvas, column, container, mouse_area, row, text};
use iced::{Element, Length};

use crate::app::Message;
use crate::state::{SlotMap, SoundEntry};
use crate::ui::sound_tile::{derive_glyph, derive_seed, derive_tone, SoundTile, SoundTileData};
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
}

#[derive(Clone, Copy)]
struct TileCtx<'a> {
    slot_ctx: SlotCtx<'a>,
    shortcuts_active: bool,
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
    };
    // Playing state visual is deferred to issue #92 (animations).
    let _ = playing;

    let rows: Vec<Element<'a, Message>> = sounds
        .chunks(grid.columns)
        .map(|chunk| {
            let tiles: Vec<Element<'a, Message>> = chunk
                .iter()
                .map(|sound| tile_view(sound, theme, ctx))
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

fn tile_view<'a>(sound: &'a SoundEntry, theme: Theme, ctx: TileCtx<'a>) -> Element<'a, Message> {
    let hotkey = if ctx.shortcuts_active {
        ctx.slot_ctx.slots.slot_for(&sound.path).and_then(|idx| {
            ctx.slot_ctx
                .triggers
                .get(idx as usize)
                .and_then(|t| t.clone())
        })
    } else {
        None
    };

    let data = SoundTileData {
        id: sound.id.clone(),
        name: sound.name.clone(),
        category: sound.category.clone(),
        tone: derive_tone(&sound.id),
        duration_secs: sound.duration_ms.unwrap_or(0) as f32 / 1000.0,
        hotkey,
        favorite: false,
        seed: derive_seed(&sound.id),
        glyph: derive_glyph(&sound.id),
    };

    let canvas_el = canvas(SoundTile::new(data, theme))
        .width(Length::Fill)
        .height(theme::component::SOUND_TILE_H);

    mouse_area(canvas_el)
        .on_press(Message::PlaySound(sound.id.clone()))
        .on_right_press(Message::OpenContextMenu(sound.id.clone()))
        .into()
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

    let menu = container(
        column![
            text(sound.map(|s| s.name.as_str()).unwrap_or(""))
                .size(theme::font::BODY)
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
