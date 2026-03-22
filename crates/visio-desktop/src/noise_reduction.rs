//! RNNoise wrapper for real-time noise suppression.
//!
//! Uses nnnoiseless (pure Rust RNNoise port). Operates on 480-sample
//! frames at 48kHz (10ms). I/O are f32 in [-32768, 32767] range.

use nnnoiseless::DenoiseState;

pub struct NoiseReducer {
    state: Box<DenoiseState<'static>>,
    input_buf: Vec<f32>,
    output_buf: Vec<i16>,
}

impl NoiseReducer {
    pub fn new() -> Self {
        Self {
            state: DenoiseState::new(),
            input_buf: Vec::with_capacity(DenoiseState::FRAME_SIZE),
            output_buf: Vec::new(),
        }
    }

    /// Process i16 PCM samples through RNNoise.
    /// Returns denoised i16 samples. May buffer internally if input
    /// is not a multiple of 480 samples.
    pub fn process(&mut self, samples: &[i16]) -> Vec<i16> {
        self.input_buf.extend(samples.iter().map(|&s| s as f32));
        self.output_buf.clear();

        while self.input_buf.len() >= DenoiseState::FRAME_SIZE {
            let mut out_frame = [0.0f32; DenoiseState::FRAME_SIZE];
            self.state
                .process_frame(&mut out_frame, &self.input_buf[..DenoiseState::FRAME_SIZE]);

            self.output_buf.extend(
                out_frame
                    .iter()
                    .map(|&s| s.round().clamp(-32768.0, 32767.0) as i16),
            );

            self.input_buf.drain(..DenoiseState::FRAME_SIZE);
        }

        self.output_buf.clone()
    }

    #[cfg(test)]
    pub fn reset(&mut self) {
        self.input_buf.clear();
        self.output_buf.clear();
        self.state = DenoiseState::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_exact_frame() {
        let mut nr = NoiseReducer::new();
        let silence = vec![0i16; 480];
        let out = nr.process(&silence);
        assert_eq!(out.len(), 480);
    }

    #[test]
    fn process_partial_frame_buffers() {
        let mut nr = NoiseReducer::new();
        let partial = vec![0i16; 240];
        let out = nr.process(&partial);
        assert_eq!(out.len(), 0);
        let out = nr.process(&partial);
        assert_eq!(out.len(), 480);
    }

    #[test]
    fn reset_clears_buffer() {
        let mut nr = NoiseReducer::new();
        let partial = vec![0i16; 240];
        nr.process(&partial);
        nr.reset();
        let out = nr.process(&partial);
        assert_eq!(out.len(), 0);
    }
}
