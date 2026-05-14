use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared_db::{
    BacktestCandidateRecord, BacktestRepository, BacktestTaskRecord, NewBacktestTaskRecord,
    SharedDb,
};
use std::collections::BTreeSet;

use crate::services::martingale_publish_service::{
    PublishPortfolioRequest, PublishPortfolioResponse,
};
use crate::services::{
    auth_service::AuthError,
    martingale_publish_service::{MartingalePublishService, PublishError, PublishIntentResponse},
};

#[derive(Clone)]
pub struct BacktestService {
    repo: BacktestRepository,
    publish: MartingalePublishService,
}

#[derive(Debug, Deserialize)]
pub struct CreateBacktestTaskRequest {
    pub strategy_type: String,
    #[serde(default)]
    pub symbols: Vec<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpsertQuotaRequest {
    #[serde(default = "default_max_symbols")]
    pub max_symbols: usize,
}

#[derive(Debug, Serialize)]
pub struct QuotaPolicyResponse {
    pub owner: String,
    pub policy: Value,
}

#[derive(Debug, Serialize)]
pub struct DeleteBacktestTaskResponse {
    pub task_id: String,
    pub deleted: bool,
}

impl BacktestService {
    pub fn new(db: SharedDb, publish: MartingalePublishService) -> Self {
        Self {
            repo: db.backtest_repo(),
            publish,
        }
    }

    pub fn create_task(
        &self,
        owner: &str,
        request: CreateBacktestTaskRequest,
    ) -> Result<BacktestTaskRecord, BacktestError> {
        let strategy_type = request.strategy_type;
        let mut config = Value::Object(request.extra);
        let effective_symbols = effective_task_symbols(&strategy_type, &request.symbols, &config)?;
        self.validate_quota(owner, effective_symbols.len())?;
        if let Value::Object(map) = &mut config {
            map.insert("symbols".to_owned(), json!(effective_symbols));
        }
        Ok(self.repo.create_task(NewBacktestTaskRecord {
            owner: owner.to_owned(),
            strategy_type,
            config,
            summary: json!({}),
        })?)
    }

    pub fn list_tasks(&self, owner: &str) -> Result<Vec<BacktestTaskRecord>, BacktestError> {
        Ok(self.repo.list_tasks_for_owner(owner)?)
    }

    pub fn get_task(
        &self,
        owner: &str,
        task_id: &str,
    ) -> Result<BacktestTaskRecord, BacktestError> {
        self.owned_task(owner, task_id)
    }

    pub fn pause_task(
        &self,
        owner: &str,
        task_id: &str,
    ) -> Result<BacktestTaskRecord, BacktestError> {
        let task = self.owned_task(owner, task_id)?;
        if task.status != "running" {
            return Err(BacktestError::conflict(
                "task can only be paused from running",
            ));
        }
        self.repo.transition_task(task_id, "paused")?;
        self.owned_task(owner, task_id)
    }

    pub fn resume_task(
        &self,
        owner: &str,
        task_id: &str,
    ) -> Result<BacktestTaskRecord, BacktestError> {
        let task = self.owned_task(owner, task_id)?;
        if task.status != "paused" {
            return Err(BacktestError::conflict(
                "task can only be resumed from paused",
            ));
        }
        self.repo.transition_task(task_id, "running")?;
        self.owned_task(owner, task_id)
    }

    pub fn cancel_task(
        &self,
        owner: &str,
        task_id: &str,
    ) -> Result<BacktestTaskRecord, BacktestError> {
        let task = self.owned_task(owner, task_id)?;
        if !matches!(task.status.as_str(), "queued" | "running" | "paused") {
            return Err(BacktestError::conflict(
                "task cannot be cancelled from current status",
            ));
        }
        self.repo.transition_task(task_id, "cancelled")?;
        self.owned_task(owner, task_id)
    }

    pub fn archive_task(
        &self,
        owner: &str,
        task_id: &str,
    ) -> Result<BacktestTaskRecord, BacktestError> {
        self.owned_task(owner, task_id)?;
        Ok(self.repo.archive_task(task_id)?)
    }

