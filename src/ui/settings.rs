#[allow(unused_imports)]
use iced::{
    widget::{button, column, container, row, scrollable, text, Column, Row, Space},
    Alignment, Element, Length,
};

use crate::app::{HonkHonk, Message, SettingsSection};
use crate::settings::{
    ControlType, SettingCategory, SettingDef, SettingId, SettingValue, SETTINGS_REGISTRY,
};
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
            text("←").size(theme::font::BODY).color(t.ink()),
            text("Back to sounds").size(theme::font::BODY).color(t.ink()),
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
            .size(theme::font::TITLE)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            })
            .color(t.ink()),
        text("· ruffle feathers").size(theme::font::LABEL).color(t.ink_dim()),
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
                    .size(theme::font::BODY)
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
pub fn render_setting_row<'a>(
    def: &'a SettingDef,
    state: &'a HonkHonk,
    t: Theme,
) -> Element<'a, Message> {
    let value = get_setting_value(def.id, state);

    let label_col = column![
        text(def.label).size(theme::font::BODY).color(t.ink()).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }),
        text(def.hint).size(theme::font::LABEL).color(t.ink_dim()),
    ]
    .spacing(theme::space::XS)
    .width(Length::Fixed(260.0));

    let control: Element<'_, Message> = match (&def.control, value) {
        (ControlType::Button, _) => {
            let msg = setting_message(def.id, SettingValue::None);
            button(text(def.label).size(theme::font::BODY).color(t.ink()))
                .on_press(msg)
                .padding([8.0, 18.0])
                .style(move |_t, _s| button::Style {
                    background: Some(theme::bg_color(t.panel())),
                    border: theme::tile_border(t.hairline2(), 1.0),
                    ..Default::default()
                })
                .into()
        }
        _ => text("—").size(theme::font::BODY).color(t.ink_faint()).into(),
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
        other => {
            // Safety net: if this fires, a SettingDef was added without updating this function.
            debug_assert!(
                false,
                "setting_message: unhandled SettingId {:?} — add an arm here when wiring a backend",
                other
            );
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
            text(title).size(theme::font::TITLE).color(t.ink()).font(iced::Font {
                weight: iced::font::Weight::Bold,
                style: iced::font::Style::Italic,
                ..Default::default()
            }),
            text(subtitle).size(theme::font::BODY).color(t.ink_dim()),
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

pub fn view_audio_section<'a>(state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    // Status badge — always active if user reached settings
    let dot = container(iced::widget::Space::new())
        .width(theme::space::SM)
        .height(theme::space::SM)
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(t.good())),
            border: iced::Border {
                radius: iced::border::Radius::from(4.0),
                ..Default::default()
            },
            ..Default::default()
        });

    let status_badge = container(
        column![
            row![
                dot,
                text("Audio engine active")
                    .size(theme::font::LABEL)
                    .color(t.ink())
                    .font(iced::Font {
                        weight: iced::font::Weight::Bold,
                        ..Default::default()
                    }),
            ]
            .spacing(theme::space::SM)
            .align_y(Alignment::Center),
            text("honkhonk-mix · honkhonk-mic")
                .size(theme::font::LABEL)
                .color(t.ink_dim())
                .font(iced::Font {
                    family: iced::font::Family::Monospace,
                    ..Default::default()
                }),
        ]
        .spacing(theme::space::XS),
    )
    .padding(theme::space::MD)
    .style(move |_t| container::Style {
        background: Some(theme::bg_color(t.panel())),
        border: iced::Border {
            color: t.hairline(),
            width: 1.0,
            radius: theme::radius::MD,
        },
        ..Default::default()
    });

    // Registry rows for Audio — empty in Phase 2, populated by issues #71/#72
    let registry_rows = SETTINGS_REGISTRY
        .iter()
        .filter(|d| matches!(d.category, SettingCategory::Audio))
        .fold(column![].spacing(0.0), |col, def| {
            col.push(render_setting_row(def, state, t))
        });

    section_layout(
        "Audio",
        "Where HonkHonk listens and speaks.",
        column![status_badge, registry_rows]
            .spacing(theme::space::LG)
            .into(),
        t,
    )
}

