# Renderer Selection — Design Spec

**Issue:** #73  
**Branch:** feat/renderer-selection  
**Phase:** 2

## Goal

Wire the `HONKHONK_RENDERER` env var (documented but non-functional) and expose renderer selection in the settings panel as a "GPU acceleration" toggle, persisted in `AppConfig`.

## Out of Scope

- Any renderer backend beyond wgpu and tiny-skia
- Live renderer switching without restart
- Auto-fallback logic (user's explicit choice, not automatic)

## Data Model

### `Renderer` enum — `src/state/config.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Renderer {
    #[default]
    Wgpu,
    TinySkia,
}

impl Renderer {
    pub fn setting_index(self) -> usize {
        match self { Renderer::Wgpu => 0, Renderer::TinySkia => 1 }
    }
    pub fn from_setting_index(i: usize) -> Self {
        if i == 1 { Renderer::TinySkia } else { Renderer::Wgpu }
    }
}
```

### `AppConfig` field

```rust
#[serde(default)]
pub renderer: Renderer,
```

`Default` impl sets `renderer: Renderer::Wgpu`.

## Precedence Logic

Evaluated in `main.rs` before `iced::application`:

```
HONKHONK_RENDERER=software   →  TinySkia
HONKHONK_RENDERER=tiny-skia  →  TinySkia
HONKHONK_RENDERER=wgpu       →  Wgpu
(absent or unrecognized)      →  AppConfig.renderer
```

Implemented as a pure function that takes `Option<&str>` (not `std::env::var` directly) so it is unit-testable without env mutation:

```rust
pub fn effective_renderer(env_val: Option<&str>, config_pref: Renderer) -> Renderer {
    match env_val {
        Some("software") | Some("tiny-skia") => Renderer::TinySkia,
        Some("wgpu")                          => Renderer::Wgpu,
        _                                     => config_pref,
    }
}
```

Called in `main.rs`:

```rust
let renderer = effective_renderer(
    std::env::var("HONKHONK_RENDERER").ok().as_deref(),
    config.renderer,
);
```

## Iced 0.13 Renderer Wiring (Investigation Task)

This is the first implementation task. Two possible outcomes:

**Path 1 — Iced exposes a runtime API:** Wire `renderer` through the `iced::application` builder or `iced::Settings`. Prefer this if available.

**Path 2 — No runtime API (expected):** Before `iced::application`, call:
```rust
if renderer == Renderer::TinySkia {
    // Tell wgpu compositor to use software backend
    std::env::set_var("WGPU_BACKEND", "gl");  // or equivalent
}
```
Exact env var must be verified against Iced 0.13 / wgpu internals during investigation. Document in a code comment with the reason.

If neither path is viable, the `Renderer` enum and config field still ship; the toggle saves the preference for a future release when the API is available. The settings hint already communicates "restart required" so the deferred-wiring case is not user-visible.

## Settings UI

### Registry entry — `src/settings/mod.rs`

```rust
SettingDef {
    id: SettingId::Renderer,
    category: SettingCategory::Appearance,
    label: "GPU acceleration",
    hint: "Disable for VMs or older hardware. Takes effect after restart.",
    control: ControlType::Toggle,
},
```

### `get_setting_value` arm — `src/ui/settings.rs`

```rust
SettingId::Renderer => SettingValue::Bool(state.config.renderer == Renderer::Wgpu),
```

### `setting_message` arm — `src/ui/settings.rs`

```rust
(SettingId::Renderer, SettingValue::Bool(v)) =>
    Message::RendererChanged(if v { Renderer::Wgpu } else { Renderer::TinySkia }),
```

### `Message` variant — `src/app.rs`

```rust
RendererChanged(Renderer),
```

### `update()` handler — `src/app.rs`

Immutable pattern — produces new config, saves to disk:

```rust
Message::RendererChanged(r) => {
    self.config = AppConfig { renderer: r, ..self.config.clone() };
    if let Err(e) = self.config.save() {
        eprintln!("warning: failed to save config: {e}");
    }
    Task::none()
}
```

No live renderer switch. No restart triggered programmatically. The registry `hint` field ("Takes effect after restart.") is always visible beneath the toggle — no extra UI state required.

## Error Handling

- Unrecognized `HONKHONK_RENDERER` value → falls back to config (no warning; treated as absent)
- Config save failure in handler → `eprintln!` warning only, consistent with other settings handlers

## Tests

### `src/state/config.rs`

| Test | Asserts |
|------|---------|
| `renderer_default_is_wgpu` | `AppConfig::default().renderer == Renderer::Wgpu` |
| `renderer_round_trips_json` | both variants serialize/deserialize correctly |
| `missing_renderer_field_deserializes_to_wgpu` | old config JSON without `renderer` key loads as `Wgpu` |

### `src/settings/mod.rs`

| Test | Asserts |
|------|---------|
| `renderer_entry_exists_in_appearance_category` | `SettingId::Renderer` in registry with `SettingCategory::Appearance` |
| `renderer_control_is_toggle` | `ControlType::Toggle` |

### `src/main.rs` (or `src/lib.rs`)

| Test | Asserts |
|------|---------|
| `effective_renderer_software_overrides_wgpu_config` | env `"software"` + config `Wgpu` → `TinySkia` |
| `effective_renderer_tiny_skia_alias_works` | env `"tiny-skia"` → `TinySkia` |
| `effective_renderer_wgpu_env_overrides_tiny_skia_config` | env `"wgpu"` + config `TinySkia` → `Wgpu` |
| `effective_renderer_no_env_uses_config` | env absent → returns config value |
| `effective_renderer_unknown_env_falls_back_to_config` | env `"opengl"` + config `TinySkia` → `TinySkia` |

## Files Changed

| File | Change |
|------|--------|
| `src/state/config.rs` | Add `Renderer` enum + `AppConfig.renderer` field + `Default` update |
| `src/settings/mod.rs` | Add `SettingDef` for `Renderer`; update `audio_category_has_two_entries` test → audio still 2, appearance grows |
| `src/ui/settings.rs` | Add `get_setting_value` arm + `setting_message` arm |
| `src/app.rs` | Add `Message::RendererChanged(Renderer)` + `update()` handler |
| `src/main.rs` | Add `effective_renderer` fn + call before `iced::application`; wire result |
