//! The x87's extended precision, as the original computes: the Borland RTL
//! sets the control word to `0x1332` (`0x480eac`), so every operation is
//! rounded to a 64-bit mantissa, and rounded again to a double when stored
//! (docs/gadgets/logic.md 1.8). Doubles alone round once, which now and
//! then differs in the last bit.

use std::cmp::Ordering;

/// An x87 register's value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ext {
    /// `mant · 2^exp`, `mant` with its top bit set, or zero with `mant` 0.
    Finite { neg: bool, exp: i32, mant: u64 },
    /// An infinity or a NaN (worked out as doubles).
    Special(f64),
}

/// `x · 2^k`, exact while the result is a double.
fn ldexp(mut x: f64, mut k: i32) -> f64 {
    while k > 1000 {
        x *= f64::from_bits(0x7e7 << 52);
        k -= 1000;
    }
    while k < -1000 {
        x *= f64::from_bits(0x017 << 52);
        k += 1000;
    }
    let bits = u64::try_from(k + 1023).unwrap_or(0) << 52;
    x * f64::from_bits(bits)
}

/// `m · 2^e` (plus a little more if `sticky`) rounded to 64 bits, to
/// nearest, ties to even.
fn round(neg: bool, m: u128, e: i32, sticky: bool) -> Ext {
    if m == 0 {
        return Ext::Finite { neg, exp: 0, mant: 0 };
    }
    let lz = m.leading_zeros();
    let m = m << lz;
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let (mut hi, lo, mut exp) = ((m >> 64) as u64, m as u64, e - lz as i32 + 64);
    let half = 1 << 63;
    if lo > half || (lo == half && (sticky || hi & 1 == 1)) {
        hi = hi.wrapping_add(1);
        if hi == 0 {
            hi = half;
            exp += 1;
        }
    }
    Ext::Finite { neg, exp, mant: hi }
}

impl From<f64> for Ext {
    fn from(x: f64) -> Ext {
        if !x.is_finite() {
            return Ext::Special(x);
        }
        let bits = x.to_bits();
        let neg = bits >> 63 != 0;
        let e = i32::try_from((bits >> 52) & 0x7ff).unwrap_or(0);
        let f = bits & ((1 << 52) - 1);
        let (m, exp) = if e == 0 { (f, -1074) } else { (f | 1 << 52, e - 1075) };
        if m == 0 {
            return Ext::Finite { neg, exp: 0, mant: 0 };
        }
        let lz = m.leading_zeros();
        #[allow(clippy::cast_possible_wrap)]
        Ext::Finite {
            neg,
            exp: exp - lz as i32,
            mant: m << lz,
        }
    }
}

impl From<i32> for Ext {
    /// `fild`: exact.
    fn from(i: i32) -> Ext {
        round(i < 0, u128::from(i.unsigned_abs()), 0, false)
    }
}

impl Ext {
    /// Stored to a double (`fstp qword`): rounded to nearest, ties to
    /// even; out of range is an infinity (the original would trap).
    #[must_use]
    pub fn to_f64(self) -> f64 {
        let (neg, exp, mant) = match self {
            Ext::Finite { neg, exp, mant } => (neg, exp, mant),
            Ext::Special(x) => return x,
        };
        let sign = if neg { -1.0 } else { 1.0 };
        if mant == 0 {
            return sign * 0.0;
        }
        // 53 bits, fewer for a denormal.
        let shift = (-1074 - exp).clamp(11, 127);
        let m = u128::from(mant);
        let s = shift.unsigned_abs();
        let (mut keep, rem, half) = (m >> s, m & ((1 << s) - 1), 1u128 << (s - 1));
        if rem > half || (rem == half && keep & 1 == 1) {
            keep += 1;
        }
        // keep <= 2^53: exact.
        #[allow(clippy::cast_precision_loss)]
        let k = keep as f64;
        sign * ldexp(k, exp + shift)
    }

