use anyhow::Result;
use std::env;
use std::path::Path;

#[derive(Clone)]
pub struct Config {
    pub rpc_url: String,
    pub wss_url: Option<String>,
    pub relay_cmd: Option<String>,
    pub private_key: String,
    pub flap_bnb: f64,
    pub fourmeme_bnb: f64,
    pub gas_gwei: f64,
    pub max_gas_gwei: f64,
    pub gas_limit: u64,
    pub priority_gas_multiplier: f64,
    pub snipe_flap: bool,
    pub snipe_fourmeme: bool,
    pub nonce_refresh_interval: f64,
    pub blockrazor_forwarder: Option<String>,
    pub relay_endpoint: Option<String>,
    pub api_key: Option<String>,
    pub sendtx_cmd: Option<String>,
}

fn load_dotenvy() {
    let paths = [
        ".env",
        "config.env",
        "../config.env",
        "../bsc-sniper/config.env",
    ];
    for p in paths {
        if Path::new(p).exists() {
            let _ = dotenvy::from_filename(p);
        }
    }
}

pub fn relay_env_from_env() -> std::collections::HashMap<String, String> {
    let mut m = std::collections::HashMap::new();
    for key in &["RELAY_ENDPOINT", "API_KEY", "BLOCKRAZOR_FAST_MODE_URL", "BLOCKRAZOR_FAST_AUTH"] {
        if let Ok(v) = env::var(key) {
            if !v.is_empty() {
                m.insert((*key).to_string(), v);
            }
        }
    }
    m
}

pub fn load() -> Result<Config> {
    load_dotenvy();

    let rpc_url = env::var("RPC_URL")
        .or_else(|_| env::var("RPC_URL_HTTPS"))
        .unwrap_or_else(|_| "https://bsc-dataseed.binance.org".into());
    let wss_url = env::var("WSS_URL").ok();
    let relay_cmd = env::var("RELAY_CMD").ok().filter(|s| !s.is_empty());
    let private_key = env::var("PRIVATE_KEY").map_err(|_| {
        anyhow::anyhow!(
            "PRIVATE_KEY not set. Set it in .env or config.env in the project dir (current dir: {:?})",
            env::current_dir().unwrap_or_default()
        )
    })?;
    let flap_bnb: f64 = env::var("FLAP_BUY_BNB").unwrap_or_else(|_| "0.000001".into()).parse().unwrap_or(1e-6);
    let fourmeme_bnb: f64 = env::var("FOURMEME_BUY_BNB").unwrap_or_else(|_| "0.000001".into()).parse().unwrap_or(1e-6);
    let gas_gwei: f64 = env::var("GAS_GWEI").unwrap_or_else(|_| "10".into()).parse().unwrap_or(10.0);
    let max_gas_gwei: f64 = env::var("MAX_GAS_GWEI").unwrap_or_else(|_| "0".into()).parse().unwrap_or(0.0);
    let gas_limit: u64 = env::var("GAS_LIMIT").unwrap_or_else(|_| "350000".into()).parse().unwrap_or(350_000);
    let priority_gas_multiplier: f64 = env::var("PRIORITY_GAS_MULTIPLIER").unwrap_or_else(|_| "1.5".into()).parse().unwrap_or(1.5);
    let snipe_flap = env::var("SNIPE_FLAP").unwrap_or_else(|_| "true".into()).to_lowercase() == "true";
    let snipe_fourmeme = env::var("SNIPE_FOURMEME").unwrap_or_else(|_| "true".into()).to_lowercase() == "true";
    let nonce_refresh_interval: f64 = env::var("NONCE_REFRESH_INTERVAL").unwrap_or_else(|_| "1.5".into()).parse().unwrap_or(1.5);
    let blockrazor_forwarder = env::var("BLOCKRAZOR_FORWARDER_ADDRESS").ok().filter(|s| !s.is_empty());
    let relay_endpoint = env::var("RELAY_ENDPOINT").ok().filter(|s| !s.is_empty());
    let api_key = env::var("API_KEY").ok().filter(|s| !s.is_empty());
    let sendtx_cmd = env::var("SENDTX_CMD").ok().filter(|s| !s.is_empty());

    Ok(Config {
        rpc_url,
        wss_url,
        relay_cmd,
        private_key,
        flap_bnb,
        fourmeme_bnb,
        gas_gwei,
        max_gas_gwei,
        gas_limit,
        priority_gas_multiplier,
        snipe_flap,
        snipe_fourmeme,
        nonce_refresh_interval,
        blockrazor_forwarder,
        relay_endpoint,
        api_key,
        sendtx_cmd,
    })
}
