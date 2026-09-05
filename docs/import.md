# Bulk import and Wayland file drops

Open **Import**, choose a folder through the FileChooser portal, enter a folder
path, or drop files/folders onto the existing window. Scanning and audio analysis
run off the UI thread. Selecting another folder or cancelling invalidates pending
results. Neither scanning nor cancellation changes the library or source files.
Only one scan worker runs at a time, including across closing and reopening the
review. Drops arriving during a scan accumulate into one pending source set;
the current worker is cancelled and its completion starts the latest scan.
The source set accepts at most 1,000 distinct paths. Additional drops display a
limit warning; choosing a replacement folder or closing the review resets it.

The review lists filename-derived names, parent-folder categories, duration and
source size. Select/exclude individual files, edit names/categories, cycle a row's
color, or apply category/color/normalization/trimming to selected rows. Preview
plays the proposed processing through the existing audio engine. Stop preview or
close the review to stop it. Invalid audio remains visible with its error and is
excluded. Confirmation failures include the source path and remain available to
retry; successfully imported rows are removed from the draft.

Confirmation creates copies under `$XDG_DATA_HOME/honkhonk/imported/<category>`
(the normal XDG fallback applies). Filename allocation uses `create_new`, so a
collision, including an existing symlink, cannot overwrite another file. Category
names cannot escape the destination; existing category symlinks are rejected.
Unprocessed copies preserve source bytes. Processed copies are float32 WAVs;
normalization sets the absolute sample peak to 0.9, and trimming removes frames
outside the first/last frame with an absolute sample above 0.001. Entirely silent
files keep their duration. Originals are never rewritten. Names and colors use
the existing sound metadata store; category folders survive ordinary rescans.

Warnings flag peaks at or above 0.98, generic names, and at least 100 ms of leading
silence. These are sample-peak heuristics, not perceptual loudness measurements.
Limits are 1,000 sounds / 10,000 visited filesystem entries per scan, 64 MiB per
source file, and 12 million decoded samples per file. Limits produce visible
errors. Decode buffers are not retained across the entire review.

## Backend dependency provenance

Iced 0.14 already converts winit's `DroppedFile` events into its `FileDropped`
event, but winit 0.30.13 does not implement their Wayland producer. The local
Cargo patch retains that exact winit release and adds only a Wayland receiver.
It uses the existing connection, surfaces, per-seat state, SCTK data devices and
calloop loop. It creates no auxiliary window, foreign-display backend, compositor
extension, or X11 fallback, and adds no system dependency.

The unchanged dependency snapshot is the published crate:

- Source: <https://static.crates.io/crates/winit/winit-0.30.13.crate>
- SHA256: `a6755fa58a9f8350bd1e472d4c3fcc25f824ec358933bba33306d0b63df5978d`
- The original Apache-2.0 license and upstream attribution remain in `vendor/winit`.
- Existing files changed: `src/platform_impl/linux/wayland/state.rs` and
  `src/platform_impl/linux/wayland/seat/mod.rs`.
- Added files: `src/platform_impl/linux/wayland/seat/data_device.rs` and
  `src/platform_impl/linux/wayland/seat/drop_transfer.rs`.

Compare those files with the checksum-verified archive to reconstruct the
authored patch independently of the unchanged dependency snapshot. The root
`Cargo.lock` selects the path dependency. Flatpak's generated source list omits
the superseded registry archive/checksum pair and uses the source-tree patch.

The receiver accepts only local `file://` URIs from `text/uri-list`, negotiates
copy semantics, and reads a nonblocking socket endpoint passed through the
standard data-offer FD mechanism. Each callback reads at most 32 KiB, then yields
for 10 ms. Transfers time out after five seconds, accept at most 1 MiB / 1,000
paths, and stop if the seat or target window disappears. URI decoding preserves
non-UTF-8 Unix paths and rejects remote authorities, malformed percent escapes,
NUL/control bytes, query strings and fragments.

Primary integration references:

- [Iced 0.14 event conversion](https://github.com/iced-rs/iced/blob/0.14.0/winit/src/conversion.rs)
- [winit 0.30.13 state](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/linux/wayland/state.rs)
- [SCTK 0.19.2 data-device example](https://github.com/Smithay/client-toolkit/blob/v0.19.2/examples/data_device.rs)

## Verification

The root suite exercises real temporary audio fixtures, processing/copy
invariants, metadata persistence, app message transitions, and the actual
authored backend URI/transfer module through `tests/import_drop.rs`. Root builds
compile the patched Wayland producer. No Iced rendering tests were added.

Interactive drops have not been exercised against a live compositor in this
session. Before release, test folder drops from Dolphin on KDE, Files on GNOME,
and a file manager under Sway/Hyprland, including fast drops, multiple files,
cancelled scans, source removal, and closing a window during transfer. Confirm
clipboard behavior is unchanged. Flatpak packaging has not been built here.

The optional standalone upstream winit test suite could not download an
uncached dev-dependency into the environment's read-only Cargo home. Its full
format check also reports unrelated pristine upstream formatting; those files
are deliberately retained byte-for-byte. Neither check is reported as passed.
