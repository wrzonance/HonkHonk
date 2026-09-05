use iced::{
    Alignment, Element, Length,
    widget::{column, container, pick_list, row, text},
};

use super::common::{label_hint_column, section_layout};
use super::controls::render_setting_row;
use crate::app::{HonkHonk, Message};
use crate::settings::{SETTINGS_REGISTRY, SettingCategory};
use crate::ui::theme::{self, Hh, Theme};

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

pub(super) fn view_audio_section<'a>(state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    let status_badge = audio_status(t);
    let registry_rows = SETTINGS_REGISTRY
        .iter()
        .filter(|setting| setting.category == SettingCategory::Audio)
        .fold(column![].spacing(0.0), |column, setting| {
            column.push(render_setting_row(setting, state, t, false))
        });

    section_layout(
        "Audio",
        "Where HonkHonk listens and speaks.",
        column![
            status_badge,
            registry_rows,
            crate::ui::audio_processing::global(state.config.processing),
            input_device_row(state, t),
            monitor_device_row(state, t)
        ]
        .spacing(theme::space::LG)
        .into(),
        t,
    )
}

fn audio_status(t: Theme) -> Element<'static, Message> {
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

    container(
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
    })
    .into()
}

fn input_device_row<'a>(state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    let options: Vec<InputDeviceOption> = std::iter::once(InputDeviceOption::Auto)
        .chain(state.input_devices.iter().map(|(node_name, display_name)| {
            InputDeviceOption::Device {
                node_name: node_name.clone(),
                display_name: display_name.clone(),
            }
        }))
        .collect();
    let selected = Some(match &state.config.input_device {
        None => InputDeviceOption::Auto,
        Some(name) => state
            .input_devices
            .iter()
            .find(|(node, _)| node == name)
            .map(|(node_name, display_name)| InputDeviceOption::Device {
                node_name: node_name.clone(),
                display_name: display_name.clone(),
            })
            .unwrap_or(InputDeviceOption::Auto),
    });

    device_row(
        "Microphone input",
        "Which real mic to mix into the virtual mic.",
        pick_list(options, selected, |option| match option {
            InputDeviceOption::Auto => Message::InputDeviceChanged(None),
            InputDeviceOption::Device { node_name, .. } => {
                Message::InputDeviceChanged(Some(node_name))
            }
        })
        .width(Length::Fixed(280.0))
        .into(),
        t,
    )
}

fn monitor_device_row<'a>(state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    let options: Vec<MonitorDeviceOption> = std::iter::once(MonitorDeviceOption::Default)
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
    let selected = Some(match &state.config.monitor_device {
        None => MonitorDeviceOption::Default,
        Some(name) => state
            .monitor_devices
            .iter()
            .find(|(node, _)| node == name)
            .map(|(node_name, display_name)| MonitorDeviceOption::Device {
                node_name: node_name.clone(),
                display_name: display_name.clone(),
            })
            .unwrap_or(MonitorDeviceOption::Default),
    });

    device_row(
        "Monitor output",
        "Where HonkHonk plays sounds for you to hear.",
        pick_list(options, selected, |option| match option {
            MonitorDeviceOption::Default => Message::MonitorDeviceChanged(None),
            MonitorDeviceOption::Device { node_name, .. } => {
                Message::MonitorDeviceChanged(Some(node_name))
            }
        })
        .width(Length::Fixed(280.0))
        .into(),
        t,
    )
}

fn device_row<'a>(
    label: &'static str,
    hint: &'static str,
    control: Element<'a, Message>,
    t: Theme,
) -> Element<'a, Message> {
    container(
        row![label_hint_column(label, hint, t), control,]
            .spacing(theme::space::XL)
            .align_y(Alignment::Center)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding([18.0, 0.0])
    .into()
}
