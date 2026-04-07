use chrono::{DateTime, TimeZone, Utc};
use num_bigint::BigUint;
use num_traits::Zero;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use shared_db::SharedDb;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::{Error as IoError, ErrorKind},
    sync::Arc,
};
use tokio::sync::Mutex;

use crate::processor::ObservedChainTransfer;

const ERC20_TRANSFER_TOPIC: &str =
    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";
const ERC20_DECIMALS_CALL: &str = "0x313ce567";
const DEFAULT_EVM_INITIAL_LOOKBACK_BLOCKS: u64 = 128;
const DEFAULT_SOL_SIGNATURE_LIMIT: usize = 100;

#[derive(Debug, Clone)]
pub struct RpcRuntimeConfig {
    pub eth_rpc_url: String,
    pub bsc_rpc_url: String,
    pub sol_rpc_url: String,
    pub token_registry: BTreeMap<(String, String), String>,
    pub evm_initial_lookback_blocks: u64,
    pub sol_signature_limit: usize,
}

#[derive(Debug, Clone)]
pub struct SweepExecutorConfig {
    pub url: String,
    pub auth_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SweepExecutorResponse {
    tx_hash: Option<String>,
    signature: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PollCursorState {
    evm_last_scanned_block: HashMap<(String, String), u64>,
    sol_seen_signatures: HashSet<String>,
    token_decimals: HashMap<(String, String), u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmObservedLog {
    pub to_address: String,
    pub amount: String,
    pub tx_hash: String,
    pub block_number: u64,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolanaObservedTransfer {
    pub owner_or_token_account: String,
    pub amount: String,
    pub signature: String,
    pub slot: u64,
    pub observed_at: DateTime<Utc>,
}

pub fn parse_runtime_config() -> Result<RpcRuntimeConfig, IoError> {
    let eth_rpc_url = required_env("CHAIN_RPC_URL_ETH")?;
    let bsc_rpc_url = required_env("CHAIN_RPC_URL_BSC")?;
    let sol_rpc_url = required_env("CHAIN_RPC_URL_SOL")?;
    let mut token_registry = BTreeMap::new();

    for (chain, asset, env_name) in [
        ("ETH", "USDT", "CHAIN_TOKEN_CONTRACT_ETH_USDT"),
        ("ETH", "USDC", "CHAIN_TOKEN_CONTRACT_ETH_USDC"),
        ("BSC", "USDT", "CHAIN_TOKEN_CONTRACT_BSC_USDT"),
        ("BSC", "USDC", "CHAIN_TOKEN_CONTRACT_BSC_USDC"),
        ("SOL", "USDT", "CHAIN_TOKEN_MINT_SOL_USDT"),
        ("SOL", "USDC", "CHAIN_TOKEN_MINT_SOL_USDC"),
    ] {
        if let Some(value) = optional_env(env_name) {
            token_registry.insert((chain.to_string(), asset.to_string()), value);
        }
    }

    let evm_initial_lookback_blocks = std::env::var("CHAIN_LISTENER_EVM_INITIAL_LOOKBACK_BLOCKS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_EVM_INITIAL_LOOKBACK_BLOCKS);
    let sol_signature_limit = std::env::var("CHAIN_LISTENER_SOL_SIGNATURE_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_SOL_SIGNATURE_LIMIT);

    Ok(RpcRuntimeConfig {
        eth_rpc_url,
        bsc_rpc_url,
        sol_rpc_url,
        token_registry,
        evm_initial_lookback_blocks,
        sol_signature_limit,
    })
}

pub fn parse_sweep_executor_config() -> Result<SweepExecutorConfig, IoError> {
    Ok(SweepExecutorConfig {
        url: required_env("SWEEP_EXECUTOR_URL")?,
        auth_token: optional_env("SWEEP_EXECUTOR_AUTH_TOKEN"),
    })
}

pub async fn submit_sweep_transfer(
    http: &Client,
    config: &SweepExecutorConfig,
    sweep_job_id: u64,
    chain: &str,
    asset: &str,
    from_address: &str,
    to_address: &str,
    amount: &str,
) -> Result<String, IoError> {
    let mut request = http.post(&config.url).json(&json!({
        "sweep_job_id": sweep_job_id,
        "chain": chain,
        "asset": asset,
        "from_address": from_address,
        "to_address": to_address,
        "amount": amount,
    }));
    if let Some(token) = &config.auth_token {
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .await
        .map_err(http_error)?
        .error_for_status()
        .map_err(http_error)?;
    let payload: SweepExecutorResponse = response.json().await.map_err(http_error)?;
    payload
        .tx_hash
        .or(payload.signature)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| IoError::new(ErrorKind::InvalidData, "sweep executor response missing tx reference"))
}

pub async fn sweep_transfer_confirmed(
    http: &Client,
    config: &RpcRuntimeConfig,
    chain: &str,
    tx_hash: &str,
) -> Result<bool, IoError> {
    match chain {
        "ETH" => evm_transaction_confirmed(http, &config.eth_rpc_url, tx_hash).await,
        "BSC" => evm_transaction_confirmed(http, &config.bsc_rpc_url, tx_hash).await,
        "SOL" => solana_signature_confirmed(http, &config.sol_rpc_url, tx_hash).await,
        _ => Err(IoError::new(ErrorKind::InvalidInput, "unsupported sweep chain")),
    }
}

async fn evm_transaction_confirmed(http: &Client, rpc_url: &str, tx_hash: &str) -> Result<bool, IoError> {
    let receipt = rpc_call(http, rpc_url, "eth_getTransactionReceipt", json!([tx_hash])).await?;
    if receipt.is_null() {
        return Ok(false);
    }
    let status = receipt
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    Ok(!status.is_empty() && status != "0x0")
}

async fn solana_signature_confirmed(http: &Client, rpc_url: &str, signature: &str) -> Result<bool, IoError> {
    let value = rpc_call(
        http,
        rpc_url,
        "getSignatureStatuses",
        json!([[signature], { "searchTransactionHistory": true }]),
    )
    .await?;
    let statuses = value
        .get("value")
        .and_then(Value::as_array)
        .ok_or_else(|| IoError::new(ErrorKind::InvalidData, "signature status value missing"))?;
    let Some(status) = statuses.first() else {
        return Ok(false);
    };
    if status.is_null() {
        return Ok(false);
    }
    let confirmation_status = status
        .get("confirmationStatus")
        .and_then(Value::as_str)
        .unwrap_or_default();
    Ok(matches!(confirmation_status, "confirmed" | "finalized"))
}

pub fn evm_transfer_to_observed(
    chain: &str,
    asset: &str,
    log: EvmObservedLog,
    latest_block: u64,
) -> Result<ObservedChainTransfer, IoError> {
    let confirmations = latest_block
        .checked_sub(log.block_number)
        .map(|distance| distance + 1)
        .ok_or_else(|| IoError::new(ErrorKind::InvalidData, "latest block is behind log block"))?;

    Ok(ObservedChainTransfer {
        chain: chain.trim().to_uppercase(),
        asset: asset.trim().to_uppercase(),
        address: log.to_address,
        amount: log.amount,
        tx_hash: log.tx_hash,
        confirmations: Some(confirmations as u32),
        observed_at: log.observed_at,
    })
}

pub fn solana_transfer_to_observed(
    asset: &str,
    transfer: SolanaObservedTransfer,
    latest_slot: u64,
) -> Result<ObservedChainTransfer, IoError> {
    let confirmations = latest_slot
        .checked_sub(transfer.slot)
        .map(|distance| distance + 1)
        .ok_or_else(|| IoError::new(ErrorKind::InvalidData, "latest slot is behind observed slot"))?;

    Ok(ObservedChainTransfer {
        chain: "SOL".to_string(),
        asset: asset.trim().to_uppercase(),
        address: transfer.owner_or_token_account,
        amount: transfer.amount,
        tx_hash: transfer.signature,
        confirmations: Some(confirmations as u32),
        observed_at: transfer.observed_at,
    })
}

pub async fn collect_observed_transfers(
    db: &SharedDb,
    http: &Client,
    config: &RpcRuntimeConfig,
    state: &Arc<Mutex<PollCursorState>>,
) -> Result<Vec<ObservedChainTransfer>, IoError> {
    let enabled_addresses = db
        .list_deposit_addresses()
        .map_err(storage_error)?
        .into_iter()
        .filter(|record| record.is_enabled)
        .collect::<Vec<_>>();

    let mut grouped_addresses: HashMap<String, Vec<String>> = HashMap::new();
    for record in enabled_addresses {
        grouped_addresses
            .entry(record.chain.clone())
            .or_default()
            .push(record.address);
    }

    let mut collected = Vec::new();

    for chain in ["ETH", "BSC"] {
        let Some(addresses) = grouped_addresses.get(chain) else {
            continue;
        };
        for asset in ["USDT", "USDC"] {
            let Some(contract) = config
                .token_registry
                .get(&(chain.to_string(), asset.to_string()))
                .cloned()
            else {
                continue;
            };
            let rpc_url = if chain == "ETH" {
                &config.eth_rpc_url
            } else {
                &config.bsc_rpc_url
            };
            let latest_block = evm_latest_block(http, rpc_url).await?;
            let from_block = {
                let mut guard = state.lock().await;
                let key = (chain.to_string(), asset.to_string());
                let next_block = guard
                    .evm_last_scanned_block
                    .get(&key)
                    .copied()
                    .map(|block| block.saturating_add(1))
                    .unwrap_or_else(|| {
                        latest_block.saturating_sub(config.evm_initial_lookback_blocks.saturating_sub(1))
                    });
                guard.evm_last_scanned_block.insert(key, latest_block);
                next_block
            };
            if from_block > latest_block {
                continue;
            }

            let decimals = {
                let mut guard = state.lock().await;
                let key = (chain.to_string(), asset.to_string());
                if let Some(value) = guard.token_decimals.get(&key).copied() {
                    value
                } else {
                    let value = evm_token_decimals(http, rpc_url, &contract).await?;
                    guard.token_decimals.insert(key, value);
                    value
                }
            };

            let logs = evm_transfer_logs(
                http,
                rpc_url,
                &contract,
                from_block,
                latest_block,
                addresses,
                decimals,
            )
            .await?;

            for log in logs {
                collected.push(evm_transfer_to_observed(chain, asset, log, latest_block)?);
            }
        }
    }

    if let Some(addresses) = grouped_addresses.get("SOL") {
        let latest_slot = solana_latest_slot(http, &config.sol_rpc_url).await?;
        for asset in ["USDT", "USDC"] {
            let Some(mint) = config
                .token_registry
                .get(&("SOL".to_string(), asset.to_string()))
                .cloned()
            else {
                continue;
            };
            for address in addresses {
                let transfers = solana_token_transfers(
                    http,
                    &config.sol_rpc_url,
                    address,
                    &mint,
                    config.sol_signature_limit,
                    state,
                )
                .await?;
                for transfer in transfers {
                    collected.push(solana_transfer_to_observed(asset, transfer, latest_slot)?);
                }
            }
        }
    }

    Ok(collected)
}

async fn evm_latest_block(http: &Client, rpc_url: &str) -> Result<u64, IoError> {
    let value = rpc_call(http, rpc_url, "eth_blockNumber", json!([])).await?;
    parse_hex_u64(value.as_str().unwrap_or_default())
}

async fn evm_token_decimals(http: &Client, rpc_url: &str, contract: &str) -> Result<u32, IoError> {
    let value = rpc_call(
        http,
        rpc_url,
        "eth_call",
        json!([
            {
                "to": contract,
                "data": ERC20_DECIMALS_CALL,
            },
            "latest"
        ]),
    )
    .await?;
    let raw = parse_hex_u64(value.as_str().unwrap_or_default())?;
    u32::try_from(raw).map_err(|_| IoError::new(ErrorKind::InvalidData, "token decimals overflow"))
}

async fn evm_transfer_logs(
    http: &Client,
    rpc_url: &str,
    contract: &str,
    from_block: u64,
    to_block: u64,
    monitored_addresses: &[String],
    decimals: u32,
) -> Result<Vec<EvmObservedLog>, IoError> {
    if monitored_addresses.is_empty() {
        return Ok(Vec::new());
    }

    let topics = monitored_addresses
        .iter()
        .map(|address| json!(to_topic_address(address)))
        .collect::<Vec<_>>();
    let response = rpc_call(
        http,
        rpc_url,
        "eth_getLogs",
        json!([{
            "address": contract,
            "fromBlock": format!("0x{from_block:x}"),
            "toBlock": format!("0x{to_block:x}"),
            "topics": [ERC20_TRANSFER_TOPIC, Value::Null, topics]
        }]),
    )
    .await?;
    let logs: Vec<EvmLogResponse> = serde_json::from_value(response)
        .map_err(|error| IoError::new(ErrorKind::InvalidData, error.to_string()))?;
    let mut block_timestamps = HashMap::new();
    let mut projected = Vec::new();

    for log in logs {
        let tx_hash = log
            .transaction_hash
            .clone()
            .ok_or_else(|| IoError::new(ErrorKind::InvalidData, "missing transactionHash"))?;
        let block_number = parse_hex_u64(&log.block_number)?;
        let to_topic = log
            .topics
            .get(2)
            .ok_or_else(|| IoError::new(ErrorKind::InvalidData, "missing transfer destination topic"))?;
        let to_address = topic_to_address(to_topic)?;
        let observed_at = if let Some(cached) = block_timestamps.get(&block_number).copied() {
            cached
        } else {
            let ts = evm_block_timestamp(http, rpc_url, block_number).await?;
            block_timestamps.insert(block_number, ts);
            ts
        };
        projected.push(EvmObservedLog {
            to_address,
            amount: hex_amount_to_decimal_string(&log.data, decimals)?,
            tx_hash,
            block_number,
            observed_at,
        });
    }

    Ok(projected)
}

async fn evm_block_timestamp(
    http: &Client,
    rpc_url: &str,
    block_number: u64,
) -> Result<DateTime<Utc>, IoError> {
    let value = rpc_call(
        http,
        rpc_url,
        "eth_getBlockByNumber",
        json!([format!("0x{block_number:x}"), false]),
    )
    .await?;
    let timestamp = value
        .get("timestamp")
        .and_then(Value::as_str)
        .ok_or_else(|| IoError::new(ErrorKind::InvalidData, "block timestamp missing"))?;
    let unix = parse_hex_u64(timestamp)?;
    Utc.timestamp_opt(unix as i64, 0)
        .single()
        .ok_or_else(|| IoError::new(ErrorKind::InvalidData, "invalid block timestamp"))
}

async fn solana_latest_slot(http: &Client, rpc_url: &str) -> Result<u64, IoError> {
    let value = rpc_call(http, rpc_url, "getSlot", json!([])).await?;
    value
        .as_u64()
        .ok_or_else(|| IoError::new(ErrorKind::InvalidData, "getSlot result missing"))
}

async fn solana_token_transfers(
    http: &Client,
    rpc_url: &str,
    address: &str,
    mint: &str,
    limit: usize,
    state: &Arc<Mutex<PollCursorState>>,
) -> Result<Vec<SolanaObservedTransfer>, IoError> {
    let signatures_value = rpc_call(
        http,
        rpc_url,
        "getSignaturesForAddress",
        json!([
            address,
            {
                "limit": limit,
            }
        ]),
    )
    .await?;
    let signatures: Vec<SolanaSignatureInfo> = serde_json::from_value(signatures_value)
        .map_err(|error| IoError::new(ErrorKind::InvalidData, error.to_string()))?;
    let mut collected = Vec::new();

    for signature in signatures {
        let signature_key = format!("SOL:{}", signature.signature);
        {
            let guard = state.lock().await;
            if guard.sol_seen_signatures.contains(&signature_key) {
                continue;
            }
        }
        let tx_value = rpc_call(
            http,
            rpc_url,
            "getTransaction",
            json!([
                signature.signature,
                {
                    "encoding": "jsonParsed",
                    "maxSupportedTransactionVersion": 0
                }
            ]),
        )
        .await?;
        let transaction: SolanaTransactionResponse = serde_json::from_value(tx_value)
            .map_err(|error| IoError::new(ErrorKind::InvalidData, error.to_string()))?;
        let Some(transfer) = extract_solana_transfer(&transaction, address, mint)? else {
            continue;
        };
        {
            let mut guard = state.lock().await;
            guard.sol_seen_signatures.insert(signature_key);
        }
        collected.push(SolanaObservedTransfer {
            owner_or_token_account: address.to_string(),
            amount: transfer.amount,
            signature: transfer.signature,
            slot: transfer.slot,
            observed_at: transfer.observed_at,
        });
    }

    Ok(collected)
}

fn extract_solana_transfer(
    transaction: &SolanaTransactionResponse,
    monitored_address: &str,
    mint: &str,
) -> Result<Option<ExtractedSolanaTransfer>, IoError> {
    let instructions = transaction
        .transaction
        .message
        .instructions
        .as_slice();

    for instruction in instructions {
        let Some(parsed) = instruction.parsed.as_ref() else {
            continue;
        };
        let Some(info) = parsed.get("info") else {
            continue;
        };
        let destination = info
            .get("destination")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if destination != monitored_address {
            continue;
        }
        let instruction_mint = info.get("mint").and_then(Value::as_str).unwrap_or_default();
        if instruction_mint != mint {
            continue;
        }

        let amount = if let Some(ui_amount) = info
            .get("tokenAmount")
            .and_then(|value| value.get("uiAmountString"))
            .and_then(Value::as_str)
        {
            ui_amount.to_string()
        } else if let (Some(raw), Some(decimals)) = (
            info.get("amount").and_then(Value::as_str),
            info.get("decimals").and_then(Value::as_u64),
        ) {
            integer_amount_to_decimal_string(raw, decimals as u32)?
        } else {
            continue;
        };

        let observed_at = transaction
            .block_time
            .and_then(|block_time| Utc.timestamp_opt(block_time, 0).single())
            .unwrap_or_else(Utc::now);
        return Ok(Some(ExtractedSolanaTransfer {
            amount,
            signature: transaction.transaction.signatures.first().cloned().unwrap_or_default(),
            slot: transaction.slot,
            observed_at,
        }));
    }

    Ok(None)
}

async fn rpc_call(
    http: &Client,
    rpc_url: &str,
    method: &str,
    params: Value,
) -> Result<Value, IoError> {
    let response = http
        .post(rpc_url)
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        }))
        .send()
        .await
        .map_err(http_error)?
        .error_for_status()
        .map_err(http_error)?;
    let payload: RpcEnvelope = response.json().await.map_err(http_error)?;
    if let Some(error) = payload.error {
        return Err(IoError::new(
            ErrorKind::Other,
            format!("rpc {method} failed: {}", error.message),
        ));
    }
    payload
        .result
        .ok_or_else(|| IoError::new(ErrorKind::InvalidData, format!("rpc {method} result missing")))
}

fn required_env(name: &str) -> Result<String, IoError> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| IoError::new(ErrorKind::InvalidInput, format!("{name} is required")))
}

fn optional_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn parse_hex_u64(value: &str) -> Result<u64, IoError> {
    let trimmed = value.trim().trim_start_matches("0x");
    u64::from_str_radix(trimmed, 16)
        .map_err(|error| IoError::new(ErrorKind::InvalidData, error.to_string()))
}

fn hex_amount_to_decimal_string(value: &str, decimals: u32) -> Result<String, IoError> {
    let trimmed = value.trim().trim_start_matches("0x");
    let amount = BigUint::parse_bytes(trimmed.as_bytes(), 16)
        .ok_or_else(|| IoError::new(ErrorKind::InvalidData, "invalid transfer amount hex"))?;
    biguint_to_decimal_string(amount, decimals)
}

fn integer_amount_to_decimal_string(value: &str, decimals: u32) -> Result<String, IoError> {
    let amount = BigUint::parse_bytes(value.as_bytes(), 10)
        .ok_or_else(|| IoError::new(ErrorKind::InvalidData, "invalid transfer amount"))?;
    biguint_to_decimal_string(amount, decimals)
}

fn biguint_to_decimal_string(amount: BigUint, decimals: u32) -> Result<String, IoError> {
    if decimals == 0 {
        return Ok(amount.to_string());
    }
    let scale = BigUint::from(10u32).pow(decimals);
    let integer = &amount / &scale;
    let fractional = &amount % &scale;
    if fractional.is_zero() {
        return Ok(integer.to_string());
    }
    let mut fractional_text = fractional.to_string();
    let width = decimals as usize;
    if fractional_text.len() < width {
        fractional_text = format!("{:0>width$}", fractional_text, width = width);
    }
    while fractional_text.ends_with('0') {
        fractional_text.pop();
    }
    Ok(format!("{}.{}", integer, fractional_text))
}

fn to_topic_address(address: &str) -> String {
    let trimmed = address.trim().trim_start_matches("0x").to_ascii_lowercase();
    format!("0x{:0>64}", trimmed)
}

fn topic_to_address(topic: &str) -> Result<String, IoError> {
    let trimmed = topic.trim().trim_start_matches("0x");
    if trimmed.len() != 64 {
        return Err(IoError::new(ErrorKind::InvalidData, "invalid topic address width"));
    }
    Ok(format!("0x{}", &trimmed[24..]).to_ascii_lowercase())
}

fn storage_error(error: shared_db::SharedDbError) -> IoError {
    IoError::new(ErrorKind::Other, error.to_string())
}

fn http_error(error: reqwest::Error) -> IoError {
    IoError::new(ErrorKind::Other, error.to_string())
}

#[derive(Debug, Deserialize)]
struct RpcEnvelope {
    result: Option<Value>,
    error: Option<RpcErrorEnvelope>,
}

#[derive(Debug, Deserialize)]
struct RpcErrorEnvelope {
    message: String,
}

#[derive(Debug, Deserialize)]
struct EvmLogResponse {
    #[serde(rename = "transactionHash")]
    transaction_hash: Option<String>,
    #[serde(rename = "blockNumber")]
    block_number: String,
    topics: Vec<String>,
    data: String,
}

#[derive(Debug, Deserialize)]
struct SolanaSignatureInfo {
    signature: String,
}

#[derive(Debug, Deserialize)]
struct SolanaTransactionResponse {
    slot: u64,
    #[serde(rename = "blockTime")]
    block_time: Option<i64>,
    transaction: SolanaTransactionEnvelope,
}

#[derive(Debug, Deserialize)]
struct SolanaTransactionEnvelope {
    signatures: Vec<String>,
    message: SolanaMessageEnvelope,
}

#[derive(Debug, Deserialize)]
struct SolanaMessageEnvelope {
    instructions: Vec<SolanaInstructionEnvelope>,
}

#[derive(Debug, Deserialize)]
struct SolanaInstructionEnvelope {
    parsed: Option<Value>,
}

#[derive(Debug)]
struct ExtractedSolanaTransfer {
    amount: String,
    signature: String,
    slot: u64,
    observed_at: DateTime<Utc>,
}
