use iced::widget::canvas;
use iced::widget::{container, row, space, text, Column, Space};
use iced::{Border, Element, Length};

use crate::app::Message;
use crate::state::SoundEntry;
use crate::ui::theme::{self, Hh, Theme};
use crate::ui::volume;
use crate::ui::waveform::{render_key, RenderKey};

/// Owns the persistent waveform `canvas::Cache` and the key describing what is
/// currently cached. Held in app state across frames; the cache is cleared
/// **only** when [`NowPlaying::sync`] observes a key change — never rebuilt in
/// `view()`. This is the persistent-cache pattern ADR-009 requires proving
/// before the tile grid (#13/#92).
#[derive(Default)]
pub struct NowPlaying {
    cache: canvas::Cache,
    key: Option<RenderKey>,
}

impl NowPlaying {
    /// Reconciles the cache with the current playback state. Returns `true` when
    /// the cached geometry was invalidated (content changed), `false` when the
    /// existing cache is reused. Call once per update/view glue — the only place
    /// the cache is ever cleared.
    pub fn sync(&mut self, playing: Option<&str>, progress: f32) -> bool {
        let next = render_key(playing, progress);
        if self.key.as_ref() == Some(&next) {
            return false;
        }
        self.cache.clear();
        self.key = Some(next);
        true
    }

    #[cfg(test)]
    pub(crate) fn current_key(&self) -> Option<&RenderKey> {
        self.key.as_ref()
    }
}

pub fn view_now_playing<'a>(
    playing: Option<&'a str>,
    sounds: &'a [SoundEntry],
    progress: f32,
    vol: f32,
) -> Element<'a, Message> {
    let t = Theme::Dark;

    let sound = match playing {
        Some(id) => sounds.iter().find(|s| s.id == id),
        None => None,
    };

    let sound = match sound {
        Some(s) => s,
        None => return Space::new().into(),
    };

    let content = row![
        view_placeholder(t),
        view_sound_info(sound, t),
        view_progress_bar(progress, t),
        space::horizontal(),
        volume::view_volume(vol),
    ]
    .spacing(theme::space::LG)
    .align_y(iced::Alignment::Center);

    container(content)
        .width(Length::Fill)
        .padding([theme::space::MD, theme::space::XL])
        .style(move |_theme| container::Style {
            background: Some(theme::bg_color(t.panel())),
            border: Border {
                color: t.hairline(),
                width: 1.0,
                radius: iced::border::Radius::default(),
            },
            ..Default::default()
        })
        .into()
}

fn view_placeholder(t: Theme) -> Element<'static, Message> {
    container(Space::new())
        .width(theme::component::ARTWORK_SQ)
        .height(theme::component::ARTWORK_SQ)
        .style(move |_theme| container::Style {
            background: Some(theme::bg_color(t.bg())),
            border: Border {
                color: t.hairline(),
                width: 1.0,
                radius: theme::radius::MD,
            },
            ..Default::default()
        })
        .into()
}

fn view_sound_info<'a>(sound: &'a SoundEntry, t: Theme) -> Element<'a, Message> {
    let name = text(sound.name.clone())
        .size(theme::font::BODY)
        .color(t.ink());
    let subtitle = text(format!(
        "HONKING NOW \u{00b7} {} \u{00b7} {}",
        sound.category,
        crate::ui::fmt_duration(sound.duration_ms),
    ))
    .size(theme::font::LABEL)
    .color(t.ink_dim());
    Column::new()
        .push(name)
        .push(subtitle)
        .spacing(theme::space::XS)
        .into()
}

fn view_progress_bar(progress: f32, t: Theme) -> Element<'static, Message> {
    let filled_width = (progress.clamp(0.0, 1.0) * 320.0).round();

    let filled = container(Space::new())
        .width(filled_width)
        .height(theme::component::PROGRESS_BAR_H)
        .style(move |_theme| container::Style {
            background: Some(theme::bg_color(t.accent())),
            border: Border {
                radius: theme::radius::SM,
                ..Default::default()
            },
            ..Default::default()
        });

    container(filled)
        .width(320.0)
        .height(theme::component::PROGRESS_BAR_H)
        .style(move |_theme| container::Style {
            background: Some(theme::bg_color(t.bg())),
            border: Border {
                radius: theme::radius::SM,
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sync_clears_and_stores_key() {
        let mut np = NowPlaying::default();
        assert!(np.current_key().is_none());
        let cleared = np.sync(Some("s1"), 0.0);
        assert!(cleared, "first sync must populate the cache key");
        assert!(np.current_key().is_some());
    }

    #[test]
    fn same_state_reuses_cache() {
        let mut np = NowPlaying::default();
        np.sync(Some("s1"), 0.5);
        let cleared = np.sync(Some("s1"), 0.5);
        assert!(
            !cleared,
            "identical state must reuse the cache, not clear it"
        );
    }

    #[test]
    fn sub_bucket_progress_jitter_reuses_cache() {
        let mut np = NowPlaying::default();
        np.sync(Some("s1"), 0.5000);
        // A tiny progress tick within the same bucket must NOT thrash the cache.
        let cleared = np.sync(Some("s1"), 0.5001);
        assert!(!cleared, "sub-bucket jitter must not invalidate the cache");
    }

    #[test]
    fn changing_sound_clears_cache() {
        let mut np = NowPlaying::default();
        np.sync(Some("s1"), 0.5);
        assert!(np.sync(Some("s2"), 0.5), "new sound must invalidate cache");
    }

    #[test]
    fn crossing_a_progress_bucket_clears_cache() {
        let mut np = NowPlaying::default();
        np.sync(Some("s1"), 0.0);
        assert!(
            np.sync(Some("s1"), 1.0),
            "large progress jump must invalidate"
        );
    }

    #[test]
    fn stopping_playback_clears_cache() {
        let mut np = NowPlaying::default();
        np.sync(Some("s1"), 0.5);
        assert!(np.sync(None, 0.0), "stopping must invalidate the cache");
    }
}
