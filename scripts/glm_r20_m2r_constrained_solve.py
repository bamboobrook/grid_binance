#!/usr/bin/env python3
"""Round 20 P4 M2R fix: constrained factor-neutral projection solve.

Plan §6.3 constraints:
  sum(abs(w)) = 1
  abs(sum(w * beta_i)) <= 0.10
  max abs(w_i) <= 0.25
  long gross >= 0.35 AND short gross >= 0.35
  actual legs >= 6

The betas are mostly positive (altcoin beta to BTC). To achieve factor-neutrality
we need a long-short basket where the weighted beta sums near zero. This module
implements an iterative projection: start equal-weight, alternate projections
onto (a) beta-neutral hyperplane, (b) box [−0.25, 0.25], (c) normalize sum|w|=1,
until convergence, then verify long/short gross >= 0.35.
"""
import json
import math
import os

ART = "docs/superpowers/artifacts/glm-martingale-core-round20"
P4 = os.path.join(ART, "p4")


def solve_constrained_weights(betas, max_abs_w=0.25, beta_exposure_limit=0.10,
                              min_gross_per_side=0.35, n_iter=2000, tol=1e-9):
    """Iterative projection onto the constrained set. Returns (weights, exposure,
    long_gross, short_gross, feasible) or None if infeasible."""
    n = len(betas)
    if n < 6:
        return None
    # Start: equal weight with half long / half short signs based on beta sign.
    # For mostly-positive betas, we need more short weight to cancel exposure.
    w = [1.0/n] * n
    # initial signs: legs with above-median beta get shorted (negative w),
    # below-median get long (positive w). This is a heuristic start; the
    # projection will adjust magnitudes.
    sorted_beta_idx = sorted(range(n), key=lambda i: betas[i])
    half = n // 2
    signs = [0] * n
    for i in sorted_beta_idx[:half]:  # lowest beta -> long
        signs[i] = 1
    for i in sorted_beta_idx[half:]:  # highest beta -> short
        signs[i] = -1
    w = [signs[i] / n for i in range(n)]

    def project_beta_neutral(w):
        """Project onto sum(w*beta)=0 hyperplane."""
        s = sum(wi * bi for wi, bi in zip(w, betas))
        norm2 = sum(bi * bi for bi in betas)
        if norm2 < 1e-12:
            return w
        # w <- w - (s / norm2) * beta
        return [wi - (s / norm2) * bi for wi, bi in zip(w, betas)]

    def project_box(w):
        return [max(-max_abs_w, min(max_abs_w, wi)) for wi in w]

    def project_normalize(w):
        total = sum(abs(wi) for wi in w)
        if total < 1e-12:
            return w
        return [wi / total for wi in w]

    prev_w = None
    for it in range(n_iter):
        w = project_beta_neutral(w)
        w = project_box(w)
        w = project_normalize(w)
        if prev_w is not None:
            diff = max(abs(wi - pwi) for wi, pwi in zip(w, prev_w))
            if diff < tol:
                break
        prev_w = list(w)

    exposure = abs(sum(wi * bi for wi, bi in zip(w, betas)))
    long_gross = sum(abs(wi) for wi in w if wi > 0)
    short_gross = sum(abs(wi) for wi in w if wi < 0)
    max_w = max(abs(wi) for wi in w)
    total = sum(abs(wi) for wi in w)
    feasible = (exposure <= beta_exposure_limit and max_w <= max_abs_w + 1e-9
                and long_gross >= min_gross_per_side - 1e-9
                and short_gross >= min_gross_per_side - 1e-9
                and abs(total - 1.0) < 1e-6)
    return {"weights": [round(x, 8) for x in w], "exposure": round(exposure, 6),
            "long_gross": round(long_gross, 6), "short_gross": round(short_gross, 6),
            "max_w": round(max_w, 6), "total_abs": round(total, 6),
            "feasible": feasible, "iterations": it + 1}


def main():
    d = json.load(open(os.path.join(P4, "families-fit.json")))
    fixed_folds = []
    all_feasible = True
    for f in d["fold_fits"]:
        m2r = f["M2R"]["frozen_fits"]
        if not m2r:
            fixed_folds.append(f)
            continue
        betas = m2r[0]["betas"]
        sol = solve_constrained_weights(betas)
        if sol is None or not sol["feasible"]:
            print(f"{f['fold']}: infeasible (betas={betas})")
            all_feasible = False
            f["M2R"]["solve_status"] = "infeasible"
            fixed_folds.append(f)
            continue
        # update the frozen fit with constrained weights
        m2r[0]["weights"] = sol["weights"]
        # recompute direction signs from weight signs
        m2r[0]["leg_direction_signs"] = [1 if w > 0 else -1 for w in sol["weights"]]
        m2r[0]["diagnostics"]["beta_exposure"] = sol["exposure"]
        m2r[0]["diagnostics"]["beta_exposure_pass"] = sol["exposure"] <= 0.10
        m2r[0]["diagnostics"]["long_gross"] = sol["long_gross"]
        m2r[0]["diagnostics"]["short_gross"] = sol["short_gross"]
        m2r[0]["diagnostics"]["max_w"] = sol["max_w"]
        m2r[0]["diagnostics"]["constrained_solve_feasible"] = sol["feasible"]
        f["M2R"]["beta_exposure_pass"] = sol["exposure"] <= 0.10
        f["M2R"]["blocked"] = False
        f["M2R"]["solve_status"] = "constrained_feasible"
        print(f"{f['fold']}: exposure={sol['exposure']} long={sol['long_gross']} short={sol['short_gross']} max_w={sol['max_w']} feasible={sol['feasible']} iter={sol['iterations']}")
        fixed_folds.append(f)
    d["fold_fits"] = fixed_folds
    d["m2r_constrained_solve"] = {"all_feasible": all_feasible, "method": "iterative projection: beta-neutral hyperplane -> box[-0.25,0.25] -> normalize sum|w|=1"}
    out = os.path.join(P4, "families-fit.json")
    with open(out, "w") as fh:
        json.dump(d, fh, indent=2)
    print(f"\nall_feasible: {all_feasible}")
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
