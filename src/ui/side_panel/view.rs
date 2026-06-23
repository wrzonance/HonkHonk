//! Renders the side-panel drawer. Closed, it shows only a small grab handle at
//! the right edge — the rest of the window stays clear and clickable. Open, it is
//! a fading scrim plus the body sliding in from the right edge; the handle sits
//! *beneath* the panel, so an open drawer reads as one fluid object (the handle
//! is revealed only as the panel slides back out). The body slides via `Float`
//! (stock Iced 0.14): laid out at natural width, then translated by the hidden
//! fraction and re-hosted as an overlay, so it keeps full width off-screen and
//! input lands where it is drawn.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, float, mouse_area, row, scrollable, text, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Vector};

use crate::app::Message;
use crate::ui::theme::{self, Hh, Theme};

/// Static configuration + messages a consumer hands the framework.
pub struct SidePanelConfig {
    /// Panel body width in logical px.
    pub panel_w: f32,
    /// Pull-tab width in logical px.
    pub tab_w: f32,
    /// Title shown in the drawer header.
    pub title: &'static str,
    /// Emitted when the pull tab is pressed.
    pub on_toggle: Message,
    /// Emitted when the scrim or the header ✕ is pressed.
    pub on_close: Message,
}

/// Max scrim opacity at full open.
const SCRIM_MAX: f32 = 0.5;
/// Header / ✕ font size.
const TITLE_SIZE: f32 = 14.0;
/// Chevron glyph size.
const GLYPH_SIZE: f32 = 18.0;
/// Height of the right-edge grab handle — a small centered nub, deliberately not
/// full height, so it never covers the toolbar or the right column of tiles.
const HANDLE_H: f32 = 96.0;

/// Builds the side-panel layer for animation `progress` (0=closed..1=open),
/// wrapping `body` as the drawer content. Window-agnostic — the `Float` slide and
/// `Fill` scrim need no window size; #144 derives its feather origin from
/// [`panel_geometry`](super::panel_geometry) at its own call site.
pub fn view_side_panel<'a>(
    cfg: SidePanelConfig,
    progress: f32,
    body: Element<'a, Message>,
    t: Theme,
) -> Element<'a, Message> {
    let progress = progress.clamp(0.0, 1.0);

    // Fully closed: only the grab handle. No scrim, no full-width layer — the grid
    // stays entirely visible and clickable except under the small handle.
    if progress <= 0.0 {
        return edge_handle(&cfg, t);
    }

    let panel_w = cfg.panel_w;
    let sliding = container(
        float(panel_body(&cfg, body, t))
            .translate(move |_content, _viewport| Vector::new((1.0 - progress) * panel_w, 0.0)),
    )
    .align_right(Length::Fill)
    .height(Length::Fill);

    let alpha = SCRIM_MAX * progress;
    let scrim = mouse_area(
        container(Space::new().width(Length::Fill).height(Length::Fill)).style(move |_| {
            container::Style {
                background: Some(Background::Color(Color {
                    a: alpha,
                    ..Color::BLACK
                })),
                ..Default::default()
            }
        }),
    )
    .on_press(cfg.on_close.clone());

    // Handle beneath the panel: covered when open, revealed as the panel slides
    // out — so the open drawer is one fluid object, not a panel plus a stub.
    iced::widget::stack![scrim, edge_handle(&cfg, t), sliding]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Small grab handle pinned to the right edge, vertically centered — the panel's
/// open affordance (closing is via the ✕, the scrim, or Escape). As a layer it
/// occupies only the handle's bounds, so clicks elsewhere fall through to the grid.
fn edge_handle<'a>(cfg: &SidePanelConfig, t: Theme) -> Element<'a, Message> {
    let handle = button(
        container(text("\u{2039}").size(GLYPH_SIZE).color(t.ink()))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(cfg.tab_w))
    .height(Length::Fixed(HANDLE_H))
    .on_press(cfg.on_toggle.clone())
    .style(move |_th, _s| handle_style(t));

    container(handle)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Right)
        .align_y(Vertical::Center)
        .into()
}

/// Drawer body: title bar (+ ✕) over the scrollable content.
fn panel_body<'a>(
    cfg: &SidePanelConfig,
    body: Element<'a, Message>,
    t: Theme,
) -> Element<'a, Message> {
    let header = row![
        text(cfg.title).size(TITLE_SIZE).color(t.ink()),
        Space::new().width(Length::Fill),
        close_button(cfg, t),
    ]
    .align_y(Alignment::Center);

    let content = column![header, scrollable(body).height(Length::Fill)]
        .spacing(theme::space::MD)
        .padding(theme::space::LG);

    container(content)
        .width(Length::Fixed(cfg.panel_w))
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(theme::bg_color(t.panel())),
            border: Border {
                color: t.hairline(),
                width: 1.0,
                radius: theme::radius::MD,
            },
            ..Default::default()
        })
        .into()
}

fn close_button<'a>(cfg: &SidePanelConfig, t: Theme) -> Element<'a, Message> {
    // Borderless icon button so the ✕ reads as a glyph, not a second tab-like chip.
    button(text("\u{2715}").size(TITLE_SIZE).color(t.ink_dim()))
        .on_press(cfg.on_close.clone())
        .style(move |_th, _s| button::Style {
            background: None,
            text_color: t.ink(),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: theme::radius::SM,
            },
            ..Default::default()
        })
        .into()
}

fn handle_style(t: Theme) -> button::Style {
    button::Style {
        background: Some(theme::bg_color(t.panel())),
        text_color: t.ink(),
        border: Border {
            color: t.hairline(),
            width: 1.0,
            radius: theme::radius::MD,
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> SidePanelConfig {
        // Uses pre-existing Message variants so this smoke test does not depend on
        // the new effects-panel messages (added in the app-wiring task).
        SidePanelConfig {
            panel_w: 400.0,
            tab_w: 28.0,
            title: "Test",
            on_toggle: Message::StopAll,
            on_close: Message::StopAll,
        }
    }

    #[test]
    fn builds_across_progress() {
        for p in [0.0_f32, 0.5, 1.0] {
            let _el = view_side_panel(cfg(), p, text("body").into(), Theme::Dark);
        }
    }
}
