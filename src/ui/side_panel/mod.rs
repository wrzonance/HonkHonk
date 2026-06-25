//! Reusable side-panel framework: a right-edge drawer with a pull tab that slides
//! over the main view. The effects panel (#143) is the first consumer; future
//! settings panels reuse it. Animation lives in [`anim`], geometry (the #144 hook)
//! in [`geometry`], rendering in [`view`].

mod anim;
mod flourish;
mod flourish_view;
mod geometry;
mod view;

pub use anim::{PanelAnim, SLIDE_DURATION};
pub use flourish::{
    BURST_DURATION, BurstOrigin, BurstSource, FeatherParticle, PanelFlourish, PanelTransition,
    panel_burst_origin,
};
pub use flourish_view::view_panel_flourish;
pub use geometry::{PanelRect, panel_geometry};
pub use view::{SidePanelConfig, view_side_panel};
