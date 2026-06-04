/// Per-sound editor overlay.
///
/// Renders as a modal sheet centred over the main window. Allows the user to:
/// - toggle favorite
/// - adjust per-sound volume (0.0 – 2.0)
/// - rename (display name override)
///
/// Out of scope for this PR: trim, hotkey reassignment, color chooser.
use iced::widget::{button, column, container, row, slider, space, text, text_input};
use iced::{Element, Length};

use crate::app::Message;
use crate::state::{SoundEntry, SoundMeta};
use crate::ui::theme::{self, Hh, Theme};

/// View context passed in from app.rs.
pub struct EditorCtx<'a> {
    pub sound: &'a SoundEntry,
    /// Snapshot of the current persisted meta (favorite flag etc.).
    pub meta: SoundMeta,
    /// Current draft display name (held in app state while the editor is open).
    pub draft_name: &'a str,
    /// Current draft volume (held in app state while the editor is open).
    pub draft_volume: f32,
}

/// Full-window overlay: dim + centred sheet.
pub fn view_editor_overlay<'a>(ctx: EditorCtx<'a>, t: Theme) -> Element<'a, Message> {
    let sheet = view_sheet(ctx, t);

    // Centred sheet via a full-window container with padding trick
    let centred = container(sheet)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(iced::Padding {
            top: 80.0,
            left: 0.0,
            right: 0.0,
            bottom: 0.0,
        });

    // Dismiss-on-click backdrop
    let backdrop = iced::widget::mouse_area(
        container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(iced::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.55,
                })),
                ..Default::default()
            }),
    )
    .on_press(Message::CloseSoundEditor);

    iced::widget::stack![backdrop, centred]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view_sheet<'a>(ctx: EditorCtx<'a>, t: Theme) -> Element<'a, Message> {
    let sound_id = ctx.sound.id.clone();
    let fav = ctx.meta.favorite;
    let draft_name = ctx.draft_name;
    let draft_volume = ctx.draft_volume;

    let header = view_header(ctx.sound, fav, t);
    let name_row = view_name_row(draft_name, t);
    let volume_row = view_volume_row(draft_volume, sound_id.clone(), t);
    let footer = view_footer(sound_id, t);

    let body = column![header, name_row, volume_row, footer]
        .spacing(0)
        .width(Length::Fixed(560.0));

    container(body)
        .style(move |_| container::Style {
            background: Some(theme::bg_color(t.bg())),
            border: theme::tile_border(t.hairline(), 1.0),
            ..Default::default()
        })
        .width(Length::Fixed(560.0))
        .padding(0)
        .center_x(Length::Fill)
        .into()
}

fn view_header<'a>(sound: &'a SoundEntry, fav: bool, t: Theme) -> Element<'a, Message> {
    let sound_id = sound.id.clone();

    let fav_label = if fav { "★ Favorited" } else { "☆ Favorite" };
    let fav_btn = button(text(fav_label).size(theme::font::LABEL).color(t.ink()))
        .on_press(Message::ToggleFavorite(sound_id))
        .padding([theme::space::XS, theme::space::MD])
        .style(move |_t, _s| button::Style {
            background: Some(theme::bg_color(if fav {
                iced::Color::from_rgb(0.98, 0.91, 0.5)
            } else {
                t.panel()
            })),
            text_color: if fav {
                iced::Color::from_rgb(0.48, 0.36, 0.12)
            } else {
                t.ink()
            },
            border: theme::tile_border(t.hairline(), 1.0),
            ..Default::default()
        });

    let close_btn = button(text("✕").size(14).color(t.ink_dim()))
        .on_press(Message::CloseSoundEditor)
        .style(move |_t, _s| button::Style {
            background: None,
            text_color: t.ink_dim(),
            ..Default::default()
        });

    let category = text(sound.category.clone())
        .size(theme::font::LABEL)
        .color(t.ink_dim());

    container(
        row![category, space::horizontal(), fav_btn, close_btn]
            .spacing(theme::space::SM)
            .align_y(iced::Alignment::Center),
    )
    .padding([theme::space::MD, theme::space::LG])
    .style(move |_| container::Style {
        background: Some(theme::bg_color(t.panel())),
        border: iced::Border {
            color: t.hairline(),
            width: 0.0,
            radius: iced::border::radius(0.0),
        },
        ..Default::default()
    })
    .width(Length::Fill)
    .into()
}

