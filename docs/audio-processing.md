# Playback audio controls

Open **Settings → Audio → Playback processing** to change the defaults.
Normalization and global compression are enabled on a fresh installation and
when reading settings saved by older versions.

- **Normalization** measures K-weighted, gated integrated loudness and targets
  **−18 LUFS**, with a maximum **12 dB boost**. Silence and clips shorter than
  the measurement window (400 ms) keep their original gain. This uses the
  loudness measurement underlying EBU R128; it is not a claim of full broadcast
  R128 conformance, and the target differs from R128's −23 LUFS recommendation.
- **Compression** reduces peaks above the threshold according to the ratio.
  A lower threshold or higher ratio produces stronger compression. Attack is
  5 ms and release is 80 ms. Channels share one detector to preserve balance.
- **Limiting** caps the final mixed soundboard sample peaks at **98% of full
  scale** while global compression is enabled. Disabling compression retains
  the existing full-scale clipping guard. This is a sample-peak limiter, not
  an oversampled true-peak limiter. It cannot guarantee hardware safety or
  constrain volume added by devices, PipeWire, other applications, or your mic.

Global compression updates immediately for active soundboard voices. Changes
to normalization and per-sound controls take effect on the next playback.
Both tiles and macros use these controls; a macro step's gain is multiplied
by the sound's saved gain. Effects run before the final mixed-output limiter.
Monitoring and the soundboard's virtual-sink feed receive the same processing.
Microphone passthrough and routed external applications are outside this stage.

## Per-sound settings

Open a tile's context menu and choose its existing sound editor. Audio controls
appear after the file's identity has been read in the background; names and
tags remain editable while loading.

- **Per-sound volume** multiplies master volume, from 0% to 200%.
- **Normalization** follows the global setting, or explicitly enables/disables
  normalization for this sound.
- **Original / Force mono / Force stereo** preserves channels, averages channels
  to mono, or duplicates mono to stereo. Mono conversion can cancel audio whose
  channels are in opposite phase. Stereo sources retain their original stereo
  channels in Force stereo mode.
- **Pan** is a stereo balance control: center preserves both channels; moving
  right attenuates the left, and moving left attenuates the right. A panned mono
  sound is expanded to stereo. Nonzero pan therefore takes precedence over a
  mono output request.
- **Per-sound compression** optionally compresses that voice before effects.
  It is off by default because global compression already processes the mix.

Save applies the draft; Cancel discards it. Audio settings are associated with
a SHA-256 hash of the complete file bytes. An identical file inherits those
settings on playback or when its editor opens, even after a move, rename, or
application restart. Editing or re-encoding the file changes its identity. New
content replacing a previously identified file starts with default audio settings;
recognized content
restores its saved audio settings. The first identity check preserves legacy
path-based audio settings.
Names, tags, favorites, and artwork stay associated with their library paths.
Settings for known content remain available after it leaves the library.
For files edited in place, restart HonkHonk to discard cached audio before replaying.

## Silent channels and file formats

If one stereo channel is nearly silent throughout the clip and the other has
meaningful audio, HonkHonk copies the active channel into the silent channel
before measuring loudness. A notice explains the repair. Active stereo channels
are preserved, including intentionally imbalanced stereo. A truly silent lane
cannot be distinguished from an intentional hard pan; use the Pan control to
restore that placement if needed.

Symphonia decodes supported integer and floating-point file formats into
interleaved f32 PCM. That conversion does **not** resample the file. HonkHonk
declares F32LE and the decoded native sample rate to PipeWire. PipeWire's stream
adapter handles device sample-format/rate conversion and channel mapping, so
no extra resampler or system dependency is added here. Preserve keeps all source
channels and the existing backend mapping. Force stereo always outputs two
channels: mono is duplicated, stereo stays intact, and sources with more than
two channels use the arithmetic mean of all channels duplicated left and right.
Speaker positions are unavailable, so this fallback does not assume a surround
layout and loses spatial separation. Force mono uses the same all-channel mean.
Opposite-polarity channels may cancel when averaged.

The current engine shares one active stream format across concurrent voices.
Starting a sound whose rate or output channel count differs interrupts existing
voices, even in Concurrent mode. Export clips to a common rate and channel count
for overlap without this fallback. A new stream's native rate is then negotiated
by PipeWire; a single file need not match the device's rate.

Bulk import's optional peak normalization modifies the copied file. Playback
normalization measures that resulting audio and applies one playback gain; it
does not repeat import trimming or overwrite the source. Measurement and hashing
run on background workers, and the cache retains reusable PCM.

Playback failures show the technical reason and a remedy. Check that files are
readable and fully downloaded; re-export damaged or unsupported files as PCM
WAV. If PipeWire reports a stream error, check that its service and the selected
output device are available.

## Technical references

- [EBU R128 recommendation](https://tech.ebu.ch/publications/r128)
- [ebur128 measurement API](https://docs.rs/ebur128/0.1.10/ebur128/struct.EbuR128.html)
- [PipeWire adapter format, rate and channel conversion](https://docs.pipewire.org/devel/page_man_pipewire-props_7.html)
- [Symphonia conversion and decoder source](https://docs.rs/crate/symphonia/0.5.5/source/)

Channel routing retains the original shared decoded samples. Playback converts
only complete frames requested by each output buffer, without allocating another
clip-sized buffer on the PipeWire loop. Progress follows consumed source frames,
so forcing mono or expanding mono to stereo does not change duration. Sink and
monitor maintain independent cursors with the same routing and processing.
