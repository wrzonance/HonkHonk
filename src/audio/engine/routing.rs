use super::super::registry::RegistryGuard;
use super::*;

pub(super) struct RoutingRuntime {
    pub router: Rc<RefCell<Router>>,
    pub registry: Rc<RegistryGuard>,
    pub sink_ports: Rc<RefCell<Vec<u32>>>,
    pub core: pipewire::core::CoreRc,
    pub events: mpsc::Sender<AudioEvent>,
    pub feedback_pending: Rc<Cell<bool>>,
}

impl RoutingRuntime {
    pub fn tick(
        &self,
        streams: &mpsc::Receiver<streams::StreamEvent>,
        events: &mpsc::Receiver<RouterEvent>,
    ) {
        if self.feedback_pending.replace(false) {
            self.feedback();
        }
        self.registry.recheck_routes();
        self.router
            .borrow_mut()
            .update_sink_ports(self.sink_ports.borrow().clone());
        self.router.borrow_mut().recheck_routes();
        while let Ok(event) = streams.try_recv() {
            self.stream_event(event);
        }
        self.router.borrow_mut().retry_pending(&self.core);
        while let Ok(event) = events.try_recv() {
            self.router_event(event);
        }
    }

    fn feedback(&self) {
        let now = std::time::Instant::now();
        let app = self.router.borrow().latest_source();
        let mic = self.registry.latest_source();
        let suspected_source = match choose_suspect(app, mic) {
            Some(Suspect::Application(id)) => self.router.borrow_mut().trip_feedback(id, now),
            Some(Suspect::Microphone) => self.registry.trip_feedback(now),
            None => None,
        };
        let _ = self
            .events
            .send(AudioEvent::FeedbackDetected { suspected_source });
    }

    fn stream_event(&self, event: streams::StreamEvent) {
        use streams::StreamEvent;
        let _ = self.events.send(AudioEvent::Stream(event.clone()));
        let mut router = self.router.borrow_mut();
        match event {
            StreamEvent::SourceAdded {
                id,
                app_name,
                app_binary,
                app_pid,
                ..
            } => {
                router.on_source_added(id, app_name, app_binary, app_pid);
            }
            StreamEvent::SourceRemoved { id } => router.on_source_removed(id),
            StreamEvent::PortAdded {
                id,
                node_id,
                channel,
                direction,
                ..
            } => router.on_port_added(id, node_id, channel, direction),
            StreamEvent::PortRemoved { id } => router.on_port_removed(id),
            StreamEvent::SourceUpdated { .. } | StreamEvent::SourceLevelChanged { .. } => {}
        }
    }

    fn router_event(&self, event: RouterEvent) {
        use super::super::error::RouterError;
        let audio = match event {
            RouterEvent::RouteCreated { node_id, .. }
            | RouterEvent::AutoReconnected { node_id, .. } => AudioEvent::RoutingChanged {
                node_id,
                enabled: true,
            },
            RouterEvent::RouteDestroyed { node_id } => AudioEvent::RoutingChanged {
                node_id,
                enabled: false,
            },
            RouterEvent::Error(RouterError::UnsafeRoute { node_id, reason }) => {
                AudioEvent::RouteRejected { node_id, reason }
            }
            RouterEvent::Error(error) => AudioEvent::Error(EngineErrorEvent::Routing {
                detail: error.to_string(),
            }),
            RouterEvent::SourceDisconnected { .. } => return,
        };
        let _ = self.events.send(audio);
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Suspect {
    Application(u32),
    Microphone,
}

fn choose_suspect(
    app: Option<(std::time::Instant, u32)>,
    mic: Option<(std::time::Instant, u32)>,
) -> Option<Suspect> {
    if app.is_some() && app >= mic {
        app.map(|(_, id)| Suspect::Application(id))
    } else {
        mic.map(|_| Suspect::Microphone)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn only_active_sources_can_be_suspected_and_latest_activation_wins() {
        let now = Instant::now();
        assert_eq!(choose_suspect(None, None), None);
        assert_eq!(
            choose_suspect(Some((now, 3)), None),
            Some(Suspect::Application(3))
        );
        assert_eq!(
            choose_suspect(None, Some((now, 2))),
            Some(Suspect::Microphone)
        );
        assert_eq!(
            choose_suspect(Some((now, 3)), Some((now + Duration::from_secs(1), 2))),
            Some(Suspect::Microphone)
        );
    }
}
