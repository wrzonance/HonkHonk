# Renderer Selection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire `HONKHONK_RENDERER` env var and expose a "GPU acceleration" toggle in settings, persisted in `AppConfig`, so users can switch to software rendering for VMs/older hardware.

**Architecture:** Add a `Renderer` enum to `AppConfig`. On startup, `main.rs` reads a pure `effective_renderer()` function (env var → config → default) and sets `ICED_BACKEND` before `iced::application` runs. The settings toggle writes the config; the inline registry hint communicates "restart required."

**Tech Stack:** Rust, Iced 0.13, serde_json, existing settings registry pattern (`src/settings/mod.rs`).

---

## File Map

| File | Change |
|------|--------|
| `src/state/config.rs` | Add `Renderer` enum + `AppConfig.renderer` field |
| `src/main.rs` | Add `effective_renderer` fn + call before `iced::application` |
| `src/settings/mod.rs` | Add `SettingDef` for `SettingId::Renderer` |
| `src/ui/settings.rs` | Add `get_setting_value` + `setting_message` arms |
| `src/app.rs` | Add `Message::RendererChanged(Renderer)` + `update()` handler |

---

### Task 1: Investigate Iced 0.13 renderer selection API

**Files:**
- Modify: `src/main.rs` (add one comment documenting the finding)

The key question: does Iced 0.13 compiled with both `wgpu` and `tiny-skia` features respect an `ICED_BACKEND` env var for runtime renderer selection?

- [ ] **Step 1: Check if `ICED_BACKEND` is read by Iced**

```bash
grep -r "ICED_BACKEND" ~/.cargo/registry/src/ 2>/dev/null | grep "iced" | head -20
```

Also check:
```bash
grep -r "tiny.skia\|tiny_skia" ~/.cargo/registry/src/ 2>/dev/null | grep "iced_graphics\|iced_wgpu" | grep "env\|var\|backend" | head -20
```

- [ ] **Step 2: Record finding in `main.rs` as a comment**

