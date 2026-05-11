use backtest_engine::market_data::MarketDataSource;
use backtest_engine::sqlite_market_data::SqliteMarketDataSource;
use chrono::NaiveDate;
use clap::Parser;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(about = "Read-only SQLite market data diagnostic probe")]
struct Args {
    #[arg(long)]
    db_path: PathBuf,
    #[arg(long, value_delimiter = ',')]
    symbols: Vec<String>,
    #[arg(long, default_value = "0")]
    from: String,
    #[arg(long, default_value = "9223372036854775807")]
    to: String,
    #[arg(long, default_value = "1m")]
    interval: String,
}

fn main() {
    let args = Args::parse();
    let from_ms = parse_time_ms_or_exit("--from", &args.from);
    let to_ms = parse_time_ms_or_exit("--to", &args.to);
    validate_time_range_or_exit(from_ms, to_ms);

    println!("db_path: {}", args.db_path.display());
    match fs::metadata(&args.db_path) {
        Ok(metadata) => {
            println!("file_exists: true");
            println!("file_len_bytes: {}", metadata.len());
            println!("file_readonly_attr: {}", metadata.permissions().readonly());
        }
        Err(err) => {
            println!("file_exists: false");
            println!("file_metadata_error: {err}");
        }
    }

    let source = match SqliteMarketDataSource::open_readonly(&args.db_path) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("open_error: {err}");
            std::process::exit(1);
        }
    };

    match source.sqlite_version() {
        Ok(version) => println!("sqlite_version: {version}"),
        Err(err) => println!("sqlite_version_error: {err}"),
    }

    match source.schema_fingerprint() {
        Ok(fingerprint) => println!("schema_fingerprint: {fingerprint}"),
        Err(err) => println!("schema_fingerprint_error: {err}"),
    }

    match source.table_names() {
        Ok(tables) => println!("tables: {}", tables.join(",")),
        Err(err) => println!("tables_error: {err}"),
    }

    let symbols = if args.symbols.is_empty() {
        match source.list_symbols() {
            Ok(symbols) => symbols,
            Err(err) => {
                eprintln!("symbols_error: {err}");
                std::process::exit(1);
            }
        }
    } else {
        args.symbols
    };

    for symbol in symbols {
        match source.load_klines(&symbol, from_ms, to_ms, &args.interval) {
            Ok(bars) => println!("symbol={symbol} klines={}", bars.len()),
            Err(err) => println!("symbol={symbol} klines_error={err}"),
        }
        match source.load_agg_trades(&symbol, from_ms, to_ms) {
            Ok(trades) => println!("symbol={symbol} agg_trades={}", trades.len()),
            Err(err) => println!("symbol={symbol} agg_trades_error={err}"),
        }
    }
}

fn parse_time_ms_or_exit(flag: &str, value: &str) -> i64 {
    match parse_time_ms(value) {
        Ok(ms) => ms,
        Err(err) => {
            eprintln!("invalid {flag} value `{value}`: {err}");
            std::process::exit(2);
        }
    }
}

fn parse_time_ms(value: &str) -> Result<i64, String> {
    if let Ok(ms) = value.parse::<i64>() {
        return Ok(ms);
    }

    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|err| format!("expected epoch milliseconds or YYYY-MM-DD: {err}"))?;
    let datetime = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| format!("invalid date start for {value}"))?;
    Ok(datetime.and_utc().timestamp_millis())
}

fn validate_time_range(from_ms: i64, to_ms: i64) -> Result<(), String> {
    if from_ms <= to_ms {
        Ok(())
    } else {
        Err(format!("--from ({from_ms}) must be <= --to ({to_ms})"))
    }
}

fn validate_time_range_or_exit(from_ms: i64, to_ms: i64) {
    if let Err(err) = validate_time_range(from_ms, to_ms) {
        eprintln!("invalid time range: {err}");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_time_ms, validate_time_range};

    #[test]
    fn parse_time_ms_accepts_epoch_millis() {
        assert_eq!(
            parse_time_ms("1704067200000").expect("millis"),
            1704067200000
        );
    }

    #[test]
    fn parse_time_ms_accepts_yyyy_mm_dd() {
        assert_eq!(parse_time_ms("2024-01-01").expect("date"), 1704067200000);
    }

    #[test]
    fn validate_time_range_rejects_from_after_to() {
        let err = validate_time_range(2000, 1000).expect_err("invalid range");

        assert!(err.contains("--from"));
        assert!(err.contains("--to"));
    }
}
