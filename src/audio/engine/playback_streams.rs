use super::{playback, AudioEvent, EngineCtx, EngineErrorEvent, SINK_NODE_NAME};

#[derive(Default)]
pub(super) struct PlaybackStreams {
    sink_stream: Option<playback::PlaybackStream>,
    monitor_stream: Option<playback::PlaybackStream>,
    sample_rate: u32,
    channels: u16,
}

impl PlaybackStreams {
    fn has_format(&self, sample_rate: u32, channels: u16) -> bool {
        self.sink_stream.is_some() && self.sample_rate == sample_rate && self.channels == channels
    }

    fn active_format_conflict(&self, sample_rate: u32, channels: u16) -> bool {
        self.sample_rate != 0 && (self.sample_rate != sample_rate || self.channels != channels)
    }

    fn monitor_enabled(&self) -> bool {
        self.monitor_stream.is_some()
    }

    fn reset(&mut self) {
        self.sink_stream = None;
        self.monitor_stream = None;
        self.sample_rate = 0;
        self.channels = 0;
    }
}

pub(super) fn active_format_conflict(ctx: &EngineCtx, sample_rate: u32, channels: u16) -> bool {
    !ctx.voices.borrow().is_empty()
        && ctx
            .playback_streams
            .borrow()
            .active_format_conflict(sample_rate, channels)
}

pub(super) fn monitor_enabled(ctx: &EngineCtx) -> bool {
    ctx.playback_streams.borrow().monitor_enabled()
}

pub(super) fn stream_format(ctx: &EngineCtx) -> Option<(u32, u16)> {
    let streams = ctx.playback_streams.borrow();
    streams
        .sink_stream
        .as_ref()
        .map(|_| (streams.sample_rate, streams.channels))
}

pub(super) fn ensure_playback_streams(ctx: &EngineCtx, sample_rate: u32, channels: u16) -> bool {
    if ctx.voices.borrow().is_empty() {
        let mut streams = ctx.playback_streams.borrow_mut();
        if !streams.has_format(sample_rate, channels) {
            streams.reset();
        }
    }

    if !ensure_sink_stream(ctx, sample_rate, channels) {
        return false;
    }
    ensure_monitor_stream(ctx, sample_rate, channels);
    true
}

fn ensure_sink_stream(ctx: &EngineCtx, sample_rate: u32, channels: u16) -> bool {
    if ctx
        .playback_streams
        .borrow()
        .has_format(sample_rate, channels)
    {
        return true;
    }

    match playback::create_sink_mix_stream(
        ctx.core.clone(),
        ctx.voices.clone(),
        SINK_NODE_NAME,
        sample_rate,
        channels,
    ) {
        Ok(stream) => {
            let mut streams = ctx.playback_streams.borrow_mut();
            streams.sink_stream = Some(stream);
            streams.sample_rate = sample_rate;
            streams.channels = channels;
            true
        }
        Err(e) => {
            let _ = ctx
                .evt_tx
                .send(AudioEvent::Error(EngineErrorEvent::SinkStreamCreation {
                    detail: e.to_string(),
                }));
            false
        }
    }
}

fn ensure_monitor_stream(ctx: &EngineCtx, sample_rate: u32, channels: u16) {
    if ctx.playback_streams.borrow().monitor_stream.is_some() {
        return;
    }
    let target = ctx.monitor_target.borrow().clone();
    match playback::create_monitor_stream(
        ctx.core.clone(),
        ctx.voices.clone(),
        sample_rate,
        channels,
        target.as_deref(),
    ) {
        Ok(stream) => {
            ctx.playback_streams.borrow_mut().monitor_stream = Some(stream);
        }
        Err(e) => {
            ctx.voices.borrow_mut().stop_all_monitors();
            let _ = ctx.evt_tx.send(AudioEvent::Error(
                EngineErrorEvent::MonitorStreamUnavailable {
                    detail: e.to_string(),
                },
            ));
        }
    }
}

pub(super) fn rebuild_monitor_stream(ctx: &EngineCtx) {
    let Some((rate, channels)) = stream_format(ctx) else {
        return;
    };
    let target = ctx.monitor_target.borrow().clone();
    match playback::create_monitor_stream(
        ctx.core.clone(),
        ctx.voices.clone(),
        rate,
        channels,
        target.as_deref(),
    ) {
        Ok(stream) => {
            ctx.playback_streams.borrow_mut().monitor_stream = Some(stream);
        }
        Err(e) => {
            ctx.playback_streams.borrow_mut().monitor_stream = None;
            ctx.voices.borrow_mut().stop_all_monitors();
            let _ = ctx
                .evt_tx
                .send(AudioEvent::Error(EngineErrorEvent::MonitorStreamRebuild {
                    detail: e.to_string(),
                }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PlaybackStreams;

    #[test]
    fn active_format_change_requires_interrupt_fallback() {
        let streams = PlaybackStreams {
            sample_rate: 48_000,
            channels: 2,
            ..PlaybackStreams::default()
        };

        assert!(streams.active_format_conflict(44_100, 1));
        assert!(!streams.active_format_conflict(48_000, 2));
        assert!(!PlaybackStreams::default().active_format_conflict(48_000, 2));
    }
}
