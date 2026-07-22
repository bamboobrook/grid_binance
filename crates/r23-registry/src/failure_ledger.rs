//! Atomic failure ledger.
//!
//! §4.2 last paragraph: "non-complete terminal must atomically write a failure
//! ledger row. registry, ledger or trace append failure fails the experiment."
//!
//! The launcher writes the failure row in the same critical section as the
//! terminal registry row (see [`crate::launcher::Launcher::record_terminal`]).

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// One failure ledger row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureRow {
    pub failure_id: String,
    pub experiment_id: String,
    pub phase: String,
    pub status: String, // e.g. "failed_engine", "blocked_engine_data_or_execution"
    pub reason: String,
    /// Verbatim never-repeat rule this failure implies.
    pub never_repeat: String,
    pub recorded_at_utc: String,
}

/// Append-only JSONL failure ledger backed by a file.
pub struct FailureLedger {
    path: PathBuf,
}

impl FailureLedger {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self { path: path.as_ref().to_path_buf() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read_all(&self) -> Result<Vec<FailureRow>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let f = File::open(&self.path).with_context(|| format!("open ledger {}", self.path.display()))?;
        let reader = BufReader::new(f);
        let mut rows = Vec::new();
        for (i, line) in reader.lines().enumerate() {
            let line = line.with_context(|| format!("read ledger line {}", i))?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let row: FailureRow = serde_json::from_str(trimmed)
                .with_context(|| format!("parse ledger line {} as JSON", i))?;
            rows.push(row);
        }
        Ok(rows)
    }

    /// Atomically append one failure row.
    pub fn append(&self, row: &FailureRow) -> Result<()> {
        let mut json = serde_json::to_string(row)?;
        json.push('\n');
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("open ledger for append {}", self.path.display()))?;
        f.write_all(json.as_bytes())
            .with_context(|| format!("write ledger row {}", self.path.display()))?;
        f.flush()?;
        Ok(())
    }

    pub fn len(&self) -> Result<usize> {
        Ok(self.read_all()?.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn tmp() -> PathBuf {
        NamedTempFile::new().unwrap().into_temp_path().keep().unwrap()
    }

    #[test]
    fn append_and_read() {
        let p = tmp();
        let led = FailureLedger::new(&p);
        let row = FailureRow {
            failure_id: "R23-A01".to_string(),
            experiment_id: "exp001".to_string(),
            phase: "R0".to_string(),
            status: "blocked_engine_data_or_execution".to_string(),
            reason: "registry empty while downstream results exist".to_string(),
            never_repeat: "do not promote any G1/G2 result without a complete registry row".to_string(),
            recorded_at_utc: "2026-07-22T00:00:00Z".to_string(),
        };
        led.append(&row).unwrap();
        assert_eq!(led.len().unwrap(), 1);
        let _ = std::fs::remove_file(&p);
    }
}
