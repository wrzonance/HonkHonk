//! Pure timeline geometry. Intervals are half-open and input order is retained.
pub fn assign_lanes(intervals: &[(u64, u64)]) -> Vec<usize> {
    let mut order: Vec<_> = (0..intervals.len()).collect();
    order.sort_by_key(|&i| intervals[i].0);
    let mut ends = Vec::new();
    let mut lanes = vec![0; intervals.len()];
    for i in order {
        let (start, duration) = intervals[i];
        let lane = ends
            .iter()
            .position(|&end| end <= start)
            .unwrap_or(ends.len());
        let end = start.saturating_add(duration.max(1));
        if lane == ends.len() {
            ends.push(end);
        } else {
            ends[lane] = end;
        }
        lanes[i] = lane;
    }
    lanes
}

pub fn time_at(x: f32, grab: f32, scale: f64, snap: bool) -> u64 {
    if !scale.is_finite() || scale <= 0.0 {
        return 0;
    }
    let time = f64::from((x - grab).max(0.0)) / scale;
    let quantum = if snap { 50.0 } else { 1.0 };
    ((time / quantum).round() * quantum) as u64
}

/// At most 101 ruler ticks, independent of persisted duration or offset.
/// Normal timelines retain one-second ticks; long timelines use wider spacing.
pub fn ruler_ticks(width: f32, scale: f64) -> Vec<(f32, u64)> {
    if !scale.is_finite() || scale <= 0.0 || !width.is_finite() {
        return Vec::new();
    }
    let interval = (f64::from(width) / scale / 1000.0 / 100.0).ceil().max(1.0);
    (0..=100)
        .filter_map(|index| {
            let second = (f64::from(index) * interval) as u64;
            let x = (second as f64 * 1000.0 * scale) as f32;
            (x <= width).then_some((x, second))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsorted_overlaps_use_lowest_available_lane() {
        assert_eq!(
            assign_lanes(&[(100, 100), (0, 150), (150, 50), (200, 10)]),
            vec![1, 0, 0, 0]
        );
        assert_eq!(assign_lanes(&[(0, 10), (0, 10), (0, 10)]), vec![0, 1, 2]);
        assert_eq!(assign_lanes(&[(0, 10), (10, 10)]), vec![0, 0]);
        assert!(assign_lanes(&[]).is_empty());
    }

    #[test]
    fn pointer_mapping_preserves_grab_clamps_and_snaps() {
        assert_eq!(time_at(130.0, 30.0, 0.1, false), 1000);
        assert_eq!(time_at(10.0, 30.0, 0.1, true), 0);
        assert_eq!(time_at(12.6, 0.0, 0.1, true), 150);
        assert_eq!(time_at(12.4, 0.0, 0.1, true), 100);
    }

    #[test]
    fn ruler_work_is_bounded_at_normal_and_extreme_scales() {
        for scale in [0.1, 1.0e-16] {
            let ticks = ruler_ticks(16_000.0, scale);
            assert!(!ticks.is_empty() && ticks.len() <= 101);
            assert!(
                ticks
                    .iter()
                    .all(|(x, _)| x.is_finite() && *x >= 0.0 && *x <= 16_000.0)
            );
            assert!(ticks.windows(2).all(|pair| pair[0].0 < pair[1].0));
        }
        let normal = ruler_ticks(800.0, 0.1);
        assert_eq!(normal.len(), 9);
        assert_eq!(normal[1], (100.0, 1));
        assert_eq!(
            time_at(100.0, 0.0, 1.0e-16, false),
            1_000_000_000_000_000_000
        );
    }

    #[test]
    fn exhaustive_three_interval_cases_are_nonoverlapping_and_use_minimum_lanes() {
        let intervals: Vec<_> = (0..4)
            .flat_map(|start| (1..4).map(move |duration| (start, duration)))
            .collect();
        for a in &intervals {
            for b in &intervals {
                for c in &intervals {
                    let input = [*a, *b, *c];
                    let lanes = assign_lanes(&input);
                    for i in 0..3 {
                        for j in i + 1..3 {
                            if lanes[i] == lanes[j] {
                                assert!(
                                    input[i].0 + input[i].1 <= input[j].0
                                        || input[j].0 + input[j].1 <= input[i].0
                                );
                            }
                        }
                    }
                    let simultaneous = (0..8)
                        .map(|time| {
                            input
                                .iter()
                                .filter(|(start, length)| *start <= time && time < start + length)
                                .count()
                        })
                        .max()
                        .unwrap();
                    assert_eq!(lanes.iter().max().unwrap() + 1, simultaneous);
                }
            }
        }
    }
}
