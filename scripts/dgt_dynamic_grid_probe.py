#!/usr/bin/env python3
"""Research-only Dynamic Grid Trading probe for martingale frontier exploration."""
from __future__ import annotations

import argparse
import itertools
import json
import math
import sqlite3
from pathlib import Path

MS_PER_DAY = 86_400_000
LIVE_PARITY_STATUS = "research_only"

SEGMENTS = {
    "h1_2023": (1672531200000, 1688169599999),
    "h2_2023": (1688169600000, 1704067199999),
    "2024": (1704067200000, 1735689599999),
    "2025": (1735689600000, 1767225599999),
    "2026_ytd": (1767225600000, 1780271999999),
    "full": (1672531200000, 1780271999999),
}

PROFILE_TARGETS = {
    "conservative": {"ann": 50.0, "dd": 10.0},
    "balanced": {"ann": 90.0, "dd": 20.0},
    "aggressive": {"ann": 110.0, "dd": 30.0},
}


def grid_levels(center_price: float, spacing: float, half_grid_count: int) -> list[float]:
    if center_price <= 0:
        raise ValueError("center_price must be positive")
    if spacing <= 0:
        raise ValueError("spacing must be positive")
    if half_grid_count <= 0:
        raise ValueError("half_grid_count must be positive")
    return [
        center_price * (1.0 - spacing * step)
        for step in range(half_grid_count, 0, -1)
    ] + [center_price] + [
        center_price * (1.0 + spacing * step)
        for step in range(1, half_grid_count + 1)
    ]


def simulate_dgt_symbol(
    symbol: str,
    bars: list[dict],
    principal_quote: float,
    grid_spacing: float,
    half_grid_count: int,
    fee_bps: float = 8.0,
) -> dict:
    if not bars:
        raise ValueError(f"no bars for {symbol}")
    if principal_quote <= 0:
        raise ValueError("principal_quote must be positive")
    fee_rate = fee_bps / 10_000.0
    grid_count = half_grid_count * 2
    center_price = float(bars[0]["open"])
    levels = grid_levels(center_price, grid_spacing, half_grid_count)
    lower = levels[0]
    upper = levels[-1]
    cursor = half_grid_count
    traversals = 0
    reset_count = 0
    usdt_wallet = 0.0
    inventory_qty = 0.0
    active_principal = principal_quote
    total_input_quote = principal_quote
    max_input_quote = principal_quote
    total_fee_quote = 0.0
    points = []
    last_close = center_price

    def equity(price: float) -> float:
        return usdt_wallet + inventory_qty * price + active_principal

    def fund_next_grid() -> None:
        nonlocal usdt_wallet, total_input_quote, max_input_quote
        if usdt_wallet >= principal_quote:
            usdt_wallet -= principal_quote
        else:
            needed = principal_quote - usdt_wallet
            total_input_quote += needed
            max_input_quote = max(max_input_quote, total_input_quote)
            usdt_wallet = 0.0

    def reset_grid(price: float) -> None:
        nonlocal center_price, levels, lower, upper, cursor, traversals, reset_count
        center_price = price
        levels = grid_levels(center_price, grid_spacing, half_grid_count)
        lower = levels[0]
        upper = levels[-1]
        cursor = half_grid_count
        traversals = 0
        reset_count += 1

    def arbitrage_profit(extra_traversals: int) -> float:
        gross = max(0.0, extra_traversals) * (principal_quote / grid_count) * grid_spacing
        fees = max(0.0, extra_traversals) * (principal_quote / grid_count) * fee_rate * 2.0
        return gross - fees

    for index, bar in enumerate(bars):
        path = [float(bar["open"]), float(bar["low"]), float(bar["high"]), float(bar["close"])]
        if index > 0:
            path[0] = last_close
        for start_price, end_price in zip(path, path[1:]):
            if start_price < end_price:
                while cursor < grid_count and start_price <= levels[cursor + 1] < end_price:
                    cursor += 1
                    traversals += 1
            elif start_price > end_price:
                while cursor > 0 and end_price <= levels[cursor - 1] < start_price:
                    cursor -= 1
                    traversals += 1

            if end_price >= upper or cursor == grid_count:
                profitable_steps = half_grid_count + max(0, traversals - half_grid_count)
                profit = arbitrage_profit(profitable_steps)
                usdt_wallet += principal_quote + profit
                total_fee_quote += profitable_steps * (principal_quote / grid_count) * fee_rate * 2.0
                fund_next_grid()
                reset_grid(end_price)

            if end_price <= lower or cursor == 0:
                profit = arbitrage_profit(max(0, traversals - half_grid_count))
                usdt_wallet += profit
                total_fee_quote += max(0, traversals - half_grid_count) * (principal_quote / grid_count) * fee_rate * 2.0
                inventory_qty += (principal_quote / max(center_price, 1e-12)) * (1.0 - fee_rate)
                fund_next_grid()
                reset_grid(end_price)

        last_close = float(bar["close"])
        points.append({"timestamp_ms": int(bar["timestamp_ms"]), "equity_quote": equity(last_close)})

    return {
        "name": f"dgt:{symbol}:gs{grid_spacing}:h{half_grid_count}:p{principal_quote}",
        "kind": "dgt",
        "symbols": [symbol],
        "points": points,
        "max_input_quote": max_input_quote,
        "max_capital_used_quote": max_input_quote,
        "total_input_quote": total_input_quote,
        "reset_count": reset_count,
        "total_fee_quote": total_fee_quote,
        "principal_quote": principal_quote,
        "grid_spacing": grid_spacing,
        "half_grid_count": half_grid_count,
        "live_parity_status": LIVE_PARITY_STATUS,
    }
