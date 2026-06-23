//! Reusable side-panel framework: a right-edge drawer with a pull tab that slides
//! over the main view. The effects panel (#143) is the first consumer; future
//! settings panels reuse it. Animation lives in [`anim`], geometry (the #144 hook)
//! in [`geometry`], rendering in [`view`].

mod anim;

pub use anim::{PanelAnim, SLIDE_DURATION};
