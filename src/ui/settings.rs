#[allow(unused_imports)]
use iced::{
    widget::{button, column, container, pick_list, row, scrollable, text, Column, Row, Space},
    Alignment, Element, Length,
};

use crate::app::{HonkHonk, Message, SettingsSection};

/// SPDX license shown on the About screen, sourced from `Cargo.toml`'s
/// `license = "MIT"` field at compile time so the displayed value can never
/// drift from the project's real license. See issue #128.
const LICENSE: &str = env!("CARGO_PKG_LICENSE");

#[derive(Debug, Clone, PartialEq, Eq)]
enum MonitorDeviceOption {
    Default,
    Device {
        node_name: String,
        display_name: String,
    },
}

impl std::fmt::Display for MonitorDeviceOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Default => write!(f, "System default"),
            Self::Device { display_name, .. } => write!(f, "{display_name}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InputDeviceOption {
    Auto,
    Device {
        node_name: String,
        display_name: String,
    },
}

impl std::fmt::Display for InputDeviceOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "Auto (system default)"),
            Self::Device { display_name, .. } => write!(f, "{display_name}"),
        }
    }
}
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
            text("Back to sounds")
                .size(theme::font::BODY)
                .color(t.ink()),
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
        text("· ruffle feathers")
            .size(theme::font::LABEL)
            .color(t.ink_dim()),
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
        SettingsSection::Appearance => view_appearance_section(state, t),
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
        text(def.label)
            .size(theme::font::BODY)
            .color(t.ink())
            .font(iced::Font {
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
        (ControlType::Radio(options), SettingValue::Index(current)) => {
            let id = def.id;
            options
                .iter()
                .enumerate()
                .fold(row![].spacing(theme::space::XS), |r, (i, label)| {
                    let msg = setting_message(id, SettingValue::Index(i));
                    let active = i == current;
                    r.push(
                        button(text(*label).size(theme::font::BODY).color(if active {
                            t.bg()
                        } else {
                            t.ink()
                        }))
                        .on_press(msg)
                        .padding([6.0, 14.0])
                        .style(move |_t, _s| button::Style {
                            background: Some(theme::bg_color(if active {
                                t.ink()
                            } else {
                                t.panel()
                            })),
                            border: theme::tile_border(t.hairline2(), 1.0),
                            ..Default::default()
                        }),
                    )
                })
                .into()
        }
        (ControlType::Toggle, SettingValue::Bool(v)) => render_toggle(def.id, v, t),
        (ControlType::Slider { min, max, step }, SettingValue::F32(v)) => {
            render_slider(def.id, v, (*min, *max, *step), t)
        }
        _ => text("—")
            .size(theme::font::BODY)
            .color(t.ink_faint())
            .into(),
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

pub fn get_setting_value(id: SettingId, state: &HonkHonk) -> SettingValue {
    match id {
        SettingId::RescanLibrary => SettingValue::None,
        SettingId::Theme => SettingValue::Index(state.config.theme.setting_index()),
        SettingId::Density => SettingValue::Index(state.config.density.setting_index()),
        SettingId::MicPassthrough => SettingValue::Bool(state.config.mic_passthrough),
        SettingId::MicPassthroughLevel => SettingValue::F32(state.config.mic_passthrough_level),
        SettingId::Renderer => {
            SettingValue::Bool(state.config.renderer == crate::state::Renderer::Wgpu)
        }
        _ => SettingValue::None,
    }
}

pub fn setting_message(id: SettingId, value: SettingValue) -> Message {
    match (id, value) {
        (SettingId::RescanLibrary, _) => Message::RescanLibrary,
        (SettingId::Theme, SettingValue::Index(i)) => {
            Message::ThemeChanged(crate::ui::theme::Theme::from_setting_index(i))
        }
        (SettingId::Density, SettingValue::Index(i)) => {
            Message::DensityChanged(crate::state::config::Density::from_setting_index(i))
        }
        (SettingId::MicPassthrough, SettingValue::Bool(v)) => Message::MicPassthroughChanged(v),
        (SettingId::MicPassthroughLevel, SettingValue::F32(v)) => {
            Message::MicPassthroughLevelChanged(v)
        }
        (SettingId::Renderer, SettingValue::Bool(v)) => Message::RendererChanged(if v {
            crate::state::Renderer::Wgpu
        } else {
            crate::state::Renderer::TinySkia
        }),
        other => {
            debug_assert!(
                false,
                "setting_message: unhandled ({:?}) — add an arm here when wiring a backend",
                other
            );
            Message::RescanLibrary
        }
    }
}

fn render_toggle(id: SettingId, v: bool, t: Theme) -> Element<'static, Message> {
    row![
        button(
            text("On")
                .size(theme::font::BODY)
                .color(if v { t.bg() } else { t.ink() }),
        )
        .on_press(setting_message(id, SettingValue::Bool(true)))
        .padding([6.0, 14.0])
        .style(move |_t, _s| button::Style {
            background: Some(theme::bg_color(if v { t.ink() } else { t.panel() })),
            border: theme::tile_border(t.hairline2(), 1.0),
            ..Default::default()
        }),
        button(
            text("Off")
                .size(theme::font::BODY)
                .color(if !v { t.bg() } else { t.ink() }),
        )
        .on_press(setting_message(id, SettingValue::Bool(false)))
        .padding([6.0, 14.0])
        .style(move |_t, _s| button::Style {
            background: Some(theme::bg_color(if !v { t.ink() } else { t.panel() })),
            border: theme::tile_border(t.hairline2(), 1.0),
            ..Default::default()
        }),
    ]
    .spacing(theme::space::XS)
    .into()
}

fn render_slider(
    id: SettingId,
    v: f32,
    range: (f32, f32, f32),
    t: Theme,
) -> Element<'static, Message> {
    let (min, max, step) = range;
    row![
        iced::widget::slider(min..=max, v, move |x| {
            setting_message(id, SettingValue::F32(x))
        })
        .step(step)
        .width(Length::Fixed(200.0)),
        text(format!("{:.0}%", v * 100.0))
            .size(theme::font::LABEL)
            .color(t.ink_dim())
            .font(iced::Font {
                family: iced::font::Family::Monospace,
                ..Default::default()
            }),
    ]
    .spacing(theme::space::SM)
    .align_y(Alignment::Center)
    .into()
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
                .size(theme::font::TITLE)
                .color(t.ink())
                .font(iced::Font {
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

    let device_options: Vec<MonitorDeviceOption> = std::iter::once(MonitorDeviceOption::Default)
        .chain(
            state
                .monitor_devices
                .iter()
                .map(|(node_name, display_name)| MonitorDeviceOption::Device {
                    node_name: node_name.clone(),
                    display_name: display_name.clone(),
                }),
        )
        .collect();

    let selected_device = Some(match &state.config.monitor_device {
        None => MonitorDeviceOption::Default,
        Some(name) => state
            .monitor_devices
            .iter()
            .find(|(n, _)| n == name)
            .map(|(n, d)| MonitorDeviceOption::Device {
                node_name: n.clone(),
                display_name: d.clone(),
            })
            .unwrap_or(MonitorDeviceOption::Default),
    });

    let input_options: Vec<InputDeviceOption> = std::iter::once(InputDeviceOption::Auto)
        .chain(state.input_devices.iter().map(|(node_name, display_name)| {
            InputDeviceOption::Device {
                node_name: node_name.clone(),
                display_name: display_name.clone(),
            }
        }))
        .collect();

    let selected_input = Some(match &state.config.input_device {
        None => InputDeviceOption::Auto,
        Some(name) => state
            .input_devices
            .iter()
            .find(|(n, _)| n == name)
            .map(|(n, d)| InputDeviceOption::Device {
                node_name: n.clone(),
                display_name: d.clone(),
            })
            .unwrap_or(InputDeviceOption::Auto),
    });

    let input_row = container(
        row![
            column![
                text("Microphone input")
                    .size(theme::font::BODY)
                    .color(t.ink())
                    .font(iced::Font {
                        weight: iced::font::Weight::Bold,
                        ..Default::default()
                    }),
                text("Which real mic to mix into the virtual mic.")
                    .size(theme::font::LABEL)
                    .color(t.ink_dim()),
            ]
            .spacing(theme::space::XS)
            .width(Length::Fixed(260.0)),
            pick_list(input_options, selected_input, |opt: InputDeviceOption| {
                match opt {
                    InputDeviceOption::Auto => Message::InputDeviceChanged(None),
                    InputDeviceOption::Device { node_name, .. } => {
                        Message::InputDeviceChanged(Some(node_name))
                    }
                }
            })
            .width(Length::Fixed(280.0)),
        ]
        .spacing(theme::space::XL)
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding([18.0, 0.0]);

    let device_row = container(
        row![
            column![
                text("Monitor output")
                    .size(theme::font::BODY)
                    .color(t.ink())
                    .font(iced::Font {
                        weight: iced::font::Weight::Bold,
                        ..Default::default()
                    }),
                text("Where HonkHonk plays sounds for you to hear.")
                    .size(theme::font::LABEL)
                    .color(t.ink_dim()),
            ]
            .spacing(theme::space::XS)
            .width(Length::Fixed(260.0)),
            pick_list(
                device_options,
                selected_device,
                |opt: MonitorDeviceOption| {
                    match opt {
                        MonitorDeviceOption::Default => Message::MonitorDeviceChanged(None),
                        MonitorDeviceOption::Device { node_name, .. } => {
                            Message::MonitorDeviceChanged(Some(node_name))
                        }
                    }
                }
            )
            .width(Length::Fixed(280.0)),
        ]
        .spacing(theme::space::XL)
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding([18.0, 0.0]);

    section_layout(
        "Audio",
        "Where HonkHonk listens and speaks.",
        column![status_badge, registry_rows, input_row, device_row]
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

    let add_btn = button(
        text("+ Add a folder")
            .size(theme::font::BODY)
            .color(t.ink_dim()),
    )
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
            container(
                text(*fmt)
                    .size(theme::font::LABEL)
                    .color(t.ink_dim())
                    .font(iced::Font {
                        family: iced::font::Family::Monospace,
                        weight: iced::font::Weight::Bold,
                        ..Default::default()
                    }),
            )
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
            text(status_text)
                .size(theme::font::LABEL)
                .color(t.ink())
                .font(iced::Font {
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
                        text(trigger)
                            .size(theme::font::LABEL)
                            .color(t.ink())
                            .font(iced::Font {
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

pub fn view_appearance_section<'a>(state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    let registry_rows = SETTINGS_REGISTRY
        .iter()
        .filter(|d| matches!(d.category, SettingCategory::Appearance))
        .fold(column![].spacing(0.0), |col, def| {
            col.push(render_setting_row(def, state, t))
        });

    section_layout(
        "Appearance",
        "How honky should HonkHonk look today?",
        registry_rows.into(),
        t,
    )
}

pub fn view_about_section(t: Theme) -> Element<'static, Message> {
    const VERSION: &str = env!("CARGO_PKG_VERSION");

    let logo_block = column![
        text("HonkHonk")
            .size(theme::font::HERO)
            .color(t.ink())
            .font(iced::Font {
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

    let license = container(text(LICENSE).size(theme::font::LABEL).color(t.ink()).font(
        iced::Font {
            family: iced::font::Family::Monospace,
            ..Default::default()
        },
    ))
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
        text("Credits")
            .size(theme::font::BODY)
            .color(t.ink())
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            }),
        text("Iced — iced-rs")
            .size(theme::font::LABEL)
            .color(t.ink_dim()),
        text("Symphonia — pdeljanov")
            .size(theme::font::LABEL)
            .color(t.ink_dim()),
        text("ashpd — bilelmoussaoui")
            .size(theme::font::LABEL)
            .color(t.ink_dim()),
        text("pipewire-rs — PipeWire project")
            .size(theme::font::LABEL)
            .color(t.ink_dim()),
        text("tray-icon — tauri-apps")
            .size(theme::font::LABEL)
            .color(t.ink_dim()),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The About screen must show the project's real license. It is sourced
    /// from Cargo.toml via `env!("CARGO_PKG_LICENSE")`, so this guards against
    /// drift between the binary's displayed license and `license = "MIT"`.
    #[test]
    fn about_license_matches_cargo_manifest() {
        assert_eq!(LICENSE, "MIT");
        assert_eq!(LICENSE, env!("CARGO_PKG_LICENSE"));
    }
}
