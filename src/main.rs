mod buy;
mod config;
mod constants;
mod detect;
mod mempool;
mod send_raw;

use anyhow::Result;
use clap::{Parser, Subcommand};
use ethers::prelude::*;
use ethers::types::Eip1559TransactionRequest;
use std::env;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{SystemTime, UNIX_EPOCH};

use config::Config;
use constants::{BSC_CHAIN_ID, BUY_GAS_LIMIT};
use detect::{
    flap_token_from_calldata, flap_token_from_receipt, fourmeme_only_token_from_receipt,
    fourmeme_token_from_receipt, is_flap_launch, is_fourmeme_launch, is_fourmeme_target,
};
use std::fs;
use mempool::{stream_pending_txs_relay, stream_pending_txs_wss, PendingTxItem};

#[derive(Parser)]
#[command(name = "bsc-sniper-rs")]
#[command(about = "BSC 0-block sniper (Flap / FourMeme)")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Run,

    Parse,

    Buy {
        #[arg(long, help = "Token contract address")]
        token: String,
        #[arg(long, help = "BNB amount to spend (e.g. 0.001)")]
        amount: String,
        #[arg(long, default_value = "1.0", help = "Slippage tolerance percent (e.g. 1.0)")]
        slippage: String,
        #[arg(long, value_parser = ["flap", "fourmeme"], help = "Platform: flap or fourmeme")]
        platform: String,
    },
}

fn utc_ts() -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let secs = now.as_secs();
    let nanos = now.subsec_nanos();
    let (day, rest) = (secs / 86400, secs % 86400);
    let (hour, rest) = (rest / 3600, rest % 3600);
    let (min, sec) = (rest / 60, rest % 60);
    let (y, m, d) = days_to_ymd(day as u32);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}Z", y, m, d, hour, min, sec, nanos)
}

fn days_to_ymd(days: u32) -> (u32, u32, u32) {
    let mut d = days as i64 + 719468;
    let era = (if d >= 0 { d } else { d - 146096 }) / 146097;
    let doe = d - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe + era * 400) as u32;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

static CACHED_NONCE: AtomicU64 = AtomicU64::new(0);
static CACHED_MAX_FEE: AtomicU64 = AtomicU64::new(0);
static CACHED_TIP: AtomicU64 = AtomicU64::new(0);
static CACHED_FLAP_BNB_WEI: StdMutex<Option<u128>> = StdMutex::new(None);
static CACHED_FOURMEME_BNB_WEI: StdMutex<Option<u128>> = StdMutex::new(None);

fn get_cached_gas(config: &Config) -> (u128, u128) {
    let max_fee = CACHED_MAX_FEE.load(Ordering::SeqCst) as u128;
    let tip = CACHED_TIP.load(Ordering::SeqCst) as u128;
    let floor_max = (config.gas_gwei * 1e9 * config.priority_gas_multiplier) as u128;
    let floor_tip = (config.gas_gwei * 1e9 * (config.priority_gas_multiplier - 1.0)) as u128;
    (max_fee.max(floor_max), tip.max(floor_tip))
}

async fn refresh_gas_cache<P: ethers::providers::JsonRpcClient>(provider: &Provider<P>, config: &Config) {
    let floor_wei = (config.gas_gwei * 1e9) as u128;
    if let Ok(gp) = provider.get_gas_price().await {
        let chain_wei = gp.as_u128().max(floor_wei);
        let base = (chain_wei as f64 * 1.15) as u128;
        let base = base.max(chain_wei);
        let tip = (base as f64 * (config.priority_gas_multiplier - 1.0)) as u128;
        let max_fee = (base as f64 * config.priority_gas_multiplier) as u128;
        CACHED_MAX_FEE.store(max_fee.min(u64::MAX as u128) as u64, Ordering::SeqCst);
        CACHED_TIP.store(tip.min(u64::MAX as u128) as u64, Ordering::SeqCst);
    }
}

