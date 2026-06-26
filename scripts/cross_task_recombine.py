#!/usr/bin/env python3
"""方向4: 跨任务候选 equity curve 重组
从多个 conservative 任务的 portfolio_top3 提取 equity curve，
测试跨任务 2-3 成员组合是否有 dd<=10% 的更高 ann。
用法: python3 cross_task_recombine.py
"""
import json
import subprocess
import sys
from itertools import combinations

DB_CMD = ["docker", "exec", "grid-binance-postgres-1", "psql", "-U", "postgres", "-d", "grid_binance", "-At", "-c"]

TASK_IDS = [
    "fk-18-conservative-seed887-lshort2-20260616",
    "fk-18-conservative-seed521-lshort30-20260618",
    "fk-18-conservative-seed521-tailstop-20260619",
    "fk-18-conservative-seed521-b2trend-20260620",
    "fk-18-conservative-seed521-b2v2fix-20260622",
    "fk-18-conservative-seed521-dir1lowadx-20260622",
]

def query_json(sql):
    result = subprocess.run(DB_CMD + [sql], capture_output=True, text=True)
    if result.returncode != 0:
        print(f"SQL error: {result.stderr}")
        return None
    raw = result.stdout.strip()
    if not raw:
        return None
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        return None

def extract_portfolio_members(task_id):
    """提取任务 portfolio_top3 中所有成员的 equity curve（portfolio 级别，5000 点）"""
    sql = f"""
    SELECT jsonb_path_query_array(
      summary,
      '$.portfolio_top3[*].members[*].{{"sym": $.symbol, "ann": $.annualized_return_pct, "dd": $.max_drawdown_pct, "cand": $.candidate_id}}'
    )
    FROM backtest_tasks WHERE task_id='{task_id}';
    """
    data = query_json(sql)
    if not data:
        return []

    # Also get portfolio-level equity curves
    sql2 = f"""
    SELECT jsonb_path_query_array(
      summary,
      '$.portfolio_top3[*].{{"pid": $.portfolio_id, "ann": $.annualized_return_pct, "dd": $.max_drawdown_pct, "ec": $.equity_curve}}'
    )
    FROM backtest_tasks WHERE task_id='{task_id}';
    """
    portfolios = query_json(sql2)
    return portfolios or []

def main():
    all_portfolios = []
    for task_id in TASK_IDS:
        portfolios = extract_portfolio_members(task_id)
        for p in portfolios:
            p["task_id"] = task_id
            all_portfolios.append(p)
        if portfolios:
            print(f"  {task_id}: {len(portfolios)} portfolios")

    print(f"\nTotal portfolios: {len(all_portfolios)}")

    if len(all_portfolios) < 2:
        print("Not enough portfolios for recombination")
        return

    # Test all 2-member combos across different tasks
    print("\n=== Cross-task 2-member combos (50/50 blend) ===")
    best_ann = 0
    best_combo = None

    for p1, p2 in combinations(all_portfolios, 2):
        if p1["task_id"] == p2["task_id"]:
            continue  # Skip same-task combos (already optimized)

        ec1 = p1.get("ec", [])
        ec2 = p2.get("ec", [])
        if len(ec1) < 100 or len(ec2) < 100:
            continue

        # Align by timestamp_ms (use shorter curve's length)
        min_len = min(len(ec1), len(ec2))
        blended = []
        for i in range(min_len):
            eq = (ec1[i].get("equity_quote", 10000) + ec2[i].get("equity_quote", 10000)) / 2
            blended.append(eq)

        # Calculate ann and dd from blended curve
        initial = blended[0]
        final = blended[-1]
        total_return = (final - initial) / initial * 100

        peak = initial
        max_dd = 0
        for eq in blended:
            peak = max(peak, eq)
            dd = (peak - eq) / peak * 100 if peak > 0 else 0
            max_dd = max(max_dd, dd)

        # Rough annualization (3.4 years window)
        ann = total_return / 3.4 * 100 / 100 * 100  # Already in pct, annualize
        # Actually: ann = ((final/initial)^(1/3.4) - 1) * 100
        if final > 0 and initial > 0:
            ann = ((final / initial) ** (1 / 3.4) - 1) * 100
        else:
            ann = 0

        if max_dd <= 10 and ann > best_ann:
            best_ann = ann
            best_combo = (p1, p2, ann, max_dd, total_return)
            print(f"  NEW BEST: ann={ann:.2f}% dd={max_dd:.2f}% ret={total_return:.1f}%"
                  f" | {p1['task_id'][-20:]} (ann={p1['ann']:.1f}%/dd={p1['dd']:.1f}%)"
                  f" + {p2['task_id'][-20:]} (ann={p2['ann']:.1f}%/dd={p2['dd']:.1f}%)")

    if best_combo:
        print(f"\n=== BEST CROSS-TASK COMBO ===")
        print(f"  ann={best_combo[2]:.2f}% dd={best_combo[3]:.2f}% ret={best_combo[4]:.1f}%")
        print(f"  Member 1: {best_combo[0]['task_id']} (ann={best_combo[0]['ann']}% dd={best_combo[0]['dd']}%)")
        print(f"  Member 2: {best_combo[1]['task_id']} (ann={best_combo[1]['ann']}% dd={best_combo[1]['dd']}%)")
    else:
        print("\n  No cross-task combo with dd<=10% found")

    # Also report best single portfolio
    print("\n=== Best single portfolios ===")
    for p in sorted(all_portfolios, key=lambda x: float(x.get("ann", 0)), reverse=True)[:5]:
        print(f"  ann={float(p['ann']):.2f}% dd={float(p['dd']):.2f}% | {p['task_id']}")

if __name__ == "__main__":
    main()
