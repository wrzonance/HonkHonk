#[allow(unused_imports)]
use iced::{
    widget::{button, column, container, row, scrollable, text, Column, Row, Space},
    Alignment, Element, Length,
};

use crate::app::{HonkHonk, Message, SettingsSection};
use crate::settings::{ControlType, SettingDef, SettingId, SettingValue};
use crate::ui::theme::{self, Hh, Theme};

/// Top-level settings view — full window swap.
pub fn view_settings(state: &HonkHonk, t: Theme) -> Element<'_, Message> {
    let header = settings_header(t);
    let sidebar = settings_sidebar(&state.settings_section, t);
    let content = settings_content(state, t);
    let body = row![sidebar, content].height(Length::Fill);
    column![header, body]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn settings_header(t: Theme) -> Element<'static, Message> {
    let back_btn = button(
        row![
            text("←").size(14).color(t.ink()),
            text("Back to sounds").size(13).color(t.ink()),
        ]
        .spacing(theme::space::SM)
        .align_y(Alignment::Center),
    )
    .on_press(Message::ShowMain)
    .padding([8.0, 14.0])
    .style(move |_t, _s| button::Style {
        background: Some(theme::bg_color(t.panel())),
        border: theme::tile_border(t.hairline2(), 1.0),
        ..Default::default()
    });

    let title = row![
        text("Settings")
            .size(22)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            })
            .color(t.ink()),
        text("· tweak the honk").size(12).color(t.ink_dim()),
    ]
    .spacing(theme::space::MD)
    .align_y(Alignment::Center);

    container(
        row![back_btn, title]
            .spacing(theme::space::LG)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([theme::space::MD, theme::space::XL])
    .style(move |_t| container::Style {
        border: iced::Border {
            color: t.hairline(),
            width: 1.0,
            radius: iced::border::Radius::default(),
        },
        ..Default::default()
    })
    .into()
}

fn settings_sidebar<'a>(active: &'a SettingsSection, t: Theme) -> Element<'a, Message> {
    let items: &[(&str, SettingsSection)] = &[
        ("Audio", SettingsSection::Audio),
        ("Library", SettingsSection::Library),
        ("Hotkeys", SettingsSection::Hotkeys),
        ("Appearance", SettingsSection::Appearance),
        ("About", SettingsSection::About),
    ];

    let nav = items.iter().fold(
        column![].spacing(theme::space::XS),
        |col, (label, section)| {
            let is_active = active == section;
            let item = button(
                text(*label)
                    .size(13)
                    .color(if is_active { t.bg() } else { t.ink() })
                    .font(iced::Font {
                        weight: iced::font::Weight::Bold,
                        ..Default::default()
                    }),
            )
            .on_press(Message::ShowSettingsSection(section.clone()))
            .width(Length::Fill)
            .padding([theme::space::SM, theme::space::MD])
            .style(move |_t, _s| button::Style {
                background: Some(theme::bg_color(if is_active {
                    t.ink()
                } else {
                    iced::Color::TRANSPARENT
                })),
                border: theme::tile_border(iced::Color::TRANSPARENT, 0.0),
                ..Default::default()
            });
            col.push(item)
        },
    );

    container(column![nav].width(Length::Fixed(220.0)))
        .height(Length::Fill)
        .padding(theme::space::MD)
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(t.panel())),
            border: iced::Border {
                color: t.hairline(),
                width: 1.0,
                radius: iced::border::Radius::default(),
            },
            ..Default::default()
        })
        .into()
}

fn settings_content<'a>(state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    let body: Element<'_, Message> = match &state.settings_section {
        SettingsSection::Audio => view_audio_section(state, t),
        SettingsSection::Library => view_library_section(state, t),
        SettingsSection::Hotkeys => view_hotkeys_section(state, t),
        SettingsSection::Appearance => view_appearance_section(t),
        SettingsSection::About => view_about_section(t),
    };

    scrollable(
        container(body)
            .width(Length::Fill)
            .padding([theme::space::XL, theme::space::XXL]),
    )
    .height(Length::Fill)
    .into()
}

