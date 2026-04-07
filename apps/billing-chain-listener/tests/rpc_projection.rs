use billing_chain_listener::rpc::{
    evm_transfer_to_observed, parse_runtime_config, solana_transfer_to_observed, EvmObservedLog,
    SolanaObservedTransfer,
};
use chrono::{TimeZone, Utc};

#[test]
fn runtime_config_requires_all_chain_rpc_urls() {
    std::env::remove_var("CHAIN_RPC_URL_ETH");
    std::env::remove_var("CHAIN_RPC_URL_BSC");
    std::env::remove_var("CHAIN_RPC_URL_SOL");

    let error = parse_runtime_config().expect_err("rpc config should reject missing urls");
    assert!(error.to_string().contains("CHAIN_RPC_URL_ETH"));
}

#[test]
fn evm_transfer_projection_carries_confirmations_and_normalized_fields() {
    let observed = evm_transfer_to_observed(
        "BSC",
        "USDT",
        EvmObservedLog {
            to_address: "0xabc".to_string(),
            amount: "20.50000000".to_string(),
            tx_hash: "0xhash".to_string(),
            block_number: 120,
            observed_at: Utc.with_ymd_and_hms(2026, 4, 4, 12, 0, 0).unwrap(),
        },
        123,
    )
    .expect("projection");

    assert_eq!(observed.chain, "BSC");
    assert_eq!(observed.asset, "USDT");
    assert_eq!(observed.address, "0xabc");
    assert_eq!(observed.amount, "20.50000000");
    assert_eq!(observed.tx_hash, "0xhash");
    assert_eq!(observed.confirmations, Some(4));
}

#[test]
fn solana_transfer_projection_uses_slot_confirmations() {
    let observed = solana_transfer_to_observed(
        "USDC",
        SolanaObservedTransfer {
            owner_or_token_account: "So11111111111111111111111111111111111111112".to_string(),
            amount: "54.000000".to_string(),
            signature: "5ignature".to_string(),
            slot: 800,
            observed_at: Utc.with_ymd_and_hms(2026, 4, 4, 12, 30, 0).unwrap(),
        },
        804,
    )
    .expect("projection");

    assert_eq!(observed.chain, "SOL");
    assert_eq!(observed.asset, "USDC");
    assert_eq!(
        observed.address,
        "So11111111111111111111111111111111111111112"
    );
    assert_eq!(observed.amount, "54.000000");
    assert_eq!(observed.tx_hash, "5ignature");
    assert_eq!(observed.confirmations, Some(5));
}
