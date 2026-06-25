use iced::Size;

use crate::ui::theme;

pub const TILE_INSET: f32 = 6.0;
const MOTION_BLEED: f32 = 18.0;
const MAX_ROTATION_DEGREES: f32 = 3.0;
const MOTION_SCALE_RESERVE: f32 = 1.05;

pub fn tile_slot_height() -> f32 {
    theme::component::SOUND_TILE_H + MOTION_BLEED * 2.0
}

pub fn fitted_tile_size(slot: Size) -> Size {
    let height = visible_tile_height().min((slot.height - TILE_INSET * 2.0).max(0.0));
    let width = (slot.width - TILE_INSET * 2.0)
        .min(max_width_inside_slot(slot, height))
        .max(0.0);
    Size::new(width, height)
}

fn visible_tile_height() -> f32 {
    (theme::component::SOUND_TILE_H - TILE_INSET * 2.0).max(0.0)
}

fn max_width_inside_slot(slot: Size, height: f32) -> f32 {
    let radians = MAX_ROTATION_DEGREES.to_radians();
    let sin = radians.sin();
    let cos = radians.cos();
    let scaled_height = height * MOTION_SCALE_RESERVE;

    let by_width = if cos <= f32::EPSILON {
        slot.width
    } else {
        (slot.width - scaled_height * sin) / (cos * MOTION_SCALE_RESERVE)
    };
    let by_height = if sin <= f32::EPSILON {
        slot.width
    } else {
        (slot.height - scaled_height * cos) / (sin * MOTION_SCALE_RESERVE)
    };

    by_width.min(by_height)
}

#[cfg(test)]
fn rotated_bounds(size: Size, degrees: f32) -> Size {
    let radians = degrees.abs().to_radians();
    let sin = radians.sin();
    let cos = radians.cos();

    Size::new(
        size.width * cos + size.height * sin,
        size.width * sin + size.height * cos,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 0.01;

    #[test]
    fn tile_slot_height_reserves_motion_bleed() {
        assert!(tile_slot_height() > theme::component::SOUND_TILE_H);
    }

    #[test]
    fn fitted_tile_size_keeps_rotated_bounds_inside_slot() {
        for width in [120.0, 360.0, 720.0] {
            let slot = Size::new(width, tile_slot_height());
            let tile = fitted_tile_size(slot);
            let animated_tile = Size::new(
                tile.width * MOTION_SCALE_RESERVE,
                tile.height * MOTION_SCALE_RESERVE,
            );
            let bounds = rotated_bounds(animated_tile, MAX_ROTATION_DEGREES);

            assert!(
                bounds.width <= slot.width + EPSILON,
                "rotated width {} exceeded slot width {}",
                bounds.width,
                slot.width
            );
            assert!(
                bounds.height <= slot.height + EPSILON,
                "rotated height {} exceeded slot height {}",
                bounds.height,
                slot.height
            );
        }
    }

    #[test]
    fn fitted_tile_size_keeps_visible_tile_height_stable() {
        let slot = Size::new(360.0, tile_slot_height());
        let tile = fitted_tile_size(slot);

        assert_eq!(tile.height, visible_tile_height());
    }
}