pub fn view_library_section<'a>(state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    // --- Folder list ---
    let folder_rows: Vec<Element<'_, Message>> = state
        .config
        .sound_directories
        .iter()
        .map(|path| {
            let path_clone = path.clone();
            let remove_btn = button(text("×").size(theme::font::BODY).color(t.ink_faint()))
                .on_press(Message::RemoveSoundDirectory(path_clone))
                .padding(4.0)
                .style(move |_t, _s| button::Style {
                    background: None,
                    border: iced::Border::default(),
                    ..Default::default()
                });

            container(
                row![
                    text(path.display().to_string())
                        .size(theme::font::LABEL)
                        .color(t.ink())
                        .font(iced::Font {
                            family: iced::font::Family::Monospace,
                            ..Default::default()
                        })
                        .width(Length::Fill),
                    remove_btn,
                ]
                .spacing(theme::space::SM)
                .align_y(Alignment::Center),
            )
            .padding([10.0, 12.0])
            .width(Length::Fill)
            .style(move |_t| container::Style {
                background: Some(theme::bg_color(t.panel())),
                border: iced::Border {
                    color: t.hairline(),
                    width: 1.0,
                    radius: theme::radius::MD,
                },
                ..Default::default()
            })
            .into()
        })
        .collect();

    let add_btn = button(text("+ Add a folder").size(theme::font::BODY).color(t.ink_dim()))
        .on_press(Message::AddSoundDirectory)
        .width(Length::Fill)
        .padding([9.0, 12.0])
        .style(move |_t, _s| button::Style {
            background: None,
            // Iced Border has no dash/style field — solid hairline approximates the dashed design intent
            border: iced::Border {
                color: t.hairline2(),
                width: 1.5,
                radius: theme::radius::MD,
            },
            ..Default::default()
        });

    let folders_widget = column![
        Column::with_children(folder_rows).spacing(theme::space::XS),
        add_btn,
    ]
    .spacing(theme::space::XS)
    .width(Length::Fixed(540.0));

    let folders_row = row![
        column![
            text("Sound folders")
                .size(theme::font::BODY)
                .color(t.ink())
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                }),
            text("HonkHonk watches these folders. Drop in MP3 / WAV / OGG / FLAC.")
                .size(theme::font::LABEL)
                .color(t.ink_dim()),
        ]
        .spacing(theme::space::XS)
        .width(Length::Fixed(260.0)),
        folders_widget,
    ]
    .spacing(theme::space::XL)
    .align_y(Alignment::Start)
    .width(Length::Fill);

    // --- Registry rows (RescanLibrary button) ---
    let registry_rows = SETTINGS_REGISTRY
        .iter()
        .filter(|d| matches!(d.category, SettingCategory::Library))
        .fold(column![].spacing(0.0), |col, def| {
            col.push(render_setting_row(def, state, t))
        });

    // --- Supported formats (static display) ---
    const FORMATS: &[&str] = &["MP3", "WAV", "OGG Vorbis", "FLAC", "AAC", "Opus"];

    let format_pills: Vec<Element<'_, Message>> = FORMATS
        .iter()
        .map(|fmt| {
            container(text(*fmt).size(theme::font::LABEL).color(t.ink_dim()).font(iced::Font {
                family: iced::font::Family::Monospace,
                weight: iced::font::Weight::Bold,
                ..Default::default()
            }))
            .padding([5.0, 11.0])
            .style(move |_t| container::Style {
                background: Some(theme::bg_color(t.panel())),
                border: iced::Border {
                    color: t.hairline2(),
                    width: 1.0,
                    radius: theme::radius::PILL,
                },
                ..Default::default()
            })
            .into()
        })
        .collect();

    let formats_row = row![
        column![
            text("Supported formats")
                .size(theme::font::BODY)
                .color(t.ink())
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                }),
            text("Decoded via Symphonia — pure Rust.")
                .size(theme::font::LABEL)
                .color(t.ink_dim()),
        ]
        .spacing(theme::space::XS)
        .width(Length::Fixed(260.0)),
        Row::with_children(format_pills).spacing(theme::space::XS),
    ]
    .spacing(theme::space::XL)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    section_layout(
        "Library",
        "Where HonkHonk looks for your sounds.",
        column![folders_row, registry_rows, formats_row]
            .spacing(theme::space::LG)
            .into(),
        t,
    )
}

