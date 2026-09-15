use super::*;

pub(super) fn create_virtual_sink(
    core: &pipewire::core::CoreRc,
) -> Result<pipewire::node::Node, AudioError> {
    let sink_props = pipewire::properties::properties! {
        "factory.name" => "support.null-audio-sink",
        "node.name" => SINK_NODE_NAME,
        "node.description" => SINK_DESCRIPTION,
        "media.class" => "Audio/Sink/Virtual",
        "audio.position" => "[FL,FR]",
        "object.linger" => "false",
    };
    core.create_object("adapter", &sink_props)
        .map_err(|e| AudioError::VirtualSinkCreation(e.to_string()))
}

/// First-run decision: create the virtual source programmatically only when
/// no `honkhonk-mic` node already exists (i.e. no packaged/user conf.d has
/// declared it). When it already exists we reuse it and never recreate.
pub(super) fn should_create_source(source_already_exists: bool) -> bool {
    !source_already_exists
}

/// Pure scan: does a `pw-dump` (JSON) or `pw-cli` text blob mention a node
/// whose `node.name` is our virtual source? Matches the quoted name token so a
/// substring like `honkhonk-mic-foo` does not false-positive. Tolerant of both
/// `pw-cli` form (`node.name = "honkhonk-mic"`) and `pw-dump` JSON form
/// (`"node.name": "honkhonk-mic",`).
pub(super) fn source_present_in_dump(dump: &str) -> bool {
    let needle = format!("\"{SOURCE_NODE_NAME}\"");
    dump.lines().any(|line| {
        let l = line.trim().trim_start_matches('"');
        l.starts_with("node.name") && l.contains(&needle)
    })
}

/// Probe PipeWire (via `pw-dump`) for an existing `honkhonk-mic` node.
/// Returns `false` if the tool is missing or fails — the caller then falls
/// back to programmatic creation, which itself fails gracefully without PW.
pub(super) fn source_already_exists() -> bool {
    std::process::Command::new("pw-dump")
        .output()
        .ok()
        .map(|o| source_present_in_dump(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or(false)
}

pub(super) fn create_virtual_source(
    core: &pipewire::core::CoreRc,
) -> Result<pipewire::node::Node, AudioError> {
    let source_props = pipewire::properties::properties! {
        "factory.name" => "support.null-audio-sink",
        "node.name" => SOURCE_NODE_NAME,
        "node.description" => SOURCE_DESCRIPTION,
        "media.class" => "Audio/Source/Virtual",
        "audio.position" => "[FL,FR]",
        // Lingering: the programmatically-created source survives app exit
        // (until reboot) as a first-run bridge until a packaged/user conf.d
        // takes effect. See ADR-004. The internal mixing sink stays linger=false.
        "object.linger" => "true",
    };
    core.create_object("adapter", &source_props)
        .map_err(|e| AudioError::VirtualSourceCreation(e.to_string()))
}

/// Write the per-user conf.d bridge, reporting failures as non-fatal events.
/// Returns whether a new file was written.
pub(super) fn write_first_run_confd(evt_tx: &mpsc::Sender<AudioEvent>) -> bool {
    match confd::user_confd_dir() {
        Ok(dir) => confd::write_user_confd_in(&dir).unwrap_or_else(|e| {
            let _ = evt_tx.send(AudioEvent::Error(EngineErrorEvent::ConfdWrite {
                detail: e.to_string(),
            }));
            false
        }),
        Err(e) => {
            let _ = evt_tx.send(AudioEvent::Error(EngineErrorEvent::ConfdPath {
                detail: e.to_string(),
            }));
            false
        }
    }
}

/// Ensure the persistent virtual source exists (issue #49).
///
/// If a `honkhonk-mic` node already exists (packaged/user conf.d case) we reuse
/// it and create nothing — returns `None`. Otherwise (dev/unpackaged first run)
/// we create it programmatically (lingering), write the per-user conf.d bridge,
/// and emit `SourceFirstRun`. The returned `Node`, when `Some`, is held to
/// end-of-scope and is NEVER explicitly destroyed: a lingering node survives
/// app exit, and the conf.d bridge re-creates it next session regardless.
pub(super) fn ensure_virtual_source(
    core: &pipewire::core::CoreRc,
    evt_tx: &mpsc::Sender<AudioEvent>,
) -> Result<Option<pipewire::node::Node>, AudioError> {
    if !should_create_source(source_already_exists()) {
        return Ok(None);
    }
    let node = create_virtual_source(core)?;
    let confd_written = write_first_run_confd(evt_tx);
    let _ = evt_tx.send(AudioEvent::SourceFirstRun { confd_written });
    Ok(Some(node))
}
