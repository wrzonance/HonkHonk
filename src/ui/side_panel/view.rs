//! Renders the side-panel drawer: a fading scrim over the main view plus a
//! pull-tab + body that slides in from the right edge. The slide uses `Float`
//! (stock Iced 0.14): the drawer is laid out at natural size, then translated by
//! the hidden fraction; while translated `Float` re-hosts it as an overlay, so
//! the body keeps full width off-screen and input lands at the drawn position.

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

/// Builds the side-panel overlay layer for animation `progress` (0=closed..1=open),
/// wrapping `body` as the drawer content. The `Float`-based slide and `Fill` scrim
/// are window-agnostic, so no window size is needed here; the #144 feather puff
/// will derive its origin from [`panel_geometry`](super::panel_geometry) at its
/// own call site.
pub fn view_side_panel<'a>(
    cfg: SidePanelConfig,
    progress: f32,
    body: Element<'a, Message>,
    t: Theme,
) -> Element<'a, Message> {
    let progress = progress.clamp(0.0, 1.0);
    let panel_w = cfg.panel_w;

    let drawer = row![tab(&cfg, progress, t), panel_body(&cfg, body, t)].height(Length::Fill);
    let floated = float(drawer)
        .translate(move |_content, _viewport| Vector::new((1.0 - progress) * panel_w, 0.0));
    let anchored = container(floated)
        .align_right(Length::Fill)
        .height(Length::Fill);

    if progress <= 0.0 {
        return anchored.into();
    }

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

    iced::widget::stack![scrim, anchored]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Vertical pull tab. Chevron hints the action: "‹" to pull open, "›" to push closed.
fn tab<'a>(cfg: &SidePanelConfig, progress: f32, t: Theme) -> Element<'a, Message> {
    let glyph = if progress > 0.5 {
        "\u{203A}"
    } else {
        "\u{2039}"
    };
    button(
        container(text(glyph).size(GLYPH_SIZE).color(t.ink()))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(cfg.tab_w))
    .height(Length::Fill)
    .on_press(cfg.on_toggle.clone())
    .style(move |_th, _s| tab_style(t))
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

fn tab_style(t: Theme) -> button::Style {
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
