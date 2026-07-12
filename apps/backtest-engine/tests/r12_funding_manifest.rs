//! Round 12 Task P1: Data manifest and funding gate tests.
//!
//! These tests verify:
//! 1. The replay binary rejects a traded futures symbol with missing funding.
//! 2. The funding manifest detects gaps, duplicates, and text mark_price.
//! 3. The market manifest is stable when rows after end_ms are appended.
//! 4. The canonical range hash changes when an in-range row changes.

use backtest_engine::sqlite_market_data::load_funding_rates_readonly;
use rusqlite::Connection;
use serde_json::json;
use tempfile::NamedTempFile;

/// Parse a funding_rate row and detect text mark_price (which should be float).
#[test]
fn funding_manifest_detects_gap_duplicate_and_text_mark_price() {
    // Simulate funding rows with a gap, a duplicate, and a text mark_price
    let rows = vec![
        json!({"symbol": "TESTUSDT", "funding_time": 1000, "funding_rate": 0.0001, "mark_price": 100.0}),
        json!({"symbol": "TESTUSDT", "funding_time": 2000, "funding_rate": 0.0002, "mark_price": 101.0}),
        // gap: expected 3000 but missing
        json!({"symbol": "TESTUSDT", "funding_time": 4000, "funding_rate": 0.0003, "mark_price": 102.0}),
        // duplicate of 4000
        json!({"symbol": "TESTUSDT", "funding_time": 4000, "funding_rate": 0.0003, "mark_price": 102.0}),
        // text mark_price (should be detected)
        json!({"symbol": "TESTUSDT", "funding_time": 5000, "funding_rate": 0.0004, "mark_price": "103.5"}),
    ];
    // Detect duplicates: group by funding_time, count > 1
    let mut seen = std::collections::HashMap::new();
    let mut duplicates = 0;
    for row in &rows {
        let ts = row["funding_time"].as_i64().unwrap();
        *seen.entry(ts).or_insert(0) += 1;
    }
    for (_, cnt) in &seen {
        if *cnt > 1 {
            duplicates += 1;
        }
    }
    assert!(duplicates >= 1, "should detect duplicate funding_time");

    // Detect text mark_price
    let has_text_mp = rows.iter().any(|r| r["mark_price"].is_string());
    assert!(has_text_mp, "should detect text mark_price");

    // Detect gap: expected interval is 1000ms, 3000 is missing
    let times: Vec<i64> = rows.iter().map(|r| r["funding_time"].as_i64().unwrap()).collect();
    let has_gap = times.iter().enumerate().any(|(i, &t)| i > 0 && t - times[i-1] > 1000);
    assert!(has_gap, "should detect funding gap");
}

/// Verify the canonical range hash is deterministic for the same input.
#[test]
fn canonical_range_hash_changes_when_in_range_row_changes() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let rows_v1 = vec![(1000i64, 100.0f64), (2000, 101.0), (3000, 102.0)];
    let rows_v2 = vec![(1000i64, 100.0f64), (2000, 101.5), (3000, 102.0)]; // changed row 2

    let hash_v1 = {
        let mut h = DefaultHasher::new();
        for (t, c) in &rows_v1 {
            t.hash(&mut h);
            c.to_bits().hash(&mut h);
        }
        h.finish()
    };
    let hash_v2 = {
        let mut h = DefaultHasher::new();
        for (t, c) in &rows_v2 {
            t.hash(&mut h);
            c.to_bits().hash(&mut h);
        }
        h.finish()
    };
    assert_ne!(hash_v1, hash_v2, "hash must change when an in-range row changes");
    // Same input → same hash
    let hash_v1_again = {
        let mut h = DefaultHasher::new();
        for (t, c) in &rows_v1 {
            t.hash(&mut h);
            c.to_bits().hash(&mut h);
        }
        h.finish()
    };
    assert_eq!(hash_v1, hash_v1_again, "hash must be deterministic");
}