    pub fn delete_task(
        &self,
        owner: &str,
        task_id: &str,
    ) -> Result<DeleteBacktestTaskResponse, BacktestError> {
        let task = self.owned_task(owner, task_id)?;
        if matches!(task.status.as_str(), "queued" | "running" | "paused") {
            return Err(BacktestError::conflict(
                "task must be cancelled or completed before delete",
            ));
        }
        self.repo.delete_task(owner, task_id).map_err(|error| {
            let message = error.to_string();
            if message.contains("portfolio") || message.contains("cancelled or completed") {
                BacktestError::conflict(message)
            } else if message.contains("not found") {
                BacktestError::not_found("task not found")
            } else {
                BacktestError::from(error)
            }
        })?;
        Ok(DeleteBacktestTaskResponse {
            task_id: task_id.to_owned(),
            deleted: true,
        })
    }

    pub fn list_candidates(
        &self,
        owner: &str,
        task_id: &str,
    ) -> Result<Vec<BacktestCandidateRecord>, BacktestError> {
        self.owned_task(owner, task_id)?;
        Ok(self.repo.list_candidates(task_id)?)
    }

    pub fn get_candidate(
        &self,
        owner: &str,
        candidate_id: &str,
    ) -> Result<BacktestCandidateRecord, BacktestError> {
        let tasks = self.repo.list_tasks_for_owner(owner)?;
        for task in tasks {
            if let Some(candidate) = self
                .repo
                .list_candidates(&task.task_id)?
                .into_iter()
                .find(|candidate| candidate.candidate_id == candidate_id)
            {
                return Ok(candidate);
            }
        }
        Err(BacktestError::not_found("candidate not found"))
    }

    pub fn create_publish_intent(
        &self,
        owner: &str,
        candidate_id: &str,
    ) -> Result<PublishIntentResponse, BacktestError> {
        let candidate = self.get_candidate(owner, candidate_id)?;
        self.publish
            .create_pending_portfolio(owner, &candidate)
            .map_err(BacktestError::from)
    }

    pub fn publish_portfolio(
        &self,
        owner: &str,
        request: PublishPortfolioRequest,
    ) -> Result<PublishPortfolioResponse, BacktestError> {
        let task = self.owned_task(owner, &request.task_id)?;
        if !matches!(task.status.as_str(), "succeeded" | "completed") {
            return Err(BacktestError::conflict(
                "task must be succeeded before publishing portfolio",
            ));
        }
        let requested_candidate_ids = request
            .items
            .iter()
            .map(|item| item.candidate_id.as_str())
            .collect::<BTreeSet<_>>();
        let candidates = self
            .repo
            .list_candidates(&request.task_id)?
            .into_iter()
            .filter(|candidate| requested_candidate_ids.contains(candidate.candidate_id.as_str()))
            .collect::<Vec<_>>();
        self.publish
            .publish_portfolio(owner, request, candidates)
            .map_err(BacktestError::from)
    }

    pub fn get_quota_policy(&self, owner: &str) -> Result<QuotaPolicyResponse, BacktestError> {
        let policy = self
            .repo
            .find_quota_policy(owner)?
            .map(|record| record.policy)
            .unwrap_or_else(|| json!({ "max_symbols": default_max_symbols() }));
        Ok(QuotaPolicyResponse {
            owner: owner.to_owned(),
            policy,
        })
    }

    pub fn upsert_quota_policy(
        &self,
        owner: &str,
        request: UpsertQuotaRequest,
    ) -> Result<QuotaPolicyResponse, BacktestError> {
        let record = self
            .repo
            .upsert_quota_policy(owner, json!({ "max_symbols": request.max_symbols }))?;
        Ok(QuotaPolicyResponse {
            owner: record.owner,
            policy: record.policy,
        })
    }

    fn owned_task(&self, owner: &str, task_id: &str) -> Result<BacktestTaskRecord, BacktestError> {
        let task = self
            .repo
            .find_task(task_id)?
            .ok_or_else(|| BacktestError::not_found("task not found"))?;
        if task.owner != owner {
            return Err(BacktestError::not_found("task not found"));
        }
        Ok(task)
    }

