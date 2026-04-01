use sqlx::{PgPool, Postgres, Row, Transaction};

use crate::SharedDbError;

pub struct PostgresTransaction<'a> {
    inner: Transaction<'a, Postgres>,
}

impl<'a> PostgresTransaction<'a> {
    pub fn inner_mut(&mut self) -> &mut Transaction<'a, Postgres> {
        &mut self.inner
    }

    pub async fn next_sequence(&mut self, name: &str) -> Result<u64, SharedDbError> {
        next_sequence_in(&mut self.inner, name).await
    }

    pub async fn commit(self) -> Result<(), SharedDbError> {
        self.inner.commit().await.map_err(SharedDbError::from)
    }

    pub async fn rollback(self) -> Result<(), SharedDbError> {
        self.inner.rollback().await.map_err(SharedDbError::from)
    }
}

pub async fn begin(pool: &PgPool) -> Result<PostgresTransaction<'_>, SharedDbError> {
    pool.begin()
        .await
        .map(|inner| PostgresTransaction { inner })
        .map_err(SharedDbError::from)
}

pub async fn next_sequence(pool: &PgPool, name: &str) -> Result<u64, SharedDbError> {
    let mut transaction = begin(pool).await?;
    let next = transaction.next_sequence(name).await?;
    transaction.commit().await?;
    Ok(next)
}

pub async fn next_sequence_in(
    transaction: &mut Transaction<'_, Postgres>,
    name: &str,
) -> Result<u64, SharedDbError> {
    sqlx::query(
        "INSERT INTO shared_sequences (name, value)
         VALUES ($1, 0)
         ON CONFLICT (name) DO NOTHING",
    )
    .bind(name)
    .execute(&mut **transaction)
    .await
    .map_err(SharedDbError::from)?;

    let row = sqlx::query(
        "UPDATE shared_sequences
         SET value = value + 1, updated_at = now()
         WHERE name = $1
         RETURNING value",
    )
    .bind(name)
    .fetch_one(&mut **transaction)
    .await
    .map_err(SharedDbError::from)?;

    let next: i64 = row.try_get("value").map_err(SharedDbError::from)?;
    Ok(next as u64)
}