    fn parts(self) -> Option<(bool, i32, u64)> {
        match self {
            Ext::Finite { neg, exp, mant } => Some((neg, exp, mant)),
            Ext::Special(_) => None,
        }
    }

    #[must_use]
    pub fn neg(self) -> Ext {
        match self {
            Ext::Finite { neg, exp, mant } => Ext::Finite { neg: !neg, exp, mant },
            Ext::Special(x) => Ext::Special(-x),
        }
    }

    /// `fadd`.
    #[must_use]
    pub fn add(self, other: impl Into<Ext>) -> Ext {
        let other = other.into();
        let (Some(a), Some(b)) = (self.parts(), other.parts()) else {
            return Ext::Special(self.to_f64() + other.to_f64());
        };
        if b.2 == 0 {
            return if a.2 == 0 {
                Ext::Finite {
                    neg: a.0 && b.0,
                    exp: 0,
                    mant: 0,
                }
            } else {
                self
            };
        }
        if a.2 == 0 {
            return other;
        }
        let (big, small) = if (a.1, a.2) >= (b.1, b.2) { (a, b) } else { (b, a) };
        // 62 spare bits below the larger, and one above for a carry.
        let ma = u128::from(big.2) << 62;
        let mb = u128::from(small.2) << 62;
        let d = big.1 - small.1;
        let (mb, sticky) = if d >= 126 {
            (0, true)
        } else {
            let d = d.unsigned_abs();
            (mb >> d, mb & ((1 << d) - 1) != 0)
        };
        let e = big.1 - 62;
        if big.0 == small.0 {
            round(big.0, ma + mb, e, sticky)
        } else {
            let diff = ma - mb;
            if diff == 0 && !sticky {
                return Ext::Finite {
                    neg: false,
                    exp: 0,
                    mant: 0,
                };
            }
            // What was shifted out makes it a little less than `diff`.
            round(big.0, diff - u128::from(sticky), e, sticky)
        }
    }

    /// `fsub`.
    #[must_use]
    pub fn sub(self, other: impl Into<Ext>) -> Ext {
        self.add(other.into().neg())
    }

    /// `fmul`.
    #[must_use]
    pub fn mul(self, other: impl Into<Ext>) -> Ext {
        let other = other.into();
        let (Some(a), Some(b)) = (self.parts(), other.parts()) else {
            return Ext::Special(self.to_f64() * other.to_f64());
        };
        round(a.0 != b.0, u128::from(a.2) * u128::from(b.2), a.1 + b.1, false)
    }

    /// `fdiv`.
    #[must_use]
    pub fn div(self, other: impl Into<Ext>) -> Ext {
        let other = other.into();
        let (Some(a), Some(b)) = (self.parts(), other.parts()) else {
            return Ext::Special(self.to_f64() / other.to_f64());
        };
        if b.2 == 0 {
            return Ext::Special(self.to_f64() / other.to_f64());
        }
        let neg = a.0 != b.0;
        if a.2 == 0 {
            return Ext::Finite { neg, exp: 0, mant: 0 };
        }
        // Two 64-bit digits of the quotient and whether any remains.
        let n = u128::from(a.2) << 63;
        let d = u128::from(b.2);
        let (q1, r1) = (n / d, n % d);
        let (q2, r2) = ((r1 << 64) / d, (r1 << 64) % d);
        round(neg, q1 << 64 | q2, a.1 - b.1 - 127, r2 != 0)
    }

    /// `fsqrt`.
    #[must_use]
    pub fn sqrt(self) -> Ext {
        let Some((neg, e, mant)) = self.parts() else {
            return Ext::Special(self.to_f64().sqrt());
        };
        if mant == 0 {
            return self;
        }
        if neg {
            return Ext::Special(f64::NAN);
        }
        // An even exponent, and the mantissa high in 128 bits so the root
        // has 64.
        let s = if e.rem_euclid(2) == 1 { 63 } else { 64 };
        let n = u128::from(mant) << s;
        let r = n.isqrt();
        // The root is never halfway: round up past r + 1/2.
        let up = n - r * r > r;
        #[allow(clippy::cast_possible_truncation)]
        let mut m = r as u64;
        let mut exp = (e - s) / 2;
        if up {
            m = m.wrapping_add(1);
            if m == 0 {
                m = 1 << 63;
                exp += 1;
            }
        }
        Ext::Finite {
            neg: false,
            exp,
            mant: m,
        }
    }
}

