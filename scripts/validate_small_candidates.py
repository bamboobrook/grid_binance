#!/usr/bin/env python3
import json
import subprocess
import sys


SRC = "/tmp/small_search_scaled_h1_sample20.json"
OUT = "/tmp/small_full_validation_from_h1.json"


def cfg_for(row):
    cfg = {
        "direction_mode": "long_and_short",
        "strategies": [],
        "risk_limits": {"max_global_budget_quote": str(row["budget"])},
    }
    for idx, direction in enumerate(["long", "short"]):
        triggers = [{"cooldown": {"seconds": int(row["cooldown_seconds"])}}]
        if row["adx_min"] is not None:
            triggers.append(
                {"indicator_expression": {"expression": f"adx(14) > {int(row['adx_min'])}"}}
            )
        cfg["strategies"].append(
            {
                "strategy_id": f"full-{row['symbol']}-{direction}-{idx}",
                "symbol": row["symbol"],
                "market": "usd_m_futures",
                "direction": direction,
                "direction_mode": "long_and_short",
                "margin_mode": "isolated",
                "leverage": int(row["leverage"]),
                "spacing": {"fixed_percent": {"step_bps": int(row["step_bps"])}},
                "sizing": {
                    "multiplier": {
                        "first_order_quote": str(row["first_order_quote"]),
                        "multiplier": str(row["multiplier"]),
                        "max_legs": int(row["max_legs"]),
                    }
                },
                "take_profit": {"percent": {"bps": int(row["take_profit_bps"])}},
                "stop_loss": {
                    "strategy_drawdown_pct": {"pct_bps": int(row["stop_loss_bps"])}
                },
                "indicators": [{"atr": {"period": 21}}, {"adx": {"period": 14}}],
                "entry_triggers": triggers,
                "risk_limits": {
                    "max_active_cycles": None,
                    "max_global_budget_quote": None,
                    "max_symbol_budget_quote": None,
                    "max_direction_budget_quote": None,
                    "max_strategy_budget_quote": None,
                    "max_global_drawdown_quote": None,
                },
                "portfolio_weight_pct": "50",
            }
        )
    return {"portfolio_config": cfg}


def selected_rows():
    root = json.load(open(SRC))
    rows = []
    for frontier in root["frontier_by_budget"].values():
        for key in [
            "highest_annualized",
            "best_under_dd10",
            "best_under_dd20",
            "best_under_dd30",
            "lowest_dd_over_ann50",
            "lowest_dd_over_ann90",
            "lowest_dd_over_ann110",
        ]:
            rows.extend(frontier.get(key, []))
    seen = set()
    uniq = []
    for row in rows:
        key = (
            row["budget"],
            row["symbol"],
            row["first_order_quote"],
            row["leverage"],
            row["multiplier"],
            row["max_legs"],
            row["step_bps"],
            row["take_profit_bps"],
            row["cooldown_seconds"],
            row["adx_min"],
            row["stop_loss_bps"],
        )
        if key not in seen:
            seen.add(key)
            uniq.append(row)
    uniq.sort(
        key=lambda row: (
            row["aggressive_pass"] or row["balanced_pass"] or row["conservative_pass"],
            row["annualized_return_pct"] - 3 * row["max_drawdown_pct"],
        ),
        reverse=True,
    )
    return uniq[:36]


def main():
    rows = selected_rows()
    print("selected", len(rows), flush=True)
    out = []
    for idx, row in enumerate(rows):
        path = f"/tmp/full_candidate_{idx}.json"
        json.dump(cfg_for(row), open(path, "w"))
        cmd = [
            "target/release/portfolio_budget_replay",
            "--config",
            path,
            "--budget",
            str(row["budget"]),
            "--start-ms",
            "1672531200000",
            "--end-ms",
            "1780271999999",
            "--market-data",
            "data/market_data_full.db",
            "--funding-data",
            "data/funding_rates.db",
            "--profile",
            "aggressive",
            "--portfolio-id",
            f"small_full_{idx}",
            "--exchange-min-notional",
            "5",
        ]
        proc = subprocess.run(cmd, capture_output=True, text=True)
        if proc.returncode != 0:
            print("ERR", idx, proc.stderr[:500], flush=True)
            continue
        result = json.loads(proc.stdout)
        on_budget = result["on_budget"]
        rec = dict(row)
        rec.update(
            {
                "full_annualized_return_pct": on_budget["annualized_return_pct"],
                "full_max_drawdown_pct": on_budget["max_drawdown_pct"],
                "full_total_return_pct": on_budget["total_return_pct"],
                "full_min_equity_quote": on_budget["min_equity_quote"],
                "full_principal_breached": on_budget["principal_breached"],
                "full_trade_count": result["trade_count"],
                "full_stop_count": result["stop_count"],
                "full_fee_slip": (result.get("total_fee_quote") or 0)
                + (result.get("total_slippage_quote") or 0),
                "full_max_capital_used_quote": result["on_max_capital_used"][
                    "max_capital_used_quote"
                ],
            }
        )
        ann = rec["full_annualized_return_pct"]
        dd = rec["full_max_drawdown_pct"]
        rec["full_conservative_pass"] = (
            ann > 50 and dd <= 10 and not rec["full_principal_breached"]
        )
        rec["full_balanced_pass"] = (
            ann > 90 and dd <= 20 and not rec["full_principal_breached"]
        )
        rec["full_aggressive_pass"] = (
            ann > 110 and dd <= 30 and not rec["full_principal_breached"]
        )
        out.append(rec)
        print(
            idx,
            row["budget"],
            row["symbol"],
            "H1 ann/dd",
            round(row["annualized_return_pct"], 1),
            round(row["max_drawdown_pct"], 1),
            "FULL ann/dd",
            round(ann, 1),
            round(dd, 1),
            "pass",
            rec["full_conservative_pass"],
            rec["full_balanced_pass"],
            rec["full_aggressive_pass"],
            flush=True,
        )
    json.dump(out, open(OUT, "w"), indent=2)
    print("wrote", OUT, len(out), flush=True)


if __name__ == "__main__":
    sys.exit(main())
