#!/usr/bin/env python3
"""R22 S1/S2/S3 dynamic pair selectors for continuous prequential.

S0 = OLS static (already implemented in r3)
S1 = PC1 dynamic: select pairs by residual against first PC of the universe
S2 = KSS hazard: Kapetanios-Shin-Snell nonlinear unit root test (STR-ECM)
S3 = Delayed cointegration: Engle-Granger with break detection

Each selector returns top-K disjoint pairs for a given fit window.
"""
import math, itertools, hashlib, json, sqlite3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

UNIVERSE = ["BTCUSDT","ETHUSDT","BNBUSDT","SOLUSDT","XRPUSDT","DOGEUSDT",
            "ADAUSDT","TRXUSDT","LINKUSDT","LTCUSDT","BCHUSDT","DOTUSDT",
            "AVAXUSDT","ATOMUSDT","NEARUSDT","APTUSDT","AAVEUSDT",
            "ALGOUSDT","COMPUSDT","UNIUSDT"]

def load_log_prices_daily(symbol, start_ms, end_ms, freq_hours=24):
    conn = sqlite3.connect(f"file:{ROOT}/data/market_data_full.db?mode=ro", uri=True)
    cur = conn.cursor()
    mod = freq_hours * 3600 * 1000
    cur.execute("SELECT open_time, close FROM klines WHERE symbol=? "
        "AND market_type='futures_usdt_perp' AND timeframe='1m' "
        "AND open_time>=? AND open_time<=? AND open_time % ? = 0 ORDER BY open_time",
        (symbol, start_ms, end_ms, mod))
    rows = cur.fetchall(); conn.close()
    return {t: math.log(float(c)) for t, c in rows if c and float(c) > 0}

def fit_pair_ols(la, lb):
    common = sorted(set(la) & set(lb))
    n = len(common)
    if n < 60: return None
    xs = [lb[t] for t in common]; ys = [la[t] for t in common]
    mx = sum(xs)/n; my = sum(ys)/n
    sxx = sum((x-mx)**2 for x in xs)
    sxy = sum((xs[i]-mx)*(ys[i]-my) for i in range(n))
    beta = sxy/sxx if sxx > 0 else 1.0
    mu = my - beta*mx
    resid = [ys[i]-beta*xs[i]-mu for i in range(n)]
    mr = sum(resid)/n; vr = sum((r-mr)**2 for r in resid)/n
    sigma = vr**0.5
    if vr <= 0 or n <= 2: return None
    phi = sum((resid[i]-mr)*(resid[i-1]-mr) for i in range(1,n))/(n*vr)
    hl = (-0.693147/math.log(phi)) if 0 < phi < 1 else 999.0
    dr = [resid[i]-resid[i-1] for i in range(1,n)]
    rl = [resid[i-1]-mr for i in range(1,n)]
    sxx2 = sum(x*x for x in rl)
    if sxx2 <= 0: return None
    alpha = sum(dr[i]*rl[i] for i in range(len(dr)))/sxx2
    fe = [dr[i]-alpha*rl[i] for i in range(len(dr))]
    se2 = sum(e*e for e in fe)/(len(dr)-2)
    sea = math.sqrt(se2/sxx2)
    adf_t = alpha/sea if sea > 0 else 0.0
    z = [(r-mr)/sigma for r in resid]
    pnl = []; pos = 0
    for i in range(1, len(z)):
        if pos == 0 and abs(z[i-1]) > 1.0: pos = -1 if z[i-1] > 0 else 1
        elif pos != 0 and abs(z[i-1]) < 0.5: pos = 0
        if pos != 0: pnl.append(pos*(z[i]-z[i-1])*sigma)
    sharpe = 0.0
    if len(pnl) > 5:
        m = sum(pnl)/len(pnl); v = sum((p-m)**2 for p in pnl)/len(pnl)
        sharpe = (m/v**0.5)*(len(pnl)**0.5) if v > 0 else 0
    return {"beta":beta,"mu":mu,"sigma":sigma,"half_life_h":hl*24,
            "adf_t":adf_t,"ac1":phi,"mr_sharpe":sharpe,"n":n}

