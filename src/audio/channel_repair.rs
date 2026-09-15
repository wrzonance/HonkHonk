//! Conservative repair for decoded stereo PCM with one effectively dead lane.
//!
//! This runs once on the decode worker, never on PipeWire's real-time thread.

const DEAD_CHANNEL_PEAK: f32 = 0.000_1;
const LIVE_CHANNEL_RMS: f32 = 0.003_162_277_7;
// Bound cancellation drift independently of clip length with about 3% extra energy work.
const ENERGY_REBASE_WINDOWS: usize = 16;

/// Copies a meaningful live lane over one dead lane, returning whether it did so.
pub(super) fn repair_dead_stereo_channel(
    samples: &mut [f32],
    sample_rate: u32,
    channels: u16,
) -> bool {
    if !is_valid_stereo(samples, sample_rate, channels) {
        return false;
    }

    let peaks = channel_peaks(samples);
    let dead = [peaks[0] <= DEAD_CHANNEL_PEAK, peaks[1] <= DEAD_CHANNEL_PEAK];
    if dead[0] == dead[1] {
        return false;
    }

    let live_lane = usize::from(dead[0]);
    if !has_live_block(samples, sample_rate, live_lane) {
        return false;
    }

    copy_live_lane(samples, live_lane);
    true
}

fn is_valid_stereo(samples: &[f32], sample_rate: u32, channels: u16) -> bool {
    channels == 2
        && sample_rate > 0
        && !samples.is_empty()
        && samples.len().is_multiple_of(2)
        && samples.iter().all(|sample| sample.is_finite())
}

fn channel_peaks(samples: &[f32]) -> [f32; 2] {
    samples
        .as_chunks::<2>()
        .0
        .iter()
        .fold([0.0_f32; 2], |peaks, frame| {
            [peaks[0].max(frame[0].abs()), peaks[1].max(frame[1].abs())]
        })
}

fn has_live_block(samples: &[f32], sample_rate: u32, lane: usize) -> bool {
    let frames_per_block = sample_rate as usize / 20;
    if frames_per_block == 0 {
        return false;
    }
    let frame_count = samples.len() / 2;
    let window_frames = frames_per_block.min(frame_count);
    let mut energy = window_energy(samples, lane, 0, window_frames);
    if energy_is_live(energy, window_frames) {
        return true;
    }

    for incoming_frame in window_frames..frame_count {
        let window_start = incoming_frame + 1 - window_frames;
        energy = advance_window_energy(samples, lane, window_start, window_frames, energy);
        if energy_is_live(energy, window_frames) {
            return true;
        }
    }
    false
}

fn sample_energy(sample: f32) -> f64 {
    f64::from(sample).powi(2)
}

fn window_energy(samples: &[f32], lane: usize, window_start: usize, window_frames: usize) -> f64 {
    samples[window_start * 2..(window_start + window_frames) * 2]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|frame| sample_energy(frame[lane]))
        .sum()
}

fn advance_window_energy(
    samples: &[f32],
    lane: usize,
    window_start: usize,
    window_frames: usize,
    energy: f64,
) -> f64 {
    let rebase_interval = window_frames * ENERGY_REBASE_WINDOWS;
    if window_start.is_multiple_of(rebase_interval) {
        return window_energy(samples, lane, window_start, window_frames);
    }

    let incoming_frame = window_start + window_frames - 1;
    let outgoing_frame = window_start - 1;
    energy + sample_energy(samples[incoming_frame * 2 + lane])
        - sample_energy(samples[outgoing_frame * 2 + lane])
}

fn energy_is_live(energy: f64, frames: usize) -> bool {
    let mean_square = energy / frames as f64;
    let threshold_squared = f64::from(LIVE_CHANNEL_RMS).powi(2);
    mean_square + f64::EPSILON >= threshold_squared
}

