//! Bench support: deterministic fixtures and headless render drivers.
//!
//! The drivers run the *production* `sound_grid::view_grid()` through Iced
//! 0.14's headless render path:
//!   1. `view_grid(..)` builds the real `Element` tree.
//!   2. `UserInterface::build` runs the **layout** pass.
//!   3. `UserInterface::draw` runs the **draw** pass (primitive generation).
//!   4. The renderer rasterizes — tiny-skia into a CPU `Pixmap` (always
//!      available), wgpu into an offscreen texture (when an adapter exists).
//!
//! This is exactly the per-frame work ADR-009 anchors a baseline against: PR
//! #96 regressed it by rebuilding a canvas `Cache` on every `view()`.

use std::path::PathBuf;

use iced::{Color, Element, Font, Pixels, Point, Rectangle, Size, Theme};
use iced_core::mouse;
use iced_core::renderer::Style;
use iced_graphics::{Shell, Viewport};
use iced_runtime::user_interface::{Cache, UserInterface};

use honkhonk::app::Message;
use honkhonk::state::{AudioFormat, SlotMap, SoundEntry, SoundMetaStore};
use honkhonk::ui::sound_grid::{GridCtx, view_grid};

const CATEGORIES: &[&str] = &["Honk", "Memes", "Reactions", "Voicelines", "Music", "SFX"];

/// Logical viewport the grid is laid out and rasterized into.
const VIEW_W: u32 = 1200;
const VIEW_H: u32 = 900;
const SCALE: f32 = 1.0;
const DEFAULT_TEXT_SIZE: f32 = 14.0;

/// HonkHonk dark-theme background (`#171410`), opaque. Matches the real window
/// clear color so rasterization does representative blending.
const BG: Color = Color {
    r: 0.090,
    g: 0.078,
    b: 0.063,
    a: 1.0,
};

// ───────────────────────────── Fixtures ─────────────────────────────

/// Builds `n` deterministic sound entries. Ids are 16 hex chars so the
/// production `Tone::from_index(u64::from_str_radix(&id[..8], 16))` path in
/// `tile_view` runs exactly as it does at runtime.
pub fn make_sounds(n: usize) -> Vec<SoundEntry> {
    (0..n)
        .map(|i| {
            let category = CATEGORIES[i % CATEGORIES.len()];
            SoundEntry {
                id: format!("{:016x}", (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)),
                name: format!("{category} sound number {i}"),
                path: PathBuf::from(format!("/sounds/{category}/sound_{i}.ogg")),
                format: AudioFormat::Ogg,
                duration_ms: if i % 2 == 0 {
                    Some(1_000 + (i as u64) * 37)
                } else {
                    None
                },
                category: category.to_owned(),
            }
        })
        .collect()
}

/// Borrows entries into the `&[&SoundEntry]` slice `view_grid` expects.
pub fn sound_refs(sounds: &[SoundEntry]) -> Vec<&SoundEntry> {
    sounds.iter().collect()
}

/// Owns the per-grid context state (`SlotMap`, trigger labels, meta store) so a
/// `GridCtx` can borrow from it for the duration of a bench iteration. Empty /
/// default state represents the common case (no slots bound, no favorites).
pub struct GridFixture {
    slots: SlotMap,
    triggers: [Option<String>; 20],
    sound_meta: SoundMetaStore,
}

impl GridFixture {
    pub fn new() -> Self {
        Self {
            slots: SlotMap::default(),
            triggers: std::array::from_fn(|_| None),
            sound_meta: SoundMetaStore::default(),
        }
    }

    pub fn grid_ctx(&self, columns: usize) -> GridCtx<'_> {
        GridCtx {
            slots: &self.slots,
            triggers: &self.triggers,
            shortcuts_active: false,
            columns,
            sound_meta: &self.sound_meta,
        }
    }
}

