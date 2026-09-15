//! Persistent static geometry, with an uncached playhead/drag overlay (ADR-009).
use super::geometry::{assign_lanes, ruler_ticks, time_at};
use crate::app::Message;
use crate::app::macro_editor::{Drag, EditorMessage, EditorState};
use crate::state::{Macro, SoundEntry};
use crate::ui::theme::{Hh, Theme as AppTheme};
use iced::widget::canvas::{self, Cache, Frame, Geometry, Path, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, mouse};

const LANE_HEIGHT: f32 = 38.0;
const RULER: f32 = 25.0;
const MAX_WIDTH: f64 = 16_000.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Bar {
    pub start: u64,
    pub duration: u64,
    pub lane: usize,
    pub label: String,
    pub missing: bool,
}

pub(crate) struct TimelineState {
    cache: Cache,
    pub bars: Vec<Bar>,
    pub scale: f64,
    theme: AppTheme,
}

impl Default for TimelineState {
    fn default() -> Self {
        Self {
            cache: Cache::new(),
            bars: Vec::new(),
            scale: 0.1,
            theme: AppTheme::Dark,
        }
    }
}

impl TimelineState {
    pub fn sync(&mut self, value: Option<&Macro>, sounds: &[SoundEntry], theme: AppTheme) {
        let mut bars: Vec<_> = value
            .into_iter()
            .flat_map(|m| &m.steps)
            .map(|step| {
                let sound = sounds.iter().find(|s| s.path == step.sound);
                Bar {
                    start: step.start_offset_ms,
                    duration: sound.and_then(|s| s.duration_ms).unwrap_or(500).max(1),
                    lane: 0,
                    label: sound
                        .map(|s| s.name.clone())
                        .unwrap_or_else(|| "Missing sound".into()),
                    missing: sound.is_none(),
                }
            })
            .collect();
        let intervals: Vec<_> = bars.iter().map(|b| (b.start, b.duration)).collect();
        for (bar, lane) in bars.iter_mut().zip(assign_lanes(&intervals)) {
            bar.lane = lane;
        }
        if self.bars != bars || self.theme != theme {
            self.cache.clear();
            self.bars = bars;
            self.theme = theme;
            // Reserve two pixels for the minimum-width placeholder at the end.
            self.scale = ((MAX_WIDTH - 2.0) / (self.end_ms() + 3000.0)).min(0.1);
        }
    }

    pub fn size(&self) -> Size {
        let lanes = self.bars.iter().map(|b| b.lane + 1).max().unwrap_or(1);
        Size::new(
            (((self.end_ms() + 3000.0) * self.scale + 2.0).clamp(800.0, MAX_WIDTH)) as f32,
            (lanes as f32 * LANE_HEIGHT + RULER).max(240.0),
        )
    }

    fn end_ms(&self) -> f64 {
        // Convert before adding so even start + duration > u64::MAX fits.
        self.bars
            .iter()
            .map(|bar| bar.start as f64 + bar.duration as f64)
            .fold(0.0, f64::max)
    }

    pub(super) fn rectangle(&self, bar: &Bar) -> Rectangle {
        Rectangle::new(
            Point::new(
                (bar.start as f64 * self.scale) as f32,
                RULER + bar.lane as f32 * LANE_HEIGHT,
            ),
            Size::new(
                ((bar.duration as f64 * self.scale) as f32).max(2.0),
                LANE_HEIGHT - 5.0,
            ),
        )
    }

    fn hit(&self, point: Point) -> Option<usize> {
        self.bars
            .iter()
            .position(|bar| self.rectangle(bar).contains(point))
    }
}

pub(crate) struct Timeline<'a> {
    pub editor: &'a EditorState,
}

