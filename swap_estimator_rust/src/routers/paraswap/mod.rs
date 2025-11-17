use intents_models::constants::chains::is_native_token_evm_address;

#[allow(clippy::module_inception)]
pub mod paraswap;
pub mod rate_limit;
pub mod requests;
pub mod responses;

/// Converts a decimal slippage value to the percentage format required by Paraswap API.
///
/// for 2.5% slippage, set the value to 2.5 * 100 = 250; for 10% = 1000.
///
/// # Arguments
///
/// * `slippage` - The slippage value in decimal format (e.g., 2.0 for 2%)
///
/// # Returns
///
/// The slippage value in Paraswap's format (e.g., 200 for 2%).
///
pub fn get_paraswap_format_slippage(slippage: f64) -> u32 {
    (slippage * 100.0) as u32
}

pub fn update_paraswap_native_token(token_address: String) -> String {
    if is_native_token_evm_address(&token_address) {
        "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string()
    } else {
        token_address
    }
}

pub fn get_paraswap_max_slippage() -> u32 {
    // Sometimes it fails with 0 amountOutMin. 50% will be enough anyway
    5_000 // 50%
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_paraswap_format_slippage() {
        assert_eq!(get_paraswap_format_slippage(5.0), 500);
    }

    #[test]
    fn test_update_paraswap_native_token() {
        assert_eq!(
            update_paraswap_native_token("0xeeeeeEeEeeeeeeeEeEeeeeeeeEeEeeeeeeeEeEee".to_string()),
            "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string()
        );
        assert_eq!(
            update_paraswap_native_token("0x0000000000000000000000000000000000000000".to_string()),
            "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string()
        );
        assert_eq!(
            update_paraswap_native_token(
                "0x420000000000000000000000000000000000000006".to_string()
            ),
            "0x420000000000000000000000000000000000000006".to_string()
        );
    }
}
