use iced::Size;

use crate::ui::theme;

const TILE_INSET: f32 = 6.0;
// Covers a four-column grid near a 2560px-wide window; wider cells shrink the
// drawn tile instead of increasing row height or drawing into neighboring rows.
const ROTATION_CLEARANCE_WIDTH: f32 = 600.0;
pub const MAX_ROTATION_DEGREES: f32 = 3.0;

pub fn tile_slot_height() -> f32 {
    theme::component::SOUND_TILE_H.max(
        rotated_bounds(
            Size::new(ROTATION_CLEARANCE_WIDTH, visible_tile_height()),
            MAX_ROTATION_DEGREES,
        )
        .height
        .ceil(),
    )
}

pub fn fitted_tile_size(slot: Size) -> Size {
    let mut height = visible_tile_height().min((slot.height - TILE_INSET * 2.0).max(0.0));
    let width = (slot.width - TILE_INSET * 2.0)
        .min(max_width_inside_slot(slot, height))
        .max(0.0);
    height = height.min(max_height_inside_slot(slot, width)).max(0.0);

    Size::new(width, height)
}

fn visible_tile_height() -> f32 {
    (theme::component::SOUND_TILE_H - TILE_INSET * 2.0).max(0.0)
}

fn max_width_inside_slot(slot: Size, height: f32) -> f32 {
    let radians = MAX_ROTATION_DEGREES.to_radians();
    let sin = radians.sin();
    let cos = radians.cos();

    // Degenerate guards keep future zero/near-90 degree constants finite.
    let by_width = if cos <= f32::EPSILON {
        slot.width
    } else {
        (slot.width - height * sin) / cos
    };
    let by_height = if sin <= f32::EPSILON {
        slot.width
    } else {
        (slot.height - height * cos) / sin
    };

    by_width.min(by_height)
}

fn max_height_inside_slot(slot: Size, width: f32) -> f32 {
    let radians = MAX_ROTATION_DEGREES.to_radians();
    let sin = radians.sin();
    let cos = radians.cos();

    // Degenerate guards keep future zero/near-90 degree constants finite.
    let by_width = if sin <= f32::EPSILON {
        slot.height
    } else {
        (slot.width - width * cos) / sin
    };
    let by_height = if cos <= f32::EPSILON {
        slot.height
    } else {
        (slot.height - width * sin) / cos
    };

    by_width.min(by_height)
}

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
    fn tile_slot_height_reserves_rotation_clearance() {
        assert!(tile_slot_height() > theme::component::SOUND_TILE_H);
    }

    #[test]
    fn tile_slot_height_reserves_only_static_rotation_clearance() {
        assert!(tile_slot_height() < theme::component::SOUND_TILE_H + 24.0);
    }

    #[test]
    fn fitted_tile_size_keeps_rotated_bounds_inside_slot() {
        for width in [6.0, 120.0, 360.0, 720.0] {
            let slot = Size::new(width, tile_slot_height());
            let tile = fitted_tile_size(slot);
            let bounds = rotated_bounds(tile, MAX_ROTATION_DEGREES);

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