def kss_test(resid):
    """Kapetanios-Shin-Shell nonlinear unit root test.
    Tests H0: unit root vs H1: STAR(ESTAR) mean reversion.
    t-stat on delta in: delta_resid = delta * resid^3 + eps"""
    n = len(resid)
    if n < 30: return None
    mr = sum(resid)/n
    r = [x - mr for x in resid]
    # KSS regression: d(r_t) = delta * r_{t-1}^3 + eps
    dr = [r[i]-r[i-1] for i in range(1,n)]
    r3 = [r[i-1]**3 for i in range(1,n)]
    sxx = sum(x*x for x in r3)
    if sxx <= 0: return None
    delta = sum(dr[i]*r3[i] for i in range(len(dr)))/sxx
    fe = [dr[i]-delta*r3[i] for i in range(len(dr))]
    se2 = sum(e*e for e in fe)/(len(dr)-1)
    sea = math.sqrt(se2/sxx)
    return delta/sea if sea > 0 else 0.0

def detect_break(resid):
    """Simple break detection: compare variance in first half vs second half."""
    n = len(resid)
    if n < 40: return False
    mid = n // 2
    v1 = sum((x-sum(resid[:mid])/mid)**2 for x in resid[:mid])/mid
    v2 = sum((x-sum(resid[mid:])/(n-mid))**2 for x in resid[mid:])/(n-mid)
    ratio = max(v1,v2)/(min(v1,v2)+1e-12)
    return ratio > 3.0  # significant variance break

def select_S0_OLS(prices, universe):
    """S0: OLS static pair selection (baseline)."""
    candidates = []
    for a, b in itertools.combinations(universe, 2):
        f = fit_pair_ols(prices.get(a,{}), prices.get(b,{}))
        if f is None: continue
        if f["adf_t"] >= -2.85 or f["half_life_h"] >= 168: continue
        if f["mr_sharpe"] < 0.3: continue
        candidates.append((a, b, f))
    candidates.sort(key=lambda x: -x[2]["mr_sharpe"])
    used = set(); groups = []
    for a, b, f in candidates:
        if a in used or b in used: continue
        groups.append({"pair":(a,b), **f}); used.update({a,b})
        if len(groups) >= 6: break
    return groups

def select_S1_PC1(prices, universe):
    """S1: PC1 dynamic — select pairs by residual against first PC."""
    # Compute PC1 of the universe log prices
    common = sorted(set.intersection(*[set(prices.get(s,{}).keys()) for s in universe]))
    n = len(common)
    if n < 60: return []
    syms = universe
    k = len(syms)
    P = [[prices.get(s,{}).get(t, 0) for s in syms] for t in common]
    means = [sum(P[i][j] for i in range(n))/n for j in range(k)]
    Pc = [[P[i][j]-means[j] for j in range(k)] for i in range(n)]
    cov = [[0.0]*k for _ in range(k)]
    for i in range(n):
        for a in range(k):
            for b in range(a,k):
                cov[a][b] += Pc[i][a]*Pc[i][b]
    for a in range(k):
        for b in range(k):
            cov[a][b] /= n
    for a in range(k):
        for b in range(a+1,k):
            cov[b][a] = cov[a][b]
    v = [1.0/math.sqrt(k)]*k
    for _ in range(100):
        nv = [sum(cov[a][b]*v[b] for b in range(k)) for a in range(k)]
        norm = math.sqrt(sum(x*x for x in nv)) or 1.0
        v = [x/norm for x in nv]
    pc1 = {common[i]: sum(v[j]*Pc[i][j] for j in range(k)) for i in range(n)}
    # Select pairs by residual against PC1
    candidates = []
    for sym in syms:
        f = fit_pair_ols(prices.get(sym,{}), pc1)
        if f is None: continue
        # KSS test for nonlinear MR
        resid_common = sorted(set(prices.get(sym,{}).keys()) & set(pc1.keys()))
        resid = [prices[sym][t] - f["beta"]*pc1[t] - f["mu"] for t in resid_common]
        kss = kss_test(resid)
        if kss is not None and kss > -1.5: continue  # KSS gate
        if f["half_life_h"] >= 168: continue
        if f["mr_sharpe"] < 0.3: continue
        candidates.append((sym, "PC1", f))
    candidates.sort(key=lambda x: -x[2]["mr_sharpe"])
    used = set(); groups = []
    for sym, _, f in candidates:
        if sym in used: continue
        groups.append({"pair":(sym, "PC1"), **f}); used.add(sym)
        if len(groups) >= 6: break
    return groups

