//! Render baseline bench for the sound grid (ADR-009).
//!
//! Measures the production `sound_grid::view_grid()` through Iced's headless
//! render path (layout + draw + rasterize) at 50 / 200 / 500 tiles on both the
//! tiny-skia and wgpu renderers. See `docs/benchmarks/grid-render-baseline.md`.

mod support;

use criterion::{criterion_group, criterion_main, Criterion};

fn placeholder(c: &mut Criterion) {
    c.bench_function("placeholder", |b| b.iter(|| 1 + 1));
}

criterion_group!(benches, placeholder);
criterion_main!(benches);
