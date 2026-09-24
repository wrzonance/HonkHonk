use super::super::feedback::FeedbackMonitor;
use super::super::routing_graph::{GraphWatcher, RoutingGraph};
use super::routing::RoutingRuntime;
use super::*;

/// Own every listener and proxy until the routing timer is dropped.
pub(super) struct RoutingSetup {
    pub routing: RoutingRuntime,
    pub sink_id: Rc<Cell<Option<u32>>>,
    stream_rx: mpsc::Receiver<streams::StreamEvent>,
    router_rx: mpsc::Receiver<RouterEvent>,
    _sink: pipewire::node::Node,
    _source: Option<pipewire::node::Node>,
    _graph_watcher: GraphWatcher,
    _stream_watcher: streams::StreamWatcher,
    _feedback_monitor: FeedbackMonitor,
}

impl RoutingSetup {
    pub fn new(
        core: pipewire::core::CoreRc,
        events: mpsc::Sender<AudioEvent>,
        preferred: Option<String>,
        passthrough: bool,
    ) -> Result<Self, AudioError> {
        let sink = create_virtual_sink(&core)?;
        let source = ensure_virtual_source(&core, &events)?;
        let graph = Rc::new(RefCell::new(RoutingGraph::default()));
        let graph_watcher = GraphWatcher::start(&core, graph.clone())?;
        let sink_id = Rc::new(Cell::new(None));
        let sink_ports = Rc::new(RefCell::new(Vec::new()));
        let registry = Rc::new(setup_registry_listener(
            &core,
            RegistryConfig {
                graph: graph.clone(),
                shared_sink_id: sink_id.clone(),
                default_source_name: preferred,
                mic_passthrough: Rc::new(Cell::new(passthrough)),
                evt_tx: events.clone(),
                shared_sink_ports: sink_ports.clone(),
            },
        )?);
        let (stream_watcher, stream_rx) = spawn_stream_watcher(&core)?;
        let (router_tx, router_rx) = mpsc::channel();
        let router = Rc::new(RefCell::new(Router::new(router_tx)));
        router.borrow_mut().use_graph(graph);
        let feedback_pending = Rc::new(Cell::new(false));
        let feedback_monitor =
            FeedbackMonitor::start(core.clone(), feedback_pending.clone(), events.clone())?;
        Ok(Self {
            routing: RoutingRuntime {
                router,
                registry,
                sink_ports,
                core,
                events,
                feedback_pending,
            },
            sink_id,
            stream_rx,
            router_rx,
            _sink: sink,
            _source: source,
            _graph_watcher: graph_watcher,
            _stream_watcher: stream_watcher,
            _feedback_monitor: feedback_monitor,
        })
    }

    pub fn tick(&self) {
        self.routing.tick(&self.stream_rx, &self.router_rx);
    }
}
