# ADR-004: Persistent virtual source via PipeWire conf.d

## Status: Accepted

## Context
HonkHonk's virtual microphone (`honkhonk-mic`) previously existed only while
the app ran (`object.linger = false`). On exit/crash, Discord/OBS lost the
selected input and fell back to default — the #1 UX complaint for app-lifetime
virtual-audio tools (EasyEffects, Soundux). Users expect the mic to behave like
a device (VoiceMeeter, NoiseTorch), persisting across app restarts and reboots.

CLAUDE.md referenced an ADR-004 ("persistent sink, no per-sound nodes") that
was never written. That intent meant "persistent within a session". This ADR
supersedes that intent and creates the file: the **source** is now
system-persistent; the internal mixing **sink** remains session-persistent.

## Decision
Ship a PipeWire `pipewire.conf.d` drop-in
(`/usr/share/pipewire/pipewire.conf.d/50-honkhonk.conf`) via every system
package. It declares a lingering `support.null-audio-sink` exposing
`media.class = Audio/Source/Virtual` named `honkhonk-mic`. PipeWire owns the
device's lifecycle; it exists whether or not the app runs and across reboots.

App startup queries the registry (via `pw-dump`) for `honkhonk-mic`:
- **Found** (packaged/conf.d case): reuse it, skip programmatic creation.
- **Absent** (dev/unpackaged/Flatpak/AppImage first run): create it
  programmatically with `object.linger = true` (survives app exit until reboot)
  AND write a per-user drop-in to
  `$XDG_CONFIG_HOME/pipewire/pipewire.conf.d/50-honkhonk.conf` as the
  persistence bridge, then surface a one-time UI notice
  (`AudioEvent::SourceFirstRun`).

Only the source persists. The internal mixing sink (`honkhonk-mix`), mic
passthrough links, and playback streams stay app-lifetime and are torn down on
shutdown (RAII drop at end of the engine main loop). We never explicitly
destroy the source node: a lingering node survives the app, and the conf.d
bridge re-creates it next session regardless.

### Alternatives rejected
- **systemd user service**: heavier, adds a unit to manage, still needs a node
  definition; conf.d achieves persistence with one declarative file and zero
  running process.
- **WirePlumber routing rules**: WirePlumber policy is for *routing*, not for
  *declaring* a static null sink; wrong layer, more fragile across WirePlumber
  versions, and out of scope (#17 handles auto-routing).
- **Both devices persistent**: the internal sink has no external consumers; no
  audio is processed without the app running, so a persistent sink would be
  dead weight and could confuse users browsing device lists.

### Why `/usr/share` not an `/etc` conffile
Vendor PipeWire drop-ins live under `/usr/share/pipewire/pipewire.conf.d/`.
This is read-only vendor config, not user-edited policy, so it is intentionally
NOT marked a dpkg conffile. Users who want to disable it remove the package or
shadow it with a higher-numbered drop-in under `~/.config`.

## Consequences
- Discord/OBS keep their `HonkHonk Mic` selection across app restarts/reboots.
- An idle null-audio-sink uses zero CPU; the device is always visible in audio
  settings after install + a PipeWire restart (or reboot).
- New conf.d files require a PipeWire restart to take effect; reboot does this
  naturally. First install before a reboot needs a manual
  `systemctl --user restart pipewire` — documented for packagers.
- Uninstall must drop the device: the .deb `postrm` restarts the user's
  PipeWire so it stops loading the removed drop-in (best-effort, never fails
  removal).
- Flatpak/AppImage cannot install host conf.d; they rely on the first-run
  fallback (lingering node + per-user conf.d), so behavior is consistent across
  distribution channels.
- The app must tolerate a pre-existing device: it skips creation and only wires
  links, so double-creation and `node.name` collisions cannot occur.
- The registry probe shells out to `pw-dump`. If it is missing (e.g. CI without
  PipeWire), the probe reports "absent" and the engine falls back to
  programmatic creation, which itself fails gracefully without a PipeWire
  server — no crash, no panic.