fn get_next_nonce() -> u64 {
    CACHED_NONCE.fetch_add(1, Ordering::SeqCst)
}

async fn bnb_wei_affordable<P: ethers::providers::JsonRpcClient>(
    provider: &Provider<P>,
    wallet: &LocalWallet,
    config: &Config,
    requested_bnb_wei: u128,
) -> u128 {
    let balance = provider.get_balance(wallet.address(), None).await.unwrap_or_default();
    let balance = balance.as_u128();
    let max_fee = CACHED_MAX_FEE.load(Ordering::SeqCst) as u128;
    let max_fee = if max_fee == 0 {
        (config.gas_gwei * 1e9 * config.priority_gas_multiplier) as u128
    } else {
        max_fee
    };
    let gas_reserve_raw = config.gas_limit as u128 * max_fee;
    let gas_reserve = gas_reserve_raw.min(balance * 70 / 100);
    let available = balance.saturating_sub(gas_reserve);
    if available == 0 {
        return 0;
    }
    available.min(requested_bnb_wei)
}

type Client = SignerMiddleware<Provider<Http>, LocalWallet>;

async fn send_buy_tx(
    client: &Client,
    config: &Config,
    tx: Eip1559TransactionRequest,
) -> Result<(String, &'static str)> {
    use ethers::signers::Signer;
    use ethers::types::transaction::eip2718::TypedTransaction;
    let typed = TypedTransaction::Eip1559(tx);
    let sig = client.signer().sign_transaction(&typed).await?;
    let raw = typed.rlp_signed(&sig);
    let pending = client.inner().send_raw_transaction(raw).await?;
    Ok((format!("{:?}", pending.tx_hash()), "rpc"))
}

