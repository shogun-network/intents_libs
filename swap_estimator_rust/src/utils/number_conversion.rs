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
}
