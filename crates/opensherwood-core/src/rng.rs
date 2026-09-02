//! Seeded, serialisable, named random number streams (PCG32, O'Neill 2014, a public-domain
//! algorithm). Every draw is counted so hashes and traces can pin down where two runs diverge.

use serde::{Deserialize, Serialize};

/// Largest stream id (the PCG increment is `stream * 2 + 1`, which must fit in 64 bits).
pub const MAX_STREAM_ID: u64 = (1 << 63) - 1;

/// A PCG32 stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rng {
    /// Seed the stream was created with.
    pub seed: u64,
    /// Stream id (`0..=MAX_STREAM_ID`), part of the identity of the stream.
    pub stream: u64,
    state: u64,
    /// Number of `u32` values drawn so far (saturating).
    pub draws: u64,
}

impl Rng {
    /// Algorithm name recorded in hashes and replays.
    pub const ALGORITHM: &'static str = "pcg32";

    /// Create a stream from a seed and a stream id (ids above [`MAX_STREAM_ID`] are masked).
    #[must_use]
    pub fn new(seed: u64, stream: u64) -> Self {
        let stream = stream & MAX_STREAM_ID;
        let mut r = Rng {
            seed,
            stream,
            state: 0,
            draws: 0,
        };
        r.next_u32();
        r.state = r.state.wrapping_add(seed);
        r.next_u32();
        r.draws = 0;
        r
    }

    fn inc(&self) -> u64 {
        (self.stream << 1) | 1
    }

    /// Next 32 random bits.
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(self.inc());
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        self.draws = self.draws.saturating_add(1);
        xorshifted.rotate_right(rot)
    }

    /// Uniform integer in `0..n` (`n == 0` returns 0). Uses rejection sampling to avoid modulo bias.
    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let threshold = n.wrapping_neg() % n;
        loop {
            let r = self.next_u32();
            if r >= threshold {
                return r % n;
            }
        }
    }

    /// Internal state word (for hashing).
    #[must_use]
    pub fn state(&self) -> u64 {
        self.state
    }

    /// Validate a deserialised stream.
    pub fn validate(&self) -> Result<(), String> {
        if self.stream > MAX_STREAM_ID {
            return Err(format!("rng stream id {} out of range", self.stream));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_counted() {
        let mut a = Rng::new(42, 1);
        let mut b = Rng::new(42, 1);
        let va: Vec<u32> = (0..8).map(|_| a.next_u32()).collect();
        let vb: Vec<u32> = (0..8).map(|_| b.next_u32()).collect();
        assert_eq!(va, vb);
        assert_eq!(a.draws, 8);
        let mut c = Rng::new(42, 2);
        assert_ne!(c.next_u32(), va[0]);
        for _ in 0..100 {
            assert!(c.below(7) < 7);
        }
        assert_eq!(c.below(0), 0);
    }

    #[test]
    fn stream_ids_are_kept_and_bounded() {
        let r = Rng::new(1, u64::MAX);
        assert_eq!(r.stream, MAX_STREAM_ID);
        assert!(r.validate().is_ok());
        let mut d = Rng::new(1, 1);
        d.draws = u64::MAX;
        d.next_u32();
        assert_eq!(d.draws, u64::MAX);
    }
}