async fn do_buy(
    client: Arc<Client>,
    config: &Config,
    platform: &str,
    token: Address,
) {
    if platform == "flap" && config.snipe_flap {
        let requested = (config.flap_bnb * 1e18) as u128;
        let bnb_wei = CACHED_FLAP_BNB_WEI
            .lock()
            .unwrap()
            .unwrap_or(0)
            .min(requested);
        if bnb_wei == 0 {
            println!("{} FLAP_BUY_SKIP insufficient balance for gas + value {:?}", utc_ts(), token);
            return;
        }
        let tx_req = match buy::build_flap_buy(token, bnb_wei.into(), 0) {
            Some(t) => t,
            None => return,
        };
        let (max_fee, tip) = get_cached_gas(config);
        let nonce = get_next_nonce();
        let tx = Eip1559TransactionRequest::new()
            .to(tx_req.to.unwrap_or(ethers::types::NameOrAddress::Address(ethers::types::Address::zero())))
            .value(tx_req.value.unwrap_or_default())
            .data(tx_req.data.unwrap_or_default())
            .chain_id(BSC_CHAIN_ID)
            .gas(BUY_GAS_LIMIT)
            .max_fee_per_gas(max_fee)
            .max_priority_fee_per_gas(tip)
            .nonce(nonce);
        match send_buy_tx(&client, config, tx).await {
            Ok((tx_hash, _via)) => {
                if let Ok(mut g) = CACHED_FLAP_BNB_WEI.lock() {
                    *g = g.map(|v| v.saturating_sub(bnb_wei)).or(Some(0));
                }
                // Fetch block number for bought tx
                let block_info = if let Ok(h) = H256::from_str(tx_hash.trim_start_matches("0x")) {
                    // Poll for receipt (transaction may not be mined immediately)
                    let mut block_num = None;
                    for _ in 0..10 {
                        if let Ok(Some(receipt)) = client.get_transaction_receipt(h).await {
                            block_num = receipt.block_number;
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                    if let Some(block) = block_num {
                        format!(" block {}", block)
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                println!("{} bought_tx https://bscscan.com/tx/{}{}", utc_ts(), tx_hash, block_info)
            }
            Err(e) => println!("{} FLAP_BUY_SEND_ERR {:?}", utc_ts(), e),
        }
    } else if platform == "fourmeme" && config.snipe_fourmeme {
        let requested = (config.fourmeme_bnb * 1e18) as u128;
        let bnb_wei = CACHED_FOURMEME_BNB_WEI
            .lock()
            .unwrap()
            .unwrap_or(0)
            .min(requested);
        if bnb_wei == 0 {
            println!("{} FOURMEME_BUY_SKIP insufficient balance for gas + value {:?}", utc_ts(), token);
            return;
        }
        let tx_req = match buy::build_fourmeme_buy(token, bnb_wei.into(), ethers::types::U256::zero()) {
            Some(t) => t,
            None => return,
        };
        let (max_fee, tip) = get_cached_gas(config);
        let nonce = get_next_nonce();
        let tx = Eip1559TransactionRequest::new()
            .to(tx_req.to.unwrap_or(ethers::types::NameOrAddress::Address(ethers::types::Address::zero())))
            .value(tx_req.value.unwrap_or_default())
            .data(tx_req.data.unwrap_or_default())
            .chain_id(BSC_CHAIN_ID)
            .gas(BUY_GAS_LIMIT)
            .max_fee_per_gas(max_fee)
            .max_priority_fee_per_gas(tip)
            .nonce(nonce);
        match send_buy_tx(&client, config, tx).await {
            Ok((tx_hash, _via)) => {
                if let Ok(mut g) = CACHED_FOURMEME_BNB_WEI.lock() {
                    *g = g.map(|v| v.saturating_sub(bnb_wei)).or(Some(0));
                }
                // Fetch block number for bought tx
                let block_info = if let Ok(h) = H256::from_str(tx_hash.trim_start_matches("0x")) {
                    // Poll for receipt (transaction may not be mined immediately)
                    let mut block_num = None;
                    for _ in 0..10 {
                        if let Ok(Some(receipt)) = client.get_transaction_receipt(h).await {
                            block_num = receipt.block_number;
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                    if let Some(block) = block_num {
                        format!(" block {}", block)
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                println!("{} bought_tx https://bscscan.com/tx/{}{}", utc_ts(), tx_hash, block_info)
            }
            Err(e) => println!("{} FOURMEME_BUY_SEND_ERR {:?}", utc_ts(), e),
        }
    }
}

async fn run_buy_command(
    client: Arc<Client>,
    config: &Config,
    token_str: &str,
    amount_str: &str,
    _slippage_str: &str,
    platform: &str,
) -> Result<()> {
    let t = token_str.trim();
    let addr_str = if t.starts_with("0x") { t.to_string() } else { format!("0x{}", t) };
    let token = Address::from_str(&addr_str)
        .map_err(|e| anyhow::anyhow!("Invalid --token: {} ({})", token_str, e))?;
    let amount_bnb: f64 = amount_str.trim().parse().map_err(|_| anyhow::anyhow!("Invalid --amount: {}", amount_str))?;
    let requested = (amount_bnb * 1e18) as u128;
    let provider = client.inner();
    let bnb_wei = bnb_wei_affordable(provider, client.signer(), config, requested).await;
    if bnb_wei == 0 {
        anyhow::bail!("Insufficient balance for gas + {} BNB", amount_bnb);
    }
    let (max_fee, tip) = get_cached_gas(&config);
    let nonce = get_next_nonce();

    if platform == "flap" {
        let tx_req = buy::build_flap_buy(token, bnb_wei.into(), 0)
            .ok_or_else(|| anyhow::anyhow!("build_flap_buy failed"))?;
        let tx = Eip1559TransactionRequest::new()
            .to(tx_req.to.unwrap_or(ethers::types::NameOrAddress::Address(ethers::types::Address::zero())))
            .value(tx_req.value.unwrap_or_default())
            .data(tx_req.data.unwrap_or_default())
            .chain_id(BSC_CHAIN_ID)
            .gas(BUY_GAS_LIMIT)
            .max_fee_per_gas(max_fee)
            .max_priority_fee_per_gas(tip)
            .nonce(nonce);
        match send_buy_tx(&client, &config, tx).await {
            Ok((tx_hash, via)) => println!("{} bought_tx {} token {:?} send_via {}", utc_ts(), tx_hash, token, via),
            Err(e) => anyhow::bail!("FLAP_BUY_SEND_ERR: {:?}", e),
        }
    } else if platform == "fourmeme" {
        let tx_req = buy::build_fourmeme_buy(token, bnb_wei.into(), ethers::types::U256::zero())
            .ok_or_else(|| anyhow::anyhow!("build_fourmeme_buy failed"))?;
        let tx = Eip1559TransactionRequest::new()
            .to(tx_req.to.unwrap_or(ethers::types::NameOrAddress::Address(ethers::types::Address::zero())))
            .value(tx_req.value.unwrap_or_default())
            .data(tx_req.data.unwrap_or_default())
            .chain_id(BSC_CHAIN_ID)
            .gas(BUY_GAS_LIMIT)
            .max_fee_per_gas(max_fee)
            .max_priority_fee_per_gas(tip)
            .nonce(nonce);
        match send_buy_tx(&client, &config, tx).await {
            Ok((tx_hash, via)) => println!("{} bought_tx {} token {:?} send_via {}", utc_ts(), tx_hash, token, via),
            Err(e) => anyhow::bail!("FOURMEME_BUY_SEND_ERR: {:?}", e),
        }
    } else {
        anyhow::bail!("--platform must be flap or fourmeme");
    }
    Ok(())
}

fn run_parse() -> Result<()> {
    let dir = env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join("samples");
    let entries = fs::read_dir(&dir).map_err(|e| anyhow::anyhow!("samples dir: {}", e))?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "txt").unwrap_or(false) {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let platform = if name.starts_with("flap_") {
                "flap"
            } else if name.starts_with("fourmeme_") {
                "fourmeme"
            } else {
                continue;
            };
            let content = fs::read_to_string(&path)?;
            let mut tx_hash = String::new();
            let mut data_hex = String::new();
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with("tx_hash=") {
                    tx_hash = line.strip_prefix("tx_hash=").unwrap_or("").to_string();
                } else if line.starts_with("data_hex=") {
                    data_hex = line.strip_prefix("data_hex=").unwrap_or("").to_string();
                }
            }
            let data_hex = data_hex.trim_start_matches("0x");
            let data = match hex::decode(data_hex) {
                Ok(d) => d,
                Err(_) => {
                    eprintln!("{} invalid hex", name);
                    continue;
                }
            };
            let token = if platform == "flap" {
                flap_token_from_calldata(&data)
            } else {
                None
            };
            println!("{} tx_hash={} token={:?}", name, tx_hash, token);
        }
    }
    Ok(())
}

async fn run_0block_loop(
    client: Arc<Client>,
    config: Arc<Config>,
    relay_cmd: Option<String>,
    wss_url: Option<String>,
    relay_env: std::collections::HashMap<String, String>,
) -> bool {
    let use_relay = relay_cmd.is_some();
    let use_wss = wss_url.is_some();
    if !use_relay && !use_wss {
        return false;
    }
    let sources: Vec<&str> = [if use_relay { Some("relay") } else { None }, if use_wss { Some("wss") } else { None }]
        .into_iter()
        .flatten()
        .collect();
    println!("0-block mode: listening to {} (first-seen wins). Send via RPC.", sources.join("+"));
    let mut pending_count: u64 = 0;

    let (tx_send, mut rx) = tokio::sync::mpsc::unbounded_channel::<PendingTxItem>();

    if let Some(ref cmd) = relay_cmd {
        let cmd = cmd.clone();
        let env = relay_env.clone();
        let tx = tx_send.clone();
        tokio::spawn(async move {
            match stream_pending_txs_relay(&cmd, &env).await {
                Ok(stream) => {
                    let mut stream = std::pin::pin!(stream);
                    while let Some(item) = futures::stream::StreamExt::next(&mut stream).await {
                        let _ = tx.send(item);
                    }
                    eprintln!("{} RELAY stream ended (subprocess or pipe closed)", utc_ts());
                }
                Err(e) => eprintln!("{} RELAY failed to start: {}", utc_ts(), e),
            }
        });
    }
    if let Some(ref url) = wss_url {
        let url = url.clone();
        let tx = tx_send.clone();
        tokio::spawn(async move {
            match stream_pending_txs_wss(&url).await {
                Ok(mut stream) => {
                    while let Some(item) = futures::stream::StreamExt::next(&mut stream).await {
                        let _ = tx.send(item);
                    }
                    eprintln!("{} WSS stream ended (connection closed)", utc_ts());
                }
                Err(e) => eprintln!("{} WSS failed to connect: {}", utc_ts(), e),
            }
        });
    }
    // Keep the loop running even if relay and wss both die; also prints "Still listening..." periodically
    let tx_heartbeat = tx_send.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if tx_heartbeat.send(PendingTxItem::Heartbeat).is_err() {
                break;
            }
        }
    });
    drop(tx_send);

    while let Some(item) = rx.recv().await {
        match item {
            PendingTxItem::Connected { source: _ } => println!("{} Relay connected, listening for pending txs...", utc_ts()),
            PendingTxItem::Heartbeat => println!("{} Still listening... ({} pending txs seen)", utc_ts(), pending_count),
            PendingTxItem::Error { source, message } => println!("{} {} stream: {}", utc_ts(), source.to_uppercase(), message),
            PendingTxItem::Tx { source, tx_hash, to, data } => {
                pending_count += 1;
                let mut to = to;
                let mut data = data;
                if source == "wss" {
                    if let Some(ref hash) = tx_hash {
                        if let Ok(h) = H256::from_str(hash.trim_start_matches("0x")) {
                            if let Ok(Some(t)) = client.get_transaction(h).await {
                                to = t.to.as_ref().map(|a| format!("0x{:x}", a)).unwrap_or_default();
                                data = t.input.as_ref().to_vec();
                            }
                        }
                    }
                }
                if data.len() < 4 {
                    continue;
                }
                let mint_tx_id = tx_hash.as_ref().map(String::as_str).unwrap_or("");
                if config.snipe_flap && is_flap_launch(&to, &data) {
                    let block_info = if let Some(ref hash) = tx_hash {
                        if let Ok(h) = H256::from_str(hash.trim_start_matches("0x")) {
                            // Poll for receipt (transaction may not be mined immediately)
                            let mut block_num = None;
                            for _ in 0..5 {
                                if let Ok(Some(receipt)) = client.get_transaction_receipt(h).await {
                                    block_num = receipt.block_number;
                                    break;
                                }
                                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                            }
                            if let Some(block) = block_num {
                                format!(" block {}", block)
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };
                    println!("==========\n{} Mint tx https://bscscan.com/tx/{}{}", utc_ts(), mint_tx_id, block_info);
                    if let Some(token) = flap_token_from_calldata(&data) {
                        println!("{} Flap token https://bscscan.com/token/{:?}", utc_ts(), token);
                        do_buy(client.clone(), &config, "flap", token).await;
                    }
                    continue;
                }
                if config.snipe_fourmeme && is_fourmeme_launch(&to, &data) {
                    let block_info = if let Some(ref hash) = tx_hash {
                        if let Ok(h) = H256::from_str(hash.trim_start_matches("0x")) {
                            // Poll for receipt (transaction may not be mined immediately)
                            let mut block_num = None;
                            for _ in 0..5 {
                                if let Ok(Some(receipt)) = client.get_transaction_receipt(h).await {
                                    block_num = receipt.block_number;
                                    break;
                                }
                                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                            }
                            if let Some(block) = block_num {
                                format!(" block {}", block)
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };
                    println!("==========\n{} Mint tx https://bscscan.com/tx/{}{}", utc_ts(), mint_tx_id, block_info);
                    if let Some(ref hash_str) = tx_hash {
                        if let Ok(h) = H256::from_str(hash_str.trim_start_matches("0x")) {
                            let client = client.clone();
                            let config = config.clone();
                            tokio::spawn(async move {
                                // Mint tx may not be mined yet in 0-block; poll for receipt then get token.
                                const POLL_MS: u64 = 40;
                                const MAX_POLLS: u32 = 50;
                                for _ in 0..MAX_POLLS {
                                    if let Ok(Some(receipt)) = client.get_transaction_receipt(h).await {
                                        if let Some(token) = fourmeme_token_from_receipt(&receipt.logs) {
                                            println!("{} FourMeme token https://bscscan.com/token/{:?}", utc_ts(), token);
                                            do_buy(client, &config, "fourmeme", token).await;
                                            break;
                                        }
                                    }
                                    tokio::time::sleep(std::time::Duration::from_millis(POLL_MS)).await;
                                }
                            });
                        }
                    }
                }
            }
        }
    }
    true
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if matches!(cli.command.as_ref(), Some(Commands::Parse)) {
        return run_parse();
    }

    let config = Arc::new(config::load()?);
    if config.private_key.is_empty() || config.rpc_url.is_empty() {
        anyhow::bail!("PRIVATE_KEY and RPC_URL required");
    }

    let provider = Provider::<Http>::try_from(config.rpc_url.as_str())?;
    let wallet: LocalWallet = config.private_key.parse()?;
    let wallet = wallet.with_chain_id(BSC_CHAIN_ID);
    let client = Arc::new(SignerMiddleware::new(provider, wallet));
    let provider_ref = client.inner();

    let nonce = provider_ref.get_transaction_count(client.address(), None).await?;
    CACHED_NONCE.store(nonce.as_u64(), Ordering::SeqCst);
    refresh_gas_cache(provider_ref, &config).await;

    match cli.command.unwrap_or(Commands::Run) {
        Commands::Parse => unreachable!(),
        Commands::Buy { token, amount, slippage, platform } => {
            println!("{} buy token {} amount {} BNB slippage {}% platform {}", utc_ts(), token, amount, slippage, platform);
            run_buy_command(client, &config, &token, &amount, &slippage, &platform).await?;
            return Ok(());
        }
        Commands::Run => {}
    }

    let relay_env = config::relay_env_from_env();
    let use_relay = config.relay_cmd.is_some();
    let use_wss = config.wss_url.is_some();
    if use_relay || use_wss {
        println!("0-block ENABLED: RELAY={} WSS={} (mint from mempool -> same block)", use_relay, use_wss);
    } else {
        println!("WARNING: 0-block OFF — set RELAY_CMD and/or WSS_URL or you will snipe 1 block late (block mode)");
    }
    if config.sendtx_cmd.is_some() {
        println!("Tx send: BlockRazor (SENDTX_CMD) then RPC fallback");
    } else {
        println!("Tx send: RPC only (set SENDTX_CMD for BlockRazor)");
    }
    println!("Snipe: flap={} fourmeme={}", config.snipe_flap, config.snipe_fourmeme);

    let available_wei = bnb_wei_affordable(
        provider_ref,
        client.signer(),
        &config,
        u128::MAX,
    )
    .await;
    *CACHED_FLAP_BNB_WEI.lock().unwrap() = Some(available_wei);
    *CACHED_FOURMEME_BNB_WEI.lock().unwrap() = Some(available_wei);

    let client_gas = client.clone();
    let config_gas = config.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            refresh_gas_cache(client_gas.inner(), &config_gas).await;
        }
    });

    let ran_0block = run_0block_loop(
        client.clone(),
        config.clone(),
        config.relay_cmd.clone(),
        config.wss_url.clone(),
        relay_env,
    )
    .await;
    if !ran_0block {
        println!("No RELAY_CMD or WSS configured; block mode only (1-block).");
    }
    Ok(())
}
