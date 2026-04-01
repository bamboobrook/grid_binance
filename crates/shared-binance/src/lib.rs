pub mod client;
pub mod metadata;

pub use client::{BinanceClient, ExchangeCredentialCheck};
pub use metadata::{matches_symbol_query, SymbolMetadata};
