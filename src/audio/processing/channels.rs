use super::{OutputMode, SoundProcessing};

/// Constant-size routing descriptor; PCM stays in its original shared buffer.
#[derive(Clone, Copy)]
pub struct ChannelLayout {
    source: u16,
    output: u16,
    downmix: bool,
}

impl ChannelLayout {
    pub fn new(source: u16, settings: SoundProcessing) -> Self {
        let source = source.max(1);
        let settings = settings.sanitized();
        let downmix = settings.output == OutputMode::Mono
            || (settings.output == OutputMode::Stereo && source > 2);
        let mut output = match settings.output {
            OutputMode::Mono => 1,
            OutputMode::Stereo => 2,
            OutputMode::Preserve => source,
        };
        if output == 1 && settings.pan != 0.0 {
            output = 2;
        }
        Self {
            source,
            output,
            downmix,
        }
    }

    pub fn output_channels(self) -> u16 {
        self.output
    }

    /// Convert only complete requested frames, without allocating. Return source
    /// samples consumed and output samples written; never consume a partial frame.
    pub fn fill(self, source: &[f32], output: &mut [f32], gain: f32) -> (usize, usize) {
        let frames = (source.len() / self.source as usize).min(output.len() / self.output as usize);
        for (src, dst) in source
            .chunks_exact(self.source as usize)
            .zip(output.chunks_exact_mut(self.output as usize))
            .take(frames)
        {
            if self.downmix {
                let mono: f32 = src.iter().map(|sample| *sample / self.source as f32).sum();
                dst.fill(mono * gain);
            } else if self.source == 1 {
                dst.fill(src[0] * gain);
            } else {
                for (dst, src) in dst.iter_mut().zip(src) {
                    *dst = *src * gain;
                }
            }
        }
        (frames * self.source as usize, frames * self.output as usize)
    }
}