If `ICED_BACKEND` is found in Iced source, add this comment block directly above where `effective_renderer` is called (you'll add the call in Task 3):

```rust
// Renderer selection: set ICED_BACKEND before iced::application so the
// Iced compositor picks up the preference. Both wgpu and tiny-skia are
// compiled in (see Cargo.toml features). ICED_BACKEND is read by
// iced_graphics::compositor during application init.
```

If `ICED_BACKEND` is NOT found, try `WGPU_BACKEND=gl` as an alternative (forces wgpu onto OpenGL, which may trigger tiny-skia fallback). Update the comment accordingly. If neither works, note that the config is saved for a future API and skip the `set_var` call in Task 3.

- [ ] **Step 3: Commit the investigation finding**

```bash
git add src/main.rs
git commit -m "docs(main): document Iced 0.13 renderer backend env var finding"
```

---

### Task 2: `Renderer` enum + `AppConfig.renderer` field

**Files:**
- Modify: `src/state/config.rs`

- [ ] **Step 1: Write failing tests**

Add at the bottom of the `#[cfg(test)]` block in `src/state/config.rs`:

```rust
#[test]
fn renderer_default_is_wgpu() {
    assert_eq!(AppConfig::default().renderer, Renderer::Wgpu);
}

#[test]
fn renderer_round_trips_json() {
    for (variant, expected_str) in [(Renderer::Wgpu, "\"wgpu\""), (Renderer::TinySkia, "\"tiny-skia\"")] {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, expected_str, "Renderer::{variant:?} serialized wrong");
        let back: Renderer = serde_json::from_str(&json).unwrap();
        assert_eq!(back, variant);
    }
}

#[test]
fn missing_renderer_field_deserializes_to_wgpu() {
    let json = r#"{"sound_directories":[],"volume":0.85,"window_width":900,"window_height":600,"theme":"Dark","density":"regular","mic_passthrough":true,"mic_passthrough_level":1.0}"#;
    let config: AppConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.renderer, Renderer::Wgpu);
}
```

- [ ] **Step 2: Run — expect compile failure (Renderer not defined yet)**

```bash
cargo test -q 2>&1 | head -20
```

Expected: `error[E0412]: cannot find type 'Renderer'`

- [ ] **Step 3: Add `Renderer` enum**

In `src/state/config.rs`, add after the `Density` impl block and before `fn default_true()`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Renderer {
    #[default]
    Wgpu,
    TinySkia,
}
```

- [ ] **Step 4: Add `renderer` field to `AppConfig` struct**

In the `AppConfig` struct, add after `mic_passthrough_level`:

```rust
#[serde(default)]
pub renderer: Renderer,
```

- [ ] **Step 5: Add `renderer` to `AppConfig::default()`**

In the `Default for AppConfig` impl, add `renderer: Renderer::Wgpu,` to the `Self { ... }` block.

- [ ] **Step 6: Run tests — expect pass**

```bash
cargo test -q 2>&1 | tail -5
```

Expected: all tests pass, zero failures.

- [ ] **Step 7: Commit**

```bash
git add src/state/config.rs
git commit -m "feat(state): add Renderer enum and AppConfig.renderer field (#73)"
```

---

### Task 3: `effective_renderer` pure function + wiring in `main.rs`

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Write failing tests**

Add a `#[cfg(test)]` block at the bottom of `src/main.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use honkhonk::state::config::Renderer;

    #[test]
    fn effective_renderer_software_env_overrides_wgpu_config() {
        assert_eq!(effective_renderer(Some("software"), Renderer::Wgpu), Renderer::TinySkia);
    }

    #[test]
    fn effective_renderer_tiny_skia_alias_works() {
        assert_eq!(effective_renderer(Some("tiny-skia"), Renderer::Wgpu), Renderer::TinySkia);
    }

    #[test]
    fn effective_renderer_wgpu_env_overrides_tiny_skia_config() {
        assert_eq!(effective_renderer(Some("wgpu"), Renderer::TinySkia), Renderer::Wgpu);
    }

    #[test]
    fn effective_renderer_no_env_uses_config() {
        assert_eq!(effective_renderer(None, Renderer::TinySkia), Renderer::TinySkia);
        assert_eq!(effective_renderer(None, Renderer::Wgpu), Renderer::Wgpu);
    }

    #[test]
    fn effective_renderer_unknown_env_falls_back_to_config() {
        assert_eq!(effective_renderer(Some("opengl"), Renderer::TinySkia), Renderer::TinySkia);
        assert_eq!(effective_renderer(Some(""), Renderer::Wgpu), Renderer::Wgpu);
    }
}
```

- [ ] **Step 2: Run — expect compile failure**

```bash
cargo test -q 2>&1 | head -10
```

Expected: `error[E0425]: cannot find function 'effective_renderer'`

- [ ] **Step 3: Add `effective_renderer` function**

Add before `fn main()` in `src/main.rs`:

```rust
use honkhonk::state::config::Renderer;

fn effective_renderer(env_val: Option<&str>, config_pref: Renderer) -> Renderer {
    match env_val {
        Some("software") | Some("tiny-skia") => Renderer::TinySkia,
        Some("wgpu") => Renderer::Wgpu,
        _ => config_pref,
    }
}
```

- [ ] **Step 4: Run tests — expect pass**

```bash
cargo test -q 2>&1 | tail -5
```

- [ ] **Step 5: Wire into `main()`**

In `fn main()`, after `AppConfig::load()` and before the `iced::application` call, add:

```rust
let renderer = effective_renderer(
    std::env::var("HONKHONK_RENDERER").ok().as_deref(),
    config.renderer,
);