/// Generic registry row renderer.
/// Left: label+hint (260px). Right: control widget. Bottom: hairline border.
pub fn render_setting_row<'a>(def: &'a SettingDef, state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    let value = get_setting_value(def.id, state);

    let label_col = column![
        text(def.label)
            .size(13)
            .color(t.ink())
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            }),
        text(def.hint).size(11).color(t.ink_dim()),
    ]
    .spacing(theme::space::XS)
    .width(Length::Fixed(260.0));

    let control: Element<'_, Message> = match (&def.control, value) {
        (ControlType::Button, _) => {
            let msg = setting_message(def.id, SettingValue::None);
            button(text(def.label).size(13).color(t.ink()))
                .on_press(msg)
                .padding([8.0, 18.0])
                .style(move |_t, _s| button::Style {
                    background: Some(theme::bg_color(t.panel())),
                    border: theme::tile_border(t.hairline2(), 1.0),
                    ..Default::default()
                })
                .into()
        }
        _ => text("—").size(13).color(t.ink_faint()).into(),
    };

    container(
        row![label_col, control]
            .spacing(theme::space::XL)
            .align_y(Alignment::Start)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding([18.0, 0.0])
    .into()
}

/// Read the current value of a setting from app state.
/// Add arms here when backend sub-MVPs land.
pub fn get_setting_value(id: SettingId, _state: &HonkHonk) -> SettingValue {
    match id {
        SettingId::RescanLibrary => SettingValue::None,
        _ => SettingValue::None,
    }
}

/// Map a setting id + value to the specific Message that applies it.
/// Add arms here when backend sub-MVPs land.
pub fn setting_message(id: SettingId, _value: SettingValue) -> Message {
    match id {
        SettingId::RescanLibrary => Message::RescanLibrary,
        // All other IDs are unwired stubs — no SettingDef renders them yet.
        // If this arm fires, a SettingDef was added without updating this function.
        _ => {
            debug_assert!(false, "setting_message: unhandled SettingId {:?}", id);
            Message::RescanLibrary
        }
    }
}

/// Shared section chrome: bold italic title + subtitle + 2px ink underline + body.
fn section_layout<'a>(
    title: &'static str,
    subtitle: &'static str,
    body: Element<'a, Message>,
    t: Theme,
) -> Element<'a, Message> {
    column![
        column![
            text(title)
                .size(26)
                .color(t.ink())
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    style: iced::font::Style::Italic,
                    ..Default::default()
                }),
            text(subtitle).size(13).color(t.ink_dim()),
        ]
        .spacing(theme::space::XS)
        .width(Length::Fill),
        container(Space::new())
            .width(Length::Fill)
            .height(2)
            .style(move |_t| container::Style {
                background: Some(theme::bg_color(t.ink())),
                ..Default::default()
            }),
        body,
    ]
    .spacing(theme::space::LG)
    .width(Length::Fill)
    .into()
}

// --- Section stubs (replaced in Tasks 5 and 6) ---

pub fn view_audio_section<'a>(_state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    section_layout("Audio", "Where HonkHonk listens and speaks.", column![].into(), t)
}

pub fn view_library_section<'a>(_state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    section_layout("Library", "Where HonkHonk looks for your sounds.", column![].into(), t)
}

pub fn view_hotkeys_section<'a>(_state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    section_layout(
        "Hotkeys",
        "Global shortcuts that work even when HonkHonk isn't focused.",
        column![].into(),
        t,
    )
}

pub fn view_appearance_section(t: Theme) -> Element<'static, Message> {
    section_layout("Appearance", "How honky should HonkHonk look today?", column![].into(), t)
}

pub fn view_about_section(t: Theme) -> Element<'static, Message> {
    section_layout("About", "The bird is the word.", column![].into(), t)
}
