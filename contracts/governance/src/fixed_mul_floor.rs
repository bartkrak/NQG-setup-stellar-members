/// floor(x * y / denominator), with the product computed in i128 so it cannot
/// overflow.
///
/// `None` if the result does not fit an `i64`. `denominator` must be positive.
pub(crate) fn fixed_mul_floor(x: i64, y: i64, denominator: i64) -> Option<i64> {
    let product = i128::from(x) * i128::from(y);
    i64::try_from(product.div_euclid(i128::from(denominator))).ok()
}

#[cfg(test)]
mod tests {
    use super::fixed_mul_floor;

    const ONE: i64 = 1_000_000;

    #[test]
    fn multiplies_fixed_point_values() {
        assert_eq!(fixed_mul_floor(2 * ONE, 3 * ONE, ONE), Some(6 * ONE));
        assert_eq!(fixed_mul_floor(1_500_000, ONE / 2, ONE), Some(750_000));
    }

    #[test]
    fn rounds_towards_negative_infinity() {
        // 0.000001 * 0.5 = 0.0000005, floors to 0 and to -0.000001 when negative
        assert_eq!(fixed_mul_floor(1, ONE / 2, ONE), Some(0));
        assert_eq!(fixed_mul_floor(-1, ONE / 2, ONE), Some(-1));
    }

    #[test]
    fn keeps_the_full_i64_range() {
        // The product overflows i64 but the result does not
        assert_eq!(fixed_mul_floor(i64::MAX, ONE, ONE), Some(i64::MAX));
    }

    #[test]
    fn reports_overflow() {
        assert_eq!(fixed_mul_floor(i64::MAX, 2 * ONE, ONE), None);
    }
}
