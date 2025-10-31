use std::{collections::HashMap, process, time::Duration};

use intents_models::constants::chains::ChainId;
use swap_estimator_rust::{
    error::ReportDisplayExt,
    prices::{
        TokenId,
        codex::pricing::{CodexProvider, CodexSubscription},
    },
};
use tokio::{signal, time};

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("codex ws demo error: {err}");
        process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    dotenv::dotenv().ok();

    let api_key = "".to_string();

    let provider = CodexProvider::new(api_key);
    let mut subscriptions: HashMap<String, (TokenId, CodexSubscription)> = HashMap::new();

    let demo_tokens = vec![
        (
            "WBTC",
            TokenId {
                chain: ChainId::Base,
                address: "0x0555E30da8f98308EdB960aa94C0Db47230d2B9c".to_string(),
            },
        ),
        (
            "WETH",
            TokenId {
                chain: ChainId::Base,
                address: "0x4200000000000000000000000000000000000006".to_string(),
            },
        ),
    ];

    println!("Subscribing to live prices on Base:");
    for (name, token) in demo_tokens.into_iter() {
        match provider
            .fetch_initial_prices(&[token.clone()])
            .await
            .map_err(|err| {
                format!(
                    "Failed to fetch initial HTTP price for {name}: {}",
                    err.format()
                )
            })?
            .get(&token)
        {
            Some(price) => println!(
                "[http] {name} [{}] {} => ${:.6}",
                token.chain, token.address, price.price
            ),
            None => println!(
                "[http] {name} [{}] {} => price unavailable",
                token.chain, token.address
            ),
        }

        let mut subscription = provider
            .subscribe(token.clone())
            .await
            .map_err(|err| format!("Failed to subscribe to {name}: {}", err.format()))?;

        match subscription.wait_for_price(Duration::from_secs(30)).await {
            Ok(price) => println!(
                "[init] {name} [{}] {} => ${:.6}",
                token.chain, token.address, price.price
            ),
            Err(err) => {
                return Err(format!(
                    "Timed out waiting for initial price of {name}: {}",
                    err.format()
                ));
            }
        }

        subscriptions.insert(name.to_string(), (token, subscription));
    }

    println!("\nPolling most recent prices every second (Ctrl+C to stop) …");
    listen_for_updates(&provider, subscriptions).await
}

async fn listen_for_updates(
    provider: &CodexProvider,
    mut subscriptions: HashMap<String, (TokenId, CodexSubscription)>,
) -> Result<(), String> {
    let mut interval = time::interval(Duration::from_secs(1));
    let mut step: u32 = 0;
    let mut wbtc_unsubscribed = false;

    let ctrl_c = signal::ctrl_c();
    tokio::pin!(ctrl_c);

    loop {
        tokio::select! {
            _ = &mut ctrl_c => {
                println!("\nStopping subscriptions …");
                break;
            }
            _ = interval.tick() => {
                step += 1;

                if !wbtc_unsubscribed && step == 6 {
                    if let Some((token, subscription)) = subscriptions.remove("WBTC") {
                        drop(subscription);
                        provider
                            .unsubscribe(&token)
                            .await
                            .map_err(|err| format!("Failed to unsubscribe WBTC: {}", err.format()))?;
                        println!("\n[demo] Unsubscribed WBTC – continuing with WETH\n");
                        wbtc_unsubscribed = true;
                    }
                }

                if subscriptions.is_empty() {
                    println!("No active subscriptions remaining. Exiting.");
                    break;
                }

                print_tick(step, &subscriptions);

                if step >= 12 {
                    println!("Demo finished after {step} ticks.");
                    break;
                }
            }
        }
    }

    Ok(())
}

fn print_tick(step: u32, subscriptions: &HashMap<String, (TokenId, CodexSubscription)>) {
    println!("--- tick {step} ---");

    let mut entries: Vec<_> = subscriptions.iter().collect();
    entries.sort_by(|(a, _), (b, _)| a.cmp(b));

    for (name, (token, subscription)) in entries {
        match subscription.latest() {
            Some(price) => println!(
                "[{}] {name:<4} {} => ${:.6}",
                token.chain, token.address, price.price
            ),
            None => println!(
                "[{}] {name:<4} {} => awaiting update …",
                token.chain, token.address
            ),
        }
    }
    println!();
}