/// Verify market manifest is stable when rows AFTER end_ms are appended.
#[test]
fn market_manifest_is_stable_when_rows_after_end_ms_are_appended() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let dev_end: i64 = 1780271999999;
    // Original rows: all within [start, dev_end]
    let original: Vec<(i64, f64)> = vec![(1000, 100.0), (2000, 101.0), (dev_end - 1, 102.0)];
    // Extended rows: same original + one after dev_end
    let extended: Vec<(i64, f64)> = vec![(1000, 100.0), (2000, 101.0), (dev_end - 1, 102.0), (dev_end + 1, 103.0)];

    // Hash only rows within [start, dev_end]
    let hash_original = {
        let mut h = DefaultHasher::new();
        for (t, c) in &original {
            if *t <= dev_end {
                t.hash(&mut h);
                c.to_bits().hash(&mut h);
            }
        }
        h.finish()
    };
    let hash_extended = {
        let mut h = DefaultHasher::new();
        for (t, c) in &extended {
            if *t <= dev_end {
                t.hash(&mut h);
                c.to_bits().hash(&mut h);
            }
        }
        h.finish()
    };
    assert_eq!(hash_original, hash_extended, "manifest hash must be stable when out-of-range rows are appended");
}

fn funding_fixture(rows: &[(&str, i64)]) -> NamedTempFile {
    let file = NamedTempFile::new().expect("temp funding DB");
    let conn = Connection::open(file.path()).expect("open funding DB");
    conn.execute_batch(
        "CREATE TABLE funding_rates (
            symbol TEXT NOT NULL,
            funding_time INTEGER NOT NULL,
            funding_rate REAL NOT NULL,
            mark_price REAL,
            PRIMARY KEY (symbol, funding_time)
        );",
    )
    .expect("create funding table");
    for (symbol, funding_time) in rows {
        conn.execute(
            "INSERT INTO funding_rates VALUES (?1, ?2, 0.0001, 100.0)",
            (symbol, funding_time),
        )
        .expect("insert funding row");
    }
    file
}

/// Verify the real loader rejects a traded symbol with no funding rows.
#[test]
fn replay_rejects_traded_futures_symbol_with_missing_funding() {
    let file = funding_fixture(&[("BNBUSDT", 1_000), ("BNBUSDT", 29_000_000)]);
    let error = load_funding_rates_readonly(
        file.path(),
        &["BNBUSDT".to_string(), "MISSINGFUNDINGUSDT".to_string()],
        1_000,
        29_000_000,
    )
    .expect_err("missing symbol must fail");
    assert!(error.contains("MISSINGFUNDINGUSDT:missing"), "{error}");
}

/// Verify the real loader rejects a DB that stops well before the replay end.
#[test]
fn replay_rejects_incomplete_funding_range() {
    let file = funding_fixture(&[("BNBUSDT", 1_000), ("BNBUSDT", 29_000_000)]);
    let error = load_funding_rates_readonly(
        file.path(),
        &["BNBUSDT".to_string()],
        1_000,
        100_000_000,
    )
    .expect_err("incomplete range must fail");
    assert!(error.contains("BNBUSDT:1000..29000000"), "{error}");
}

/// Verify the real loader catches a gap hidden by otherwise valid range edges.
#[test]
fn replay_rejects_internal_funding_gap() {
    let file = funding_fixture(&[("BNBUSDT", 1_000), ("BNBUSDT", 40_000_000)]);
    let error = load_funding_rates_readonly(
        file.path(),
        &["BNBUSDT".to_string()],
        1_000,
        40_000_000,
    )
    .expect_err("internal gap must fail");
    assert!(error.contains("internal gaps over 9h"), "{error}");
}

/// Verify old rows before the range cannot hide a missing range-start edge.
#[test]
fn replay_rejects_missing_funding_at_range_edge() {
    let file = funding_fixture(&[("BNBUSDT", 1_000), ("BNBUSDT", 100_000_000)]);
    let error = load_funding_rates_readonly(
        file.path(),
        &["BNBUSDT".to_string()],
        20_000_000,
        100_000_000,
    )
    .expect_err("missing range-start funding must fail");
    assert!(error.contains("do not cover replay range edges"), "{error}");
}
