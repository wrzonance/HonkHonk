use iced::widget::{Column, Row, Space, button, column, container, row, scrollable, text};
use iced::{Element, Length};

mod controls;
mod empty;
#[cfg(test)]
mod grid_tests;
mod macro_slot;
mod sound;
#[cfg(test)]
mod tests;

use crate::app::{HonkHonk, Message};
use crate::state::{Macro, MacroStore, SlotContent, SlotMap, SoundEntry};
use crate::ui::list_controls::sort;
use crate::ui::search_bar;
use crate::ui::theme::{self, Hh, Theme, Tone};

/// Bundles the shared slot-manager view state to stay under clippy's
/// `too-many-arguments` threshold (5).
#[derive(Clone, Copy)]
pub struct SlotManagerCtx<'a> {
    pub slots: &'a SlotMap,
    pub slot_triggers: &'a [Option<String>; 20],
    pub sounds: &'a [SoundEntry],
    pub macros: &'a MacroStore,
    pub selected_slot: Option<u8>,
    /// Whether portal v2 `configure_shortcuts()` is available on this DE/backend.
    pub configure_available: bool,
}

/// A slot's content resolved against the live sound/macro collections — the
/// single render-time resolution point shared by the grid and the sidebar. A
/// reference that no longer resolves (a deleted sound file, a removed macro)
/// degrades to [`SlotView::Empty`]; resolution never mutates `ctx` and never
/// self-clears the dangling reference (that only happens at activation time,
/// in [`crate::app::slots`]).
#[derive(Debug, Clone, Copy, PartialEq)]
enum SlotView<'a> {
    Empty,
    Sound(&'a SoundEntry),
    Macro(&'a Macro),
}

/// Resolves slot `idx`'s content. Pure/read-only.
fn resolve_slot<'a>(idx: u8, ctx: &SlotManagerCtx<'a>) -> SlotView<'a> {
    match ctx.slots.content(idx) {
        None => SlotView::Empty,
        Some(SlotContent::Sound(path)) => ctx
            .sounds
            .iter()
            .find(|s| &s.path == path)
            .map_or(SlotView::Empty, SlotView::Sound),
        Some(SlotContent::Macro(id)) => ctx.macros.get(id).map_or(SlotView::Empty, SlotView::Macro),
    }
}

/// Counts slots holding either content kind — a sound or a macro. Must use
/// `SlotMap::content`, not `SlotMap::get` (sound-only), or macro slots are
/// undercounted.
fn bound_count(slots: &SlotMap) -> usize {
    (0u8..20).filter(|&i| slots.content(i).is_some()).count()
}

/// A macro's label for display. `MacroStore::add`/`rename` accept a blank or
/// whitespace-only name, which would otherwise render as an invisible label
/// — indistinguishable from every other unnamed macro. Every surface that
/// shows a macro name goes through here so the assignment list, the slot
/// tile and the sidebar agree on what an unnamed macro is called (#169
/// review).
pub(crate) fn display_name(macro_def: &Macro) -> &str {
    if macro_def.name.trim().is_empty() {
        "Untitled macro"
    } else {
        &macro_def.name
    }
}

/// Tiles per grid row. The slot map is a fixed 20 slots, so an unfiltered
/// grid is exactly four full rows; filtering is what makes a partial row
/// possible (see `slot_grid`).
const SLOT_COLUMNS: usize = 5;

/// Filler tiles needed to complete a row holding `tiles_in_row` real tiles.
/// Mirrors `sound_grid::missing_tile_slots`.
fn missing_tile_slots(tiles_in_row: usize) -> usize {
    SLOT_COLUMNS.saturating_sub(tiles_in_row)
}

pub(super) fn tone_for(sound: &SoundEntry) -> Tone {
    let idx = sound
        .id
        .get(..8)
        .and_then(|s| u64::from_str_radix(s, 16).ok())
        .unwrap_or(0) as usize;
    Tone::from_index(idx)
}

