#!/bin/bash
# Monitor backtest tasks every 5 minutes for FlyingKid account
# Usage: ./scripts/monitor_martingale_backtests.sh

set -euo pipefail

INTERVAL_SECS=${1:-300}  # default 5 minutes

while true; do
    echo "=== $(date -u '+%Y-%m-%d %H:%M:%S UTC') ==="

    docker exec grid-binance-postgres-1 psql -U postgres -d grid_binance -P pager=off -c "
    SELECT task_id,
           status,
           summary->>'stage' AS stage,
           summary->>'progress_pct' AS pct,
           summary->>'current_symbol' AS symbol,
           summary->>'processed_candidates' AS proc,
           summary->>'total_candidates' AS total,
           summary->>'rss_mb' AS rss_mb,
           updated_at
    FROM backtest_tasks
    WHERE owner='flyingkid2022@outlook.com'
    ORDER BY updated_at DESC
    LIMIT 20;
    "

    echo ""
    docker stats --no-stream --format 'table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}' 2>/dev/null | grep -E 'backtest|NAME' || true
    echo ""
    free -h | head -2

    sleep "$INTERVAL_SECS"
done
