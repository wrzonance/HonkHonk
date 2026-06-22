//! Render baseline bench for the sound grid (ADR-009).
//!
//! Measures the production `sound_grid::view_grid()` through Iced's headless
//! render path (layout + draw + rasterize) at 50 / 200 / 500 tiles on both the
//! tiny-skia and wgpu renderers. See `docs/benchmarks/grid-render-baseline.md`.

mod support;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use support::{
    init_wgpu, make_sounds, render_tiny_skia, self_check, sound_refs, try_render_wgpu, GridFixture,
};

/// Tile counts ADR-009 anchors the baseline against.
const SIZES: [usize; 3] = [50, 200, 500];
/// Default grid width (matches `Density::Regular` / 5-column layout).
const COLUMNS: usize = 5;

/// tiny-skia (software) baseline — always available, including on headless CI.
fn bench_tiny_skia(c: &mut Criterion) {
    self_check();
    let mut group = c.benchmark_group("grid_render_tiny_skia");
    for &n in &SIZES {
        let sounds = make_sounds(n);
        let fx = GridFixture::new();
        let refs = sound_refs(&sounds);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| render_tiny_skia(&refs, fx.grid_ctx(COLUMNS)));
        });
    }
    group.finish();
}

/// wgpu (GPU) baseline — skipped with a printed note when no adapter exists.
fn bench_wgpu(c: &mut Criterion) {
    let Some(gpu) = init_wgpu() else {
        eprintln!(
            "grid_render_wgpu: no wgpu adapter available — skipping GPU baseline. \
             (Expected on headless CI; run locally on a GPU host to record numbers.)"
        );
        return;
    };
    let mut group = c.benchmark_group("grid_render_wgpu");
    for &n in &SIZES {
        let sounds = make_sounds(n);
        let fx = GridFixture::new();
        let refs = sound_refs(&sounds);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| try_render_wgpu(&refs, fx.grid_ctx(COLUMNS), &gpu));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_tiny_skia, bench_wgpu);
criterion_main!(benches);
