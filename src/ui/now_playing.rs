use iced::widget::canvas;
use iced::widget::{container, row, space, text, Column, Space};
use iced::{Border, Element, Length};

use crate::app::Message;
use crate::audio::Envelope;
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
        // Hot path: compare without allocating an owned id. Only build the
        // owned `RenderKey` when the key actually changes (i.e. on a clear).
        if let Some(key) = &self.key {
            if key.matches(playing, progress) {
                return false;
            }
        }
        self.cache.clear();
        self.key = Some(render_key(playing, progress));
        true
    }

    #[cfg(test)]
    pub(crate) fn current_key(&self) -> Option<&RenderKey> {
        self.key.as_ref()
    }
}

/// Canvas program for the now-playing waveform. `draw` paints the static bars
/// through the persistent cache (reused frame-to-frame) and overlays the moving
/// playhead separately, so the expensive bar tessellation happens once per
/// sound — not once per frame (the ADR-009 anti-pattern PR #96 hit).
struct WaveformProgram<'a> {
    cache: &'a canvas::Cache,
    samples: Vec<f32>,
    progress: f32,
    bar: iced::Color,
    bar_dim: iced::Color,
    accent: iced::Color,
}

impl<Message> canvas::Program<Message> for WaveformProgram<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        use iced::widget::canvas::{Path, Stroke};
        use iced::{Point, Size};

        let n = self.samples.len() as f32;
        let gap = 2.0;
        let bar_w = ((bounds.width - gap * (n - 1.0)) / n).max(1.0);

        // Static bars: drawn once and reused. `Cache::draw` only re-runs this
        // closure when the cache was cleared (NowPlaying::sync on a key change).
        // The `bounds.size()` argument is what triggers re-tessellation on window
        // resize — Iced treats a size change as a cache miss automatically.
        let played_to = (self.progress.clamp(0.0, 1.0) * bounds.width).round();
        let bars = self.cache.draw(renderer, bounds.size(), |frame| {
            // The played/unplayed bar split is baked into the CACHED geometry via
            // `played_to` here: the SMOOTH progress sampled at the last cache
            // rebuild. Rebuilds happen on bucket crossings (RenderKey's bucket)
            // and on bounds changes (the size arg above), so the cached split
            // advances in bucket-sized steps while the overlay line below moves
            // every frame. If future code makes the cached content depend on
            // state NOT in `RenderKey` or the cache bounds, bars will go stale.
            for (i, &h) in self.samples.iter().enumerate() {
                let x = i as f32 * (bar_w + gap);
                let bh = (h * bounds.height).max(1.0);
                let y = (bounds.height - bh) / 2.0;
                // Strict `<`: at 0% progress (played_to == 0.0) no bar reads as played.
                let color = if x < played_to {
                    self.bar
                } else {
                    self.bar_dim
                };
                frame.fill_rectangle(Point::new(x, y), Size::new(bar_w, bh), color);
            }
        });

        // Playhead overlay: a thin accent line at the progress position, drawn
        // fresh each frame WITHOUT invalidating the cached bars above.
        let mut overlay = canvas::Frame::new(renderer, bounds.size());
        let line = Path::line(
            Point::new(played_to, 0.0),
            Point::new(played_to, bounds.height),
        );
        overlay.stroke(
            &line,
            Stroke::default().with_color(self.accent).with_width(2.0),
        );

        vec![bars, overlay.into_geometry()]
    }
}

pub fn view_now_playing<'a>(
    now_playing: &'a NowPlaying,
    sound: Option<&'a SoundEntry>,
    progress: f32,
    vol: f32,
    envelope: Option<&Envelope>,
) -> Element<'a, Message> {
    let t = Theme::Dark;

    let Some(sound) = sound else {
        return Space::new().into();
    };

    let content = row![
        view_placeholder(t),
        view_sound_info(sound, t),
        view_waveform(now_playing, envelope, progress, t),
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

/// Builds the canvas widget backed by the persistent cache. Display bars are
/// max-pooled from the cached `Envelope`; a missing envelope renders a flat
/// baseline (never fake bars). The cache is owned by `now_playing`.
fn view_waveform<'a>(
    now_playing: &'a NowPlaying,
    envelope: Option<&Envelope>,
    progress: f32,
    t: Theme,
) -> Element<'a, Message> {
    use crate::ui::waveform::WAVEFORM_BARS;
    let samples = match envelope {
        Some(env) => env.bars(WAVEFORM_BARS),
        None => vec![0.0; WAVEFORM_BARS],
    };
    let program = WaveformProgram {
        cache: &now_playing.cache,
        samples,
        progress,
        bar: t.accent(),
        bar_dim: t.ink_faint(),
        accent: t.ink(),
    };
    // `canvas::Canvas::new` (the struct) is used here rather than the
    // `canvas(program)` free function because `use iced::widget::canvas;`
    // above shadows the helper name.
    canvas::Canvas::new(program)
        .width(320.0)
        .height(theme::component::ARTWORK_SQ)
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
        // This assertion depends on the bucket width (1/PROGRESS_BUCKETS ≈ 0.021)
        // being larger than the 0.0001 jitter — a future bucketing change that
        // shrinks buckets below this gap would be a genuine intentional trade-off,
        // not a sync bug.
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

    #[test]
    fn steady_playback_frames_reuse_cache() {
        // Simulate 60 consecutive frames of the SAME sound at the SAME progress
        // bucket (the common idle-render case Iced triggers on every Message).
        // Exactly one clear (the first); the rest reuse — proving the cache is
        // NOT rebuilt per frame (the ADR-009 anti-pattern).
        let mut np = NowPlaying::default();
        let mut clears = 0;
        for _ in 0..60 {
            if np.sync(Some("s1"), 0.5) {
                clears += 1;
            }
        }
        assert_eq!(clears, 1, "only the first frame may clear; rest reuse");
    }

    #[test]
    fn smooth_progress_clears_once_per_bucket_not_per_frame() {
        // Advance progress smoothly across one bucket worth of frames; the cache
        // clears at most once per bucket boundary, never every frame.
        use crate::ui::waveform::PROGRESS_BUCKETS;
        let mut np = NowPlaying::default();
        let step = 1.0 / (PROGRESS_BUCKETS as f32 * 4.0); // 4 frames per bucket
        let mut clears = 0;
        let mut p = 0.0;
        for _ in 0..16 {
            if np.sync(Some("s1"), p) {
                clears += 1;
            }
            p += step;
        }
        // 16 frames spanning ~4 buckets ⇒ far fewer clears than frames.
        assert!(
            clears <= 5,
            "got {clears} clears in 16 frames — cache thrashing"
        );
    }
}
