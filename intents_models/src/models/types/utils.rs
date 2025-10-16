use crate::models::types::common::TransferDetails;
use std::collections::HashSet;

pub fn get_number_of_unique_receivers(
    main_destination_token: &str,
    main_destination_wallet: &str,
    extra_transfers: &Option<Vec<TransferDetails>>,
) -> usize {
    let mut res = HashSet::<(String, String)>::new();
    res.insert((
        main_destination_token.to_string(),
        main_destination_wallet.to_string(),
    ));

    if let Some(extra_transfers) = extra_transfers {
        res.extend(
            extra_transfers
                .clone()
                .into_iter()
                .filter(|t| t.amount != 0)
                .map(|t| (t.token, t.receiver)),
        );
    }

    res.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_get_number_of_unique_receivers() {
        let usdc = "usdc".to_string();
        let usdt = "usdt".to_string();

        let wallet_1 = "wallet_1".to_string();
        let wallet_2 = "wallet_2".to_string();

        let res = get_number_of_unique_receivers(&usdc, &wallet_1, &None);
        assert_eq!(res, 1);

        let extra_transfers: Vec<TransferDetails> = vec![TransferDetails {
            token: usdc.clone(),
            receiver: wallet_1.clone(),
            amount: 1,
        }];

        let res = get_number_of_unique_receivers(&usdc, &wallet_1, &Some(extra_transfers));
        assert_eq!(res, 1);

        let extra_transfers: Vec<TransferDetails> = vec![TransferDetails {
            token: usdc.clone(),
            receiver: wallet_2.clone(),
            amount: 1,
        }];

        let res = get_number_of_unique_receivers(&usdc, &wallet_1, &Some(extra_transfers));
        assert_eq!(res, 2);

        let extra_transfers: Vec<TransferDetails> = vec![
            TransferDetails {
                token: usdc.clone(),
                receiver: wallet_1.clone(),
                amount: 1,
            },
            TransferDetails {
                token: usdt.clone(),
                receiver: wallet_1.clone(),
                amount: 1,
            },
            TransferDetails {
                token: usdt.clone(),
                receiver: wallet_2.clone(),
                amount: 1,
            },
        ];

        let res = get_number_of_unique_receivers(&usdc, &wallet_1, &Some(extra_transfers));
        assert_eq!(res, 3);
    }
}