// Apply renderer preference before Iced initialises its compositor.
// (See comment added in Task 1 for the env var rationale.)
if renderer == Renderer::TinySkia {
    // ICED_BACKEND is read by iced_graphics compositor during init.
    // If Task 1 investigation found a different var, update here.
    std::env::set_var("ICED_BACKEND", "tiny-skia");
}
```

Note: if Task 1 found that `ICED_BACKEND` does not work, omit the `set_var` block and add a comment: `// Runtime selection not available in Iced 0.13 — preference saved to config for future use.`

- [ ] **Step 6: Build to confirm no compile errors**

```bash
cargo build 2>&1 | grep -E "^error" | head -10
```

Expected: no errors.

- [ ] **Step 7: Commit**

```bash
git add src/main.rs
git commit -m "feat(main): wire effective_renderer with env var precedence (#73)"
```

---

### Task 4: Settings registry entry

**Files:**
- Modify: `src/settings/mod.rs`

- [ ] **Step 1: Write failing tests**

Add to the `#[cfg(test)]` block in `src/settings/mod.rs`:

```rust
#[test]
fn renderer_entry_exists_in_appearance_category() {
    let def = SETTINGS_REGISTRY
        .iter()
        .find(|d| matches!(d.id, SettingId::Renderer))
        .expect("Renderer must be in SETTINGS_REGISTRY");
    assert!(matches!(def.category, SettingCategory::Appearance));
}

#[test]
fn renderer_control_is_toggle() {
    let def = SETTINGS_REGISTRY
        .iter()
        .find(|d| matches!(d.id, SettingId::Renderer))
        .expect("Renderer must be in SETTINGS_REGISTRY");
    assert!(matches!(def.control, ControlType::Toggle));
}

#[test]
fn appearance_category_has_three_entries() {
    let count = SETTINGS_REGISTRY
        .iter()
        .filter(|d| matches!(d.category, SettingCategory::Appearance))
        .count();
    assert_eq!(count, 3, "Appearance must have Theme + Density + Renderer");
}
```

- [ ] **Step 2: Run — expect failure**

```bash
cargo test -q settings 2>&1 | tail -10
```

Expected: `appearance_category_has_three_entries` FAIL (currently 2 entries).

- [ ] **Step 3: Add registry entry**

In `SETTINGS_REGISTRY` in `src/settings/mod.rs`, add after the `Density` entry:

```rust
SettingDef {
    id: SettingId::Renderer,
    category: SettingCategory::Appearance,
    label: "GPU acceleration",
    hint: "Disable for VMs or older hardware. Takes effect after restart.",
    control: ControlType::Toggle,
},
```

- [ ] **Step 4: Run tests — expect pass**

```bash
cargo test -q settings 2>&1 | tail -5
```

- [ ] **Step 5: Commit**

```bash
git add src/settings/mod.rs
git commit -m "feat(settings): register Renderer toggle in Appearance category (#73)"
```

---

### Task 5: Message variant + `get_setting_value` + `setting_message` + `update()` handler

**Files:**
- Modify: `src/app.rs`
- Modify: `src/ui/settings.rs`

- [ ] **Step 1: Write failing test for the message variant**

Add to the `#[cfg(test)]` block in `src/app.rs`:

```rust
#[test]
fn renderer_changed_message_round_trips() {
    use crate::state::config::Renderer;
    let msg_wgpu = Message::RendererChanged(Renderer::Wgpu);
    assert!(matches!(msg_wgpu, Message::RendererChanged(Renderer::Wgpu)));
    let msg_tiny = Message::RendererChanged(Renderer::TinySkia);
    assert!(matches!(msg_tiny, Message::RendererChanged(Renderer::TinySkia)));
}

#[test]
fn renderer_changed_update_saves_to_config() {
    use crate::state::config::Renderer;
    let mut app = make_test_app();
    assert_eq!(app.config.renderer, Renderer::Wgpu); // default
    let _ = app.update(Message::RendererChanged(Renderer::TinySkia));
    assert_eq!(app.config.renderer, Renderer::TinySkia);
    let _ = app.update(Message::RendererChanged(Renderer::Wgpu));
    assert_eq!(app.config.renderer, Renderer::Wgpu);
}
```

