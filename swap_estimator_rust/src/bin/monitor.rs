use std::collections::{HashMap, HashSet};
use std::process;

use intents_models::constants::chains::ChainId;
use intents_models::log::init_tracing;
use swap_estimator_rust::monitoring::manager::MonitorManager;
use swap_estimator_rust::monitoring::messages::{MonitorAlert, MonitorRequest};
use swap_estimator_rust::prices::TokenId;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::sync::{broadcast, mpsc, oneshot};

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("monitor error: {err}");
        process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    dotenv::dotenv().ok();
    init_tracing(false);

    let codex_api_key = std::env::var("CODEX_API_KEY")
        .map_err(|_| "CODEX_API_KEY environment variable is not set".to_string())?;

    // Alerts channel (manager -> this binary)
    let (alert_tx, mut alert_rx) = broadcast::channel::<MonitorAlert>(100);
    // Requests channel (this binary -> manager)
    let (monitor_tx, monitor_rx) = mpsc::channel::<MonitorRequest>(100);

    // Spawn manager
    let manager = MonitorManager::new(monitor_rx, alert_tx, codex_api_key, (true, 5));
    tokio::spawn(async move {
        if let Err(e) = manager.run().await {
            eprintln!("MonitorManager stopped with error: {e:?}");
        }
    });

    // Spawn alerts listener
    tokio::spawn(async move {
        while let Ok(alert) = alert_rx.recv().await {
            match alert {
                MonitorAlert::SwapIsFeasible { order_id } => {
                    println!("[ALERT] Swap is feasible for order_id={order_id}");
                }
            }
        }
    });

    println!("Interactive monitor REPL ready.");
    println!("Commands:");
    println!(
        "  check <order_id> <src_chain> <dst_chain> <token_in> <token_out> <amount_in:u128> <amount_out:u128> <solver_last_bid:Option<u128>>"
    );
    println!("  remove <order_id>");
    println!("  prices <chain:address> [chain:address...]");
    println!("  quit");

    let stdin = BufReader::new(io::stdin());
    let mut lines = stdin.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let cmd = parts.next().unwrap_or_default();

        match cmd {
            "quit" | "exit" => {
                println!("Exiting…");
                break;
            }

            "remove" => {
                let order_id = match parts.next() {
                    Some(v) => v.to_string(),
                    None => {
                        eprintln!("Usage: remove <order_id>");
                        continue;
                    }
                };
                if let Err(e) = monitor_tx
                    .send(MonitorRequest::RemoveCheckSwapFeasibility { order_id })
                    .await
                {
                    eprintln!("Failed to send RemoveCheckSwapFeasibility: {e}");
                } else {
                    println!("Remove request sent");
                }
            }

            // check a 8453 7565164 0x833589fcd6edb6e08f4c7c32d4f71b54bda02913 orcaEKTdK7LKz57vaAYr9QeNsVEPfiu6QeMU1kektZE 1500000 1030000
            "check" => {
                // check <order_id> <src_chain> <dst_chain> <token_in> <token_out> <amount_in:u128> <amount_out:u128> <margin:f64>
                let order_id = match parts.next() {
                    Some(v) => v.to_string(),
                    None => {
                        eprintln!(
                            "Usage: check <order_id> <src_chain> <dst_chain> <token_in> <token_out> <amount_in> <amount_out> <margin>"
                        );
                        continue;
                    }
                };
                let src_chain = match parts.next().and_then(parse_chain_id) {
                    Some(v) => v,
                    None => {
                        eprintln!("Invalid or missing <src_chain>");
                        continue;
                    }
                };
                let dst_chain = match parts.next().and_then(parse_chain_id) {
                    Some(v) => v,
                    None => {
                        eprintln!("Invalid or missing <dst_chain>");
                        continue;
                    }
                };
                let token_in = match parts.next() {
                    Some(v) => v.to_string(),
                    None => {
                        eprintln!("Missing <token_in>");
                        continue;
                    }
                };
                let token_out = match parts.next() {
                    Some(v) => v.to_string(),
                    None => {
                        eprintln!("Missing <token_out>");
                        continue;
                    }
                };
                let amount_in: u128 = match parts.next().and_then(|s| s.parse().ok()) {
                    Some(v) => v,
                    None => {
                        eprintln!("Invalid <amount_in>");
                        continue;
                    }
                };
                let amount_out: u128 = match parts.next().and_then(|s| s.parse().ok()) {
                    Some(v) => v,
                    None => {
                        eprintln!("Invalid <amount_out>");
                        continue;
                    }
                };
                let solver_last_bid: Option<u128> = parts.next().and_then(|s| s.parse().ok());

                if let Err(e) = monitor_tx
                    .send(MonitorRequest::CheckSwapFeasibility {
                        order_id,
                        src_chain,
                        dst_chain,
                        token_in,
                        token_out,
                        amount_in,
                        amount_out,
                        solver_last_bid,
                        extra_expenses: HashMap::new(),
                    })
                    .await
                {
                    eprintln!("Failed to send CheckSwapFeasibility: {e}");
                } else {
                    println!("Check request sent");
                }
            }

            "prices" => {
                // prices <chain:address> [chain:address...]
                let mut token_ids: HashSet<TokenId> = HashSet::new();
                let mut any = false;
                for tok in parts {
                    if let Some((c, a)) = tok.split_once(':') {
                        if let Some(chain) = parse_chain_id(c) {
                            token_ids.insert(TokenId::new(chain, a.to_string()));
                            any = true;
                        } else {
                            eprintln!("Unknown chain '{c}'");
                        }
                    } else {
                        eprintln!("Invalid token format '{tok}', expected chain:address");
                    }
                }
                if !any {
                    eprintln!("Usage: prices <chain:address> [chain:address...]");
                    continue;
                }

                let (tx, rx) = oneshot::channel();
                if let Err(e) = monitor_tx
                    .send(MonitorRequest::GetCoinsData {
                        token_ids,
                        resp: tx,
                    })
                    .await
                {
                    eprintln!("Failed to send GetCoinsData: {e}");
                    continue;
                }
                match rx.await {
                    Ok(Ok(map)) => {
                        for (id, price) in map {
                            println!(
                                "Price {}:{} => price={}, decimals={}",
                                id.chain, id.address, price.price, price.decimals
                            );
                        }
                    }
                    Ok(Err(e)) => eprintln!("GetCoinsData error: {e:?}"),
                    Err(e) => eprintln!("GetCoinsData oneshot recv error: {e}"),
                }
            }

            other => {
                eprintln!("Unknown command '{other}'");
            }
        }
    }

    Ok(())
}

fn parse_chain_id(s: &str) -> Option<ChainId> {
    // Parse s to u32
    if let Ok(id_num) = s.parse::<u32>() {
        ChainId::try_from(id_num).ok()
    } else {
        ChainId::try_from(s).ok()
    }
}
