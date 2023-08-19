use core::{fmt, ops::*};

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct L32(u32);

impl L32 {
    /// The value 0.0
    pub const ZERO: Self = Self(0x40000000);
    /// The real value 1.0
    pub const ONE: Self = Self(0);

    /// Raw transmutation to `u32`.
    #[inline]
    pub const fn to_bits(self) -> u32 {
        self.0
    }

    /// Raw transmutation from `u32`.
    #[inline]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Calculates the square root.
    ///
    /// This function operates only on the absolute value for efficiency.
    #[inline]
    pub fn sqrt(self) -> Self {
        // We find the square root by dividing the exponent by 2, but we need to make sure
        // to use an arithmetic shift as the exponent is signed.
        let exp = ((self.0 << 1) as i32) >> 2;
        // Clear the sign bit as it still contains the sign bit of the exponent
        let mut res = Self(exp as u32 & 0x7FFFFFFF);

        if self == Self::ZERO {
            res = Self::ZERO;
        }

        res
    }

    /// Convert the number to an integer exponent and a signed 1.31 mantissa in range (-1, 1).
    /// This is inherently a lossy conversion as the logarithmic form contains many irrational numbers,
    /// in addition to the error introduced by the amount of bits we use for the mantissa.
    #[inline]
    fn to_exp_mantissa(self) -> (i32, i32) {
        let exp = (self.0 << 1) as i32 >> 25;
    }
}

impl fmt::Debug for L32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO: proper formatting
        self.0.fmt(f)
    }
}

impl Default for L32 {
    #[inline]
    fn default() -> Self {
        Self::ZERO
    }
}

impl Mul<L32> for L32 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: L32) -> Self {
        // The sign is determined trivially.
        let sign = (self.0 ^ rhs.0) & 0x80000000;
        // Multiplication is equivalent to adding the exponents.
        let exp = self.0.wrapping_add(rhs.0) & 0x7FFFFFFF;
        let mut res = Self(sign | exp);

        if self == Self::ZERO || rhs == Self::ZERO {
            res = Self::ZERO;
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
        // The sign is determined trivially.
        let sign = (self.0 ^ rhs.0) & 0x80000000;
        // Division is equivalent to subtracting the exponents.
        let exp = self.0.wrapping_sub(rhs.0) & 0x7FFFFFFF;
        let mut res = Self(sign | exp);

        // We don't check if rhs is 0 to save instructions, the result will be some overflowed
        // value.
        if self == Self::ZERO {
            res = Self::ZERO;
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

impl Add for L32 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        // Addition is a hard operation in a LNS, it requires evaluating either of the functions:
        // log2(2^i + 2^j) = i + log2(1 + 2^(j - i))
        // log2(2^i - 2^j) = i + log2(1 - 2^(j - i))
        // Where i > j and equality is handled seperately
        // Which function to use depends on the signs of the arguments.
        // Practically this translates two one or two lookup tables with a series of transformations
        // to keep the table size reasonable. These transformations are quite complex and still require
        // about 32KiB of lookup storage at 31 bit precision with 0.5 ulp error.
        // Therefore we opt for a simpler approach of performing the addition in a pseudo floating point
        // format and converting the number before and after. This only requires a few constants instead
        // of a whole LUT and thus allows for higher throughput in SIMD code as it obviates
        // gather instructions.

        let (self_exp, self_mantissa) = self.to_exp_mantissa();
        let (rhs_exp, rhs_mantissa) = rhs.to_exp_mantissa();
        // Addition is commutative so by arranging the arguments by magnitude we simplify
        // normalizing the arguments.
        let delta = self_exp - rhs_exp;
        let (a_mantissa, b_mantissa, shift_amount) = if delta < 0 {
            (rhs_mantissa, self_mantissa, (-delta) as u32)
        } else {
            (self_mantissa, rhs_mantissa, delta as u32)
        };

        let mut b_normalized = b_mantissa.wrapping_shr(shift_amount);
        // If we shift out all the fraction bits the result should be 0 regardless of the sign of the
        // mantissa. Otherwise we have a bias for negative numbers as for a positive number this will
        // yield 0 while with negative numbers it will yield -2^-31.
        if shift_amount >= 31 {
            b_normalized = 0;
        }
        // The actual addition
        let res_mantissa = a_mantissa + b_normalized

        // Rounding happens in the real domain, i.e. we round based on the represented value instead
        // of the exponent value

        res.0 |= result_sign;

        if self == Self::ZERO {
            res = rhs;
        }
        if rhs == Self::ZERO {
            res = self;
        }

        res
    }
}

impl AddAssign for L32 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for L32 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        self + -rhs
    }
}

