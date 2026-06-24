use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use iced::widget::canvas;
use iced::widget::{container, row, space, text, Column, Space};
use iced::{Border, Element, Length};

use crate::app::Message;
use crate::audio::{Envelope, ENVELOPE_BUCKETS};
use crate::state::SoundEntry;
use crate::ui::playhead::PlayheadClock;
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
    active_id: Option<String>,
    playhead: Option<PlayheadClock>,
    display_progress: f32,
    envelopes: HashMap<String, Arc<Envelope>>,
}

pub struct PlaybackStart<'a> {
    pub id: &'a str,
    pub duration: Duration,
    pub samples: &'a [f32],
    pub channels: u16,
    pub now: Instant,
}

impl NowPlaying {
    /// Starts the now-playing lifecycle for decoded PCM. The envelope is cached
    /// before per-sound volume is applied, so waveform shape is stable across
    /// volume edits.
    pub fn start(&mut self, playback: PlaybackStart<'_>) {
        let PlaybackStart {
            id,
            duration,
            samples,
            channels,
            now,
        } = playback;
        self.active_id = Some(id.to_owned());
        self.playhead = Some(PlayheadClock::new(duration, now));
        self.display_progress = 0.0;
        self.envelopes.entry(id.to_owned()).or_insert_with(|| {
            Arc::new(Envelope::from_samples(samples, channels, ENVELOPE_BUCKETS))
        });
        self.sync_active();
    }

    /// Advances the frame-interpolated playhead from the owned wall-clock
    /// clock. No-op while idle.
    pub fn tick(&mut self, now: Instant) {
        if let Some(clock) = &self.playhead {
            self.display_progress = self.display_progress.max(clock.display(now));
            self.sync_active();
        }
    }

    /// Clears active playback UI state. Per-sound envelopes remain cached for
    /// future plays, matching the existing decode-cache behavior.
    pub fn clear(&mut self) {
        self.active_id = None;
        self.playhead = None;
        self.display_progress = 0.0;
        self.sync_active();
    }

    pub fn display_progress(&self) -> f32 {
        self.display_progress
    }

    pub fn envelope(&self, id: &str) -> Option<Arc<Envelope>> {
        self.envelopes.get(id).map(Arc::clone)
    }

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

    fn sync_active(&mut self) -> bool {
        let active_id = self.active_id.clone();
        self.sync(active_id.as_deref(), self.display_progress)
    }

    #[cfg(test)]
    pub(crate) fn current_key(&self) -> Option<&RenderKey> {
        self.key.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn has_playhead(&self) -> bool {
        self.playhead.is_some()
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
mod tests;
