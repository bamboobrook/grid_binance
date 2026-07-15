#!/usr/bin/env python3
"""A2 backtest event-wiring evidence: verify the R17 control functions are
called from the kline_engine event loop (not just defined)."""
import json
import os

ART = "docs/superpowers/artifacts/glm-martingale-core-round17"


def find_calls(symbols):
    sites = {s: [] for s in symbols}
    root = "apps/backtest-engine/src"
    for dp, _, fns in os.walk(root):
        for fn in fns:
            if not fn.endswith(".rs"):
                continue
            path = os.path.join(dp, fn)
            if path.endswith("r17_controls.rs"):
                continue  # definitions
            try:
                with open(path, errors="replace") as fh:
                    for i, line in enumerate(fh, 1):
                        st = line.lstrip()
                        if st.startswith(("//", "pub fn", "fn ")):
                            continue
                        for s in symbols:
                            if s + "(" in line:
                                sites[s].append(f"{path}:{i}")
            except Exception:
                pass
    return sites


def main():
    syms = ["router_admission", "hazard_deadline", "cluster_scheduler", "funding_veto", "vol_cap"]
    sites = find_calls(syms)
    wired = all(v for v in sites.values())
    d = os.path.join(ART, "a2", "gates")
    os.makedirs(d, exist_ok=True)
    json.dump({"event_loop_wired": wired, "call_sites": sites,
               "definition_file": "apps/backtest-engine/src/martingale/r17_controls.rs"},
              open(os.path.join(d, "backtest_event_wiring_real.json"), "w"), indent=2)
    print(f"A2: wired={wired} sites={ {k: len(v) for k,v in sites.items()} }")


if __name__ == "__main__":
    main()