impl canvas::Program<Message> for Timeline<'_> {
    type State = ();

    fn update(
        &self,
        _: &mut (),
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let message = self.pointer_event(event, bounds, cursor)?;
        Some(canvas::Action::publish(Message::MacroEditor(message)).and_capture())
    }

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let state = &self.editor.timeline;
        let static_geometry = state
            .cache
            .draw(renderer, bounds.size(), |frame| draw_bars(frame, state));
        let mut overlay = Frame::new(renderer, bounds.size());
        if self.editor.preview_start.is_some() {
            let x = (self.editor.playhead_ms as f64 * state.scale) as f32;
            overlay.stroke(
                &Path::line(Point::new(x, 0.0), Point::new(x, bounds.height)),
                Stroke::default()
                    .with_color(state.theme.ink())
                    .with_width(2.0),
            );
        }
        self.draw_ghost(&mut overlay);
        vec![static_geometry, overlay.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _: &(),
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if self.editor.dragging.is_some() {
            mouse::Interaction::Grabbing
        } else if cursor
            .position_in(bounds)
            .and_then(|p| self.editor.timeline.hit(p))
            .is_some()
        {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }
}

impl Timeline<'_> {
    pub(super) fn pointer_event(
        &self,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<EditorMessage> {
        let point = cursor.position_in(bounds);
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let point = point?;
                let index = self.editor.timeline.hit(point)?;
                let grab = point.x
                    - (self.editor.timeline.bars[index].start as f64 * self.editor.timeline.scale)
                        as f32;
                Some(EditorMessage::MoveStart(index, grab))
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                Some(EditorMessage::Menu(self.editor.timeline.hit(point?)?))
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { .. })
                if self.editor.dragging.is_some() =>
            {
                cursor
                    .position()
                    .map(|p| EditorMessage::Pointer(Point::new(p.x - bounds.x, p.y - bounds.y)))
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if self.editor.dragging.is_some() =>
            {
                let target = match self.editor.dragging {
                    Some(Drag::Step { .. }) => cursor
                        .position()
                        .map(|p| Point::new(p.x - bounds.x, p.y - bounds.y)),
                    _ => point,
                };
                Some(EditorMessage::Release(target))
            }
            _ => None,
        }
    }

    fn draw_ghost(&self, frame: &mut Frame) {
        let (Some(drag), Some(pointer)) = (&self.editor.dragging, self.editor.pointer) else {
            return;
        };
        let state = &self.editor.timeline;
        let (grab, duration, lane) = match drag {
            Drag::Step { index, grab } => {
                let Some(bar) = state.bars.get(*index) else {
                    return;
                };
                (*grab, bar.duration, bar.lane)
            }
            Drag::Sound(_) => (
                0.0,
                500,
                state.bars.iter().map(|b| b.lane + 1).max().unwrap_or(0),
            ),
        };
        let time = time_at(pointer.x, grab, state.scale, self.editor.snap);
        let rectangle = state.rectangle(&Bar {
            start: time,
            duration,
            lane,
            label: String::new(),
            missing: false,
        });
        frame.fill_rectangle(
            rectangle.position(),
            rectangle.size(),
            Color {
                a: 0.4,
                ..state.theme.accent()
            },
        );
    }
}

fn draw_bars(frame: &mut Frame, state: &TimelineState) {
    frame.fill_rectangle(Point::ORIGIN, frame.size(), state.theme.panel());
    for (x, second) in ruler_ticks(frame.width(), state.scale) {
        frame.stroke(
            &Path::line(Point::new(x, RULER), Point::new(x, frame.height())),
            Stroke::default().with_color(state.theme.hairline()),
        );
        frame.fill_text(canvas::Text {
            content: if second < 1_000_000 {
                format!("{second}s")
            } else {
                format!("{:.1e}s", second as f64)
            },
            position: Point::new(x + 3.0, 2.0),
            color: state.theme.ink(),
            size: 12.0.into(),
            ..Default::default()
        });
    }
    for bar in &state.bars {
        let rect = state.rectangle(bar);
        let color = if bar.missing {
            Color::from_rgb(0.6, 0.2, 0.2)
        } else {
            state.theme.accent()
        };
        frame.fill_rectangle(rect.position(), rect.size(), color);
    }
}
