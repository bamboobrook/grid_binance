use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactManifest {
    pub path: PathBuf,
    pub checksum_sha256: String,
    pub kind: String,
    pub candidate_id: String,
    pub row_count: usize,
    pub byte_size: u64,
    pub content_type: String,
    pub format: String,
}

pub fn write_json_artifact(
    root: &Path,
    candidate_id: &str,
    kind: &str,
    rows: &[Value],
) -> Result<ArtifactManifest, String> {
    write_artifact_at(root, candidate_id, kind, rows)
}

pub fn write_task_json_artifact(
    root: &Path,
    task_id: &str,
    candidate_id: &str,
    kind: &str,
    rows: &[Value],
) -> Result<ArtifactManifest, String> {
    validate_segment(task_id, "task_id")?;
    let scoped_root = root.join(task_id);
    write_artifact_at(&scoped_root, candidate_id, kind, rows)
}

fn write_artifact_at(
    root: &Path,
    candidate_id: &str,
    kind: &str,
    rows: &[Value],
) -> Result<ArtifactManifest, String> {
    validate_segment(candidate_id, "candidate_id")?;
    validate_segment(kind, "kind")?;

    let dir = root.to_path_buf();
    fs::create_dir_all(&dir).map_err(|error| format!("create artifact dir: {error}"))?;
    let path = dir.join(format!("{candidate_id}-{kind}.jsonl"));
    let temp_suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("artifact temp clock error: {error}"))?
        .as_nanos();
    let temp_path = dir.join(format!(
        ".{candidate_id}-{kind}.{}.{temp_suffix}.tmp",
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&temp_path)
        .map_err(|error| format!("create temp artifact file: {error}"))?;
    let mut hasher = Sha256::new();
    let mut byte_size = 0_u64;

    for row in rows {
        let line =
            serde_json::to_vec(row).map_err(|error| format!("serialize json row: {error}"))?;
        file.write_all(&line)
            .map_err(|error| format!("write json row: {error}"))?;
        file.write_all(b"\n")
            .map_err(|error| format!("write json newline: {error}"))?;
        hasher.update(&line);
        hasher.update(b"\n");
        byte_size += line.len() as u64 + 1;
    }
    file.flush()
        .map_err(|error| format!("flush artifact file: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("sync artifact file: {error}"))?;
    drop(file);
    fs::rename(&temp_path, &path).map_err(|error| format!("rename artifact file: {error}"))?;
    File::open(&dir)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("sync artifact dir: {error}"))?;

    Ok(ArtifactManifest {
        path,
        checksum_sha256: hex_digest(hasher.finalize().as_slice()),
        kind: kind.to_owned(),
        candidate_id: candidate_id.to_owned(),
        row_count: rows.len(),
        byte_size,
        content_type: "application/x-ndjson".to_owned(),
        format: "jsonl".to_owned(),
    })
}

pub fn verify_artifact(manifest: &ArtifactManifest) -> Result<(), String> {
    let file = File::open(&manifest.path).map_err(|error| format!("open artifact: {error}"))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut row_count = 0_usize;
    let mut byte_size = 0_u64;
    let mut buffer = Vec::new();

    loop {
        buffer.clear();
        let read = reader
            .read_until(b'\n', &mut buffer)
            .map_err(|error| format!("read artifact: {error}"))?;
        if read == 0 {
            break;
        }
        byte_size += read as u64;
        hasher.update(&buffer);
        let json_slice = buffer.strip_suffix(b"\n").unwrap_or(&buffer);
        serde_json::from_slice::<Value>(json_slice)
            .map_err(|error| format!("invalid jsonl row: {error}"))?;
        row_count += 1;
    }

    let checksum = hex_digest(hasher.finalize().as_slice());
    if checksum != manifest.checksum_sha256 {
        return Err(format!(
            "artifact checksum mismatch for {}",
            manifest.path.display()
        ));
    }
    if row_count != manifest.row_count {
        return Err(format!(
            "artifact row count mismatch: expected {}, got {row_count}",
            manifest.row_count
        ));
    }
    if byte_size != manifest.byte_size {
        return Err(format!(
            "artifact byte size mismatch: expected {}, got {byte_size}",
            manifest.byte_size
        ));
    }
    Ok(())
}

fn validate_segment(value: &str, name: &str) -> Result<(), String> {
    if value.is_empty()
        || value.contains('/')
        || value.contains('\\')
        || value == "."
        || value == ".."
    {
        return Err(format!("invalid artifact {name}: {value}"));
    }
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_manifest_detects_checksum_mismatch() {
        let temp = tempfile::tempdir().unwrap();
        let manifest = write_json_artifact(
            temp.path(),
            "candidate-1",
            "equity",
            &[serde_json::json!({"equity": 100.0})],
        )
        .unwrap();
        std::fs::write(&manifest.path, b"corrupted").unwrap();
        assert!(verify_artifact(&manifest).is_err());
    }

    #[test]
    fn task_scoped_artifacts_do_not_overwrite_same_candidate_kind() {
        let temp = tempfile::tempdir().unwrap();

        let first = write_task_json_artifact(
            temp.path(),
            "task-1",
            "seed-1-0",
            "summary",
            &[serde_json::json!({"task": 1})],
        )
        .unwrap();
        let second = write_task_json_artifact(
            temp.path(),
            "task-2",
            "seed-1-0",
            "summary",
            &[serde_json::json!({"task": 2})],
        )
        .unwrap();

        assert_ne!(first.path, second.path);
        verify_artifact(&first).unwrap();
        verify_artifact(&second).unwrap();
    }

    #[test]
    fn overwrite_write_keeps_manifest_checksum_valid() {
        let temp = tempfile::tempdir().unwrap();
        let first = write_task_json_artifact(
            temp.path(),
            "task-1",
            "candidate-1",
            "summary",
            &[serde_json::json!({"version": 1})],
        )
        .unwrap();
        let second = write_task_json_artifact(
            temp.path(),
            "task-1",
            "candidate-1",
            "summary",
            &[
                serde_json::json!({"version": 2}),
                serde_json::json!({"version": 3}),
            ],
        )
        .unwrap();

        assert_eq!(first.path, second.path);
        assert_ne!(first.checksum_sha256, second.checksum_sha256);
        verify_artifact(&second).unwrap();
    }
}
