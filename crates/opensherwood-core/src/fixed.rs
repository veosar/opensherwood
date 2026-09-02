//! 24.8 fixed-point arithmetic used for all authoritative positions and speeds.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// Signed 24.8 fixed-point number.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Fixed(pub i32);

impl Fixed {
    /// Fractional bits.
    pub const SHIFT: u32 = 8;
    /// Scale factor.
    pub const ONE: Fixed = Fixed(1 << Self::SHIFT);
    /// Zero.
    pub const ZERO: Fixed = Fixed(0);

    /// From an integer.
    #[must_use]
    pub const fn from_int(v: i32) -> Self {
        Fixed(v << Self::SHIFT)
    }

    /// From raw 24.8 bits (what the protocol transports as `x256`).
    #[must_use]
    pub const fn from_raw(v: i32) -> Self {
        Fixed(v)
    }

    /// Raw bits.
    #[must_use]
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// Integer part (floor).
    #[must_use]
    pub const fn floor(self) -> i32 {
        self.0 >> Self::SHIFT
    }

    /// Nearest integer.
    #[must_use]
    pub const fn round(self) -> i32 {
        (self.0 + (1 << (Self::SHIFT - 1))) >> Self::SHIFT
    }

    /// Absolute value (saturating).
    #[must_use]
    pub const fn abs(self) -> Self {
        Fixed(self.0.saturating_abs())
    }

    /// Integer square root of a raw 48.16 product, used by [`Fixed::length`].
    fn isqrt(v: u64) -> u64 {
        if v < 2 {
            return v;
        }
        let mut x = 1u64 << (v.ilog2() / 2 + 1);
        loop {
            let y = x.midpoint(v / x);
            if y >= x {
                return x;
            }
            x = y;
        }
    }

    /// Length of the vector `(dx, dy)`, exact in fixed point (integer sqrt of the squared length).
    #[must_use]
    pub fn length(dx: Fixed, dy: Fixed) -> Fixed {
        let sq = i64::from(dx.0) * i64::from(dx.0) + i64::from(dy.0) * i64::from(dy.0);
        Fixed(Self::isqrt(sq as u64) as i32)
    }

    /// Multiply by an integer.
    #[must_use]
    pub const fn mul_int(self, v: i32) -> Self {
        Fixed(self.0.saturating_mul(v))
    }
}

impl Add for Fixed {
    type Output = Fixed;
    fn add(self, o: Fixed) -> Fixed {
        Fixed(self.0.saturating_add(o.0))
    }
}
impl Sub for Fixed {
    type Output = Fixed;
    fn sub(self, o: Fixed) -> Fixed {
        Fixed(self.0.saturating_sub(o.0))
    }
}
impl Mul for Fixed {
    type Output = Fixed;
    fn mul(self, o: Fixed) -> Fixed {
        Fixed(((i64::from(self.0) * i64::from(o.0)) >> Self::SHIFT) as i32)
    }
}
impl Div for Fixed {
    type Output = Fixed;
    fn div(self, o: Fixed) -> Fixed {
        if o.0 == 0 {
            return Fixed(if self.0 < 0 { i32::MIN } else { i32::MAX });
        }
        Fixed(((i64::from(self.0) << Self::SHIFT) / i64::from(o.0)) as i32)
    }
}
impl Neg for Fixed {
    type Output = Fixed;
    fn neg(self) -> Fixed {
        Fixed(self.0.saturating_neg())
    }
}
impl AddAssign for Fixed {
    fn add_assign(&mut self, o: Fixed) {
        *self = *self + o;
    }
}
impl SubAssign for Fixed {
    fn sub_assign(&mut self, o: Fixed) {
        *self = *self - o;
    }
}
impl fmt::Display for Fixed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/256", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic() {
        let a = Fixed::from_int(3);
        let b = Fixed::from_int(2);
        assert_eq!((a * b).floor(), 6);
        assert_eq!((a / b).raw(), 384);
        assert_eq!(
            Fixed::length(Fixed::from_int(3), Fixed::from_int(4)),
            Fixed::from_int(5)
        );
        assert_eq!(Fixed::from_raw(383).round(), 1);
        assert_eq!(Fixed::from_raw(384).round(), 2);
    }
}
