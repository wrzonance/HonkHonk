# Search Bar Focus Bug Fix

## Status: Approved

## Problem

`view_search_bar()` in `src/ui/search_bar.rs` returns structurally different widget trees depending on whether `query` is empty:

- Empty → `container(input)`
- Non-empty → `stack![input, overlay]`

When the user types the first character, Iced's reconciler sees a tree structure change and treats the `text_input` as a new widget instance, resetting its focus state. The input loses focus after every first keystroke.

## Fix

Always emit `stack![input, second_layer]`. The second layer is:

- **Empty query:** an empty `row![]` — no hit area, no render cost
- **Non-empty query:** the existing overlay container with the ✕ clear button

The widget tree shape is now identical across all query states. Iced preserves the `text_input` identity and focus across re-renders.

Padding logic is unchanged: asymmetric right padding is applied only when `query` is non-empty (reserves visual space for the clear button). Padding changes do not affect widget identity.

## Scope

- **In scope:** fix focus loss on first keystroke
- **Out of scope:** esc-to-blur, click-outside-to-blur (already work via Iced default focus model)

## Files Changed

| File | Change |
|------|--------|
| `src/ui/search_bar.rs` | Remove early-return branch; always use `stack!`; pass empty `row![]` when query is empty |

## Testing

- `cargo test` — existing `search_changed_updates_query` and `search_filters_sounds` tests verify state correctness
- Manual smoke: type multiple characters without re-clicking; all characters should appear
- `cargo clippy -- -D warnings` and `cargo fmt -- --check` must pass
