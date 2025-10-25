use crate::error::{Error, EstimatorResult};
use error_stack::{ResultExt, report};

pub fn decimal_string_to_u128(s: &str, decimals: u8) -> EstimatorResult<u128> {
    let decimals: usize = decimals.into();
    // Split the string by the decimal point
    let parts: Vec<&str> = s.split('.').collect();

    // Parse the integer part
    let integer_part = parts[0].parse::<u128>().change_context(Error::ParseError)?;

    // Handle the decimal part if it exists
    let decimal_part = if parts.len() > 1 {
        let decimal_str = parts[1];
        // Ensure we only use up to the specified number of decimal places
        let trimmed = if decimal_str.len() > decimals {
            &decimal_str[..decimals]
        } else {
            decimal_str
        };

        let decimal_value = trimmed.parse::<u128>().change_context(Error::ParseError)?;

        // Adjust based on the number of decimal digits (padding with zeros if needed)
        let scaling_factor = 10u128.pow((decimals - trimmed.len()) as u32);
        decimal_value * scaling_factor
    } else {
        0
    };

    // Combine integer and decimal parts (assuming 6 decimal places of precision)
    Ok(integer_part * 10u128.pow(decimals as u32) + decimal_part)
}

pub fn u128_to_f64(value: u128, decimals: u8) -> f64 {
    // Divide in integer space first to minimize precision loss
    let divisor = 10u128.pow(decimals as u32);
    let whole_part = (value / divisor) as f64;
    let fractional_part = (value % divisor) as f64 / divisor as f64;

    whole_part + fractional_part
}

/// Converts an f64 value to u128 with the specified number of decimal places
///
/// # Arguments
/// * `value` - The f64 value to convert
/// * `decimals` - The number of decimal places to preserve
///
/// # Returns
/// * `AppResult<u128, String>` - The converted value or an error
pub fn f64_to_u128(value: f64, decimals: u8) -> EstimatorResult<u128> {
    // Check for negative values
    if value < 0.0 {
        return Err(
            report!(Error::Unknown).attach_printable("Cannot convert negative value to u128")
        );
    }

    // Check for NaN or infinity
    if !value.is_finite() {
        return Err(report!(Error::Unknown)
            .attach_printable("Cannot convert NaN or infinite value to u128"));
    }

    // Calculate the scaling factor (10^decimals)
    let scaling_factor = 10f64.powi(decimals as i32);

    // Scale and round the value
    let scaled_value = (value * scaling_factor).round();

    // Check if the result fits in u128
    if scaled_value > u128::MAX as f64 {
        return Err(report!(Error::Unknown).attach_printable("Value too large for u128"));
    }

    Ok(scaled_value as u128)
}

pub fn u128_to_u64(x: u128, ctx: &'static str) -> EstimatorResult<u64> {
    u64::try_from(x)
        .change_context(Error::ParseError)
        .attach_printable(format!("Failed to parse {ctx} from u128 to u64"))
}

pub fn slippage_to_bps(slippage_percent: f64) -> EstimatorResult<u64> {
    // 1. Check for non-finite values
    if !slippage_percent.is_finite() {
        return Err(
            report!(Error::ParseError).attach_printable("Slippage percentage is not finite")
        );
    }

    // 2. Check that the value is not negative, if your logic assumes non-negative
    if slippage_percent < 0.0 {
        return Err(report!(Error::ParseError).attach_printable("Slippage percentage is negative"));
    }

    // 3. Scale to value in basis points (bps)
    let scaled = slippage_percent * 100.0;

    // 4. Check that scaled fits in u64
    if scaled > (u64::MAX as f64) {
        return Err(report!(Error::ParseError).attach_printable("Slippage percentage is too large"));
    }

    // 5. truncate to remove any fractional bps
    let truncated = scaled.trunc();

    // 6. Safe conversion to u64 type
    let result = truncated as u64;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decimal_string_to_u128() {
        let result = decimal_string_to_u128("123.456789", 6);
        assert_eq!(result.unwrap(), 123456789);
    }

    #[test]
    fn test_u128_to_f64() {
        let result = u128_to_f64(123456789, 6);
        assert_eq!(result, 123.456789);
    }

    #[test]
    fn test_f64_to_u128() {
        assert_eq!(f64_to_u128(123.456789, 6).unwrap(), 123456789);
        assert_eq!(f64_to_u128(0.123456, 6).unwrap(), 123456);
        assert_eq!(f64_to_u128(9999.999, 3).unwrap(), 9999999);
    }

    #[test]
    fn test_f64_to_u128_errors() {
        assert!(f64_to_u128(-123.456, 6).is_err());
        assert!(f64_to_u128(f64::NAN, 6).is_err());
        assert!(f64_to_u128(f64::INFINITY, 6).is_err());
    }

    #[test]
    fn test_slippage_to_bps_monotonic() {
        let mut last = slippage_to_bps(0.0).unwrap();
        for s in (1..=10_000).map(|x| x as f64 / 100.0) {
            let cur = slippage_to_bps(s).unwrap();
            assert!(cur >= last, "bps should be non-decreasing");
            last = cur;
        }
    }

    #[test]
    fn test_u128_f64_roundtrip_with_tolerance() {
        // u128 -> f64 -> u128 loses precision; check bounded error for small magnitudes
        let decimals = 6u8;
        for v in [0u128, 1, 123, 123_456, 123_456_789, 9_876_543_210] {
            let f = u128_to_f64(v, decimals);
            let v_rt = f64_to_u128(f, decimals).expect("back to u128");
            // Allow at most 1 unit of the last decimal due to rounding
            let delta = if v > v_rt { v - v_rt } else { v_rt - v };
            assert!(
                delta <= 1,
                "round-trip too lossy: v={v}, v_rt={v_rt}, delta={delta}"
            );
        }
    }
}