/// Kept as a `Stack` with the base layout always at child 0, mirroring
/// `view_settings`'s #112 pattern: flipping the root between a plain
/// container (sort menu closed) and a stack (menu open) would make iced
/// discard every descendant's positional state on open/close, snapping the
/// grid scrollable back to the top. Pushing/popping the sort-menu overlay
/// as child 1 instead leaves the base subtree's diff untouched.
pub fn view_slot_manager<'a>(
    state: &'a HonkHonk,
    ctx: SlotManagerCtx<'a>,
    t: Theme,
) -> Element<'a, Message> {
    let render_order = state.slot_render_order();
    let header = slot_header(state, bound_count(ctx.slots), t);
    let divider = container(Space::new())
        .width(1)
        .height(Length::Fill)
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(t.hairline())),
            ..Default::default()
        });
    let grid = slot_grid(&ctx, &render_order, t);
    let side = sidebar(&ctx, t);
    let body = row![grid, divider, side].height(Length::Fill);
    let base: Element<'a, Message> = container(column![header, body].height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(t.bg())),
            ..Default::default()
        })
        .into();

    let mut layers: Vec<Element<'a, Message>> = vec![base];
    if let Some(sort_menu) = state.view_slot_sort_overlay(t) {
        layers.push(sort_menu);
    }

    iced::widget::Stack::with_children(layers)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn slot_header<'a>(state: &'a HonkHonk, bound_count: usize, t: Theme) -> Element<'a, Message> {
    let back_btn = button(
        row![
            text("←").size(theme::font::BODY).color(t.ink()),
            text("Back to sounds")
                .size(theme::font::BODY)
                .color(t.ink()),
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

    let title = text("Slots").size(theme::font::TITLE).color(t.ink());
    let sep = text("·").size(theme::font::BODY).color(t.ink_dim());
    let stats = text(format!("{bound_count} bound"))
        .size(theme::font::LABEL)
        .color(t.ink_dim());
    let top_row = row![back_btn, title, sep, stats]
        .spacing(theme::space::MD)
        .align_y(iced::Alignment::Center);

    let search =
        search_bar::view_slots_search_bar(state.slot_filter_query(), t, Message::SlotSearchChanged);
    let sort_chip = sort::view_sort_chip(
        state.slot_sort_state(),
        Message::ToggleSlotSortMenu,
        Message::ToggleSlotSortDirection,
        t,
    );
    let controls = row![search, sort_chip]
        .spacing(theme::space::SM)
        .align_y(iced::Alignment::Center);

    container(column![top_row, controls].spacing(theme::space::MD))
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

/// Lays out the slot tiles named in `render_order`, five per row, in that
/// exact order — reordering the grid without ever remapping a tile's own
/// identity: each tile still resolves and labels itself against its real
/// `slot_index` (`resolve_slot`, `slot_tile`), never a position-derived one.
/// `render_order` values are plain `u8` copies read once per tile; the
/// returned `Element` borrows nothing from the slice itself, only from
/// `ctx`'s own `'a` lifetime.
///
/// An empty `render_order` means the active filter query matched no slot
/// (`build_slot_rows` always emits all 20 slots absent a query) — rendered
/// as a "no matches" message instead of an empty grid.
fn slot_grid<'a>(ctx: &SlotManagerCtx<'a>, render_order: &[u8], t: Theme) -> Element<'a, Message> {
    if render_order.is_empty() {
        return container(
            text("No slots match your search.")
                .size(theme::font::BODY)
                .color(t.ink_dim()),
        )
        .padding(theme::space::LG)
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    }

    let rows: Vec<Element<'a, Message>> = render_order
        .chunks(SLOT_COLUMNS)
        .map(|chunk| {
            let mut tiles: Vec<Element<'a, Message>> = chunk
                .iter()
                .map(|&idx| {
                    let view = resolve_slot(idx, ctx);
                    let trigger = ctx.slot_triggers[idx as usize].as_deref();
                    slot_tile(idx, view, trigger, ctx.selected_slot == Some(idx), t)
                })
                .collect();

            // A filtered grid can end on a partial row. Every tile is
            // `Length::Fill`, so without fillers the survivors would split
            // the full grid width between them — a single match would render
            // as one full-width card. Pad to a whole row, as the sound grid
            // does, so a tile keeps the same width whatever the query.
            tiles.extend(
                (0..missing_tile_slots(chunk.len()))
                    .map(|_| Space::new().width(Length::Fill).into()),
            );

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
    view: SlotView<'a>,
    trigger: Option<&'a str>,
    selected: bool,
    t: Theme,
) -> Element<'a, Message> {
    match view {
        SlotView::Sound(s) => sound::bound_tile(idx, s, trigger, selected, t),
        SlotView::Macro(m) => macro_slot::tile(idx, m, trigger, selected, t),
        SlotView::Empty => empty::empty_tile(idx, selected, t),
    }
}

pub(super) fn tone_circle<'a>(tone: Tone, size: f32, t: Theme) -> Element<'a, Message> {
    let r = size / 2.0;
    container(Space::new())
        .width(size)
        .height(size)
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(tone.highlight(t.is_dark()))),
            border: iced::Border {
                radius: r.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn sidebar<'a>(ctx: &SlotManagerCtx<'a>, t: Theme) -> Element<'a, Message> {
    let inner: Element<'a, Message> = match ctx.selected_slot {
        None => text("Select a slot to inspect it")
            .size(theme::font::BODY)
            .color(t.ink_faint())
            .into(),
        Some(idx) => {
            let trigger = ctx
                .slot_triggers
                .get(idx as usize)
                .and_then(|t| t.as_deref());
            match resolve_slot(idx, ctx) {
                SlotView::Sound(s) => {
                    sound::sidebar_bound(idx, s, trigger, ctx.configure_available, t)
                }
                SlotView::Macro(m) => {
                    macro_slot::sidebar_bound(idx, m, trigger, ctx.configure_available, t)
                }
                SlotView::Empty => empty::sidebar_empty(idx, ctx.macros, t),
            }
        }
    };
    // Scrollable because the sidebar's height is fixed to the window but its
    // content is not: an empty slot lists one assign button per stored macro,
    // and a short window clips even a bound slot's controls. Without this the
    // trailing entries are unreachable (#169 review).
    container(scrollable(inner).width(Length::Fill).height(Length::Fill))
        .width(320)
        .height(Length::Fill)
        .padding(theme::space::LG)
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(t.panel())),
            ..Default::default()
        })
        .into()
}
