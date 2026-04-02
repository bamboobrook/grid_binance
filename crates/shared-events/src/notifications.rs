use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationKind {
    StrategyStarted,
    StrategyPaused,
    MembershipExpiring,
    DepositConfirmed,
    RuntimeError,
    ApiCredentialsInvalidated,
    GridFillExecuted,
    FillProfitReported,
    OverallTakeProfitTriggered,
    OverallStopLossTriggered,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationEvent {
    pub email: String,
    pub kind: NotificationKind,
    pub title: String,
    pub message: String,
    #[serde(default)]
    pub payload: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationRecord {
    pub event: NotificationEvent,
    pub telegram_delivered: bool,
    pub in_app_delivered: bool,
    pub show_expiry_popup: bool,
}