impl SubAssign for L32 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Neg for L32 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        let mut res = Self(self.0 ^ 0x80000000);

        if self == Self::ZERO {
            res = Self::ZERO;
        }

        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqrt() {
        fn test(a: u32, res: u32) {
            let test1 = L32(a).sqrt().0;
            if test1 != res {
                panic!(
                    "test case failed: sqrt({a:08X})\n expected: {res:08X}\n      got: {test1:08X}"
                );
            }
        }

        test(0xC0000000, 0xC0000000);
        test(0x80000000, 0xC0000000);
        test(0x80000001, 0xC0000000);
        test(0x81234567, 0xC0000000);
        test(0xF3FCFEF3, 0xC0000000);
        test(0xFFFFFFFF, 0xC0000000);

        test(0x00000000, 0x00000000);
        test(0x00000001, 0x00000000);
        test(0x00800000, 0x00400000);
        test(0x00800001, 0x00400000);
        test(0x3FFFFFFF, 0x1FFFFFFF);
        test(0x70006101, 0x78003080);
        test(0x7FFFFFFF, 0x7FFFFFFF);
        test(0x40000000, 0x40000000);
        test(0x40000001, 0x60000000);
        test(0x60000000, 0x70000000);
    }

    #[test]
    fn mul() {
        fn test(a: u32, b: u32, res: u32) {
            let test1 = (L32(a) * L32(b)).0;
            let test2 = (L32(b) * L32(a)).0;
            if test1 != res {
                panic!("test case failed: {a:08X} * {b:08X}\n expected: {res:08X}\n      got: {test1:08X}");
            }
            if test2 != res {
                panic!("test case not commutative: {a:08X} * {b:08X}\n expected: {res:08X}\n      got: {test2:08X}");
            }
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
            let test1 = (L32(a) / L32(b)).0;
            if test1 != res {
                panic!("test case failed: {a:08X} / {b:08X}\n expected: {res:08X}\n      got: {test1:08X}");
            }
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

    #[test]
    fn add() {
        fn test(a: u32, b: u32, res: u32) {
            let test1 = (L32(a) + L32(b)).0;
            let test2 = (L32(b) + L32(a)).0;
            if test1 != res {
                panic!("test case failed: {a:08X} + {b:08X}\n expected: {res:08X}\n      got: {test1:08X}");
            }
            if test2 != res {
                panic!("test case not commutative: {a:08X} + {b:08X}\n expected: {res:08X}\n      got: {test2:08X}");
            }
        }

        // Adding 0
        test(0xBFFFFFFF, 0x40000000, 0xBFFFFFFF);
        test(0x3FFFFFFF, 0x40000000, 0x3FFFFFFF);
        test(0x00000000, 0x40000000, 0x00000000);
        test(0x80000000, 0x40000000, 0x80000000);
        test(0xDEADBEEF, 0x40000000, 0xDEADBEEF);
        // NaR preservation
        test(0xDEADBEEF, 0xC0000000, 0xC0000000);
        test(0x00000000, 0xC0000000, 0xC0000000);
        test(0x80000000, 0xC0000000, 0xC0000000);
        test(0x40000000, 0xC0000000, 0xC0000000);
        // Overflow
        test(0x3FFFFFFF, 0x343BFAE6, 0x40000000);
        test(0xBFFFFFFF, 0x80000001, 0x40000000);
        test(0x3FFFFFFF, 0x00000001, 0x40000000);
        test(0x3FFFFFFF, 0x80000001, 0xC0000000);

        test(0xBFFFFFFF, 0x80000000, 0x3FFFFFFF);
        test(0x7FFFFFFF, 0x00000001, 0x00000000);
        test(0xFFFFFFFF, 0x00000001, 0x80000000);
        test(0xDEADBEEF, 0xBEEFDEAD, 0x1D9D9D9C);
    }
}