fn copy_live_lane(samples: &mut [f32], live_lane: usize) {
    let dead_lane = 1 - live_lane;
    for frame in samples.as_chunks_mut::<2>().0 {
        frame[dead_lane] = frame[live_lane];
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const SAMPLE_RATE: u32 = 1_000;

    fn stereo(left: &[f32], right: &[f32]) -> Vec<f32> {
        assert_eq!(left.len(), right.len());
        left.iter()
            .zip(right)
            .flat_map(|(&left, &right)| [left, right])
            .collect()
    }

    fn constant_stereo(frames: usize, left: f32, right: f32) -> Vec<f32> {
        stereo(&vec![left; frames], &vec![right; frames])
    }

    fn sample_bits(samples: &[f32]) -> Vec<u32> {
        samples.iter().map(|sample| sample.to_bits()).collect()
    }

    #[test]
    fn dead_left_is_replaced_with_unscaled_right() {
        let right = [0.25, -0.5, 0.75, -1.0];
        let mut samples = stereo(&[0.0; 4], &right);
        let original_len = samples.len();

        assert!(repair_dead_stereo_channel(&mut samples, SAMPLE_RATE, 2));
        assert_eq!(samples, stereo(&right, &right));
        assert_eq!(samples.len(), original_len);
    }

    #[test]
    fn dead_right_is_replaced_with_unscaled_left() {
        let left = [-0.25, 0.5, -0.75, 1.0];
        let mut samples = stereo(&left, &[0.0; 4]);

        assert!(repair_dead_stereo_channel(&mut samples, SAMPLE_RATE, 2));
        assert_eq!(samples, stereo(&left, &left));
    }

    #[test]
    fn dead_peak_and_live_rms_thresholds_are_inclusive() {
        let mut samples = constant_stereo(50, DEAD_CHANNEL_PEAK, LIVE_CHANNEL_RMS);

        assert!(repair_dead_stereo_channel(&mut samples, SAMPLE_RATE, 2));
        assert_eq!(
            samples,
            constant_stereo(50, LIVE_CHANNEL_RMS, LIVE_CHANNEL_RMS)
        );
    }

    #[test]
    fn live_rms_below_threshold_is_unchanged() {
        let mut samples = constant_stereo(50, 0.0, LIVE_CHANNEL_RMS - f32::EPSILON);
        let original = samples.clone();

        assert!(!repair_dead_stereo_channel(&mut samples, SAMPLE_RATE, 2));
        assert_eq!(samples, original);
    }

    #[test]
    fn below_dead_threshold_residue_is_repaired() {
        let mut samples = constant_stereo(50, DEAD_CHANNEL_PEAK / 2.0, 0.25);

        assert!(repair_dead_stereo_channel(&mut samples, SAMPLE_RATE, 2));
        assert_eq!(samples, constant_stereo(50, 0.25, 0.25));
    }

    #[test]
    fn opposite_channel_peak_above_dead_threshold_preserves_asymmetry() {
        let mut left = vec![0.0; 50];
        left[24] = DEAD_CHANNEL_PEAK + f32::EPSILON;
        let mut samples = stereo(&left, &[0.25; 50]);
        let original = samples.clone();

        assert!(!repair_dead_stereo_channel(&mut samples, SAMPLE_RATE, 2));
        assert_eq!(samples, original);
    }

    #[test]
    fn ordinary_stereo_is_unchanged() {
        let mut samples = constant_stereo(50, 0.25, -0.125);
        let original = samples.clone();

        assert!(!repair_dead_stereo_channel(&mut samples, SAMPLE_RATE, 2));
        assert_eq!(samples, original);
    }

    #[test]
    fn silence_and_two_dead_channels_are_unchanged() {
        for mut samples in [
            constant_stereo(50, 0.0, 0.0),
            constant_stereo(50, DEAD_CHANNEL_PEAK, DEAD_CHANNEL_PEAK / 2.0),
        ] {
            let original = samples.clone();
            assert!(!repair_dead_stereo_channel(&mut samples, SAMPLE_RATE, 2));
            assert_eq!(samples, original);
        }
    }

    #[test]
    fn dead_lane_without_a_live_lane_is_unchanged() {
        let mut samples = constant_stereo(50, 0.0, LIVE_CHANNEL_RMS / 2.0);
        let original = samples.clone();

        assert!(!repair_dead_stereo_channel(&mut samples, SAMPLE_RATE, 2));
        assert_eq!(samples, original);
    }

    #[test]
    fn mono_and_multichannel_buffers_are_unchanged() {
        for (mut samples, channels) in [
            (vec![0.0, 0.25, 0.0, 0.25], 1),
            (vec![0.0, 0.25, 0.5, 0.0, 0.25, 0.5], 3),
        ] {
            let original = samples.clone();
            assert!(!repair_dead_stereo_channel(
                &mut samples,
                SAMPLE_RATE,
                channels
            ));
            assert_eq!(samples, original);
        }
    }

    #[test]
    fn malformed_buffers_are_unchanged() {
        for mut samples in [Vec::new(), vec![0.0, 0.25, 0.0]] {
            let original = samples.clone();
            assert!(!repair_dead_stereo_channel(&mut samples, SAMPLE_RATE, 2));
            assert_eq!(samples, original);
        }

        let mut zero_rate = constant_stereo(50, 0.0, 0.25);
        let original = zero_rate.clone();
        assert!(!repair_dead_stereo_channel(&mut zero_rate, 0, 2));
        assert_eq!(zero_rate, original);
    }

    #[test]
    fn short_clip_uses_its_available_frames() {
        let mut samples = constant_stereo(4, 0.0, LIVE_CHANNEL_RMS);

        assert!(repair_dead_stereo_channel(&mut samples, SAMPLE_RATE, 2));
        assert_eq!(
            samples,
            constant_stereo(4, LIVE_CHANNEL_RMS, LIVE_CHANNEL_RMS)
        );
    }

    #[test]
    fn sub_twenty_hertz_clip_has_no_valid_fifty_millisecond_window() {
        let mut samples = constant_stereo(1, 0.0, 0.25);
        let original = samples.clone();

        assert!(!has_live_block(&samples, 19, 1));
        assert!(!repair_dead_stereo_channel(&mut samples, 19, 2));
        assert_eq!(samples, original);
    }

    #[test]
    fn final_full_window_can_establish_live_audio() {
        let mut right = vec![0.0; 60];
        right[50..].fill(0.01);
        let mut samples = stereo(&[0.0; 60], &right);

        assert!(repair_dead_stereo_channel(&mut samples, SAMPLE_RATE, 2));
        assert_eq!(samples, stereo(&right, &right));
    }

    #[test]
    fn sliding_energy_stays_anchored_to_the_current_window() {
        const CYCLES: usize = 200_000;
        const PATTERN: [f32; 4] = [0.002_5, 0.000_1, 0.003, 0.000_01];

        let frame_count = 1 + CYCLES * PATTERN.len();
        let mut samples = Vec::with_capacity(frame_count * 2);
        for frame in 0..frame_count {
            samples.extend_from_slice(&[0.0, PATTERN[frame % PATTERN.len()]]);
        }

        let mut energy = window_energy(&samples, 1, 0, 1);
        for window_start in 1..frame_count {
            energy = advance_window_energy(&samples, 1, window_start, 1, energy);
        }

        let exact = window_energy(&samples, 1, frame_count - 1, 1);
        assert!((energy - exact).abs() <= f64::EPSILON);
    }

    #[test]
    fn short_tail_is_not_treated_as_a_standalone_block() {
        let mut right = vec![0.0; 51];
        right[50] = 0.01;
        let mut samples = stereo(&[0.0; 51], &right);
        let original = samples.clone();

        assert!(!repair_dead_stereo_channel(&mut samples, SAMPLE_RATE, 2));
        assert_eq!(samples, original);
    }

    #[test]
    fn non_finite_samples_leave_the_entire_buffer_unchanged() {
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut samples = constant_stereo(50, 0.0, 0.25);
            samples[20] = invalid;
            let original = sample_bits(&samples);

            assert!(!repair_dead_stereo_channel(&mut samples, SAMPLE_RATE, 2));
            assert_eq!(sample_bits(&samples), original);
        }
    }

    #[test]
    fn repair_preserves_derived_duration_and_format_metadata() {
        let sample_rate = 48_000;
        let channels = 2;
        let mut samples = constant_stereo(4_800, 0.0, 0.25);
        let duration =
            Duration::from_secs_f64(samples.len() as f64 / channels as f64 / sample_rate as f64);

        assert!(repair_dead_stereo_channel(
            &mut samples,
            sample_rate,
            channels
        ));
        assert_eq!(samples.len(), 9_600);
        assert_eq!(sample_rate, 48_000);
        assert_eq!(channels, 2);
        assert_eq!(
            Duration::from_secs_f64(samples.len() as f64 / channels as f64 / sample_rate as f64),
            duration
        );
    }
}
