//! Audio settings mixer; route controls never override engine rejections.
use crate::app::{HonkHonk, Message};
use iced::{
    Element, Length,
    widget::{button, checkbox, column, container, row, text},
};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub enum MixerMessage {
    SafeMode(bool),
    ShowMonitors(bool),
    Route(u32),
    Confirm,
    Cancel,
    Undo,
    Tick(Instant),
}

fn message(value: MixerMessage) -> Message {
    Message::Mixer(value)
}

pub(crate) fn view(app: &HonkHonk) -> Element<'_, Message> {
    let mut content = column![
        text("Mixer → HonkHonk Mic").size(20),
        text("Sound effects + microphone + selected application audio"),
        microphone(app),
        checkbox(app.config.mixer_safe_mode)
            .label("Safe mode: microphone + sound effects only")
            .on_toggle(|v| message(MixerMessage::SafeMode(v))),
        checkbox(app.config.mixer_show_monitors)
            .label("Show monitor sources (diagnostics only)")
            .on_toggle(|v| message(MixerMessage::ShowMonitors(v))),
        button("Undo routing change (last 3)").on_press(message(MixerMessage::Undo)),
    ]
    .spacing(10);
    if let Some(name) = &app.mixer.feedback_source {
        content = content.push(
            text(format!("⚠ Feedback detected — {name}. Check routing."))
                .color(iced::Color::from_rgb(0.9, 0.15, 0.15)),
        );
    }
    for (&id, source) in &app.mixer.sources {
        if !source.monitor || app.config.mixer_show_monitors {
            content = content.push(source_row(app, id, source));
        }
    }
    if app.mixer.sources.is_empty() {
        content = content.push(text(
            "No application audio sources. Start playback in an app.",
        ));
    }
    if let Some(id) = app.mixer.confirm
        && let Some(source) = app.mixer.sources.get(&id)
    {
        content = content.push(column![
            text(format!("Route {} into your virtual microphone?", source.name)),
            text("This may create feedback if the app listens to HonkHonk Mic. Check its input first."),
            row![button("Confirm route").on_press(message(MixerMessage::Confirm)),
                button("Cancel").on_press(message(MixerMessage::Cancel))].spacing(10),
        ].spacing(8));
    }
    container(content).padding(12).width(Length::Fill).into()
}

fn microphone(app: &HonkHonk) -> Element<'_, Message> {
    let label = app.mixer.mic_cooldown.map_or_else(
        || "Microphone passthrough".into(),
        |until| {
            let now = app.mixer.now.unwrap_or_else(Instant::now);
            format!(
                "⚠ Microphone disconnected — retry in {:.1}s",
                until.saturating_duration_since(now).as_secs_f32()
            )
        },
    );
    checkbox(app.config.mic_passthrough)
        .label(label)
        .on_toggle_maybe(
            app.mixer
                .mic_cooldown
                .is_none()
                .then_some(Message::MicPassthroughChanged),
        )
        .into()
}

fn source_row<'a>(
    app: &'a HonkHonk,
    id: u32,
    source: &'a crate::app::mixer::MixerSource,
) -> Element<'a, Message> {
    let now = app.mixer.now.unwrap_or_else(Instant::now);
    let blocked = source.blocked(now);
    let pulse = source.cooldown.map_or(1.0, |until| {
        1.0 + (until.saturating_duration_since(now).as_secs_f32() * 12.0)
            .sin()
            .abs()
            * 2.0
    });
    let status = source_status(source, now);
    let control = button(if source.enabled {
        "Disconnect"
    } else {
        "Route"
    })
    .on_press_maybe(
        (source.enabled || (!blocked && !app.config.mixer_safe_mode))
            .then_some(message(MixerMessage::Route(id))),
    );
    container(
        row![
            column![
                text(&source.name),
                text(source.media_name.as_deref().unwrap_or("(idle)")),
                text(status)
            ]
            .spacing(4)
            .width(Length::Fill),
            control
        ]
        .spacing(12),
    )
    .padding(10)
    .width(Length::Fill)
    .style(move |_| container::Style {
        border: iced::Border {
            color: if source.warning.is_some() {
                iced::Color::from_rgb(0.9, 0.15, 0.15)
            } else {
                iced::Color::TRANSPARENT
            },
            width: pulse,
            radius: 6.0.into(),
        },
        text_color: blocked.then_some(iced::Color::from_rgb(0.5, 0.5, 0.5)),
        ..Default::default()
    })
    .into()
}

fn source_status(source: &crate::app::mixer::MixerSource, now: Instant) -> String {
    if let Some(until) = source.cooldown {
        format!(
            "⚠ Feedback risk — retry in {:.1}s",
            until.saturating_duration_since(now).as_secs_f32()
        )
    } else if source.monitor {
        "Monitor source — routing blocked".into()
    } else if source.warning == Some(crate::audio::RouteRejection::Cooldown) {
        "⚠ Feedback detected — check routing before retrying".into()
    } else if let Some(reason) = source.warning {
        format!("⚠ Feedback risk: {reason}")
    } else if source.enabled {
        "Routed".into()
    } else {
        "Not routed".into()
    }
}
