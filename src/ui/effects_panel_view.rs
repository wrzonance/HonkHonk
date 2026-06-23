//! Iced view for the voice-effects panel. Pure rendering of
//! [`EffectsUiState`](super::effects_panel::EffectsUiState); the testable logic
//! (presets → commands, state updates) lives in [`super::effects_panel`]. This
//! view is intentionally not unit-tested beyond a build smoke check.

use std::ops::RangeInclusive;

use iced::widget::{button, column, container, row, slider, text, Column, Space};
use iced::{Alignment, Border, Element, Length};

use crate::app::Message;
use crate::audio::effects::EffectSlot;
use crate::ui::effects_panel::{EffectsUiState, PresetId};
use crate::ui::side_panel::{self, SidePanelConfig};
use crate::ui::theme::{self, Hh, Theme};

// Slider geometry shared by the parameter controls.
const LABEL_W: f32 = 130.0;
const MIX_SLIDER_W: f32 = 160.0;

/// One labeled parameter slider's spec. Grouped into a struct so
/// [`labeled_slider`] stays within the `too-many-arguments` clippy limit.
struct SliderSpec {
    label: &'static str,
    range: RangeInclusive<f32>,
    value: f32,
    step: f32,
}

/// Top-level effects panel view: master row, preset chips, parameter sliders.
pub fn view_effects_panel(state: &EffectsUiState, t: Theme) -> Element<'static, Message> {
    let body = column![
        view_master_row(state, t),
        view_preset_chips(state.preset, t),
        view_param_sliders(state, t),
    ]
    .spacing(theme::space::LG)
    .width(Length::Fill);

    container(body)
        .width(Length::Fill)
        .padding(theme::space::LG)
        .style(move |_th| container::Style {
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

/// Assembles the effects controls into the reusable side-panel drawer (#143).
/// Owns the drawer's config + body wiring so `app.rs::view_main` only pushes the
/// returned layer — keeping the effects-specific glue out of the god file.
pub fn effects_side_panel_layer(
    state: &EffectsUiState,
    panel_progress: f32,
    t: Theme,
) -> Element<'static, Message> {
    let cfg = SidePanelConfig {
        panel_w: 400.0,
        tab_w: 28.0,
        title: "Voice Effects",
        on_toggle: Message::ToggleEffectsPanel,
        on_close: Message::CloseEffectsPanel,
    };
    side_panel::view_side_panel(cfg, panel_progress, view_effects_panel(state, t), t)
}

/// Master row: chain bypass toggle + wet/dry mix slider.
fn view_master_row(state: &EffectsUiState, t: Theme) -> Element<'static, Message> {
    let active = !state.chain_bypass;
    let bypass_label = if active {
        "Effects: ON"
    } else {
        "Effects: OFF"
    };
    let toggle = button(text(bypass_label).size(theme::font::LABEL).color(t.ink()))
        .on_press(Message::SetEffectBypassUi(active))
        .padding([theme::space::XS, theme::space::MD])
        .style(move |_th, _s| chip_style(t, active));

    let mix = slider(0.0..=1.0, state.wet_dry, Message::SetWetDryMix)
        .step(0.01)
        .width(Length::Fixed(MIX_SLIDER_W));
    let mix_label = text(format!("{}%", (state.wet_dry * 100.0).round() as i32))
        .size(theme::font::LABEL)
        .color(t.ink_dim());

    row![
        toggle,
        Space::new().width(Length::Fill),
        text("Mix").size(theme::font::LABEL).color(t.ink_dim()),
        mix,
        mix_label,
    ]
    .spacing(theme::space::SM)
    .align_y(Alignment::Center)
    .into()
}

/// Preset chip bar. The active preset is highlighted with the accent tone.
/// Chips are stacked vertically so each description line is readable at any
/// panel width.
fn view_preset_chips(active: PresetId, t: Theme) -> Element<'static, Message> {
    let mut chips = Column::new().spacing(theme::space::SM);
    for p in PresetId::ALL {
        let selected = p == active;
        let chip = button(
            column![
                text(format!("{} {}", p.glyph(), p.label()))
                    .size(theme::font::LABEL)
                    .color(t.ink()),
                text(p.description())
                    .size(theme::font::LABEL)
                    .color(t.ink_dim()),
            ]
            .spacing(theme::space::XS),
        )
        .on_press(Message::SelectEffectPreset(p))
        .padding([theme::space::XS, theme::space::MD])
        .width(Length::Fill)
        .style(move |_th, _s| chip_style(t, selected));
        chips = chips.push(chip);
    }
    chips.into()
}

/// Per-effect parameter sliders. Each edit emits a `SetEffectParamUi`.
fn view_param_sliders(state: &EffectsUiState, t: Theme) -> Element<'static, Message> {
    let sliders: Column<'static, Message> = column![
        labeled_slider(
            SliderSpec {
                label: "Pitch (semitones)",
                range: -12.0..=12.0,
                value: state.pitch_semitones,
                step: 0.5,
            },
            |v| param_msg(EffectSlot::Pitch, "semitones", v),
            t,
        ),
        labeled_slider(
            SliderSpec {
                label: "Carrier (Hz)",
                range: 20.0..=2000.0,
                value: state.carrier_hz,
                step: 1.0,
            },
            |v| param_msg(EffectSlot::RingMod, "carrier", v),
            t,
        ),
        labeled_slider(
            SliderSpec {
                label: "Center (Hz)",
                range: 200.0..=4000.0,
                value: state.center_hz,
                step: 1.0,
            },
            |v| param_msg(EffectSlot::Bandpass, "center", v),
            t,
        ),
        labeled_slider(
            SliderSpec {
                label: "Bandwidth (Hz)",
                range: 100.0..=4000.0,
                value: state.bandwidth_hz,
                step: 1.0,
            },
            |v| param_msg(EffectSlot::Bandpass, "bandwidth", v),
            t,
        ),
        labeled_slider(
            SliderSpec {
                label: "Noise",
                range: 0.0..=1.0,
                value: state.noise,
                step: 0.01,
            },
            |v| param_msg(EffectSlot::Bandpass, "noise", v),
            t,
        ),
    ]
    .spacing(theme::space::SM);
    sliders.into()
}

fn param_msg(slot: EffectSlot, param: &'static str, value: f32) -> Message {
    Message::SetEffectParamUi { slot, param, value }
}

fn labeled_slider(
    spec: SliderSpec,
    on_change: impl Fn(f32) -> Message + 'static,
    t: Theme,
) -> Element<'static, Message> {
    row![
        text(spec.label)
            .size(theme::font::LABEL)
            .color(t.ink_dim())
            .width(Length::Fixed(LABEL_W)),
        slider(spec.range, spec.value, on_change)
            .step(spec.step)
            .width(Length::Fill),
    ]
    .spacing(theme::space::SM)
    .align_y(Alignment::Center)
    .into()
}

fn chip_style(t: Theme, selected: bool) -> button::Style {
    let bg = if selected { t.accent() } else { t.panel() };
    button::Style {
        background: Some(theme::bg_color(bg)),
        text_color: t.ink(),
        border: Border {
            color: t.hairline(),
            width: 1.0,
            radius: theme::radius::PILL,
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_panel_view_builds_for_each_preset() {
        for p in PresetId::ALL {
            let mut state = EffectsUiState::default();
            state.apply_preset(p);
            let _el = view_effects_panel(&state, Theme::Dark);
        }
    }
}