    fn validate_quota(&self, owner: &str, symbol_count: usize) -> Result<(), BacktestError> {
        let max_symbols = self
            .repo
            .find_quota_policy(owner)?
            .and_then(|record| {
                record
                    .policy
                    .get("max_symbols")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize)
            })
            .unwrap_or_else(default_max_symbols);
        if symbol_count > max_symbols {
            return Err(BacktestError::forbidden(format!(
                "quota exceeded: max_symbols={max_symbols}"
            )));
        }
        Ok(())
    }
}

fn effective_task_symbols(
    strategy_type: &str,
    requested_symbols: &[String],
    config: &Value,
) -> Result<Vec<String>, BacktestError> {
    let requested = normalize_symbol_set(requested_symbols.iter().map(String::as_str));
    let portfolio = if strategy_type == "martingale_grid" {
        portfolio_strategy_symbols(config)?
    } else {
        BTreeSet::new()
    };

    if !requested.is_empty() && !portfolio.is_empty() && requested != portfolio {
        return Err(BacktestError::bad_request(
            "symbols do not match portfolio_config strategies",
        ));
    }

    let effective = if !portfolio.is_empty() {
        portfolio
    } else {
        requested
    };
    Ok(effective.into_iter().collect())
}

fn portfolio_strategy_symbols(config: &Value) -> Result<BTreeSet<String>, BacktestError> {
    let Some(strategies) = config
        .get("portfolio_config")
        .and_then(|portfolio| portfolio.get("strategies"))
        .and_then(Value::as_array)
    else {
        return Ok(BTreeSet::new());
    };

    let mut symbols = BTreeSet::new();
    for strategy in strategies {
        let symbol = strategy
            .get("symbol")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                BacktestError::bad_request("portfolio_config strategy symbol is required")
            })?;
        let normalized = normalize_symbol(symbol);
        if normalized.is_empty() {
            return Err(BacktestError::bad_request(
                "portfolio_config strategy symbol is required",
            ));
        }
        symbols.insert(normalized);
    }
    Ok(symbols)
}

fn normalize_symbol_set<'a>(symbols: impl Iterator<Item = &'a str>) -> BTreeSet<String> {
    symbols
        .map(normalize_symbol)
        .filter(|symbol| !symbol.is_empty())
        .collect()
}

fn normalize_symbol(symbol: &str) -> String {
    symbol.trim().to_uppercase()
}

fn default_max_symbols() -> usize {
    20
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared_db::SharedDb;

    fn request_with_symbols(count: usize) -> CreateBacktestTaskRequest {
        CreateBacktestTaskRequest {
            strategy_type: "martingale_grid".to_owned(),
            symbols: (0..count).map(|index| format!("SYM{index}USDT")).collect(),
            extra: serde_json::Map::new(),
        }
    }

    #[test]
    fn default_quota_allows_twenty_symbols_and_rejects_twenty_one() {
        let db = SharedDb::ephemeral().expect("db");
        let publish = MartingalePublishService::new(db.clone());
        let service = BacktestService::new(db, publish);

        let accepted = service.create_task("user@example.com", request_with_symbols(20));
        assert!(accepted.is_ok(), "default quota should allow 20 symbols");

        let rejected = service
            .create_task("user@example.com", request_with_symbols(21))
            .unwrap_err();
        assert_eq!(rejected.status, StatusCode::FORBIDDEN);
        assert_eq!(rejected.message, "quota exceeded: max_symbols=20");
    }
}

#[derive(Debug)]
pub struct BacktestError {
    status: StatusCode,
    message: String,
}

impl BacktestError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.into(),
        }
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }
}

impl From<AuthError> for BacktestError {
    fn from(error: AuthError) -> Self {
        let response = error.into_response();
        let status = response.status();
        Self {
            status,
            message: status.canonical_reason().unwrap_or("auth error").to_owned(),
        }
    }
}

impl From<shared_db::SharedDbError> for BacktestError {
    fn from(_error: shared_db::SharedDbError) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "backtest storage error".to_owned(),
        }
    }
}

impl From<PublishError> for BacktestError {
    fn from(error: PublishError) -> Self {
        let (status, message) = error.into_parts();
        Self { status, message }
    }
}

impl IntoResponse for BacktestError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}