impl PartialOrd for Ext {
    fn partial_cmp(&self, other: &Ext) -> Option<Ordering> {
        let (Some(a), Some(b)) = (self.parts(), other.parts()) else {
            return self.to_f64().partial_cmp(&other.to_f64());
        };
        let mag = match (a.2 == 0, b.2 == 0) {
            (true, true) => return Some(Ordering::Equal),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => (a.1, a.2).cmp(&(b.1, b.2)),
        };
        let (na, nb) = (a.0 && a.2 != 0, b.0 && b.2 != 0);
        Some(match (na, nb) {
            (false, false) => mag,
            (true, true) => mag.reverse(),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn x(v: f64) -> Ext {
        Ext::from(v)
    }

    #[test]
    fn round_trips_doubles() {
        for v in [
            0.0,
            -0.0,
            1.0,
            -2.5,
            1e300,
            -1e-300,
            5e-324,
            f64::MAX,
            f64::MIN_POSITIVE,
            0.1,
        ] {
            assert_eq!(x(v).to_f64().to_bits(), v.to_bits(), "{v}");
        }
        assert_eq!(Ext::from(-7).to_f64(), -7.0);
        assert_eq!(Ext::from(i32::MIN).to_f64(), -2_147_483_648.0);
        assert!(x(f64::NAN).to_f64().is_nan());
    }

    #[test]
    fn agrees_with_doubles_mostly() {
        let vs = [1.0, 3.0, 0.1, -7.25, 1e10, 123.456, -0.3, 2.0f64.sqrt(), 1e-200];
        for &a in &vs {
            for &b in &vs {
                assert_eq!(x(a).add(b).to_f64(), a + b, "{a} + {b}");
                assert_eq!(x(a).sub(b).to_f64(), a - b, "{a} - {b}");
                assert_eq!(x(a).mul(b).to_f64(), a * b, "{a} * {b}");
                assert_eq!(x(a).div(b).to_f64(), a / b, "{a} / {b}");
            }
            assert_eq!(x(a.abs()).sqrt().to_f64(), a.abs().sqrt(), "sqrt {a}");
        }
        assert_eq!(x(1.0).sub(1.0).to_f64().to_bits(), 0);
        assert_eq!(x(1e308).add(1e308).to_f64(), f64::INFINITY);
        assert_eq!(x(1e-310).mul(0.5).to_f64(), 1e-310 * 0.5);
    }

    #[test]
    fn rounds_twice_like_the_fpu() {
        // 1 + 2^-53 + 2^-105: rounded to 64 bits it's 1 + 2^-53, a tie
        // between doubles, which goes to the even 1.
        let b = f64::from_bits(0x3ca0_0000_0000_0001);
        assert_eq!(1.0 + b, 1.0 + f64::EPSILON);
        assert_eq!(x(1.0).add(b).to_f64(), 1.0);
        // 1 / (1 - 2^-53) = 1 + 2^-53 + 2^-106 + ...: the same tie after
        // the first rounding.
        let d = 1.0 - f64::EPSILON / 2.0;
        assert_eq!(1.0 / d, 1.0 + f64::EPSILON);
        assert_eq!(x(1.0).div(d).to_f64(), 1.0);
    }

    #[test]
    fn compares() {
        assert!(x(1.0) < x(2.0));
        assert!(x(-3.0) < x(2.0));
        assert!(x(-3.0) < x(-2.0));
        assert!(x(0.0) == x(-0.0) || x(0.0).partial_cmp(&x(-0.0)) == Some(Ordering::Equal));
        assert!(x(1.0) > x(0.0));
        assert!(x(1e308).mul(10.0) > x(f64::MAX));
    }
}