def select_S2_KSS(prices, universe):
    """S2: KSS hazard — select pairs passing nonlinear MR test."""
    candidates = []
    for a, b in itertools.combinations(universe, 2):
        f = fit_pair_ols(prices.get(a,{}), prices.get(b,{}))
        if f is None: continue
        common = sorted(set(prices.get(a,{}).keys()) & set(prices.get(b,{}).keys()))
        resid = [prices[a][t]-f["beta"]*prices[b][t]-f["mu"] for t in common]
        kss = kss_test(resid)
        if kss is None or kss > -2.0: continue  # strict KSS gate
        if f["half_life_h"] >= 168: continue
        if f["mr_sharpe"] < 0.3: continue
        candidates.append((a, b, f, kss))
    candidates.sort(key=lambda x: (-x[3], -x[2]["mr_sharpe"]))  # sort by KSS then Sharpe
    used = set(); groups = []
    for a, b, f, kss in candidates:
        if a in used or b in used: continue
        groups.append({"pair":(a,b), **f}); used.update({a,b})
        if len(groups) >= 6: break
    return groups

def select_S3_delayed(prices, universe):
    """S3: Delayed cointegration — OLS + break detection, skip pairs with breaks."""
    candidates = []
    for a, b in itertools.combinations(universe, 2):
        f = fit_pair_ols(prices.get(a,{}), prices.get(b,{}))
        if f is None: continue
        if f["adf_t"] >= -2.85 or f["half_life_h"] >= 168: continue
        if f["mr_sharpe"] < 0.3: continue
        common = sorted(set(prices.get(a,{}).keys()) & set(prices.get(b,{}).keys()))
        resid = [prices[a][t]-f["beta"]*prices[b][t]-f["mu"] for t in common]
        has_break = detect_break(resid)
        if has_break: continue  # skip pairs with structural breaks
        candidates.append((a, b, f))
    candidates.sort(key=lambda x: -x[2]["mr_sharpe"])
    used = set(); groups = []
    for a, b, f in candidates:
        if a in used or b in used: continue
        groups.append({"pair":(a,b), **f}); used.update({a,b})
        if len(groups) >= 6: break
    return groups

SELECTORS = {
    "S0_OLS": select_S0_OLS,
    "S1_PC1": select_S1_PC1,
    "S2_KSS": select_S2_KSS,
    "S3_delayed": select_S3_delayed,
}

def select_block_pairs(selector_name, block, universe=None):
    """Main entry: select pairs for a block using the named selector."""
    if universe is None:
        universe = UNIVERSE
    prices = {s: load_log_prices_daily(s, block["fit_start_ms"], block["fit_end_ms"]) for s in universe}
    fn = SELECTORS.get(selector_name, select_S0_OLS)
    return fn(prices, universe)

if __name__ == "__main__":
    import json
    PROTOCOL = json.load(open(ROOT / "docs/superpowers/artifacts/glm-martingale-core-round22/r2/prequential-protocol.json"))
    block = PROTOCOL["test_blocks"][0]
    for name in SELECTORS:
        pairs = select_block_pairs(name, block)
        print(f"{name}: {len(pairs)} pairs")
        for p in pairs[:3]:
            print(f"  {p['pair']}: adf={p['adf_t']:.2f} hl={p['half_life_h']:.1f}h sharpe={p['mr_sharpe']:.2f}")
