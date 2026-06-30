#!/usr/bin/env python3
"""2025-focused 单策略候选 → 全周期分段验证胶水 (ChatGPT P2 计划, Task 3+4)。

从 search_small_capital_martingale 的输出 JSON 里:
  1. 收集所有候选 row (best_by_budget + frontier 各档 + pass_candidates), 按完整参数去重;
  2. 筛选 P2 进组合池门槛: 2025 段 total_return_pct >= 0, 或 (>= -2 且 DD <= 12);
     排除 principal_breached;
  3. 复刻 search 的 config 构造语义 (entry_filter→trigger 表达式 / indicators=atr(21)+adx(14) /
     fixed_percent spacing / multiplier sizing / Percent TP / StrategyDrawdownPct SL);
  4. 用 portfolio_budget_replay 跑 full + 5 段, 提取每段 total_return/DD/principal_breached/
     max_capital_used, 计算 overfit flags (H1-2023 贡献比 / 2024-2026 复利合计);
  5. 自校验: 对每个候选先 replay 2025 段, 与 search row 的 2025 ret/ann/dd 比对 (容差),
     确认胶水与 search 引擎一致。

用法:
  python3 scripts/validate_2025_single_strategy_segments.py \
    --search docs/superpowers/artifacts/glm-p0-search/screen/2025_single_3000.json \
    --budget 3000 --top 25 \
    --out docs/superpowers/reports/2025_single_strategy_segments.json
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

# REPO = 脚本所在 repo root (worktree 含 P4 改动, 或 main); bin 用本 repo 的
REPO = Path(__file__).resolve().parent.parent
REPLAY_BIN = REPO / "target" / "release" / "portfolio_budget_replay"
# market/funding data 是 gitignored 大文件, 只在主 repo
MAIN_REPO = Path("/home/bumblebee/Project/grid_binance")
MARKET_DB = MAIN_REPO / "data" / "market_data_full.db"
FUNDING_DB = MAIN_REPO / "data" / "funding_rates.db"

# 段定义 (ms epoch, UTC) — 与 validate_martingale_portfolio_robustness.py SEGMENTS 一致
SEGMENTS = {
    "full": (1672531200000, 1780271999999),
    "h1_2023": (1672531200000, 1688169599999),
    "h2_2023": (1688169600000, 1704067199999),
    "2024": (1704067200000, 1735689599999),
    "2025": (1735689600000, 1767225599999),
    "2026_ytd": (1767225600000, 1780271999999),
}

PARAM_KEYS = [
    "first_order_quote", "leverage", "multiplier", "max_legs", "step_bps",
    "take_profit_bps", "cooldown_seconds", "adx_min", "stop_loss_bps", "entry_filter",
    "regime_break_ema_period", "max_cycle_age_hours",
]


# ---- entry_filter → indicator_expression (精确复刻 search 脚本 add_entry_filter_triggers) ----
def filter_expressions(entry_filter: str, direction: str) -> list[str]:
    exprs: list[str] = []
    f = entry_filter
    if f in ("trend", "trend_rsi"):
        exprs.append("close > ema(200)" if direction == "long" else "close < ema(200)")
        if f == "trend_rsi":
            exprs.append("rsi(14) < 65" if direction == "long" else "rsi(14) > 35")
    elif f == "rsi_extreme":
        exprs.append("rsi(14) < 30" if direction == "long" else "rsi(14) > 70")
    elif f == "rsi_moderate":
        exprs.append("rsi(14) < 35" if direction == "long" else "rsi(14) > 65")
    elif f == "bb_extreme":
        exprs.append("close < bb_lower(20,2.5)" if direction == "long" else "close > bb_upper(20,2.5)")
    elif f == "bb_moderate":
        exprs.append("close < bb_lower(20,2)" if direction == "long" else "close > bb_upper(20,2)")
    elif f == "rsi_bb_extreme":
        exprs.append("rsi(14) < 35" if direction == "long" else "rsi(14) > 65")
        exprs.append("close < bb_lower(20,2)" if direction == "long" else "close > bb_upper(20,2)")
    elif f == "rsi_bb_moderate":
        exprs.append("rsi(14) < 40" if direction == "long" else "rsi(14) > 60")
        exprs.append("close < bb_lower(20,1.5)" if direction == "long" else "close > bb_upper(20,1.5)")
    return exprs


def _build_stop_loss(p: dict) -> dict:
    rb = p.get("regime_break_ema_period")
    if rb is not None:
        return {"regime_break_stop": {"ema_period": int(rb), "drawdown_pct_bps": int(p["stop_loss_bps"])}}
    return {"strategy_drawdown_pct": {"pct_bps": int(p["stop_loss_bps"])}}


def _build_risk_limits(p: dict) -> dict:
    rl: dict = {}
    age = p.get("max_cycle_age_hours")
    if age is not None:
        rl["max_cycle_age_hours"] = float(age)
    return rl


def build_strategy(symbol: str, direction: str, direction_mode: str, p: dict) -> dict:
    triggers: list[dict] = [{"cooldown": {"seconds": int(p["cooldown_seconds"])}}]
    adx = p.get("adx_min")
    if adx is not None:
        triggers.append({"indicator_expression": {"expression": f"adx(14) > {adx}"}})
    for expr in filter_expressions(p["entry_filter"], direction):
        triggers.append({"indicator_expression": {"expression": expr}})
    return {
        "strategy_id": (
            f"small-{symbol}-{direction}-foq{p['first_order_quote']}-lev{p['leverage']}"
            f"-m{p['multiplier']:.2f}-legs{p['max_legs']}-step{p['step_bps']}-tp{p['take_profit_bps']}"
            f"-cd{p['cooldown_seconds']}-adx{adx or 0}-sl{p['stop_loss_bps']}-filter{p['entry_filter']}"
        ),
        "symbol": symbol,
        "market": "usd_m_futures",
        "direction": direction,
        "direction_mode": direction_mode,
        "margin_mode": "isolated",
        "leverage": int(p["leverage"]),
        "spacing": {"fixed_percent": {"step_bps": int(p["step_bps"])}},
        "sizing": {
            "multiplier": {
                "first_order_quote": _fmt_decimal(p["first_order_quote"]),
                "multiplier": _fmt_decimal(p["multiplier"]),
                "max_legs": int(p["max_legs"]),
            }
        },
        "take_profit": {"percent": {"bps": int(p["take_profit_bps"])}},
        "stop_loss": _build_stop_loss(p),
        "indicators": [{"atr": {"period": 21}}, {"adx": {"period": 14}}],
        "entry_triggers": triggers,
        "risk_limits": _build_risk_limits(p),
    }


def _fmt_decimal(v) -> str:
    # 搜索参数 first_order_quote / multiplier 来自 JSON, 可能是 float 或 str; 统一成无尾零字符串
    if isinstance(v, str):
        return v
    f = float(v)
    s = f"{f:.6f}".rstrip("0").rstrip(".")
    return s if s else "0"


def build_portfolio(row: dict, budget: float) -> dict:
    symbol = row["symbol"]
    mode = row["direction_mode"]
    p = {k: row[k] for k in PARAM_KEYS}
    strategies: list[dict] = []
    if mode == "long_only":
        strategies.append(build_strategy(symbol, "long", "long_only", p))
    elif mode == "short_only":
        strategies.append(build_strategy(symbol, "short", "short_only", p))
    else:  # long_and_short — search 在单 symbol 内拆 long+short 两条策略
        strategies.append(build_strategy(symbol, "long", "long_and_short", p))
        strategies.append(build_strategy(symbol, "short", "long_and_short", p))
    return {
        "portfolio_config": {
            "direction_mode": mode,
            "strategies": strategies,
            "risk_limits": {"max_global_budget_quote": _fmt_decimal(budget)},
        }
    }


def replay(config: dict, start_ms: int, end_ms: int, budget: float, profile: str, timeout: int = 420) -> dict | None:
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as fh:
        json.dump(config, fh)
        cfg_path = fh.name
    try:
        proc = subprocess.run(
            [
                str(REPLAY_BIN),
                "--config", cfg_path,
                "--budget", _fmt_decimal(budget),
                "--profile", profile,
                "--start-ms", str(start_ms),
                "--end-ms", str(end_ms),
                "--market-data", str(MARKET_DB),
                "--funding-data", str(FUNDING_DB),
                "--exchange-min-notional", "5",
            ],
            capture_output=True, text=True, timeout=timeout,
        )
        if proc.returncode != 0:
            return {"_error": f"replay exit {proc.returncode}: {proc.stderr[-400:]}"}
        try:
            return json.loads(proc.stdout)
        except json.JSONDecodeError as e:
            return {"_error": f"json decode: {e}"}
    except subprocess.TimeoutExpired:
        return {"_error": "timeout"}
    finally:
        os.unlink(cfg_path)


def collect_rows(search: dict) -> list[dict]:
    """合并 best_by_budget / frontier 各档 / pass_candidates, 按参数去重。"""
    seen: dict[str, dict] = {}
    buckets: list[dict] = []
    bb = search.get("best_by_budget", {}) or {}
    for rows in bb.values():
        buckets.extend(rows or [])
    fr = search.get("frontier_by_budget", {}) or {}
    for budget_block in fr.values():
        for cat_rows in (budget_block or {}).values():
            buckets.extend(cat_rows or [])
    buckets.extend(search.get("pass_candidates", []) or [])
    for r in buckets:
        key = "|".join(
            [str(r.get("symbol")), str(r.get("direction_mode"))]
            + [f"{k}={r.get(k)}" for k in PARAM_KEYS]
        )
        # 保留 2025 段指标最完整的一份
        if key not in seen:
            seen[key] = r
    return list(seen.values())


def passes_p2_pool_gate(row: dict) -> bool:
    """P2 进组合池门槛: 2025 段 ret>=0, 或 (ret>=-2 且 DD<=12); 且不击穿本金。"""
    if row.get("principal_breached"):
        return False
    ret = float(row.get("total_return_pct", -999))
    dd = float(row.get("max_drawdown_pct", 999))
    return ret >= 0.0 or (ret >= -2.0 and dd <= 12.0)


def select_candidates(p2: list[dict], mode: str, top: int) -> list[dict]:
    """按策略挑选要跑全周期 replay 的候选子集 (覆盖不同 symbol/方向/DD 档)。"""
    if mode == "top":
        return p2[:top]
    if mode == "aggressive":
        ag = [r for r in p2 if r.get("aggressive_pass")]
        return (ag[:top] if ag else p2[:top])
    if mode == "long_short":
        ls = [r for r in p2 if r.get("direction_mode") == "long_and_short"]
        ls.sort(key=lambda r: -float(r.get("total_return_pct", 0)))
        return ls[:top]
    if mode == "lowdd":
        ld = [r for r in p2 if float(r.get("max_drawdown_pct", 999)) <= 15.0]
        ld.sort(key=lambda r: float(r.get("max_drawdown_pct", 999)))
        return ld[:top]
    # diverse: 每 symbol 最高ret + 每symbol低DD档最高ret + 全部 long_and_short + aggressive_pass
    from collections import defaultdict
    bysym: dict[str, list[dict]] = defaultdict(list)
    for r in p2:
        bysym[r["symbol"]].append(r)
    picks: list[dict] = []
    seen: set[tuple] = set()

    def _key(r: dict) -> tuple:
        return (r["symbol"], r["direction_mode"], r["entry_filter"], r["first_order_quote"],
                r["leverage"], r["multiplier"], r["max_legs"], r["step_bps"],
                r["take_profit_bps"], r["cooldown_seconds"], r.get("adx_min"),
                r["stop_loss_bps"], r.get("regime_break_ema_period"),
                r.get("max_cycle_age_hours"))

    def add(r: dict) -> None:
        k = _key(r)
        if k not in seen:
            seen.add(k)
            picks.append(r)

    for _s, rs in bysym.items():
        add(max(rs, key=lambda r: float(r.get("total_return_pct", -999))))
        low = [r for r in rs if float(r.get("max_drawdown_pct", 999)) <= 12.0]
        if low:
            add(max(low, key=lambda r: float(r.get("total_return_pct", -999))))
    for r in p2:
        if r.get("direction_mode") == "long_and_short":
            add(r)
    for r in p2:
        if r.get("aggressive_pass"):
            add(r)
    return picks[:top]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--search", required=True, help="search_small_capital_martingale 输出 JSON")
    ap.add_argument("--budget", type=float, required=True, help="与 search 一致的 budget")
    ap.add_argument("--profile", default="balanced")
    ap.add_argument("--top", type=int, default=25, help="对多少个 P2 达标候选跑全周期分段 replay")
    ap.add_argument("--out", required=True, help="汇总输出 JSON")
    ap.add_argument("--list-only", action="store_true", help="只列 P2 达标候选, 不跑 replay")
    ap.add_argument(
        "--select-mode", default="top",
        choices=["top", "diverse", "aggressive", "long_short", "lowdd"],
        help="候选选择策略: top=按2025ret降序; diverse=每symbol best+lowDD+所有long_and_short+aggressive; "
             "aggressive=仅aggressive_pass; long_short=仅双方向; lowdd=DD<=15最低DD优先",
    )
    args = ap.parse_args()

    search = json.loads(Path(args.search).read_text())
    rows = collect_rows(search)
    p2 = [r for r in rows if passes_p2_pool_gate(r)]
    # 排序: 2025 段 ret 降序, DD 升序
    p2.sort(key=lambda r: (float(r.get("total_return_pct", -999)), -float(r.get("max_drawdown_pct", 999))), reverse=True)

    print(f"== search={args.search}")
    print(f"== budget={args.budget} profile={args.profile}")
    print(f"== 去重候选总数={len(rows)}  P2 进池达标={len(p2)}")
    print(
        f"== {'symbol':<10}{'mode':<14}{'filter':<16}{'rb':>5}{'age':>7}"
        f"{'2025ret':>9}{'2025dd':>8}{'legs':>5}{'mult':>6}{'tp':>5}{'sl':>6}{'foq':>7}"
    )

    def short(r):
        return (
            f"{str(r.get('symbol')):<10}{str(r.get('direction_mode')):<14}{str(r.get('entry_filter')):<16}"
            f"{str(r.get('regime_break_ema_period')):>5}{str(r.get('max_cycle_age_hours')):>7}"
            f"{float(r.get('total_return_pct', 0)):>9.2f}{float(r.get('max_drawdown_pct', 0)):>8.2f}"
            f"{r.get('max_legs'):>5}{float(r.get('multiplier', 0)):>6.2f}{r.get('take_profit_bps'):>5}{r.get('stop_loss_bps'):>6}{float(r.get('first_order_quote', 0)):>7.1f}"
        )
    for r in p2[:40]:
        print("  " + short(r))

    if args.list_only or not p2:
        Path(args.out).write_text(json.dumps({
            "search": args.search, "budget": args.budget,
            "dedup_total": len(rows), "p2_pool_passing": len(p2),
            "p2_candidates": p2[:100],
        }, ensure_ascii=False, indent=2))
        print(f"\n== list-only 模式 (或无达标候选), 已写 {args.out}")
        return 0

    selected = select_candidates(p2, args.select_mode, args.top)
    results = []
    print(f"\n== 对 top {len(selected)} 候选跑 full+5 段 replay ...")
    for idx, r in enumerate(selected, 1):
        cfg = build_portfolio(r, args.budget)
        seg_metrics: dict[str, dict] = {}
        for seg, (s_ms, e_ms) in SEGMENTS.items():
            out = replay(cfg, s_ms, e_ms, args.budget, args.profile)
            if out is None or "_error" in out:
                seg_metrics[seg] = {"_error": (out or {}).get("_error", "none")}
            else:
                ob = out.get("on_budget", {}) or {}
                seg_metrics[seg] = {
                    "total_return_pct": ob.get("total_return_pct"),
                    "annualized_return_pct": ob.get("annualized_return_pct"),
                    "max_drawdown_pct": ob.get("max_drawdown_pct"),
                    "principal_breached": ob.get("principal_breached"),
                    "min_equity_quote": ob.get("min_equity_quote"),
                    "max_capital_used_quote": out.get("max_capital_used_quote"),
                    "budget_blocked_legs": out.get("budget_blocked_legs"),
                    "trade_count": out.get("trade_count"),
                }

        # 自校验: replay 的 2025 段 vs search row 的 2025 段
        search_2025 = {"ret": float(r.get("total_return_pct", 0)), "ann": float(r.get("annualized_return_pct", 0)), "dd": float(r.get("max_drawdown_pct", 0))}
        rep = seg_metrics.get("2025", {})
        selfcheck = None
        if "total_return_pct" in rep:
            dr = abs(float(rep["total_return_pct"]) - search_2025["ret"])
            dd = abs(float(rep["max_drawdown_pct"]) - search_2025["dd"])
            selfcheck = {"dret": round(dr, 3), "ddd": round(dd, 3), "ok": dr < max(5.0, abs(search_2025["ret"]) * 0.15) and dd < 3.0}

        # overfit flags
        def ret_of(seg):
            m = seg_metrics.get(seg, {})
            v = m.get("total_return_pct")
            return float(v) if isinstance(v, (int, float)) else None
        r_h1 = ret_of("h1_2023")
        r_full = ret_of("full")
        h1_contrib = (r_h1 / r_full) if (r_h1 is not None and r_full not in (None, 0)) else None
        r24, r25, r26 = ret_of("2024"), ret_of("2025"), ret_of("2026_ytd")
        combined_24_26 = None
        if all(v is not None for v in (r24, r25, r26)):
            combined_24_26 = ((1 + r24 / 100) * (1 + r25 / 100) * (1 + r26 / 100) - 1) * 100

        full_m = seg_metrics.get("full", {})
        rec = {
            "rank": idx,
            "symbol": r.get("symbol"),
            "direction_mode": r.get("direction_mode"),
            "entry_filter": r.get("entry_filter"),
            "params": {k: r.get(k) for k in PARAM_KEYS},
            "search_2025": search_2025,
            "segments": seg_metrics,
            "selfcheck_2025": selfcheck,
            "overfit_flags": {
                "h1_2023_contribution_ratio": round(h1_contrib, 3) if h1_contrib is not None else None,
                "combined_return_2024_2026_pct": round(combined_24_26, 2) if combined_24_26 is not None else None,
                "full_total_return_pct": r_full,
                "full_ann_pct": full_m.get("annualized_return_pct"),
                "full_max_drawdown_pct": full_m.get("max_drawdown_pct"),
                "full_principal_breached": full_m.get("principal_breached"),
            },
        }
        results.append(rec)
        sc = "" if (selfcheck and selfcheck["ok"]) else " ⚠selfcheck"
        print(
            f"  [{idx:>2}] {r.get('symbol'):<9}{r.get('direction_mode'):<13}{str(r.get('entry_filter')):<15}"
            f"rb={r.get('regime_break_ema_period')} age={r.get('max_cycle_age_hours')}  "
            f"2025={r25}  full_ann={full_m.get('annualized_return_pct')}  full_dd={full_m.get('max_drawdown_pct')}"
            f"  h1%={round(h1_contrib,2) if h1_contrib is not None else None}  24-26={round(combined_24_26,1) if combined_24_26 is not None else None}{sc}"
        )

    Path(args.out).write_text(json.dumps({
        "search": args.search, "budget": args.budget, "profile": args.profile,
        "dedup_total": len(rows), "p2_pool_passing": len(p2),
        "verified_count": len(results),
        "results": results,
    }, ensure_ascii=False, indent=2))
    print(f"\n== 完成, 写 {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
