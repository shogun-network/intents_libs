use crate::error::{Error, EstimatorResult};
use error_stack::report;
use uint::construct_uint;

construct_uint! {
    pub struct U256(4);
}

/// Computes `(value * multiplier) / divisor` safely using U256
pub fn mul_div(value: u128, multiplier: u128, divisor: u128) -> EstimatorResult<u128> {
    let value = U256::from(value);
    let multiplier = U256::from(multiplier);
    let divisor = U256::from(divisor);
    if divisor.is_zero() {
        return Err(report!(Error::Unknown).attach_printable("Dividing by zero"));
    }
    let mut result = value * multiplier / divisor;

    // Convert back to u128 safely
    if result.bits() > 128 {
        return Err(report!(Error::Unknown).attach_printable("Result too large to fit in u128"));
    }

    if result == value && (multiplier > divisor) {
        // Rounding up
        result += U256::from(1);
    }

    Ok(result.as_u128())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_muldiv() {
        let a = 100_000_000_000_000_000_000_000_000u128;
        let b = 300_000_000_000_000_000_000_000_000u128;
        let c = 200_000_000_000_000_000_000_000_000u128;

        let res = mul_div(a, b, c);
        assert!(res.is_ok());
        let res = res.unwrap();
        assert_eq!(res, 150_000_000_000_000_000_000_000_000u128);
    }
}
