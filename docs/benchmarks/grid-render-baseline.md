# Sound-Grid Render Baseline (ADR-009)

ADR-009 requires a tile-grid render baseline at 50 / 200 / 500 tiles on both
renderers before any canvas/visual tile work (#13, #92) resumes. This is that
baseline and the gate procedure. It is the regression gate that would have
caught PR #96 (per-frame canvas `Cache` rebuild → grid scroll stutter) before
merge.

## What is measured

`benches/grid_render.rs` drives the **production** `sound_grid::view_grid()`
through Iced 0.14's headless render path, per iteration:

1. `view_grid(..)` — constructs the real `Element` tree (the per-frame work that
   PR #96 regressed by rebuilding a `canvas::Cache` every `view()`).
2. `UserInterface::build` — the **layout** pass.
3. `UserInterface::draw` — the **draw** pass (primitive / tessellation
   generation into the renderer's layer stack).
4. Rasterization:
   - **tiny-skia:** into a 1200×900 CPU `Pixmap` (the `HONKHONK_RENDERER=software`
     path). Fully exercised on every host, including headless CI.
   - **wgpu:** `Renderer::present` to an offscreen 1200×900 texture on a headless
     device (no swapchain), reusing one render target across iterations as a real
     frame loop does. Skipped (with a printed note) when no GPU adapter is
     available, so the harness still runs on GPU-less CI.

It does **not** measure compositor/windowing, swapchain present-to-screen,
vsync, GPU fence wait after submission, or input handling — none of which the
ADR-009 regression concern (per-frame CPU cost of rebuilding the grid) depends
on. The wgpu number measures up to queue submission, which is what the real
frame loop spends on the CPU side.

## Baseline numbers

Host: AMD Ryzen 9 5900X · NVIDIA GeForce RTX 3080 Ti · Linux 6.18 (Manjaro) ·
rustc 1.95.0 · Criterion `bench` profile (release + LTO).
Date: 2026-06-22

| Tiles | tiny-skia (median) | wgpu (median) |
|------:|-------------------:|--------------:|
|    50 |           2.224 ms |      906.6 µs |
|   200 |           3.920 ms |       3.348 ms |
|   500 |           8.274 ms |       8.091 ms |

(Numbers are per full layout + draw + raster iteration. Absolute values are
host-dependent; the gate below compares same-host before/after, not across
hosts. The monotonic growth with tile count confirms the harness measures real
per-tile work rather than a stub.)

## Reproduce

```bash
# tiny-skia is always available; wgpu needs a local GPU adapter.
cargo bench --bench grid_render

# Force the software (tiny-skia) group only:
cargo bench --bench grid_render -- grid_render_tiny_skia
```

## Use as the #13 / #92 regression gate

Before starting canvas/visual tile work, record a pre-change baseline on your
host:

```bash
git switch main
cargo bench --bench grid_render -- --save-baseline pre-change
```

After the change, on the **same host**, compare against it:

```bash
cargo bench --bench grid_render -- --baseline pre-change
```

Criterion prints the delta per (renderer, size). Treat a statistically
significant regression at 200 or 500 tiles on **either** renderer as a blocker —
that is exactly the PR #96 failure mode (scrolling stutter from a per-frame grid
rebuild) this gate exists to catch. Investigate before merging.

> The `target/criterion/` raw data is host-specific and git-ignored; the
> committed artifact is the table above, not the Criterion JSON.
