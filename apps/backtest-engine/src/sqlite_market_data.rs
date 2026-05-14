use crate::market_data::{AggTrade, KlineBar, MarketDataSource};
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const EXPECTED_SCHEMA: &[(&str, &[&str])] = &[
    ("symbols", &["symbol"]),
    (
        "klines",
        &[
            "symbol",
            "interval",
            "open_time_ms",
            "open",
            "high",
            "low",
            "close",
            "volume",
        ],
    ),
    (
        "agg_trades",
        &[
            "symbol",
            "trade_time_ms",
            "price",
            "quantity",
            "is_buyer_maker",
        ],
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarketDataSchema {
    Canonical,
    DiscordC2im,
}

#[derive(Debug)]
pub struct SqliteMarketDataSource {
    path: PathBuf,
    conn: Connection,
    schema: MarketDataSchema,
}

impl SqliteMarketDataSource {
    pub fn open_readonly(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let wal_sidecar_exists = wal_sidecar_exists(path);
        let uri = readonly_uri(path, wal_sidecar_exists);
        let conn = Connection::open_with_flags(
            &uri,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )
        .map_err(|err| {
            if !path.exists() {
                format!(
                    "SQLite market data file does not exist: {} ({err})",
                    path.display()
                )
            } else if wal_sidecar_exists {
                format!(
                    "failed to open SQLite WAL read-only {}: {err}; WAL sidecar detected, \
                     existing -wal/-shm files must be readable and may be constrained by \
                     directory/file permissions",
                    path.display()
                )
            } else {
                format!(
                    "failed to open SQLite immutable read-only {}: {err}",
                    path.display()
                )
            }
        })?;

        if !path.exists() {
            return Err(format!(
                "SQLite market data file does not exist: {}",
                path.display()
            ));
        }

        let mut source = Self {
            path: path.to_path_buf(),
            conn,
            schema: MarketDataSchema::Canonical,
        };
        source.schema = source.detect_schema()?;
        Ok(source)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn table_names(&self) -> Result<Vec<String>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT name FROM sqlite_master \
                 WHERE type = 'table' AND name NOT LIKE 'sqlite_%' \
                 ORDER BY name",
            )
            .map_err(|err| format!("failed to prepare table list query: {err}"))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|err| format!("failed to query table list: {err}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("failed to read table list: {err}"))
    }

    pub fn sqlite_version(&self) -> Result<String, String> {
        self.conn
            .query_row("SELECT sqlite_version()", [], |row| row.get::<_, String>(0))
            .map_err(|err| format!("failed to query SQLite version: {err}"))
    }

    fn detect_schema(&self) -> Result<MarketDataSchema, String> {
        let tables = self.table_names()?;
        if self.schema_matches(&tables, EXPECTED_SCHEMA)? {
            return Ok(MarketDataSchema::Canonical);
        }

        let discord_schema = &[(
            "klines",
            &[
                "symbol",
                "market_type",
                "timeframe",
                "open_time",
                "open",
                "high",
                "low",
                "close",
                "volume",
            ] as &[&str],
        )];
        if self.schema_matches(&tables, discord_schema)? {
            return Ok(MarketDataSchema::DiscordC2im);
        }

        let mut diagnostics = Vec::new();
        for (table, columns) in EXPECTED_SCHEMA {
            if !tables.iter().any(|name| name == table) {
                diagnostics.push(format!("missing table `{table}`"));
                continue;
            }
            let actual_columns = self.table_columns(table)?;
            for column in *columns {
                if !actual_columns.iter().any(|name| name == column) {
                    diagnostics.push(format!("table `{table}` missing column `{column}`"));
                }
            }
        }

        Err(format!(
            "SQLite market data schema mismatch for {}: {}; tables={:?}; expected={} or discord_c2im klines(symbol, market_type, timeframe, open_time, open, high, low, close, volume)",
            self.path.display(),
            diagnostics.join(", "),
            tables,
            expected_schema_description()
        ))
    }

    fn schema_matches(
        &self,
        tables: &[String],
        schema: &[(&str, &[&str])],
    ) -> Result<bool, String> {
        for (table, columns) in schema {
            if !tables.iter().any(|name| name == table) {
                return Ok(false);
            }
            let actual_columns = self.table_columns(table)?;
            if columns
                .iter()
                .any(|column| !actual_columns.iter().any(|name| name == column))
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn table_columns(&self, table: &str) -> Result<Vec<String>, String> {
        let pragma = format!("PRAGMA table_info({table})");
        let mut stmt = self
            .conn
            .prepare(&pragma)
            .map_err(|err| format!("failed to inspect table `{table}`: {err}"))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|err| format!("failed to query columns for `{table}`: {err}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("failed to read columns for `{table}`: {err}"))
    }
}

fn readonly_uri(path: &Path, wal_sidecar_exists: bool) -> String {
    if wal_sidecar_exists {
        format!("file:{}?mode=ro", encode_uri_path(path))
    } else {
        format!("file:{}?mode=ro&immutable=1", encode_uri_path(path))
    }
}

fn wal_sidecar_exists(path: &Path) -> bool {
    let path = path.to_string_lossy();
    PathBuf::from(format!("{path}-wal")).exists() || PathBuf::from(format!("{path}-shm")).exists()
}

fn encode_uri_path(path: &Path) -> String {
    path.to_string_lossy()
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

impl MarketDataSource for SqliteMarketDataSource {
    fn list_symbols(&self) -> Result<Vec<String>, String> {
        let query = match self.schema {
            MarketDataSchema::Canonical => "SELECT symbol FROM symbols ORDER BY symbol",
            MarketDataSchema::DiscordC2im => {
                if self
                    .table_names()?
                    .iter()
                    .any(|name| name == "market_universe")
                {
                    "SELECT symbol FROM market_universe ORDER BY symbol"
                } else {
                    "SELECT DISTINCT symbol FROM klines ORDER BY symbol"
                }
            }
        };
        let mut stmt = self
            .conn
            .prepare(query)
            .map_err(|err| format!("failed to prepare symbol query: {err}"))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|err| format!("failed to query symbols: {err}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("failed to read symbols: {err}"))
    }

    fn load_klines(
        &self,
        symbol: &str,
        start_ms: i64,
        end_ms: i64,
        interval: &str,
    ) -> Result<Vec<KlineBar>, String> {
        let query = match self.schema {
            MarketDataSchema::Canonical => {
                "SELECT symbol, open_time_ms, open, high, low, close, volume \
                 FROM klines \
                 WHERE symbol = ?1 AND interval = ?2 AND open_time_ms BETWEEN ?3 AND ?4 \
                 ORDER BY open_time_ms"
            }
            MarketDataSchema::DiscordC2im => {
                "SELECT symbol, open_time, open, high, low, close, volume \
                 FROM klines \
                 WHERE symbol = ?1 AND timeframe = ?2 AND open_time BETWEEN ?3 AND ?4 \
                   AND market_type = COALESCE((SELECT market_type FROM klines WHERE symbol = ?1 AND timeframe = ?2 AND market_type = 'futures_usdt_perp' LIMIT 1), market_type) \
                 ORDER BY open_time"
            }
        };
        let mut stmt = self
            .conn
            .prepare(query)
            .map_err(|err| format!("failed to prepare kline query: {err}"))?;
        let rows = stmt
            .query_map((symbol, interval, start_ms, end_ms), |row| {
                Ok(KlineBar {
                    symbol: row.get(0)?,
                    open_time_ms: row.get(1)?,
                    open: row.get(2)?,
                    high: row.get(3)?,
                    low: row.get(4)?,
                    close: row.get(5)?,
                    volume: row.get(6)?,
                })
            })
            .map_err(|err| format!("failed to query klines for {symbol}: {err}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("failed to read klines for {symbol}: {err}"))
    }

    fn load_agg_trades(
        &self,
        symbol: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<Vec<AggTrade>, String> {
        if self.schema == MarketDataSchema::DiscordC2im {
            return Ok(Vec::new());
        }
        let mut stmt = self
            .conn
            .prepare(
                "SELECT symbol, trade_time_ms, price, quantity, is_buyer_maker \
                 FROM agg_trades \
                 WHERE symbol = ?1 AND trade_time_ms BETWEEN ?2 AND ?3 \
                 ORDER BY trade_time_ms",
            )
            .map_err(|err| format!("failed to prepare agg trade query: {err}"))?;
        let rows = stmt
            .query_map((symbol, start_ms, end_ms), |row| {
                let is_buyer_maker: i64 = row.get(4)?;
                Ok(AggTrade {
                    symbol: row.get(0)?,
                    trade_time_ms: row.get(1)?,
                    price: row.get(2)?,
                    quantity: row.get(3)?,
                    is_buyer_maker: is_buyer_maker != 0,
                })
            })
            .map_err(|err| format!("failed to query agg trades for {symbol}: {err}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("failed to read agg trades for {symbol}: {err}"))
    }

    fn schema_fingerprint(&self) -> Result<String, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT type, name, tbl_name, COALESCE(sql, '') FROM sqlite_master \
                 WHERE name NOT LIKE 'sqlite_%' \
                 ORDER BY type, name, tbl_name, sql",
            )
            .map_err(|err| format!("failed to prepare schema fingerprint query: {err}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(format!(
                    "{}|{}|{}|{}",
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?
                ))
            })
            .map_err(|err| format!("failed to query schema fingerprint: {err}"))?;

        let mut hasher = Sha256::new();
        for row in rows {
            let line = row.map_err(|err| format!("failed to read schema row: {err}"))?;
            hasher.update(line.as_bytes());
            hasher.update(b"\n");
        }
        Ok(format!("{:x}", hasher.finalize()))
    }
}

fn expected_schema_description() -> String {
    EXPECTED_SCHEMA
        .iter()
        .map(|(table, columns)| format!("{table}({})", columns.join(", ")))
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use crate::market_data::MarketDataSource;
    use crate::sqlite_market_data::SqliteMarketDataSource;
    use rusqlite::Connection;
    use tempfile::NamedTempFile;

    fn fixture_db() -> NamedTempFile {
        let file = NamedTempFile::new().expect("temp sqlite file");
        let conn = Connection::open(file.path()).expect("open fixture db");

        conn.execute_batch(
            "
            CREATE TABLE symbols(symbol TEXT PRIMARY KEY);
            CREATE TABLE klines(
                symbol TEXT,
                interval TEXT,
                open_time_ms INTEGER,
                open REAL,
                high REAL,
                low REAL,
                close REAL,
                volume REAL
            );
            CREATE TABLE agg_trades(
                symbol TEXT,
                trade_time_ms INTEGER,
                price REAL,
                quantity REAL,
                is_buyer_maker INTEGER
            );
            INSERT INTO symbols(symbol) VALUES ('BTCUSDT'), ('ETHUSDT');
            INSERT INTO klines(symbol, interval, open_time_ms, open, high, low, close, volume)
                VALUES ('BTCUSDT', '1m', 1000, 10.0, 12.0, 9.0, 11.0, 5.0),
                       ('BTCUSDT', '1m', 2000, 11.0, 13.0, 10.0, 12.0, 6.0),
                       ('ETHUSDT', '1m', 1000, 20.0, 22.0, 19.0, 21.0, 7.0);
            INSERT INTO agg_trades(symbol, trade_time_ms, price, quantity, is_buyer_maker)
                VALUES ('BTCUSDT', 1100, 10.5, 0.1, 1),
                       ('BTCUSDT', 2100, 11.5, 0.2, 0);
            ",
        )
        .expect("seed fixture db");
        drop(conn);
        file
    }

    fn discord_c2im_fixture_db() -> NamedTempFile {
        let file = NamedTempFile::new().expect("temp sqlite file");
        let conn = Connection::open(file.path()).expect("open discord fixture db");
        conn.execute_batch(
            "
            CREATE TABLE market_universe(symbol TEXT PRIMARY KEY);
            CREATE TABLE klines(
                symbol TEXT NOT NULL,
                market_type TEXT NOT NULL DEFAULT 'spot',
                timeframe TEXT NOT NULL,
                open_time INTEGER NOT NULL,
                open REAL NOT NULL,
                high REAL NOT NULL,
                low REAL NOT NULL,
                close REAL NOT NULL,
                volume REAL NOT NULL,
                close_time INTEGER NOT NULL,
                PRIMARY KEY(symbol, market_type, timeframe, open_time)
            );
            INSERT INTO market_universe(symbol) VALUES ('BTCUSDT'), ('ETHUSDT');
            INSERT INTO klines(symbol, market_type, timeframe, open_time, open, high, low, close, volume, close_time)
                VALUES ('BTCUSDT', 'futures_usdt_perp', '1m', 1000, 10.0, 12.0, 9.0, 11.0, 5.0, 1999),
                       ('BTCUSDT', 'futures_usdt_perp', '1m', 2000, 11.0, 13.0, 10.0, 12.0, 6.0, 2999),
                       ('BTCUSDT', 'spot', '1m', 1000, 9.0, 10.0, 8.0, 9.5, 3.0, 1999),
                       ('ETHUSDT', 'futures_usdt_perp', '1m', 1000, 20.0, 22.0, 19.0, 21.0, 7.0, 1999);
            ",
        )
        .expect("seed discord fixture db");
        drop(conn);
        file
    }

    fn live_wal_fixture_db() -> (NamedTempFile, Connection) {
        let file = NamedTempFile::new().expect("temp sqlite file");
        let conn = Connection::open(file.path()).expect("open live wal fixture db");
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))
            .expect("enable wal");
        assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
        conn.execute_batch(
            "
            CREATE TABLE symbols(symbol TEXT PRIMARY KEY);
            CREATE TABLE klines(
                symbol TEXT,
                interval TEXT,
                open_time_ms INTEGER,
                open REAL,
                high REAL,
                low REAL,
                close REAL,
                volume REAL
            );
            CREATE TABLE agg_trades(
                symbol TEXT,
                trade_time_ms INTEGER,
                price REAL,
                quantity REAL,
                is_buyer_maker INTEGER
            );
            INSERT INTO symbols(symbol) VALUES ('BTCUSDT'), ('ETHUSDT');
            INSERT INTO klines(symbol, interval, open_time_ms, open, high, low, close, volume)
                VALUES ('BTCUSDT', '1m', 1000, 10.0, 12.0, 9.0, 11.0, 5.0),
                       ('BTCUSDT', '1m', 2000, 11.0, 13.0, 10.0, 12.0, 6.0);
            INSERT INTO agg_trades(symbol, trade_time_ms, price, quantity, is_buyer_maker)
                VALUES ('BTCUSDT', 1100, 10.5, 0.1, 1);
            ",
        )
        .expect("seed live wal fixture db");
        conn.execute(
            "INSERT INTO klines(symbol, interval, open_time_ms, open, high, low, close, volume)
             VALUES ('BTCUSDT', '1m', 3000, 12.0, 14.0, 11.0, 13.0, 8.0)",
            [],
        )
        .expect("write wal row");
        (file, conn)
    }

    fn sidecar_paths(path: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
        let path = path.to_string_lossy();
        (
            std::path::PathBuf::from(format!("{path}-wal")),
            std::path::PathBuf::from(format!("{path}-shm")),
        )
    }

    #[test]
    fn readonly_adapter_supports_discord_c2im_kline_schema_without_agg_trades() {
        let file = discord_c2im_fixture_db();
        let source = SqliteMarketDataSource::open_readonly(file.path())
            .expect("open readonly discord schema");

        let symbols = source.list_symbols().expect("list symbols");
        let bars = source
            .load_klines("BTCUSDT", 1000, 2000, "1m")
            .expect("load klines");
        let trades = source
            .load_agg_trades("BTCUSDT", 1000, 2000)
            .expect("missing agg trades should degrade to empty");

        assert_eq!(symbols, vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()]);
        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].open_time_ms, 1000);
        assert_eq!(bars[0].close, 11.0);
        assert!(trades.is_empty());
    }

    #[test]
    fn readonly_adapter_lists_symbols_from_fixture() {
        let file = fixture_db();
        let source = SqliteMarketDataSource::open_readonly(file.path()).expect("open readonly");

        let symbols = source.list_symbols().expect("list symbols");

        assert_eq!(symbols, vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()]);
    }

    #[test]
    fn readonly_adapter_loads_klines_from_fixture() {
        let file = fixture_db();
        let source = SqliteMarketDataSource::open_readonly(file.path()).expect("open readonly");

        let bars = source
            .load_klines("BTCUSDT", 1000, 2000, "1m")
            .expect("load klines");

        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].symbol, "BTCUSDT");
        assert_eq!(bars[0].open_time_ms, 1000);
        assert_eq!(bars[0].open, 10.0);
        assert_eq!(bars[1].close, 12.0);
    }

    #[test]
    fn readonly_adapter_rejects_missing_file_with_diagnostic() {
        let missing = std::env::temp_dir().join("missing-market-data-fixture.sqlite");
        let err =
            SqliteMarketDataSource::open_readonly(&missing).expect_err("missing file rejected");

        assert!(err.contains("does not exist"));
        assert!(err.contains(missing.to_string_lossy().as_ref()));
    }

    #[test]
    fn readonly_adapter_loads_agg_trades_from_fixture() {
        let file = fixture_db();
        let source = SqliteMarketDataSource::open_readonly(file.path()).expect("open readonly");

        let trades = source
            .load_agg_trades("BTCUSDT", 1000, 2200)
            .expect("load agg trades");

        assert_eq!(trades.len(), 2);
        assert!(trades[0].is_buyer_maker);
        assert!(!trades[1].is_buyer_maker);
    }

    #[test]
    fn readonly_adapter_returns_stable_schema_fingerprint() {
        let file = fixture_db();
        let source = SqliteMarketDataSource::open_readonly(file.path()).expect("open readonly");

        let first = source.schema_fingerprint().expect("fingerprint");
        let second = source.schema_fingerprint().expect("fingerprint");

        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
    }

    #[test]
    fn readonly_adapter_opens_live_wal_fixture_without_modifying_sidecars() {
        let (file, write_conn) = live_wal_fixture_db();
        let (wal_path, shm_path) = sidecar_paths(file.path());
        let wal_before = std::fs::metadata(&wal_path)
            .expect("live wal sidecar exists before readonly open")
            .len();
        let shm_before = std::fs::metadata(&shm_path)
            .expect("live shm sidecar exists before readonly open")
            .len();
        let wal_modified_before = std::fs::metadata(&wal_path)
            .expect("live wal sidecar exists before readonly open")
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok());
        let shm_modified_before = std::fs::metadata(&shm_path)
            .expect("live shm sidecar exists before readonly open")
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok());

        let source = SqliteMarketDataSource::open_readonly(file.path()).expect("open readonly wal");
        let symbols = source
            .list_symbols()
            .expect("list symbols from wal fixture");
        let bars = source
            .load_klines("BTCUSDT", 1000, 3000, "1m")
            .expect("load klines from wal fixture");
        drop(source);

        let wal_after = std::fs::metadata(&wal_path)
            .expect("live wal sidecar remains after readonly open")
            .len();
        let shm_after = std::fs::metadata(&shm_path)
            .expect("live shm sidecar remains after readonly open")
            .len();
        let wal_modified_after = std::fs::metadata(&wal_path)
            .expect("live wal sidecar remains after readonly open")
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok());
        let shm_modified_after = std::fs::metadata(&shm_path)
            .expect("live shm sidecar remains after readonly open")
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok());
        assert_eq!(symbols, vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()]);
        assert_eq!(bars.len(), 3);
        assert_eq!(wal_before, wal_after);
        assert_eq!(shm_before, shm_after);
        assert_eq!(wal_modified_before, wal_modified_after);
        assert_eq!(shm_modified_before, shm_modified_after);
        drop(write_conn);
    }
}