impl Default for GridFixture {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────── Renderer-agnostic core ────────────────────────

/// Builds the grid `Element` and runs Iced's layout + draw passes against the
/// provided renderer. This is the `view()`-construction + tessellation work
/// ADR-009 cares about. Rasterization is renderer-specific (done by callers).
fn layout_and_draw(sounds: &[&SoundEntry], grid: GridCtx, renderer: &mut iced::Renderer) {
    let element: Element<'_, Message> = view_grid(sounds.to_vec(), None, grid);
    let bounds = Size::new(VIEW_W as f32, VIEW_H as f32);
    let mut ui = UserInterface::build(element, bounds, Cache::new(), renderer);
    let theme = Theme::Dark;
    let style = Style {
        text_color: Color::WHITE,
    };
    ui.draw(renderer, &theme, &style, mouse::Cursor::Unavailable);
}

fn full_viewport() -> Viewport {
    Viewport::with_physical_size(Size::new(VIEW_W, VIEW_H), SCALE)
}

fn full_damage() -> [Rectangle; 1] {
    [Rectangle::new(
        Point::ORIGIN,
        Size::new(VIEW_W as f32, VIEW_H as f32),
    )]
}

// ─────────────────────────── tiny-skia driver ───────────────────────────

/// Full tiny-skia render: layout + draw + CPU rasterization into a `Pixmap`.
/// This is the `HONKHONK_RENDERER=software` path. Returns the top-left pixel so
/// the optimizer cannot elide the raster. Always available (pure CPU).
pub fn render_tiny_skia(sounds: &[&SoundEntry], grid: GridCtx) -> u32 {
    // The `Element` is generic over `iced::Renderer` (the fallback enum); its
    // `Secondary` arm *is* the tiny-skia renderer, so draw lands in its layers.
    let mut renderer =
        iced::Renderer::Secondary(iced_tiny_skia::Renderer::new(Font::DEFAULT, text_size()));
    layout_and_draw(sounds, grid, &mut renderer);

    let iced::Renderer::Secondary(ts) = &mut renderer else {
        unreachable!("constructed Secondary");
    };

    let mut pixmap = tiny_skia::Pixmap::new(VIEW_W, VIEW_H).expect("alloc pixmap");
    let mut mask = tiny_skia::Mask::new(VIEW_W, VIEW_H).expect("alloc mask");
    ts.draw(
        &mut pixmap.as_mut(),
        &mut mask,
        &full_viewport(),
        &full_damage(),
        BG,
    );

    let px = pixmap.data();
    u32::from_le_bytes([px[0], px[1], px[2], px[3]])
}

// ───────────────────────────── wgpu driver ─────────────────────────────

/// Headless wgpu context: an `iced_wgpu::Engine` (which owns the device/queue),
/// the target format, and a persistent offscreen render target. `None` when no
/// adapter exists (e.g. CI without a GPU) so the bench skips the wgpu group
/// cleanly. The target texture/view is created once and reused every iteration,
/// mirroring how a real frame loop reuses its render target — so the bench
/// measures steady-state per-frame render, not per-iteration texture allocation.
pub struct WgpuCtx {
    engine: iced_wgpu::Engine,
    format: iced_wgpu::wgpu::TextureFormat,
    // `_texture` owns the GPU allocation `view` points into; held for its
    // lifetime even though only `view` is referenced after construction.
    _texture: iced_wgpu::wgpu::Texture,
    view: iced_wgpu::wgpu::TextureView,
}

/// Attempts to create a headless wgpu device + engine + reusable render target.
/// Returns `None` if no adapter is available so callers can skip rather than
/// panic.
pub fn init_wgpu() -> Option<WgpuCtx> {
    use iced_wgpu::wgpu;

    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .ok()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("honkhonk-grid-bench"),
        ..Default::default()
    }))
    .ok()?;

    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("honkhonk-grid-bench-target"),
        size: wgpu::Extent3d {
            width: VIEW_W,
            height: VIEW_H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let engine = iced_wgpu::Engine::new(
        &adapter,
        device.clone(),
        queue,
        format,
        None,
        Shell::headless(),
    );
    Some(WgpuCtx {
        engine,
        format,
        _texture: texture,
        view,
    })
}

/// Full wgpu render: layout + draw + present to the reusable offscreen target.
/// Returns a token so the work cannot be optimized away. Requires an
/// initialized context.
pub fn try_render_wgpu(sounds: &[&SoundEntry], grid: GridCtx, gpu: &WgpuCtx) -> u32 {
    let mut renderer = iced::Renderer::Primary(iced_wgpu::Renderer::new(
        gpu.engine.clone(),
        Font::DEFAULT,
        text_size(),
    ));
    layout_and_draw(sounds, grid, &mut renderer);

    let iced::Renderer::Primary(wr) = &mut renderer else {
        unreachable!("constructed Primary");
    };

    // `present` clears the target with `BG` and re-renders each call, so reusing
    // one texture across iterations is correct. It builds the encoder, submits
    // to the engine's queue, and recalls the staging belt — the full GPU draw
    // path minus swapchain present.
    let _submission = wr.present(Some(BG), gpu.format, &gpu.view, &full_viewport());
    1
}

fn text_size() -> Pixels {
    Pixels(DEFAULT_TEXT_SIZE)
}

/// Self-check run once at bench startup (see `grid_render::main`). A
/// `harness = false` Criterion bench cannot host `#[test]`s — the bench binary
/// runs `main`, not libtest — so the invariants the production code depends on
/// are asserted here and exercised every time `cargo bench` runs. It verifies
/// that every fixture id parses as the hex `Tone` index `tile_view` derives, and
/// that a full tiny-skia layout+draw+raster cycle over the real grid completes.
/// A panic here fails the bench loudly rather than silently measuring nothing.
pub fn self_check() {
    for s in make_sounds(500) {
        let head = s.id.get(..8).expect("fixture id is at least 8 chars");
        u64::from_str_radix(head, 16).expect("fixture id head parses as hex");
    }
    let sounds = make_sounds(50);
    let refs = sound_refs(&sounds);
    let fx = GridFixture::new();
    let _ = render_tiny_skia(&refs, fx.grid_ctx(5));
}