fn view_name_row<'a>(draft_name: &'a str, t: Theme) -> Element<'a, Message> {
    let label = text("Display name").size(theme::font::LABEL).color(t.ink());
    let hint = text("Overrides the filename. Leave blank to use filename.")
        .size(theme::font::LABEL)
        .color(t.ink_dim());

    let input = text_input("Leave blank to use filename…", draft_name)
        .on_input(Message::SoundEditorNameChanged)
        .padding([theme::space::SM, theme::space::MD])
        .size(theme::font::BODY);

    container(
        column![label, hint, input]
            .spacing(theme::space::XS)
            .padding([theme::space::MD, theme::space::LG]),
    )
    .style(move |_| container::Style {
        border: iced::Border {
            color: t.hairline(),
            width: 0.0,
            radius: iced::border::radius(0.0),
        },
        ..Default::default()
    })
    .width(Length::Fill)
    .into()
}

fn view_volume_row<'a>(draft_volume: f32, sound_id: String, t: Theme) -> Element<'a, Message> {
    let label = text("Per-sound volume")
        .size(theme::font::LABEL)
        .color(t.ink());
    let hint = text("Multiplied with master volume. 100% = no change. Up to 200%.")
        .size(theme::font::LABEL)
        .color(t.ink_dim());

    let pct = format!("{}%", (draft_volume * 100.0).round() as u32);
    let pct_text = text(pct)
        .size(theme::font::BODY)
        .color(t.ink())
        .width(Length::Fixed(48.0));

    let vol_slider = slider(0.0..=2.0, draft_volume, move |v| {
        Message::SoundEditorVolumeChanged(sound_id.clone(), v)
    })
    .step(0.01);

    container(
        column![
            label,
            hint,
            row![vol_slider, pct_text]
                .spacing(theme::space::MD)
                .align_y(iced::Alignment::Center),
        ]
        .spacing(theme::space::XS)
        .padding([theme::space::MD, theme::space::LG]),
    )
    .width(Length::Fill)
    .into()
}

fn view_footer<'a>(sound_id: String, t: Theme) -> Element<'a, Message> {
    let cancel_btn = button(text("Cancel").size(13).color(t.ink()))
        .on_press(Message::CloseSoundEditor)
        .padding([theme::space::SM, theme::space::LG])
        .style(move |_t, _s| button::Style {
            background: Some(theme::bg_color(t.panel())),
            text_color: t.ink(),
            border: theme::tile_border(t.hairline(), 1.0),
            ..Default::default()
        });

    let save_btn = button(
        text("Save")
            .size(13)
            .color(iced::Color::from_rgb(0.1, 0.07, 0.03)),
    )
    .on_press(Message::SaveSoundMeta(sound_id))
    .padding([theme::space::SM, theme::space::LG])
    .style(move |_t, _s| button::Style {
        background: Some(theme::bg_color(t.accent())),
        text_color: iced::Color::from_rgb(0.1, 0.07, 0.03),
        border: iced::Border {
            color: iced::Color::TRANSPARENT,
            width: 0.0,
            radius: theme::radius::TILE,
        },
        ..Default::default()
    });

    container(
        row![space::horizontal(), cancel_btn, save_btn]
            .spacing(theme::space::SM)
            .align_y(iced::Alignment::Center),
    )
    .padding([theme::space::MD, theme::space::LG])
    .style(move |_| container::Style {
        background: Some(theme::bg_color(t.panel())),
        ..Default::default()
    })
    .width(Length::Fill)
    .into()
}