- [ ] **Step 2: Run — expect compile failure**

```bash
cargo test -q 2>&1 | head -10
```

Expected: `error[E0599]: no variant named 'RendererChanged'`

- [ ] **Step 3: Add `Message::RendererChanged` variant**

In `src/app.rs`, in the `Message` enum under the `// Appearance` comment block, add after `DensityChanged`:

```rust
RendererChanged(crate::state::config::Renderer),
```

- [ ] **Step 4: Add `update()` handler**

In the `update()` match in `src/app.rs`, add after the `Message::DensityChanged` arm:

```rust
Message::RendererChanged(r) => {
    if self.config.renderer != r {
        self.config = AppConfig {
            renderer: r,
            ..self.config.clone()
        };
        if let Err(e) = self.config.save() {
            eprintln!("honkhonk: config save error: {e}");
        }
    }
    Task::none()
}
```

- [ ] **Step 5: Wire `get_setting_value` in `src/ui/settings.rs`**

Add to the `match id` in `get_setting_value`:

```rust
SettingId::Renderer => SettingValue::Bool(state.config.renderer == crate::state::config::Renderer::Wgpu),
```

- [ ] **Step 6: Wire `setting_message` in `src/ui/settings.rs`**

Add to the `match (id, value)` in `setting_message`:

```rust
(SettingId::Renderer, SettingValue::Bool(v)) => Message::RendererChanged(
    if v { crate::state::config::Renderer::Wgpu } else { crate::state::config::Renderer::TinySkia }
),
```

- [ ] **Step 7: Run tests — expect pass**

```bash
cargo test -q 2>&1 | tail -5
```

Expected: all tests pass.

- [ ] **Step 8: Run clippy**

```bash
cargo clippy -- -D warnings 2>&1 | grep -E "^error" | head -10
```

Expected: no errors.

- [ ] **Step 9: Commit**

```bash
git add src/app.rs src/ui/settings.rs
git commit -m "feat(app,ui): wire RendererChanged message and settings toggle (#73)"
```

---

### Task 6: Final verification + push

- [ ] **Step 1: Full test suite**

```bash
cargo test 2>&1 | tail -10
```

Expected: all tests pass.

- [ ] **Step 2: Full lint**

```bash
cargo clippy -- -D warnings && cargo fmt -- --check
```

Expected: no warnings, no formatting issues.

- [ ] **Step 3: Smoke test — build and verify toggle appears in settings**

```bash
cargo build 2>&1 | grep -E "^error"
```

Then run `cargo run` and open Settings → Appearance. Verify "GPU acceleration" toggle is present with the hint "Disable for VMs or older hardware. Takes effect after restart." Toggle it off, quit, reopen — verify the preference persists (config.json should show `"renderer": "tiny-skia"`).

- [ ] **Step 4: Push branch and open PR**

```bash
git push -u origin feat/renderer-selection
gh pr create \
  --title "feat(app,state,ui): renderer selection — GPU acceleration toggle (#73)" \
  --body "$(cat <<'EOF'
## Summary
- Adds `Renderer` enum (`Wgpu` | `TinySkia`) to `AppConfig`
- `HONKHONK_RENDERER=software` env var overrides config (precedence: env → config → default)
- Settings → Appearance: \"GPU acceleration\" toggle with \"Takes effect after restart.\" hint
- Preference persisted across restarts via config.json

## Test Plan
- [ ] `cargo test` passes
- [ ] `cargo clippy -- -D warnings` clean
- [ ] Settings → Appearance shows \"GPU acceleration\" toggle
- [ ] Toggle off → quit → reopen: config.json shows `\"renderer\": \"tiny-skia\"`
- [ ] `HONKHONK_RENDERER=software cargo run` overrides config
EOF
)"
```
