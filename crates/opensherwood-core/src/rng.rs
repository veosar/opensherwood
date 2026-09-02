//! Seeded, serialisable random number streams (PCG32, O'Neill 2014, a public-domain algorithm).
//! Every draw is counted so hashes and traces can pin down where two runs diverge.

use serde::{Deserialize, Serialize};

/// A PCG32 stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rng {
    state: u64,
    inc: u64,
    /// Number of `u32` values drawn so far.
    pub draws: u64,
}

impl Rng {
    /// Create a stream from a seed and a stream id.
    #[must_use]
    pub fn new(seed: u64, stream: u64) -> Self {
        let mut r = Rng {
            state: 0,
            inc: (stream << 1) | 1,
            draws: 0,
        };
        r.next_u32();
        r.state = r.state.wrapping_add(seed);
        r.next_u32();
        r.draws = 0;
        r
    }

    /// Next 32 random bits.
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        self.draws += 1;
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

    /// Raw state for hashing.
    #[must_use]
    pub fn state(&self) -> (u64, u64) {
        (self.state, self.inc)
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
}
