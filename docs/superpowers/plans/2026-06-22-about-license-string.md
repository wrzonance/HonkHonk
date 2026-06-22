# Fix About-screen License String Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:test-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the About screen render the project's real license (`MIT`), sourced from `Cargo.toml` so it can never drift again.

**Architecture:** Extract the displayed license into a module-level `const LICENSE: &str = env!("CARGO_PKG_LICENSE")` in `src/ui/settings.rs`. The Iced view uses that const instead of the hardcoded `"GPL-3.0-or-later"` literal. A unit test asserts `LICENSE == "MIT"`, pinning the value to `Cargo.toml`'s `license = "MIT"` field and preventing future drift.

**Tech Stack:** Rust, Iced 0.14, `env!` compile-time macro.

## Global Constraints

- File size: 400 lines max (`src/ui/settings.rs` is already 954 — a pre-existing violation; do NOT grow its line count meaningfully, add only the minimal const + a small test module).
- Functions <= 50 lines; clippy `-D warnings` must pass; `cargo fmt -- --check` clean.
- No `.unwrap()` / `panic!()` in non-test code.
- `Cargo.toml` sets `license = "MIT"` (the `license` field, NOT `license-file`), so `env!("CARGO_PKG_LICENSE")` resolves to `"MIT"` at compile time. Verified.
- TDD mandatory: failing test first; pin the bugfix with a regression test.

---

### Task 1: Source the About-screen license from Cargo.toml and pin it

**Files:**
- Modify: `src/ui/settings.rs` — add `const LICENSE` near the existing `const VERSION` usage (`view_about_section`, ~line 879-918); replace the hardcoded `"GPL-3.0-or-later"` literal at line 901.
- Test: `src/ui/settings.rs` — new `#[cfg(test)] mod tests` block at end of file.

**Interfaces:**
- Produces: `const LICENSE: &str = env!("CARGO_PKG_LICENSE");` (module-private), consumed by `view_about_section` and the test.

- [ ] **Step 1: Write the failing test**

Add at end of `src/ui/settings.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The About screen must show the project's real license. It is sourced
    /// from Cargo.toml via `env!("CARGO_PKG_LICENSE")`, so this guards against
    /// drift between the binary's displayed license and `license = "MIT"`.
    #[test]
    fn about_license_matches_cargo_manifest() {
        assert_eq!(LICENSE, "MIT");
        assert_eq!(LICENSE, env!("CARGO_PKG_LICENSE"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib ui::settings::tests::about_license_matches_cargo_manifest`
Expected: FAIL to compile — `LICENSE` not found in this scope (const not yet introduced).

- [ ] **Step 3: Write minimal implementation**

In `view_about_section`, alongside `const VERSION: &str = env!("CARGO_PKG_VERSION");`, add:

```rust
    const LICENSE: &str = env!("CARGO_PKG_LICENSE");
```

Then change the license container's text from the hardcoded literal:

```rust
    let license = container(
        text(LICENSE)
```

(was `text("GPL-3.0-or-later")`).

Because the test references `LICENSE` via `use super::*`, the const must be visible at module scope — `view_about_section`'s inner `const` is not. Hoist it to module scope: define `const LICENSE: &str = env!("CARGO_PKG_LICENSE");` at the top of the file (near other module items) and reference it from the view. Remove the inner `const`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib ui::settings::tests::about_license_matches_cargo_manifest`
Expected: PASS.

- [ ] **Step 5: Verify formatting, lints, full test suite**

Run: `cargo fmt -- --check && cargo clippy -- -D warnings && cargo test`
Expected: all clean / green.

- [ ] **Step 6: Commit**

```bash
git add src/ui/settings.rs docs/superpowers/plans/2026-06-22-about-license-string.md
git commit
```

Conventional Commits message `fix(ui): source About-screen license from Cargo.toml (MIT)`, crediting the agent.
