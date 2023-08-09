use core::{fmt, ops::*};

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct L32(u32);

impl L32 {
    /// Not a Real (NaR).
    ///
    /// Exceptional value for operations where the result cannot be expressed as a real number.
    pub const NAR: Self = Self(0xC0000000);

    /// The value 0.0
    pub const ZERO: Self = Self(0x40000000);
    /// The value 1.0
    pub const ONE: Self = Self(0);

    /// Raw transmutation to u32.
    #[inline]
    pub const fn to_bits(self) -> u32 {
        self.0
    }

    /// Raw transmutation from u32.
    #[inline]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Calculates the square root.
    ///
    /// Returns NaR if the input is negative or NaR.
    #[inline]
    pub fn sqrt(self) -> Self {
        // We don't care about the sign bit because if it's set the result will be overwritten
        // with NaR anyway.
        let exp = self.0 >> 1;
        let exp_sign = self.0 & 0x40000000;
        let mut res = Self(exp_sign | exp);

        if self == Self::ZERO {
            res = Self::ZERO;
        }
        if self.0 & 0x80000000 != 0 {
            res = Self::NAR;
        }

        res
    }
}

impl fmt::Debug for L32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO: proper formatting
        self.0.fmt(f)
    }
}

impl Mul<L32> for L32 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: L32) -> Self {
        let sign = (self.0 ^ rhs.0) & 0x80000000;
        let exp = self.0.wrapping_add(rhs.0) & 0x7FFFFFFF;
        let mut res = Self(sign | exp);

        if self == Self::ZERO || rhs == Self::ZERO {
            res = Self::ZERO;
        }
        if self == Self::NAR || rhs == Self::NAR {
            res = Self::NAR;
        }

        res
    }
}

impl MulAssign<L32> for L32 {
    #[inline]
    fn mul_assign(&mut self, rhs: L32) {
        *self = *self * rhs;
    }
}

impl Div<L32> for L32 {
    type Output = L32;

    #[inline]
    fn div(self, rhs: L32) -> Self::Output {
        let sign = (self.0 ^ rhs.0) & 0x80000000;
        let exp = self.0.wrapping_sub(rhs.0) & 0x7FFFFFFF;
        let mut res = Self(sign | exp);

        if self == Self::ZERO {
            res = Self::ZERO;
        }
        if self == Self::NAR || rhs == Self::NAR || rhs == Self::ZERO {
            res = Self::NAR;
        }

        res
    }
}

impl DivAssign<L32> for L32 {
    #[inline]
    fn div_assign(&mut self, rhs: L32) {
        *self = *self / rhs;
    }
}

impl Default for L32 {
    #[inline]
    fn default() -> Self {
        Self::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqrt() {
        assert_eq!(L32::NAR.sqrt(), L32::NAR);
        assert_eq!(L32(0x80000000).sqrt(), L32::NAR);
        assert_eq!(L32(0x80000001).sqrt(), L32::NAR);
        assert_eq!(L32(0x81234567).sqrt(), L32::NAR);
        assert_eq!(L32(0xF3FCFEF3).sqrt(), L32::NAR);
        assert_eq!(L32(0xFFFFFFFF).sqrt(), L32::NAR);

        assert_eq!(L32(0x00000000).sqrt(), L32(0x00000000));
        assert_eq!(L32(0x00000001).sqrt(), L32(0x00000000));
        assert_eq!(L32(0x00800000).sqrt(), L32(0x00400000));
        assert_eq!(L32(0x00800001).sqrt(), L32(0x00400000));
        assert_eq!(L32(0x3FFFFFFF).sqrt(), L32(0x1FFFFFFF));
        assert_eq!(L32(0x70006101).sqrt(), L32(0x78003080));
        assert_eq!(L32(0x7FFFFFFF).sqrt(), L32(0x7FFFFFFF));
        assert_eq!(L32(0x40000001).sqrt(), L32(0x60000000));
        assert_eq!(L32(0x60000000).sqrt(), L32(0x70000000));
    }

    #[test]
    fn mul() {
        fn test(a: u32, b: u32, res: u32) {
            assert_eq!(L32(a) * L32(b), L32(res));
            assert_eq!(L32(b) * L32(a), L32(res));
        }

        // Overflow
        test(0xBFFFFFFF, 0x00000001, 0xC0000000);
        test(0xBFFFFFFF, 0x80000001, 0x40000000);
        test(0x3FFFFFFF, 0x00000001, 0x40000000);
        test(0x3FFFFFFF, 0x80000001, 0xC0000000);
        // Multiply by 0
        test(0xBFFFFFFF, 0x40000000, 0x40000000);
        test(0x3FFFFFFF, 0x40000000, 0x40000000);
        test(0x00000000, 0x40000000, 0x40000000);
        test(0x80000000, 0x40000000, 0x40000000);
        test(0xDEADBEEF, 0x40000000, 0x40000000);
        // NaR preservation
        test(0xDEADBEEF, 0xC0000000, 0xC0000000);
        test(0x00000000, 0xC0000000, 0xC0000000);
        test(0x80000000, 0xC0000000, 0xC0000000);
        test(0x40000000, 0xC0000000, 0xC0000000);

        test(0xBFFFFFFF, 0x80000000, 0x3FFFFFFF);
        test(0x7FFFFFFF, 0x00000001, 0x00000000);
        test(0xFFFFFFFF, 0x00000001, 0x80000000);
        test(0xDEADBEEF, 0xBEEFDEAD, 0x1D9D9D9C);
    }

    #[test]
    fn div() {
        fn test(a: u32, b: u32, res: u32) {
            assert_eq!(L32(a) / L32(b), L32(res));
        }

        // Overflow
        test(0xBFFFFFFF, 0x7FFFFFFF, 0xC0000000);
        test(0xBFFFFFFF, 0xFFFFFFFF, 0x40000000);
        test(0x3FFFFFFF, 0x7FFFFFFF, 0x40000000);
        test(0x3FFFFFFF, 0xFFFFFFFF, 0xC0000000);
        // 0/x
        test(0x40000000, 0xBFFFFFFF, 0x40000000);
        test(0x40000000, 0x3FFFFFFF, 0x40000000);
        test(0x40000000, 0x00000000, 0x40000000);
        test(0x40000000, 0x80000000, 0x40000000);
        test(0x40000000, 0xDEADBEEF, 0x40000000);
        test(0x40000000, 0x00000001, 0x40000000);
        // NaR preservation
        test(0xDEADBEEF, 0xC0000000, 0xC0000000);
        test(0x00000000, 0xC0000000, 0xC0000000);
        test(0x80000000, 0xC0000000, 0xC0000000);
        test(0xC0000000, 0xDEADBEEF, 0xC0000000);
        test(0xC0000000, 0x00000000, 0xC0000000);
        test(0xC0000000, 0x80000000, 0xC0000000);
        // Division by 0
        test(0xBFFFFFFF, 0x40000000, 0xC0000000);
        test(0x3FFFFFFF, 0x40000000, 0xC0000000);
        test(0x00000000, 0x40000000, 0xC0000000);
        test(0x80000000, 0x40000000, 0xC0000000);
        test(0xDEADBEEF, 0x40000000, 0xC0000000);
        test(0x00000001, 0x40000000, 0xC0000000);

        test(0xBFFFFFFF, 0x80000000, 0x3FFFFFFF);
        test(0x00000000, 0x00000001, 0x7FFFFFFF);
        test(0x80000000, 0x00000001, 0xFFFFFFFF);
        test(0xDEADBEEF, 0xBEEFDEAD, 0x1FBDE042);
    }
}
