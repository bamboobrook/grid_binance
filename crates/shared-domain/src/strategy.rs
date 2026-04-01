use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StrategyStatus {
    Draft,
    Running,
    Paused,
    Stopped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreflightFailure {
    pub step: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreflightReport {
    pub ok: bool,
    pub failures: Vec<PreflightFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Strategy {
    pub id: String,
    pub name: String,
    pub symbol: String,
    pub budget: String,
    pub grid_spacing_bps: u32,
    pub status: StrategyStatus,
    pub source_template_id: Option<String>,
    pub membership_ready: bool,
    pub exchange_ready: bool,
    pub symbol_ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StrategyTemplate {
    pub id: String,
    pub name: String,
    pub symbol: String,
    pub budget: String,
    pub grid_spacing_bps: u32,
    pub membership_ready: bool,
    pub exchange_ready: bool,
    pub symbol_ready: bool,
}
