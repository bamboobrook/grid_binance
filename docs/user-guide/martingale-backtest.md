# Martingale Portfolio Backtesting and Publishing

## What martingale grid means

A martingale grid places a first order and then adds larger safety orders at configured price intervals. For example, a BTCUSDT futures strategy may start with 10 USDT, add 20 USDT after a 1% adverse move, then 40 USDT after another 1%, then 80 USDT after another 1%, and take profit on the whole position. The `martingale_grid` strategy family supports spot and USDT-M futures, long/short/long+short portfolios, fixed spacing, multiplier spacing, ATR spacing, custom notional sequences, and multiple symbols inside one Portfolio.

## Why it is risky

Martingale trading does not guarantee profit. It lowers the average entry by increasing exposure, so losses can grow quickly during long one-way trends, low liquidity, wider slippage, adverse funding, exchange restrictions, or network/API incidents. Futures also add liquidation risk. In futures backtests, the initial order amount is treated as margin capital: 10 USDT initial margin at 2x opens about 20 USDT notional, while return, drawdown, and capital usage are measured against margin capital; fees and slippage are still charged on notional turnover. Before live use, configure global, symbol, direction, and strategy budgets, stop losses, circuit breakers, margin mode, leverage, and Hedge Mode requirements.

## How two-stage backtesting works

1. **K-line screening** uses OHLCV bars to conservatively simulate many parameter combinations and reject candidates that exceed budgets, stop too often, draw down too far, or lack sufficient data quality.
2. **Trade refinement** replays aggTrades or ordered trade prices for the top candidates, reducing uncertainty caused by unknown intrabar price order.

The external market SQLite database is opened read-only. The system must not create indexes, run migrations, VACUUM, checkpoint, or otherwise modify the source database.

## Survival-first scoring

Candidates are filtered before they are ranked. The survival filter rejects liquidation hits, global drawdown breaches, strategy drawdown breaches, budget breaches, excessive stops, and insufficient data quality. Only candidates that survive are ranked using weighted return, Calmar, Sortino, drawdown, stop frequency, capital utilization, and trade stability.

## Portfolio publish confirmation

Backtests produce Portfolio candidates. Users must review the risk summary, symbols, directions, isolated/cross margin mode, leverage, budgets, stops, Hedge Mode requirements, and same-symbol futures compatibility before confirming live start. The system does not auto-start a candidate. Futures long+short portfolios require Hedge Mode, and all live strategies on the same futures symbol must use compatible margin mode and leverage.

## Why live results may differ from backtests

Backtests cannot fully reproduce exchange execution. Live differences may come from order book depth, queue position, slippage, fee tier, BNB discounts, funding fees, API latency, network failures, exchange controls, minimum notional changes, needs-attention/orphan order warnings after recovery, and missing market data. Start with small capital, monitor needs-attention/orphan warnings, and watch drawdown and budget usage continuously.
