use std::collections::VecDeque;
use std::sync::Mutex;

/// Thread-safe ring buffer for decoded remote audio PCM samples.
///
/// NativeAudioStream tasks push i16 samples into this buffer.
/// Platform audio output (Android AudioTrack, desktop cpal) pulls from it.
///
/// Max capacity prevents unbounded growth if the consumer is slower than
/// the producer — old samples are discarded (better to skip than to
/// accumulate latency).
pub struct AudioPlayoutBuffer {
    buffer: Mutex<VecDeque<i16>>,
    /// Maximum number of i16 samples to store (2 seconds at 48kHz mono = 96_000).
    max_samples: usize,
}

impl AudioPlayoutBuffer {
    pub fn new() -> Self {
        // 2 seconds of 48kHz mono audio
        let max_samples = 48_000 * 2;
        Self {
            buffer: Mutex::new(VecDeque::with_capacity(max_samples)),
            max_samples,
        }
    }

    /// Push PCM samples into the buffer.
    ///
    /// If the buffer would exceed max capacity, oldest samples are dropped.
    pub fn push_samples(&self, samples: &[i16]) {
        let mut buf = self.buffer.lock().unwrap();
        buf.extend(samples.iter().copied());

        // Drop oldest samples if we exceed capacity
        let overflow = buf.len().saturating_sub(self.max_samples);
        if overflow > 0 {
            buf.drain(..overflow);
        }
    }

    /// Pull up to `out.len()` samples from the buffer.
    ///
    /// Returns the number of samples actually written. Unfilled positions
    /// in `out` are zeroed (silence).
    pub fn pull_samples(&self, out: &mut [i16]) -> usize {
        let mut buf = self.buffer.lock().unwrap();
        let available = buf.len().min(out.len());

        for (i, sample) in buf.drain(..available).enumerate() {
            out[i] = sample;
        }

        // Fill remainder with silence
        for sample in out[available..].iter_mut() {
            *sample = 0;
        }

        available
    }

    /// Clear all buffered samples (e.g., on disconnect).
    pub fn clear(&self) {
        self.buffer.lock().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_pull() {
        let buf = AudioPlayoutBuffer::new();
        let samples = vec![100i16, 200, 300, 400, 500];
        buf.push_samples(&samples);

        let mut out = vec![0i16; 3];
        let n = buf.pull_samples(&mut out);
        assert_eq!(n, 3);
        assert_eq!(out, vec![100, 200, 300]);

        let mut out2 = vec![0i16; 5];
        let n2 = buf.pull_samples(&mut out2);
        assert_eq!(n2, 2);
        assert_eq!(out2, vec![400, 500, 0, 0, 0]);
    }

    #[test]
    fn overflow_drops_oldest() {
        let buf = AudioPlayoutBuffer {
            buffer: Mutex::new(VecDeque::with_capacity(4)),
            max_samples: 4,
        };

        buf.push_samples(&[1, 2, 3, 4]);
        buf.push_samples(&[5, 6]);

        let mut out = vec![0i16; 6];
        let n = buf.pull_samples(&mut out);
        assert_eq!(n, 4);
        assert_eq!(out, vec![3, 4, 5, 6, 0, 0]);
    }

    #[test]
    fn pull_empty_returns_silence() {
        let buf = AudioPlayoutBuffer::new();
        let mut out = vec![99i16; 3];
        let n = buf.pull_samples(&mut out);
        assert_eq!(n, 0);
        assert_eq!(out, vec![0, 0, 0]);
    }

    #[test]
    fn clear_empties_buffer() {
        let buf = AudioPlayoutBuffer::new();
        buf.push_samples(&[1, 2, 3]);
        buf.clear();

        let mut out = vec![0i16; 3];
        let n = buf.pull_samples(&mut out);
        assert_eq!(n, 0);
    }
}