pub fn view_hotkeys_section<'a>(state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    use crate::shortcuts::ShortcutsStatus;

    let (dot_color, status_text) = match &state.shortcuts_status {
        ShortcutsStatus::Active => (t.good(), "Global shortcuts active"),
        ShortcutsStatus::Initializing => (t.ink_dim(), "Connecting to portal…"),
        ShortcutsStatus::Unavailable(_) => (t.accent(), "Portal unavailable"),
    };

    let dot = container(iced::widget::Space::new())
        .width(theme::space::SM)
        .height(theme::space::SM)
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(dot_color)),
            border: iced::Border {
                radius: iced::border::Radius::from(4.0),
                ..Default::default()
            },
            ..Default::default()
        });

    let portal_badge = container(
        row![
            dot,
            text(status_text).size(theme::font::LABEL).color(t.ink()).font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            }),
        ]
        .spacing(theme::space::SM)
        .align_y(Alignment::Center),
    )
    .padding(theme::space::MD)
    .style(move |_t| container::Style {
        background: Some(theme::bg_color(t.panel())),
        border: iced::Border {
            color: t.hairline(),
            width: 1.0,
            radius: theme::radius::MD,
        },
        ..Default::default()
    });

    // Slot bindings table — read-only, from portal-assigned triggers
    let bound: Vec<(u8, &str)> = state
        .slot_triggers
        .iter()
        .enumerate()
        .filter_map(|(i, opt)| opt.as_deref().map(|s| (i as u8, s)))
        .collect();

    let binding_rows: Vec<Element<'_, Message>> = if bound.is_empty() {
        vec![
            text("No hotkeys assigned yet. Use the Slot Manager to bind sounds.")
                .size(theme::font::LABEL)
                .color(t.ink_dim())
                .into(),
        ]
    } else {
        bound
            .into_iter()
            .map(|(slot, trigger)| {
                container(
                    row![
                        text(format!("Slot {}", slot + 1))
                            .size(theme::font::LABEL)
                            .color(t.ink_dim())
                            .width(Length::Fixed(60.0)),
                        text(trigger).size(theme::font::LABEL).color(t.ink()).font(iced::Font {
                            family: iced::font::Family::Monospace,
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    ]
                    .spacing(theme::space::MD)
                    .align_y(Alignment::Center),
                )
                .padding([6.0, 12.0])
                .style(move |_t| container::Style {
                    background: Some(theme::bg_color(t.panel())),
                    border: iced::Border {
                        color: t.hairline(),
                        width: 1.0,
                        radius: theme::radius::MD,
                    },
                    ..Default::default()
                })
                .into()
            })
            .collect()
    };

    let bindings = Column::with_children(binding_rows).spacing(theme::space::XS);

    section_layout(
        "Hotkeys",
        "Global shortcuts that work even when HonkHonk isn't focused.",
        column![portal_badge, bindings]
            .spacing(theme::space::LG)
            .into(),
        t,
    )
}

pub fn view_appearance_section(t: Theme) -> Element<'static, Message> {
    section_layout(
        "Appearance",
        "How honky should HonkHonk look today?",
        column![].into(),
        t,
    )
}

pub fn view_about_section(t: Theme) -> Element<'static, Message> {
    const VERSION: &str = env!("CARGO_PKG_VERSION");

    let logo_block = column![
        text("HonkHonk").size(theme::font::HERO).color(t.ink()).font(iced::Font {
            weight: iced::font::Weight::Bold,
            style: iced::font::Style::Italic,
            ..Default::default()
        }),
        text(format!("v{VERSION} · Iced 0.14"))
            .size(theme::font::BODY)
            .color(t.ink_dim()),
        text("A Wayland-native soundboard for Linux. Built with Rust, Iced, and PipeWire.")
            .size(theme::font::LABEL)
            .color(t.ink_faint()),
    ]
    .spacing(theme::space::XS);

    let license = container(
        text("GPL-3.0-or-later")
            .size(theme::font::LABEL)
            .color(t.ink())
            .font(iced::Font {
                family: iced::font::Family::Monospace,
                ..Default::default()
            }),
    )
    .padding([4.0, 10.0])
    .style(move |_t| container::Style {
        background: Some(theme::bg_color(t.panel())),
        border: iced::Border {
            color: t.hairline(),
            width: 1.0,
            radius: theme::radius::MD,
        },
        ..Default::default()
    });

    let credits = column![
        text("Credits").size(theme::font::BODY).color(t.ink()).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }),
        text("Iced — iced-rs").size(theme::font::LABEL).color(t.ink_dim()),
        text("Symphonia — pdeljanov").size(theme::font::LABEL).color(t.ink_dim()),
        text("ashpd — bilelmoussaoui").size(theme::font::LABEL).color(t.ink_dim()),
        text("pipewire-rs — PipeWire project")
            .size(theme::font::LABEL)
            .color(t.ink_dim()),
        text("tray-icon — tauri-apps").size(theme::font::LABEL).color(t.ink_dim()),
    ]
    .spacing(theme::space::XS);

    section_layout(
        "About",
        "The bird is the word.",
        column![logo_block, license, credits]
            .spacing(theme::space::XL)
            .into(),
        t,
    )
}
