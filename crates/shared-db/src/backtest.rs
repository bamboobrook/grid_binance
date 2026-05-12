use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared_domain::strategy::Decimal;
use sqlx::{PgPool, Row};

use crate::{lock_ephemeral, EphemeralState, SharedDb, SharedDbError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacktestTaskRecord {
    pub task_id: String,
    pub owner: String,
    pub status: String,
    pub strategy_type: String,
    pub config: Value,
    pub summary: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewBacktestTaskRecord {
    pub owner: String,
    pub strategy_type: String,
    pub config: Value,
    pub summary: Value,
}

impl NewBacktestTaskRecord {
    #[cfg(test)]
    pub fn fixture(owner: &str) -> Self {
        Self {
            owner: owner.to_owned(),
            strategy_type: "martingale_grid".to_owned(),
            config: serde_json::json!({ "symbol": "BTCUSDT", "timeframe": "1h" }),
            summary: serde_json::json!({}),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacktestCandidateRecord {
    pub candidate_id: String,
    pub task_id: String,
    pub status: String,
    pub rank: i32,
    pub config: Value,
    pub summary: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewBacktestCandidateRecord {
    pub task_id: String,
    pub status: String,
    pub rank: i32,
    pub config: Value,
    pub summary: Value,
}

impl NewBacktestCandidateRecord {
    #[cfg(test)]
    pub fn fixture(task_id: &str) -> Self {
        Self {
            task_id: task_id.to_owned(),
            status: "ready".to_owned(),
            rank: 1,
            config: serde_json::json!({ "spacing": "0.01", "take_profit": "0.02" }),
            summary: serde_json::json!({ "score": 1.0, "max_drawdown": "0.05" }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacktestArtifactRecord {
    pub artifact_id: String,
    pub candidate_id: String,
    pub artifact_type: String,
    pub uri: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewBacktestArtifactRecord {
    pub candidate_id: String,
    pub artifact_type: String,
    pub uri: String,
    pub metadata: Value,
}

impl NewBacktestArtifactRecord {
    #[cfg(test)]
    pub fn fixture(candidate_id: &str) -> Self {
        Self {
            candidate_id: candidate_id.to_owned(),
            artifact_type: "summary".to_owned(),
            uri: "memory://backtest-summary.json".to_owned(),
            metadata: serde_json::json!({ "content_type": "application/json" }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacktestQuotaPolicyRecord {
    pub owner: String,
    pub policy: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MartingalePortfolioRecord {
    pub portfolio_id: String,
    pub owner: String,
    pub name: String,
    pub status: String,
    pub source_task_id: String,
    pub market: String,
    pub direction: String,
    pub risk_profile: String,
    pub total_weight_pct: Decimal,
    pub config: Value,
    pub risk_summary: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub items: Vec<MartingalePortfolioItemRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MartingalePortfolioItemRecord {
    pub strategy_instance_id: String,
    pub portfolio_id: String,
    pub candidate_id: String,
    pub symbol: String,
    pub weight_pct: Decimal,
    pub leverage: i32,
    pub enabled: bool,
    pub status: String,
    pub parameter_snapshot: Value,
    pub metrics_snapshot: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewMartingalePortfolioRecord {
    pub portfolio_id: String,
    pub owner: String,
    pub name: String,
    pub status: String,
    pub source_task_id: String,
    pub market: String,
    pub direction: String,
    pub risk_profile: String,
    pub total_weight_pct: Decimal,
    pub config: Value,
    pub risk_summary: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewMartingalePortfolioItemRecord {
    pub strategy_instance_id: String,
    pub candidate_id: String,
    pub symbol: String,
    pub weight_pct: Decimal,
    pub leverage: i32,
    pub enabled: bool,
    pub status: String,
    pub parameter_snapshot: Value,
    pub metrics_snapshot: Value,
}

#[derive(Clone)]
pub struct BacktestRepository {
    backend: BacktestRepositoryBackend,
}

#[derive(Clone)]
enum BacktestRepositoryBackend {
    Runtime(PgPool),
    Ephemeral(Arc<Mutex<EphemeralState>>),
}

impl BacktestRepository {
    pub fn new(pool: PgPool) -> Self {
        Self {
            backend: BacktestRepositoryBackend::Runtime(pool),
        }
    }

    pub(crate) fn ephemeral(state: Arc<Mutex<EphemeralState>>) -> Self {
        Self {
            backend: BacktestRepositoryBackend::Ephemeral(state),
        }
    }

    pub fn create_task(
        &self,
        record: NewBacktestTaskRecord,
    ) -> Result<BacktestTaskRecord, SharedDbError> {
        match &self.backend {
            BacktestRepositoryBackend::Runtime(pool) => {
                let pool = pool.clone();
                SharedDb::block_on(async move {
                    let now = Utc::now();
                    let task_id = format!("bt_{}", now.timestamp_nanos_opt().unwrap_or_default());
                    let row = sqlx::query(
                        "INSERT INTO backtest_tasks (task_id, owner, status, strategy_type, config, summary, created_at, updated_at)
                         VALUES ($1, $2, 'queued', $3, $4, $5, $6, $6)
                         RETURNING task_id, owner, status, strategy_type, config, summary, created_at, updated_at, started_at, completed_at, error_message",
                    )
                    .bind(task_id)
                    .bind(record.owner)
                    .bind(record.strategy_type)
                    .bind(record.config)
                    .bind(record.summary)
                    .bind(now)
                    .fetch_one(&pool)
                    .await
                    .map_err(SharedDbError::from)?;
                    task_from_row(row)
                })
            }
            BacktestRepositoryBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                let task_id = next_ephemeral_id(&mut state.sequences, "backtest_task", "bt");
                let now = Utc::now();
                let task = BacktestTaskRecord {
                    task_id: task_id.clone(),
                    owner: record.owner,
                    status: "queued".to_owned(),
                    strategy_type: record.strategy_type,
                    config: record.config,
                    summary: record.summary,
                    created_at: now,
                    updated_at: now,
                    started_at: None,
                    completed_at: None,
                    error_message: None,
                };
                state.backtest_tasks.insert(task_id, task.clone());
                Ok(task)
            }
        }
    }

    pub fn find_task(&self, task_id: &str) -> Result<Option<BacktestTaskRecord>, SharedDbError> {
        match &self.backend {
            BacktestRepositoryBackend::Runtime(pool) => {
                let pool = pool.clone();
                let task_id = task_id.to_owned();
                SharedDb::block_on(async move {
                    let row = sqlx::query(
                        "SELECT task_id, owner, status, strategy_type, config, summary, created_at, updated_at, started_at, completed_at, error_message
                         FROM backtest_tasks WHERE task_id = $1",
                    )
                    .bind(task_id)
                    .fetch_optional(&pool)
                    .await
                    .map_err(SharedDbError::from)?;
                    row.map(task_from_row).transpose()
                })
            }
            BacktestRepositoryBackend::Ephemeral(state) => {
                Ok(lock_ephemeral(state)?.backtest_tasks.get(task_id).cloned())
            }
        }
    }

    pub fn claim_next_queued_task(&self) -> Result<Option<BacktestTaskRecord>, SharedDbError> {
        match &self.backend {
            BacktestRepositoryBackend::Runtime(pool) => {
                let pool = pool.clone();
                SharedDb::block_on(async move {
                    let mut tx = pool.begin().await.map_err(SharedDbError::from)?;
                    let row = sqlx::query(
                        "SELECT task_id
                         FROM backtest_tasks
                         WHERE status = 'queued'
                         ORDER BY COALESCE(
                             CASE WHEN summary->>'priority' ~ '^-?[0-9]+$' THEN (summary->>'priority')::bigint END,
                             CASE WHEN config->>'priority' ~ '^-?[0-9]+$' THEN (config->>'priority')::bigint END,
                             0
                         ) DESC, created_at ASC, task_id ASC
                         LIMIT 1
                         FOR UPDATE SKIP LOCKED",
                    )
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(SharedDbError::from)?;

                    let Some(row) = row else {
                        tx.commit().await.map_err(SharedDbError::from)?;
                        return Ok(None);
                    };
                    let task_id: String = row.try_get("task_id").map_err(SharedDbError::from)?;
                    let row = sqlx::query(
                        "UPDATE backtest_tasks
                         SET status = 'running', updated_at = now(), started_at = COALESCE(started_at, now())
                         WHERE task_id = $1 AND status = 'queued'
                         RETURNING task_id, owner, status, strategy_type, config, summary, created_at, updated_at, started_at, completed_at, error_message",
                    )
                    .bind(task_id)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(SharedDbError::from)?;
                    tx.commit().await.map_err(SharedDbError::from)?;
                    row.map(task_from_row).transpose()
                })
            }
            BacktestRepositoryBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                let task_id = state
                    .backtest_tasks
                    .values()
                    .filter(|task| task.status == "queued")
                    .max_by(|left, right| {
                        task_priority(left)
                            .cmp(&task_priority(right))
                            .then_with(|| right.created_at.cmp(&left.created_at))
                            .then_with(|| right.task_id.cmp(&left.task_id))
                    })
                    .map(|task| task.task_id.clone());
                let Some(task_id) = task_id else {
                    return Ok(None);
                };
                let task = state.backtest_tasks.get_mut(&task_id).ok_or_else(|| {
                    SharedDbError::new(format!("backtest task not found: {task_id}"))
                })?;
                let now = Utc::now();
                task.status = "running".to_owned();
                task.updated_at = now;
                if task.started_at.is_none() {
                    task.started_at = Some(now);
                }
                Ok(Some(task.clone()))
            }
        }
    }

    pub fn list_tasks_for_owner(
        &self,
        owner: &str,
    ) -> Result<Vec<BacktestTaskRecord>, SharedDbError> {
        match &self.backend {
            BacktestRepositoryBackend::Runtime(pool) => {
                let pool = pool.clone();
                let owner = owner.to_owned();
                SharedDb::block_on(async move {
                    let rows = sqlx::query(
                        "SELECT task_id, owner, status, strategy_type, config, summary, created_at, updated_at, started_at, completed_at, error_message
                         FROM backtest_tasks WHERE owner = $1 ORDER BY created_at DESC, task_id ASC",
                    )
                    .bind(owner)
                    .fetch_all(&pool)
                    .await
                    .map_err(SharedDbError::from)?;
                    rows.into_iter().map(task_from_row).collect()
                })
            }
            BacktestRepositoryBackend::Ephemeral(state) => {
                let mut items = lock_ephemeral(state)?
                    .backtest_tasks
                    .values()
                    .filter(|task| task.owner == owner)
                    .cloned()
                    .collect::<Vec<_>>();
                items.sort_by(|left, right| {
                    right
                        .created_at
                        .cmp(&left.created_at)
                        .then_with(|| left.task_id.cmp(&right.task_id))
                });
                Ok(items)
            }
        }
    }

    pub fn transition_task(&self, task_id: &str, status: &str) -> Result<(), SharedDbError> {
        validate_task_status(status)?;
        match &self.backend {
            BacktestRepositoryBackend::Runtime(pool) => {
                let pool = pool.clone();
                let task_id = task_id.to_owned();
                let status = status.to_owned();
                SharedDb::block_on(async move {
                    let result = sqlx::query(
                        "UPDATE backtest_tasks
                         SET status = $2,
                             updated_at = now(),
                             started_at = CASE WHEN $2 = 'running' AND started_at IS NULL THEN now() ELSE started_at END,
                             completed_at = CASE WHEN $2 IN ('succeeded', 'failed', 'cancelled') THEN now() ELSE completed_at END
                         WHERE task_id = $1
                           AND status NOT IN ('succeeded', 'failed', 'cancelled')
                           AND NOT (status = 'paused' AND $2 = 'queued')",
                    )
                    .bind(task_id)
                    .bind(status)
                    .execute(&pool)
                    .await
                    .map_err(SharedDbError::from)?;
                    if result.rows_affected() == 0 {
                        return Err(SharedDbError::new("backtest task not found or terminal"));
                    }
                    Ok(())
                })
            }
            BacktestRepositoryBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                let Some(task) = state.backtest_tasks.get_mut(task_id) else {
                    return Err(SharedDbError::new(format!(
                        "backtest task not found: {task_id}"
                    )));
                };
                if is_terminal_task_status(&task.status) {
                    return Err(SharedDbError::new(format!(
                        "backtest task is terminal: {task_id}"
                    )));
                }
                validate_task_transition(&task.status, status)?;
                let now = Utc::now();
                task.status = status.to_owned();
                task.updated_at = now;
                if status == "running" && task.started_at.is_none() {
                    task.started_at = Some(now);
                }
                if is_terminal_task_status(status) {
                    task.completed_at = Some(now);
                }
                Ok(())
            }
        }
    }


    pub fn update_task_summary(
        &self,
        task_id: &str,
        summary: serde_json::Value,
    ) -> Result<(), SharedDbError> {
        match &self.backend {
            BacktestRepositoryBackend::Runtime(pool) => {
                let pool = pool.clone();
                let task_id = task_id.to_owned();
                SharedDb::block_on(async move {
                    let result = sqlx::query(
                        "UPDATE backtest_tasks SET summary = summary || $2, updated_at = now() WHERE task_id = $1",
                    )
                    .bind(task_id)
                    .bind(summary)
                    .execute(&pool)
                    .await
                    .map_err(SharedDbError::from)?;
                    if result.rows_affected() == 0 {
                        return Err(SharedDbError::new("backtest task not found"));
                    }
                    Ok(())
                })
            }
            BacktestRepositoryBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                let Some(task) = state.backtest_tasks.get_mut(task_id) else {
                    return Err(SharedDbError::new(format!("backtest task not found: {task_id}")));
                };
                if let (Some(existing), Some(next)) = (task.summary.as_object_mut(), summary.as_object()) {
                    for (key, value) in next {
                        existing.insert(key.clone(), value.clone());
                    }
                } else {
                    task.summary = summary;
                }
                task.updated_at = Utc::now();
                Ok(())
            }
        }
    }

    pub fn fail_task(&self, task_id: &str, error_message: &str) -> Result<(), SharedDbError> {
        match &self.backend {
            BacktestRepositoryBackend::Runtime(pool) => {
                let pool = pool.clone();
                let task_id = task_id.to_owned();
                let error_message = error_message.to_owned();
                SharedDb::block_on(async move {
                    let result = sqlx::query(
                        "UPDATE backtest_tasks
                         SET status = 'failed', updated_at = now(), completed_at = now(), error_message = $2
                         WHERE task_id = $1 AND status NOT IN ('succeeded', 'failed', 'cancelled')",
                    )
                    .bind(task_id)
                    .bind(error_message)
                    .execute(&pool)
                    .await
                    .map_err(SharedDbError::from)?;
                    if result.rows_affected() == 0 {
                        return Err(SharedDbError::new("backtest task not found or terminal"));
                    }
                    Ok(())
                })
            }
            BacktestRepositoryBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                let Some(task) = state.backtest_tasks.get_mut(task_id) else {
                    return Err(SharedDbError::new(format!(
                        "backtest task not found: {task_id}"
                    )));
                };
                if is_terminal_task_status(&task.status) {
                    return Err(SharedDbError::new(format!(
                        "backtest task is terminal: {task_id}"
                    )));
                }
                let now = Utc::now();
                task.status = "failed".to_owned();
                task.updated_at = now;
                task.completed_at = Some(now);
                task.error_message = Some(error_message.to_owned());
                Ok(())
            }
        }
    }

    pub fn append_task_event(
        &self,
        task_id: &str,
        event_type: &str,
        payload: Value,
    ) -> Result<(), SharedDbError> {
        match &self.backend {
            BacktestRepositoryBackend::Runtime(pool) => {
                let pool = pool.clone();
                let task_id = task_id.to_owned();
                let event_type = event_type.to_owned();
                SharedDb::block_on(async move {
                    sqlx::query(
                        "INSERT INTO backtest_task_events (task_id, event_type, payload, created_at)
                         VALUES ($1, $2, $3, now())",
                    )
                    .bind(task_id)
                    .bind(event_type)
                    .bind(payload)
                    .execute(&pool)
                    .await
                    .map_err(SharedDbError::from)?;
                    Ok(())
                })
            }
            BacktestRepositoryBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                if !state.backtest_tasks.contains_key(task_id) {
                    return Err(SharedDbError::new(format!(
                        "backtest task not found: {task_id}"
                    )));
                }
                state.backtest_task_events.push(BacktestTaskEventRecord {
                    task_id: task_id.to_owned(),
                    event_type: event_type.to_owned(),
                    payload,
                    created_at: Utc::now(),
                });
                Ok(())
            }
        }
    }

    pub fn save_candidate(
        &self,
        record: NewBacktestCandidateRecord,
    ) -> Result<BacktestCandidateRecord, SharedDbError> {
        match &self.backend {
            BacktestRepositoryBackend::Runtime(pool) => {
                let pool = pool.clone();
                SharedDb::block_on(async move {
                    let now = Utc::now();
                    let candidate_id =
                        format!("btc_{}", now.timestamp_nanos_opt().unwrap_or_default());
                    let row = sqlx::query(
                        "INSERT INTO backtest_candidate_summaries (candidate_id, task_id, status, rank, config, summary, created_at, updated_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $7)
                         RETURNING candidate_id, task_id, status, rank, config, summary, created_at, updated_at",
                    )
                    .bind(candidate_id)
                    .bind(record.task_id)
                    .bind(record.status)
                    .bind(record.rank)
                    .bind(record.config)
                    .bind(record.summary)
                    .bind(now)
                    .fetch_one(&pool)
                    .await
                    .map_err(SharedDbError::from)?;
                    candidate_from_row(row)
                })
            }
            BacktestRepositoryBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                if !state.backtest_tasks.contains_key(&record.task_id) {
                    return Err(SharedDbError::new(format!(
                        "backtest task not found: {}",
                        record.task_id
                    )));
                }
                let candidate_id =
                    next_ephemeral_id(&mut state.sequences, "backtest_candidate", "bc");
                let now = Utc::now();
                let candidate = BacktestCandidateRecord {
                    candidate_id: candidate_id.clone(),
                    task_id: record.task_id,
                    status: record.status,
                    rank: record.rank,
                    config: record.config,
                    summary: record.summary,
                    created_at: now,
                    updated_at: now,
                };
                state
                    .backtest_candidates
                    .insert(candidate_id, candidate.clone());
                Ok(candidate)
            }
        }
    }

    pub fn list_candidates(
        &self,
        task_id: &str,
    ) -> Result<Vec<BacktestCandidateRecord>, SharedDbError> {
        match &self.backend {
            BacktestRepositoryBackend::Runtime(pool) => {
                let pool = pool.clone();
                let task_id = task_id.to_owned();
                SharedDb::block_on(async move {
                    let rows = sqlx::query(
                        "SELECT candidate_id, task_id, status, rank, config, summary, created_at, updated_at
                         FROM backtest_candidate_summaries WHERE task_id = $1 ORDER BY rank ASC, created_at ASC",
                    )
                    .bind(task_id)
                    .fetch_all(&pool)
                    .await
                    .map_err(SharedDbError::from)?;
                    rows.into_iter().map(candidate_from_row).collect()
                })
            }
            BacktestRepositoryBackend::Ephemeral(state) => {
                let mut items = lock_ephemeral(state)?
                    .backtest_candidates
                    .values()
                    .filter(|candidate| candidate.task_id == task_id)
                    .cloned()
                    .collect::<Vec<_>>();
                items.sort_by(|left, right| {
                    left.rank
                        .cmp(&right.rank)
                        .then_with(|| left.created_at.cmp(&right.created_at))
                });
                Ok(items)
            }
        }
    }

    pub fn save_artifact(
        &self,
        record: NewBacktestArtifactRecord,
    ) -> Result<BacktestArtifactRecord, SharedDbError> {
        match &self.backend {
            BacktestRepositoryBackend::Runtime(pool) => {
                let pool = pool.clone();
                SharedDb::block_on(async move {
                    let now = Utc::now();
                    let artifact_id =
                        format!("bta_{}", now.timestamp_nanos_opt().unwrap_or_default());
                    let row = sqlx::query(
                        "INSERT INTO backtest_artifacts (artifact_id, candidate_id, artifact_type, uri, metadata, created_at)
                         VALUES ($1, $2, $3, $4, $5, $6)
                         RETURNING artifact_id, candidate_id, artifact_type, uri, metadata, created_at",
                    )
                    .bind(artifact_id)
                    .bind(record.candidate_id)
                    .bind(record.artifact_type)
                    .bind(record.uri)
                    .bind(record.metadata)
                    .bind(now)
                    .fetch_one(&pool)
                    .await
                    .map_err(SharedDbError::from)?;
                    artifact_from_row(row)
                })
            }
            BacktestRepositoryBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                if !state.backtest_candidates.contains_key(&record.candidate_id) {
                    return Err(SharedDbError::new(format!(
                        "backtest candidate not found: {}",
                        record.candidate_id
                    )));
                }
                let artifact_id =
                    next_ephemeral_id(&mut state.sequences, "backtest_artifact", "ba");
                let artifact = BacktestArtifactRecord {
                    artifact_id: artifact_id.clone(),
                    candidate_id: record.candidate_id,
                    artifact_type: record.artifact_type,
                    uri: record.uri,
                    metadata: record.metadata,
                    created_at: Utc::now(),
                };
                state
                    .backtest_artifacts
                    .insert(artifact_id, artifact.clone());
                Ok(artifact)
            }
        }
    }

    pub fn save_candidate_with_artifact(
        &self,
        candidate_record: NewBacktestCandidateRecord,
        artifact_type: impl Into<String>,
        uri: impl Into<String>,
        metadata: Value,
    ) -> Result<(BacktestCandidateRecord, BacktestArtifactRecord), SharedDbError> {
        match &self.backend {
            BacktestRepositoryBackend::Runtime(pool) => {
                let pool = pool.clone();
                let artifact_type = artifact_type.into();
                let uri = uri.into();
                SharedDb::block_on(async move {
                    let mut tx = pool.begin().await.map_err(SharedDbError::from)?;
                    let now = Utc::now();
                    let candidate_id =
                        format!("btc_{}", now.timestamp_nanos_opt().unwrap_or_default());
                    let candidate_row = sqlx::query(
                        "INSERT INTO backtest_candidate_summaries (candidate_id, task_id, status, rank, config, summary, created_at, updated_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $7)
                         RETURNING candidate_id, task_id, status, rank, config, summary, created_at, updated_at",
                    )
                    .bind(&candidate_id)
                    .bind(candidate_record.task_id)
                    .bind(candidate_record.status)
                    .bind(candidate_record.rank)
                    .bind(candidate_record.config)
                    .bind(candidate_record.summary)
                    .bind(now)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(SharedDbError::from)?;
                    let artifact_id =
                        format!("bta_{}", now.timestamp_nanos_opt().unwrap_or_default());
                    let artifact_row = sqlx::query(
                        "INSERT INTO backtest_artifacts (artifact_id, candidate_id, artifact_type, uri, metadata, created_at)
                         VALUES ($1, $2, $3, $4, $5, $6)
                         RETURNING artifact_id, candidate_id, artifact_type, uri, metadata, created_at",
                    )
                    .bind(artifact_id)
                    .bind(&candidate_id)
                    .bind(artifact_type)
                    .bind(uri)
                    .bind(metadata)
                    .bind(now)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(SharedDbError::from)?;
                    tx.commit().await.map_err(SharedDbError::from)?;
                    Ok((
                        candidate_from_row(candidate_row)?,
                        artifact_from_row(artifact_row)?,
                    ))
                })
            }
            BacktestRepositoryBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                if !state.backtest_tasks.contains_key(&candidate_record.task_id) {
                    return Err(SharedDbError::new(format!(
                        "backtest task not found: {}",
                        candidate_record.task_id
                    )));
                }
                let candidate_id =
                    next_ephemeral_id(&mut state.sequences, "backtest_candidate", "bc");
                let now = Utc::now();
                let candidate = BacktestCandidateRecord {
                    candidate_id: candidate_id.clone(),
                    task_id: candidate_record.task_id,
                    status: candidate_record.status,
                    rank: candidate_record.rank,
                    config: candidate_record.config,
                    summary: candidate_record.summary,
                    created_at: now,
                    updated_at: now,
                };
                let artifact_id =
                    next_ephemeral_id(&mut state.sequences, "backtest_artifact", "ba");
                let artifact = BacktestArtifactRecord {
                    artifact_id: artifact_id.clone(),
                    candidate_id: candidate_id.clone(),
                    artifact_type: artifact_type.into(),
                    uri: uri.into(),
                    metadata,
                    created_at: now,
                };
                state
                    .backtest_candidates
                    .insert(candidate_id, candidate.clone());
                state
                    .backtest_artifacts
                    .insert(artifact_id, artifact.clone());
                Ok((candidate, artifact))
            }
        }
    }

    pub fn upsert_quota_policy(
        &self,
        owner: &str,
        policy: Value,
    ) -> Result<BacktestQuotaPolicyRecord, SharedDbError> {
        match &self.backend {
            BacktestRepositoryBackend::Runtime(pool) => {
                let pool = pool.clone();
                let owner = owner.to_owned();
                SharedDb::block_on(async move {
                    let row = sqlx::query(
                        "INSERT INTO backtest_quota_policies (owner, policy, created_at, updated_at)
                         VALUES ($1, $2, now(), now())
                         ON CONFLICT (owner) DO UPDATE
                         SET policy = EXCLUDED.policy, updated_at = now()
                         RETURNING owner, policy, created_at, updated_at",
                    )
                    .bind(owner)
                    .bind(policy)
                    .fetch_one(&pool)
                    .await
                    .map_err(SharedDbError::from)?;
                    quota_policy_from_row(row)
                })
            }
            BacktestRepositoryBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                let now = Utc::now();
                let record = match state.backtest_quota_policies.get(owner).cloned() {
                    Some(mut existing) => {
                        existing.policy = policy;
                        existing.updated_at = now;
                        existing
                    }
                    None => BacktestQuotaPolicyRecord {
                        owner: owner.to_owned(),
                        policy,
                        created_at: now,
                        updated_at: now,
                    },
                };
                state
                    .backtest_quota_policies
                    .insert(owner.to_owned(), record.clone());
                Ok(record)
            }
        }
    }

    pub fn find_quota_policy(
        &self,
        owner: &str,
    ) -> Result<Option<BacktestQuotaPolicyRecord>, SharedDbError> {
        match &self.backend {
            BacktestRepositoryBackend::Runtime(pool) => {
                let pool = pool.clone();
                let owner = owner.to_owned();
                SharedDb::block_on(async move {
                    let row = sqlx::query(
                        "SELECT owner, policy, created_at, updated_at FROM backtest_quota_policies WHERE owner = $1",
                    )
                    .bind(owner)
                    .fetch_optional(&pool)
                    .await
                    .map_err(SharedDbError::from)?;
                    row.map(quota_policy_from_row).transpose()
                })
            }
            BacktestRepositoryBackend::Ephemeral(state) => Ok(lock_ephemeral(state)?
                .backtest_quota_policies
                .get(owner)
                .cloned()),
        }
    }

    pub fn create_martingale_portfolio(
        &self,
        portfolio: NewMartingalePortfolioRecord,
        items: Vec<NewMartingalePortfolioItemRecord>,
    ) -> Result<MartingalePortfolioRecord, SharedDbError> {
        match &self.backend {
            BacktestRepositoryBackend::Runtime(pool) => {
                let pool = pool.clone();
                SharedDb::block_on(async move {
                    let mut tx = pool.begin().await.map_err(SharedDbError::from)?;
                    let owner = portfolio.owner.clone();
                    let source_task_id = portfolio.source_task_id.clone();
                    let task_owner: Option<String> = sqlx::query_scalar(
                        "SELECT owner FROM backtest_tasks WHERE task_id = $1",
                    )
                    .bind(&source_task_id)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(SharedDbError::from)?;
                    match task_owner {
                        Some(task_owner) if task_owner == owner => {}
                        Some(_) => {
                            return Err(SharedDbError::new(
                                "backtest task owner does not match portfolio owner",
                            ));
                        }
                        None => {
                            return Err(SharedDbError::new(format!(
                                "backtest task not found: {source_task_id}"
                            )));
                        }
                    }
                    for item in &items {
                        let candidate_task_id: Option<String> = sqlx::query_scalar(
                            "SELECT task_id FROM backtest_candidate_summaries WHERE candidate_id = $1",
                        )
                        .bind(&item.candidate_id)
                        .fetch_optional(&mut *tx)
                        .await
                        .map_err(SharedDbError::from)?;
                        match candidate_task_id {
                            Some(candidate_task_id) if candidate_task_id == source_task_id => {}
                            Some(_) => {
                                return Err(SharedDbError::new(format!(
                                    "backtest candidate does not belong to task: {}",
                                    item.candidate_id
                                )));
                            }
                            None => {
                                return Err(SharedDbError::new(format!(
                                    "backtest candidate not found: {}",
                                    item.candidate_id
                                )));
                            }
                        }
                    }
                    let row = sqlx::query(
                        "INSERT INTO martingale_portfolios (
                             portfolio_id, owner, name, status, source_task_id, market, direction,
                             risk_profile, total_weight_pct, config, risk_summary, created_at, updated_at
                         )
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::NUMERIC, $10, $11, now(), now())
                         RETURNING portfolio_id, owner, name, status, source_task_id, market, direction,
                             risk_profile, total_weight_pct::TEXT AS total_weight_pct, config, risk_summary,
                             created_at, updated_at",
                    )
                    .bind(portfolio.portfolio_id.clone())
                    .bind(portfolio.owner)
                    .bind(portfolio.name)
                    .bind(portfolio.status)
                    .bind(portfolio.source_task_id)
                    .bind(portfolio.market)
                    .bind(portfolio.direction)
                    .bind(portfolio.risk_profile)
                    .bind(portfolio.total_weight_pct.to_string())
                    .bind(portfolio.config)
                    .bind(portfolio.risk_summary)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(SharedDbError::from)?;
                    let mut record = martingale_portfolio_from_row(row)?;

                    for item in items {
                        let row = sqlx::query(
                            "INSERT INTO martingale_portfolio_items (
                                 strategy_instance_id, portfolio_id, candidate_id, symbol, weight_pct,
                                 leverage, enabled, status, parameter_snapshot, metrics_snapshot,
                                 created_at, updated_at
                             )
                             VALUES ($1, $2, $3, $4, $5::NUMERIC, $6, $7, $8, $9, $10, now(), now())
                             RETURNING strategy_instance_id, portfolio_id, candidate_id, symbol,
                                 weight_pct::TEXT AS weight_pct, leverage, enabled, status,
                                 parameter_snapshot, metrics_snapshot, created_at, updated_at",
                        )
                        .bind(item.strategy_instance_id)
                        .bind(record.portfolio_id.clone())
                        .bind(item.candidate_id)
                        .bind(item.symbol)
                        .bind(item.weight_pct.to_string())
                        .bind(item.leverage)
                        .bind(item.enabled)
                        .bind(item.status)
                        .bind(item.parameter_snapshot)
                        .bind(item.metrics_snapshot)
                        .fetch_one(&mut *tx)
                        .await
                        .map_err(SharedDbError::from)?;
                        record.items.push(martingale_portfolio_item_from_row(row)?);
                    }

                    tx.commit().await.map_err(SharedDbError::from)?;
                    Ok(record)
                })
            }
            BacktestRepositoryBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                let task = state
                    .backtest_tasks
                    .get(&portfolio.source_task_id)
                    .ok_or_else(|| {
                        SharedDbError::new(format!(
                            "backtest task not found: {}",
                            portfolio.source_task_id
                        ))
                    })?;
                if task.owner != portfolio.owner {
                    return Err(SharedDbError::new(
                        "backtest task owner does not match portfolio owner",
                    ));
                }
                if state
                    .martingale_portfolios
                    .contains_key(&portfolio.portfolio_id)
                {
                    return Err(SharedDbError::new(format!(
                        "martingale portfolio already exists: {}",
                        portfolio.portfolio_id
                    )));
                }
                for item in &items {
                    let candidate = state
                        .backtest_candidates
                        .get(&item.candidate_id)
                        .ok_or_else(|| {
                            SharedDbError::new(format!(
                                "backtest candidate not found: {}",
                                item.candidate_id
                            ))
                        })?;
                    if candidate.task_id != portfolio.source_task_id {
                        return Err(SharedDbError::new(format!(
                            "backtest candidate does not belong to task: {}",
                            item.candidate_id
                        )));
                    }
                    if state.martingale_portfolios.values().any(|existing| {
                        existing.items.iter().any(|existing_item| {
                            existing_item.strategy_instance_id == item.strategy_instance_id
                        })
                    }) {
                        return Err(SharedDbError::new(format!(
                            "martingale strategy instance already exists: {}",
                            item.strategy_instance_id
                        )));
                    }
                }
                let now = Utc::now();
                let portfolio_id = portfolio.portfolio_id.clone();
                let record = MartingalePortfolioRecord {
                    portfolio_id: portfolio.portfolio_id.clone(),
                    owner: portfolio.owner,
                    name: portfolio.name,
                    status: portfolio.status,
                    source_task_id: portfolio.source_task_id,
                    market: portfolio.market,
                    direction: portfolio.direction,
                    risk_profile: portfolio.risk_profile,
                    total_weight_pct: portfolio.total_weight_pct,
                    config: portfolio.config,
                    risk_summary: portfolio.risk_summary,
                    created_at: now,
                    updated_at: now,
                    items: items
                        .into_iter()
                        .map(|item| MartingalePortfolioItemRecord {
                            strategy_instance_id: item.strategy_instance_id,
                            portfolio_id: portfolio_id.clone(),
                            candidate_id: item.candidate_id,
                            symbol: item.symbol,
                            weight_pct: item.weight_pct,
                            leverage: item.leverage,
                            enabled: item.enabled,
                            status: item.status,
                            parameter_snapshot: item.parameter_snapshot,
                            metrics_snapshot: item.metrics_snapshot,
                            created_at: now,
                            updated_at: now,
                        })
                        .collect(),
                };
                state
                    .martingale_portfolios
                    .insert(portfolio_id, record.clone());
                Ok(record)
            }
        }
    }

    pub fn list_martingale_portfolios(
        &self,
        owner: &str,
    ) -> Result<Vec<MartingalePortfolioRecord>, SharedDbError> {
        match &self.backend {
            BacktestRepositoryBackend::Runtime(pool) => {
                let pool = pool.clone();
                let owner = owner.to_owned();
                SharedDb::block_on(async move {
                    let rows = sqlx::query(
                        "SELECT portfolio_id, owner, name, status, source_task_id, market, direction,
                             risk_profile, total_weight_pct::TEXT AS total_weight_pct, config, risk_summary,
                             created_at, updated_at
                         FROM martingale_portfolios
                         WHERE owner = $1
                         ORDER BY created_at DESC, portfolio_id ASC",
                    )
                    .bind(owner)
                    .fetch_all(&pool)
                    .await
                    .map_err(SharedDbError::from)?;
                    let mut portfolios = Vec::with_capacity(rows.len());
                    for row in rows {
                        let mut portfolio = martingale_portfolio_from_row(row)?;
                        portfolio.items =
                            fetch_martingale_portfolio_items(&pool, &portfolio.portfolio_id)
                                .await?;
                        portfolios.push(portfolio);
                    }
                    Ok(portfolios)
                })
            }
            BacktestRepositoryBackend::Ephemeral(state) => {
                let mut records: Vec<_> = lock_ephemeral(state)?
                    .martingale_portfolios
                    .values()
                    .filter(|record| record.owner == owner)
                    .cloned()
                    .collect();
                records.sort_by(|left, right| {
                    right
                        .created_at
                        .cmp(&left.created_at)
                        .then_with(|| left.portfolio_id.cmp(&right.portfolio_id))
                });
                Ok(records)
            }
        }
    }

    pub fn get_martingale_portfolio(
        &self,
        owner: &str,
        portfolio_id: &str,
    ) -> Result<Option<MartingalePortfolioRecord>, SharedDbError> {
        match &self.backend {
            BacktestRepositoryBackend::Runtime(pool) => {
                let pool = pool.clone();
                let owner = owner.to_owned();
                let portfolio_id = portfolio_id.to_owned();
                SharedDb::block_on(async move {
                    let row = sqlx::query(
                        "SELECT portfolio_id, owner, name, status, source_task_id, market, direction,
                             risk_profile, total_weight_pct::TEXT AS total_weight_pct, config, risk_summary,
                             created_at, updated_at
                         FROM martingale_portfolios
                         WHERE owner = $1 AND portfolio_id = $2",
                    )
                    .bind(owner)
                    .bind(portfolio_id)
                    .fetch_optional(&pool)
                    .await
                    .map_err(SharedDbError::from)?;
                    let Some(row) = row else {
                        return Ok(None);
                    };
                    let mut portfolio = martingale_portfolio_from_row(row)?;
                    portfolio.items =
                        fetch_martingale_portfolio_items(&pool, &portfolio.portfolio_id).await?;
                    Ok(Some(portfolio))
                })
            }
            BacktestRepositoryBackend::Ephemeral(state) => Ok(lock_ephemeral(state)?
                .martingale_portfolios
                .get(portfolio_id)
                .filter(|record| record.owner == owner)
                .cloned()),
        }
    }

    pub fn set_martingale_portfolio_status(
        &self,
        owner: &str,
        portfolio_id: &str,
        status: &str,
    ) -> Result<Option<MartingalePortfolioRecord>, SharedDbError> {
        match &self.backend {
            BacktestRepositoryBackend::Runtime(pool) => {
                let pool = pool.clone();
                let owner = owner.to_owned();
                let portfolio_id = portfolio_id.to_owned();
                let status = status.to_owned();
                SharedDb::block_on(async move {
                    let mut tx = pool.begin().await.map_err(SharedDbError::from)?;
                    let row = sqlx::query(
                        "UPDATE martingale_portfolios
                         SET status = $3, updated_at = now()
                         WHERE owner = $1 AND portfolio_id = $2
                         RETURNING portfolio_id, owner, name, status, source_task_id, market, direction,
                             risk_profile, total_weight_pct::TEXT AS total_weight_pct, config, risk_summary,
                             created_at, updated_at",
                    )
                    .bind(&owner)
                    .bind(&portfolio_id)
                    .bind(&status)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(SharedDbError::from)?;
                    let Some(row) = row else {
                        tx.commit().await.map_err(SharedDbError::from)?;
                        return Ok(None);
                    };
                    sqlx::query(
                        "UPDATE martingale_portfolio_items
                         SET status = $2, updated_at = now()
                         WHERE portfolio_id = $1",
                    )
                    .bind(&portfolio_id)
                    .bind(&status)
                    .execute(&mut *tx)
                    .await
                    .map_err(SharedDbError::from)?;
                    let mut portfolio = martingale_portfolio_from_row(row)?;
                    portfolio.items = fetch_martingale_portfolio_items_tx(
                        &mut tx,
                        &portfolio.portfolio_id,
                    )
                    .await?;
                    tx.commit().await.map_err(SharedDbError::from)?;
                    Ok(Some(portfolio))
                })
            }
            BacktestRepositoryBackend::Ephemeral(state) => {
                let mut state = lock_ephemeral(state)?;
                let Some(record) = state.martingale_portfolios.get_mut(portfolio_id) else {
                    return Ok(None);
                };
                if record.owner != owner {
                    return Ok(None);
                }
                record.status = status.to_owned();
                record.updated_at = Utc::now();
                for item in &mut record.items {
                    item.status = status.to_owned();
                    item.updated_at = record.updated_at;
                }
                Ok(Some(record.clone()))
            }
        }
    }

}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BacktestTaskEventRecord {
    pub task_id: String,
    pub event_type: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

fn next_ephemeral_id(sequences: &mut HashMap<String, u64>, sequence: &str, prefix: &str) -> String {
    let next = sequences.entry(sequence.to_owned()).or_insert(0);
    *next += 1;
    format!("{prefix}_{next}")
}

fn validate_task_status(status: &str) -> Result<(), SharedDbError> {
    if matches!(
        status,
        "queued" | "running" | "paused" | "succeeded" | "failed" | "cancelled"
    ) {
        Ok(())
    } else {
        Err(SharedDbError::new(format!(
            "invalid backtest task status: {status}"
        )))
    }
}

fn is_terminal_task_status(status: &str) -> bool {
    matches!(status, "succeeded" | "failed" | "cancelled")
}

fn validate_task_transition(current: &str, next: &str) -> Result<(), SharedDbError> {
    if current == "paused" && next == "queued" {
        Err(SharedDbError::new(
            "invalid backtest task transition: paused -> queued",
        ))
    } else {
        Ok(())
    }
}

fn task_priority(task: &BacktestTaskRecord) -> i64 {
    task.summary
        .get("priority")
        .and_then(Value::as_i64)
        .or_else(|| task.config.get("priority").and_then(Value::as_i64))
        .unwrap_or(0)
}

fn task_from_row(row: sqlx::postgres::PgRow) -> Result<BacktestTaskRecord, SharedDbError> {
    Ok(BacktestTaskRecord {
        task_id: row.try_get("task_id").map_err(SharedDbError::from)?,
        owner: row.try_get("owner").map_err(SharedDbError::from)?,
        status: row.try_get("status").map_err(SharedDbError::from)?,
        strategy_type: row.try_get("strategy_type").map_err(SharedDbError::from)?,
        config: row.try_get("config").map_err(SharedDbError::from)?,
        summary: row.try_get("summary").map_err(SharedDbError::from)?,
        created_at: row.try_get("created_at").map_err(SharedDbError::from)?,
        updated_at: row.try_get("updated_at").map_err(SharedDbError::from)?,
        started_at: row.try_get("started_at").map_err(SharedDbError::from)?,
        completed_at: row.try_get("completed_at").map_err(SharedDbError::from)?,
        error_message: row.try_get("error_message").map_err(SharedDbError::from)?,
    })
}

fn candidate_from_row(
    row: sqlx::postgres::PgRow,
) -> Result<BacktestCandidateRecord, SharedDbError> {
    Ok(BacktestCandidateRecord {
        candidate_id: row.try_get("candidate_id").map_err(SharedDbError::from)?,
        task_id: row.try_get("task_id").map_err(SharedDbError::from)?,
        status: row.try_get("status").map_err(SharedDbError::from)?,
        rank: row.try_get("rank").map_err(SharedDbError::from)?,
        config: row.try_get("config").map_err(SharedDbError::from)?,
        summary: row.try_get("summary").map_err(SharedDbError::from)?,
        created_at: row.try_get("created_at").map_err(SharedDbError::from)?,
        updated_at: row.try_get("updated_at").map_err(SharedDbError::from)?,
    })
}

fn artifact_from_row(row: sqlx::postgres::PgRow) -> Result<BacktestArtifactRecord, SharedDbError> {
    Ok(BacktestArtifactRecord {
        artifact_id: row.try_get("artifact_id").map_err(SharedDbError::from)?,
        candidate_id: row.try_get("candidate_id").map_err(SharedDbError::from)?,
        artifact_type: row.try_get("artifact_type").map_err(SharedDbError::from)?,
        uri: row.try_get("uri").map_err(SharedDbError::from)?,
        metadata: row.try_get("metadata").map_err(SharedDbError::from)?,
        created_at: row.try_get("created_at").map_err(SharedDbError::from)?,
    })
}

fn quota_policy_from_row(
    row: sqlx::postgres::PgRow,
) -> Result<BacktestQuotaPolicyRecord, SharedDbError> {
    Ok(BacktestQuotaPolicyRecord {
        owner: row.try_get("owner").map_err(SharedDbError::from)?,
        policy: row.try_get("policy").map_err(SharedDbError::from)?,
        created_at: row.try_get("created_at").map_err(SharedDbError::from)?,
        updated_at: row.try_get("updated_at").map_err(SharedDbError::from)?,
    })
}

fn martingale_portfolio_from_row(
    row: sqlx::postgres::PgRow,
) -> Result<MartingalePortfolioRecord, SharedDbError> {
    let total_weight_pct: String = row
        .try_get("total_weight_pct")
        .map_err(SharedDbError::from)?;
    Ok(MartingalePortfolioRecord {
        portfolio_id: row.try_get("portfolio_id").map_err(SharedDbError::from)?,
        owner: row.try_get("owner").map_err(SharedDbError::from)?,
        name: row.try_get("name").map_err(SharedDbError::from)?,
        status: row.try_get("status").map_err(SharedDbError::from)?,
        source_task_id: row.try_get("source_task_id").map_err(SharedDbError::from)?,
        market: row.try_get("market").map_err(SharedDbError::from)?,
        direction: row.try_get("direction").map_err(SharedDbError::from)?,
        risk_profile: row.try_get("risk_profile").map_err(SharedDbError::from)?,
        total_weight_pct: total_weight_pct.parse().map_err(|error| {
            SharedDbError::new(format!("invalid portfolio weight decimal: {error}"))
        })?,
        config: row.try_get("config").map_err(SharedDbError::from)?,
        risk_summary: row.try_get("risk_summary").map_err(SharedDbError::from)?,
        created_at: row.try_get("created_at").map_err(SharedDbError::from)?,
        updated_at: row.try_get("updated_at").map_err(SharedDbError::from)?,
        items: Vec::new(),
    })
}

fn martingale_portfolio_item_from_row(
    row: sqlx::postgres::PgRow,
) -> Result<MartingalePortfolioItemRecord, SharedDbError> {
    let weight_pct: String = row.try_get("weight_pct").map_err(SharedDbError::from)?;
    Ok(MartingalePortfolioItemRecord {
        strategy_instance_id: row
            .try_get("strategy_instance_id")
            .map_err(SharedDbError::from)?,
        portfolio_id: row.try_get("portfolio_id").map_err(SharedDbError::from)?,
        candidate_id: row.try_get("candidate_id").map_err(SharedDbError::from)?,
        symbol: row.try_get("symbol").map_err(SharedDbError::from)?,
        weight_pct: weight_pct.parse().map_err(|error| {
            SharedDbError::new(format!("invalid item weight decimal: {error}"))
        })?,
        leverage: row.try_get("leverage").map_err(SharedDbError::from)?,
        enabled: row.try_get("enabled").map_err(SharedDbError::from)?,
        status: row.try_get("status").map_err(SharedDbError::from)?,
        parameter_snapshot: row
            .try_get("parameter_snapshot")
            .map_err(SharedDbError::from)?,
        metrics_snapshot: row
            .try_get("metrics_snapshot")
            .map_err(SharedDbError::from)?,
        created_at: row.try_get("created_at").map_err(SharedDbError::from)?,
        updated_at: row.try_get("updated_at").map_err(SharedDbError::from)?,
    })
}

async fn fetch_martingale_portfolio_items(
    pool: &PgPool,
    portfolio_id: &str,
) -> Result<Vec<MartingalePortfolioItemRecord>, SharedDbError> {
    let rows = sqlx::query(
        "SELECT strategy_instance_id, portfolio_id, candidate_id, symbol,
             weight_pct::TEXT AS weight_pct, leverage, enabled, status,
             parameter_snapshot, metrics_snapshot, created_at, updated_at
         FROM martingale_portfolio_items
         WHERE portfolio_id = $1
         ORDER BY created_at ASC, strategy_instance_id ASC",
    )
    .bind(portfolio_id)
    .fetch_all(pool)
    .await
    .map_err(SharedDbError::from)?;
    rows.into_iter()
        .map(martingale_portfolio_item_from_row)
        .collect()
}

async fn fetch_martingale_portfolio_items_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    portfolio_id: &str,
) -> Result<Vec<MartingalePortfolioItemRecord>, SharedDbError> {
    let rows = sqlx::query(
        "SELECT strategy_instance_id, portfolio_id, candidate_id, symbol,
             weight_pct::TEXT AS weight_pct, leverage, enabled, status,
             parameter_snapshot, metrics_snapshot, created_at, updated_at
         FROM martingale_portfolio_items
         WHERE portfolio_id = $1
         ORDER BY created_at ASC, strategy_instance_id ASC",
    )
    .bind(portfolio_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(SharedDbError::from)?;
    rows.into_iter()
        .map(martingale_portfolio_item_from_row)
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::{
        NewBacktestArtifactRecord, NewBacktestCandidateRecord,
        NewMartingalePortfolioItemRecord, NewMartingalePortfolioRecord, NewBacktestTaskRecord,
        SharedDb,
    };
    use shared_domain::strategy::Decimal;

    #[test]
    fn ephemeral_backtest_repo_upserts_quota_policy() {
        let db = SharedDb::ephemeral().expect("db");
        let repo = db.backtest_repo();
        let first = repo
            .upsert_quota_policy("quota@example.com", serde_json::json!({ "max_symbols": 2 }))
            .expect("insert quota");
        assert_eq!(first.policy["max_symbols"], 2);
        let second = repo
            .upsert_quota_policy("quota@example.com", serde_json::json!({ "max_symbols": 1 }))
            .expect("update quota");
        assert_eq!(second.policy["max_symbols"], 1);
        let found = repo
            .find_quota_policy("quota@example.com")
            .expect("find quota")
            .expect("quota exists");
        assert_eq!(found.policy["max_symbols"], 1);
        assert_eq!(found.created_at, first.created_at);
    }

    #[test]
    fn ephemeral_backtest_repo_creates_and_updates_task() {
        let db = SharedDb::ephemeral().unwrap();
        let repo = db.backtest_repo();
        let task = repo
            .create_task(NewBacktestTaskRecord::fixture("user@example.com"))
            .unwrap();
        assert_eq!(task.status, "queued");
        repo.transition_task(&task.task_id, "running").unwrap();
        assert_eq!(
            repo.find_task(&task.task_id).unwrap().unwrap().status,
            "running"
        );
    }

    #[test]
    fn ephemeral_backtest_repo_saves_candidate_and_artifact() {
        let db = SharedDb::ephemeral().unwrap();
        let repo = db.backtest_repo();
        let task = repo
            .create_task(NewBacktestTaskRecord::fixture("user@example.com"))
            .unwrap();
        let candidate = repo
            .save_candidate(NewBacktestCandidateRecord::fixture(&task.task_id))
            .unwrap();
        let artifact = repo
            .save_artifact(NewBacktestArtifactRecord::fixture(&candidate.candidate_id))
            .unwrap();
        assert_eq!(artifact.candidate_id, candidate.candidate_id);
    }

    #[test]
    fn ephemeral_backtest_repo_saves_candidate_with_artifact_atomically() {
        let db = SharedDb::ephemeral().unwrap();
        let repo = db.backtest_repo();
        let task = repo
            .create_task(NewBacktestTaskRecord::fixture("user@example.com"))
            .unwrap();

        let (candidate, artifact) = repo
            .save_candidate_with_artifact(
                NewBacktestCandidateRecord::fixture(&task.task_id),
                "summary",
                "file:///tmp/summary.jsonl",
                serde_json::json!({"checksum_sha256": "abc"}),
            )
            .unwrap();

        assert_eq!(artifact.candidate_id, candidate.candidate_id);
        assert_eq!(repo.list_candidates(&task.task_id).unwrap().len(), 1);
    }

    #[test]
    fn martingale_portfolio_repository_round_trips_multiple_items() {
        let db = SharedDb::ephemeral().unwrap();
        let repo = db.backtest_repo();
        let task = repo
            .create_task(NewBacktestTaskRecord::fixture("user@example.com"))
            .unwrap();
        let first_candidate = repo
            .save_candidate(NewBacktestCandidateRecord {
                task_id: task.task_id.clone(),
                status: "ready".to_owned(),
                rank: 1,
                config: serde_json::json!({ "symbol": "BTCUSDT", "spacing": "0.01" }),
                summary: serde_json::json!({ "score": 1.0 }),
            })
            .unwrap();
        let second_candidate = repo
            .save_candidate(NewBacktestCandidateRecord {
                task_id: task.task_id.clone(),
                status: "ready".to_owned(),
                rank: 2,
                config: serde_json::json!({ "symbol": "BTCUSDT", "spacing": "0.02" }),
                summary: serde_json::json!({ "score": 0.9 }),
            })
            .unwrap();

        let created = repo
            .create_martingale_portfolio(
                NewMartingalePortfolioRecord {
                    portfolio_id: "mp_test".to_owned(),
                    owner: "user@example.com".to_owned(),
                    name: "BTC basket".to_owned(),
                    status: "pending_confirmation".to_owned(),
                    source_task_id: task.task_id.clone(),
                    market: "futures".to_owned(),
                    direction: "long".to_owned(),
                    risk_profile: "balanced".to_owned(),
                    total_weight_pct: Decimal::new(100, 0),
                    config: serde_json::json!({ "source": "test" }),
                    risk_summary: serde_json::json!({ "risk": "medium" }),
                },
                vec![
                    NewMartingalePortfolioItemRecord {
                        strategy_instance_id: "msi_btc_1".to_owned(),
                        candidate_id: first_candidate.candidate_id.clone(),
                        symbol: "BTCUSDT".to_owned(),
                        weight_pct: Decimal::new(50, 0),
                        leverage: 3,
                        enabled: true,
                        status: "pending_confirmation".to_owned(),
                        parameter_snapshot: serde_json::json!({ "spacing": "0.01" }),
                        metrics_snapshot: serde_json::json!({ "score": 1.0 }),
                    },
                    NewMartingalePortfolioItemRecord {
                        strategy_instance_id: "msi_btc_2".to_owned(),
                        candidate_id: second_candidate.candidate_id.clone(),
                        symbol: "BTCUSDT".to_owned(),
                        weight_pct: Decimal::new(50, 0),
                        leverage: 4,
                        enabled: true,
                        status: "pending_confirmation".to_owned(),
                        parameter_snapshot: serde_json::json!({ "spacing": "0.02" }),
                        metrics_snapshot: serde_json::json!({ "score": 0.9 }),
                    },
                ],
            )
            .unwrap();

        assert_eq!(created.items.len(), 2);
        assert_ne!(created.items[0].strategy_instance_id, created.items[1].strategy_instance_id);

        let listed = repo
            .list_martingale_portfolios("user@example.com")
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].items.len(), 2);
        assert!(listed[0].items.iter().all(|item| item.symbol == "BTCUSDT"));

        let found = repo
            .get_martingale_portfolio("user@example.com", "mp_test")
            .unwrap()
            .expect("portfolio exists");
        assert_eq!(found.items.len(), 2);
        assert!(found
            .items
            .iter()
            .any(|item| item.strategy_instance_id == "msi_btc_1"));
        assert!(found
            .items
            .iter()
            .any(|item| item.strategy_instance_id == "msi_btc_2"));
    }

    #[test]
    fn ephemeral_backtest_repo_rejects_missing_task_event() {
        let db = SharedDb::ephemeral().unwrap();
        let repo = db.backtest_repo();

        assert!(repo
            .append_task_event("missing-task", "started", serde_json::json!({}))
            .is_err());
    }

    #[test]
    fn ephemeral_backtest_repo_rejects_missing_task_candidate() {
        let db = SharedDb::ephemeral().unwrap();
        let repo = db.backtest_repo();

        assert!(repo
            .save_candidate(NewBacktestCandidateRecord::fixture("missing-task"))
            .is_err());
    }

    #[test]
    fn ephemeral_backtest_repo_rejects_missing_candidate_artifact() {
        let db = SharedDb::ephemeral().unwrap();
        let repo = db.backtest_repo();

        assert!(repo
            .save_artifact(NewBacktestArtifactRecord::fixture("missing-candidate"))
            .is_err());
    }

    #[test]
    fn ephemeral_backtest_repo_persists_paused_status_without_completing() {
        let db = SharedDb::ephemeral().unwrap();
        let repo = db.backtest_repo();
        let task = repo
            .create_task(NewBacktestTaskRecord::fixture("user@example.com"))
            .unwrap();

        repo.transition_task(&task.task_id, "running").unwrap();
        repo.transition_task(&task.task_id, "paused").unwrap();
        let paused = repo.find_task(&task.task_id).unwrap().unwrap();

        assert_eq!(paused.status, "paused");
        assert!(paused.completed_at.is_none());
    }

    #[test]
    fn ephemeral_backtest_repo_allows_paused_resume_but_rejects_requeue() {
        let db = SharedDb::ephemeral().unwrap();
        let repo = db.backtest_repo();
        let task = repo
            .create_task(NewBacktestTaskRecord::fixture("user@example.com"))
            .unwrap();

        repo.transition_task(&task.task_id, "running").unwrap();
        repo.transition_task(&task.task_id, "paused").unwrap();
        assert!(repo.transition_task(&task.task_id, "queued").is_err());
        repo.transition_task(&task.task_id, "running").unwrap();

        let resumed = repo.find_task(&task.task_id).unwrap().unwrap();
        assert_eq!(resumed.status, "running");
        assert!(resumed.completed_at.is_none());
    }

    #[test]
    fn ephemeral_backtest_repo_rejects_invalid_task_status() {
        let db = SharedDb::ephemeral().unwrap();
        let repo = db.backtest_repo();
        let task = repo
            .create_task(NewBacktestTaskRecord::fixture("user@example.com"))
            .unwrap();

        assert!(repo.transition_task(&task.task_id, "bogus").is_err());
    }

    #[test]
    fn ephemeral_backtest_repo_claims_next_queued_task() {
        let db = SharedDb::ephemeral().unwrap();
        let repo = db.backtest_repo();
        let first = repo
            .create_task(NewBacktestTaskRecord::fixture("first@example.com"))
            .unwrap();
        let second = repo
            .create_task(NewBacktestTaskRecord::fixture("second@example.com"))
            .unwrap();

        let claimed = repo.claim_next_queued_task().unwrap().unwrap();

        assert_eq!(claimed.task_id, first.task_id);
        assert_eq!(claimed.status, "running");
        assert_eq!(
            repo.find_task(&second.task_id).unwrap().unwrap().status,
            "queued"
        );
        assert!(repo.claim_next_queued_task().unwrap().is_some());
        assert!(repo.claim_next_queued_task().unwrap().is_none());
    }

    #[test]
    fn ephemeral_backtest_repo_marks_failed_with_error() {
        let db = SharedDb::ephemeral().unwrap();
        let repo = db.backtest_repo();
        let task = repo
            .create_task(NewBacktestTaskRecord::fixture("user@example.com"))
            .unwrap();

        repo.fail_task(&task.task_id, "boom").unwrap();
        let failed = repo.find_task(&task.task_id).unwrap().unwrap();

        assert_eq!(failed.status, "failed");
        assert_eq!(failed.error_message.as_deref(), Some("boom"));
        assert!(failed.completed_at.is_some());
    }

    #[test]
    fn ephemeral_backtest_repo_rejects_terminal_task_reopen() {
        let db = SharedDb::ephemeral().unwrap();
        let repo = db.backtest_repo();
        let task = repo
            .create_task(NewBacktestTaskRecord::fixture("user@example.com"))
            .unwrap();

        repo.transition_task(&task.task_id, "succeeded").unwrap();

        assert!(repo.transition_task(&task.task_id, "running").is_err());
    }
}
