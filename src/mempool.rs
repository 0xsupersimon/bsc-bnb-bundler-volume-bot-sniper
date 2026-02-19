//! Pending tx stream: WSS (eth_subscribe newPendingTransactions) and/or relay subprocess (merged).

use anyhow::Result;
use async_stream::stream;
use futures::Stream;
use futures_util::StreamExt;
use serde_json::json;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, Clone)]
pub enum PendingTxItem {
    
}

pub async fn stream_pending_txs_wss(
    wss_url: &str,
) -> Result<impl Stream<Item = PendingTxItem>> {
    
}

fn decode_raw_tx_legacy(raw: &[u8]) -> Option<(String, Vec<u8>)> {
    
}

fn decode_raw_tx_eip1559(raw: &[u8]) -> Option<(String, Vec<u8>)> {
    
}

pub async fn stream_pending_txs_relay(
    relay_cmd: &str,
    env: &HashMap<String, String>,
) -> Result<impl Stream<Item = PendingTxItem>> {
    
}
