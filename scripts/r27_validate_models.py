#!/usr/bin/env python3
"""Independent Round 27 model validator; does not import production fitters."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import resource
import sqlite3
import time
import warnings
from itertools import combinations
from multiprocessing import get_context
from pathlib import Path
from typing import Any

import numpy as np
from scipy.optimize import minimize_scalar
from scipy.stats import kendalltau, multivariate_normal, multivariate_t, norm, rankdata, t
from statsmodels.tools.sm_exceptions import InterpolationWarning
from statsmodels.tsa.statespace.structural import UnobservedComponents
from statsmodels.tsa.stattools import coint, kpss


REPO_ARTIFACT = Path("docs/superpowers/artifacts/glm-martingale-core-round27")
MARKET_DB = Path("data/market_data_full.db")
ALTS = [
    "AAVEUSDT", "ADAUSDT", "ALGOUSDT", "APTUSDT", "ATOMUSDT", "AVAXUSDT", "BCHUSDT", "BNBUSDT",
    "COMPUSDT", "CRVUSDT", "DASHUSDT", "DOGEUSDT", "DOTUSDT", "DYDXUSDT", "EGLDUSDT", "ETCUSDT",
    "ETHUSDT", "FILUSDT", "GALAUSDT", "HBARUSDT", "ICPUSDT", "INJUSDT", "LINKUSDT", "NEARUSDT",
    "SOLUSDT", "TRXUSDT", "UNIUSDT", "XRPUSDT", "ZECUSDT",
]
DAY_MS = 86_400_000
MINUTE_MS = 60_000
CACHE_ALGORITHM = "r27-independent-leg-v1"
RAW_ROOT = Path(".")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact-root", required=True)
    parser.add_argument("--workers", type=int, default=8)
    parser.add_argument("--resume", action="store_true")
    return parser.parse_args()


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(8 * 1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def clean(value: Any) -> Any:
    if isinstance(value, dict): return {key: clean(item) for key, item in value.items()}
    if isinstance(value, list): return [clean(item) for item in value]
    if isinstance(value, (np.integer,)): return int(value)
    if isinstance(value, (np.floating, float)):
        return float(value) if math.isfinite(float(value)) else None
    if isinstance(value, np.bool_): return bool(value)
    return value


def atomic_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = json.dumps(clean(value), indent=2, sort_keys=True, allow_nan=False).encode() + b"\n"
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_bytes(payload); temporary.replace(path)


def query_prices(frequency: str, symbols: tuple[str, ...], start: int, end: int) -> dict[str, tuple[np.ndarray, np.ndarray]]:
    interval = 3_600_000 if frequency == "1h" else 300_000
    connection = sqlite3.connect(f"file:{MARKET_DB}?mode=ro", uri=True)
    output = {}
    try:
        for symbol in symbols:
            rows = connection.execute(
                "SELECT close_time,close FROM klines INDEXED BY idx_klines_symbol_time "
                "WHERE symbol=? AND market_type='futures_usdt_perp' AND timeframe='1m' "
                "AND open_time>=? AND open_time<? AND (open_time % ?)=? ORDER BY open_time",
                (symbol, start, end, interval, interval - MINUTE_MS),
            ).fetchall()
            values = np.asarray(rows, dtype=float)
            output[symbol] = (values[:, 0].astype(np.int64), values[:, 1]) if values.size else (np.array([], dtype=np.int64), np.array([]))
    finally:
        connection.close()
    return output


def independent_leg(task: tuple[str, str, int, int]) -> tuple[tuple[str, str, int, int], dict[str, Any]]:
    frequency, symbol, anchor, formation = task
    start = anchor - formation * DAY_MS
    prices = query_prices(frequency, ("BTCUSDT", symbol), start, anchor)
    times, btc = prices["BTCUSDT"]; alt_times, alt = prices[symbol]
    interval = 3_600_000 if frequency == "1h" else 300_000
    expected = formation * DAY_MS // interval
    if not np.array_equal(times, alt_times) or times.size != expected:
        return task, {"sample_count": int(times.size), "sample_coverage": float(times.size / expected)}
    y, x = np.log(btc), np.log(alt)
    intercept, beta = np.linalg.lstsq(np.column_stack((np.ones(x.size), x)), y, rcond=None)[0]
    residual = y - intercept - beta * x
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", InterpolationWarning); warnings.simplefilter("ignore", RuntimeWarning)
        eg_stat, eg_p, _ = coint(y, x, trend="c", autolag="aic")
        kpss_result = kpss(residual, regression="c", nlags="auto")
    lag, delta = residual[:-1], np.diff(residual)
    phi = 1 + float(np.linalg.lstsq(np.column_stack((np.ones(lag.size), lag)), delta, rcond=None)[0][1])
    half_life = -math.log(2) / math.log(phi) if 0 < phi < 1 else math.inf
    midpoint = x.size // 2
    beta_first = float(np.linalg.lstsq(np.column_stack((np.ones(midpoint), x[:midpoint])), y[:midpoint], rcond=None)[0][1])
    beta_second = float(np.linalg.lstsq(np.column_stack((np.ones(x.size-midpoint), x[midpoint:])), y[midpoint:], rcond=None)[0][1])
    return task, {
        "sample_count": int(times.size), "sample_coverage": float(times.size/expected),
        "intercept": float(intercept), "beta": float(beta), "residual_sigma": float(np.std(residual,ddof=1)),
        "eg_coint_statistic": float(eg_stat), "eg_coint_p_value": float(eg_p),
        "kpss_statistic": float(kpss_result[0]), "kpss_p_value": float(kpss_result[1]),
        "half_life_bars": half_life,
        "robust_beta_drift": abs(beta_second-beta_first)/max(abs(float(beta)),0.10),
    }


def cache_path(task: tuple[str, str, int, int]) -> Path:
    frequency, symbol, anchor, formation = task
    return RAW_ROOT / "validator-checkpoints/model-legs-v1" / f"{frequency}-{formation}d-{anchor}-{symbol}.json"


def load_cache(task: tuple[str, str, int, int]) -> dict[str, Any] | None:
    path = cache_path(task)
    if not path.exists(): return None
    try: value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError): return None
    if value.get("algorithm") != CACHE_ALGORITHM or value.get("task") != list(task): return None
    return value.get("result") if isinstance(value.get("result"), dict) else None


def loglik(observations: np.ndarray, family: str, rho: float, nu: int | None) -> float:
    clipped = np.clip(observations, 1e-8, 1-1e-8)
    if family == "gaussian":
        transformed = norm.ppf(clipped)
        return float(np.sum(multivariate_normal.logpdf(transformed,mean=[0,0],cov=[[1,rho],[rho,1]])-norm.logpdf(transformed).sum(axis=1)))
    transformed = t.ppf(clipped,df=nu)
    return float(np.sum(multivariate_t.logpdf(transformed,shape=[[1,rho],[rho,1]],df=nu)-t.logpdf(transformed,df=nu).sum(axis=1)))


def fit_copula(left: np.ndarray, right: np.ndarray) -> dict[str, Any]:
    observations = np.column_stack((rankdata(left)/(left.size+1), rankdata(right)/(right.size+1)))
    fits=[]
    for family,nu in [("gaussian",None),*(("student_t",value) for value in range(3,31))]:
        result=minimize_scalar(lambda rho:-loglik(observations,family,float(rho),nu),bounds=(-.95,.95),method="bounded",options={"xatol":1e-8})
        fits.append({"family":family,"rho":float(result.x),"nu":nu,"aic":float((2 if family=="gaussian" else 4)+2*result.fun)})
    return min(fits,key=lambda row:(row["aic"],row["family"],row["nu"] or 0))


def independent_matching(edges: list[dict[str, Any]]) -> list[str]:
    best=(0,0.0,[])
    for count in range(1,min(3,len(edges))+1):
        for selected in combinations(edges,count):
            symbols=[value for edge in selected for value in (edge["left"],edge["right"])]
            if len(symbols)!=len(set(symbols)): continue
            ids=sorted(edge["pair_id"] for edge in selected); candidate=(count,sum(float(edge["score"]) for edge in selected),ids)
            if candidate[0]>best[0] or (candidate[0]==best[0] and (candidate[1]>best[1]+1e-12 or (abs(candidate[1]-best[1])<=1e-12 and candidate[2]<best[2]))): best=candidate
    return best[2]


def fit_pbd(spread: np.ndarray) -> dict[str, float | bool]:
    centered=spread-np.mean(spread)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        full=UnobservedComponents(centered,level=True,stochastic_level=True,irregular=True,autoregressive=1).fit(disp=False,maxiter=100)
        rw=UnobservedComponents(centered,level=True,stochastic_level=True,irregular=True).fit(disp=False,maxiter=100)
        ar=UnobservedComponents(centered,irregular=True,autoregressive=1).fit(disp=False,maxiter=100)
    params=dict(zip(full.param_names,map(float,full.params),strict=True)); total=params["sigma2.ar"]+params["sigma2.level"]+params["sigma2.irregular"]
    return {"rho":params["ar.L1"],"sigma2_m":params["sigma2.ar"],"sigma2_r":params["sigma2.level"],"sigma2_w":params["sigma2.irregular"],"finite_variance_share":params["sigma2.ar"]/total,"full_bic":float(full.bic),"rw_noise_bic":float(rw.bic),"ar_noise_bic":float(ar.bic),"converged":bool(full.mle_retvals.get("converged",False))}


def top20(anchor: int, formation: int) -> list[str]:
    start=anchor-formation*DAY_MS; connection=sqlite3.connect(f"file:{MARKET_DB}?mode=ro",uri=True); rows=[]
    try:
        for symbol in ALTS:
            values=np.asarray(connection.execute("SELECT open_time,close,volume FROM klines INDEXED BY idx_klines_symbol_time WHERE symbol=? AND market_type='futures_usdt_perp' AND timeframe='1m' AND open_time>=? AND open_time<? ORDER BY open_time",(symbol,start,anchor)).fetchall(),dtype=float)
            if values.shape[0]!=formation*1440: continue
            quote=values[:,1]*values[:,2]; daily=np.bincount(((values[:,0]-start)//DAY_MS).astype(int),weights=quote,minlength=formation)
            if np.count_nonzero(values[:,2]==0)/values.shape[0]<=.01 and np.quantile(quote,.1)>=10_000: rows.append((symbol,float(np.median(daily))))
    finally: connection.close()
    rows.sort(key=lambda row:(-row[1],row[0])); return [row[0] for row in rows[:20]]


def load_manifest(raw: Path, name: str) -> list[tuple[dict[str,Any],dict[str,Any]]]:
    manifest=json.loads((raw/"fit-snapshot-manifests"/f"{name}.json").read_text()); output=[]
    for row in manifest["snapshots"]:
        path=Path(row["path"])
        if sha256(path)!=row["sha256"]: raise RuntimeError(f"snapshot hash mismatch {path}")
        output.append((row,json.loads(path.read_text())))
    return output


def main() -> None:
    global RAW_ROOT
    args=parse_args(); started=time.time(); RAW_ROOT=Path(args.artifact_root)
    c=load_manifest(RAW_ROOT,"c0-c1"); p=load_manifest(RAW_ROOT,"p1"); violations=[]
    tasks=sorted({(snap["frequency"],leg["symbol"],int(snap["roll_anchor_ms"]),int(snap["formation_days"])) for _,snap in c for leg in snap["legs"]})
    results={}; pending=[]
    for task in tasks:
        cached=load_cache(task) if args.resume else None
        if cached is None: pending.append(task)
        else: results[task]=cached
    print(json.dumps({"stage":"legs","total":len(tasks),"cached":len(results),"pending":len(pending)}),flush=True)
    def record(item: tuple[tuple[str,str,int,int],dict[str,Any]]) -> None:
        task,result=item;results[task]=result;atomic_json(cache_path(task),{"algorithm":CACHE_ALGORITHM,"task":list(task),"result":result})
        if len(results)%100==0 or len(results)==len(tasks): print(json.dumps({"stage":"legs","completed":len(results),"total":len(tasks)}),flush=True)
    if pending:
        with get_context("fork").Pool(args.workers) as pool:
            for item in pool.imap_unordered(independent_leg,pending,chunksize=1): record(item)
    checked_legs=checked_pairs=checked_pbd=0; max_error=0.0
    fields=["sample_count","sample_coverage","intercept","beta","residual_sigma","eg_coint_statistic","eg_coint_p_value","kpss_statistic","kpss_p_value","half_life_bars","robust_beta_drift"]
    for _,snap in c:
        if top20(int(snap["roll_anchor_ms"]),int(snap["formation_days"]))!=snap["eligible_universe"]: violations.append(f"top20:{snap['frequency']}:{snap['formation_days']}:{snap['roll_anchor_ms']}")
        for leg in snap["legs"]:
            expected=results[(snap["frequency"],leg["symbol"],int(snap["roll_anchor_ms"]),int(snap["formation_days"]))];checked_legs+=1
            for field in fields:
                left,right=leg.get(field),expected.get(field)
                if left is None and right is None: continue
                if left is None and isinstance(right,float) and not math.isfinite(right): continue
                if left is None or right is None: violations.append(f"leg-null:{field}");continue
                error=abs(float(left)-float(right));max_error=max(max_error,error)
                if error>1e-8*max(1.0,abs(float(right))): violations.append(f"leg:{field}:{error}")
        for arm_name,arm in snap["arms"].items():
            actual=independent_matching(arm["pair_graph"]); expected=sorted(row["pair_id"] for row in arm["exact_matching"])
            if actual!=expected: violations.append(f"matching:{arm_name}:{snap['roll_anchor_ms']}")
            for pair in arm["exact_matching"]:
                prices=query_prices(snap["frequency"],("BTCUSDT",pair["left"],pair["right"]),int(snap["fit_start_ms"]),int(snap["fit_cutoff_ms"]))
                btc=prices["BTCUSDT"][1]; left=np.log(btc)-pair["left_leg"]["intercept"]-pair["left_leg"]["beta"]*np.log(prices[pair["left"]][1]); right=np.log(btc)-pair["right_leg"]["intercept"]-pair["right_leg"]["beta"]*np.log(prices[pair["right"]][1])
                fit=fit_copula(left,right);checked_pairs+=1
                if fit["family"]!=pair["copula"]["family"] or fit["nu"]!=pair["copula"]["nu"] or abs(fit["rho"]-pair["copula"]["rho"])>1e-6 or abs(fit["aic"]-pair["copula"]["aic"])>1e-5: violations.append(f"copula:{pair['pair_id']}:{snap['roll_anchor_ms']}")
    for _,snap in p:
        if top20(int(snap["roll_anchor_ms"]),7)!=snap["eligible_universe"]: violations.append(f"p1-top20:{snap['roll_anchor_ms']}")
        if independent_matching([row for row in snap["pair_fits"] if row.get("admission_passed")])!=sorted(row["pair_id"] for row in snap["exact_matching"]): violations.append(f"p1-matching:{snap['roll_anchor_ms']}")
        for pair in snap["exact_matching"]:
            prices=query_prices("5m",(pair["left"],pair["right"]),int(pair["fit_start_ms"]),int(pair["fit_cutoff_ms"])); spread=np.log(prices[pair["left"]][1])-np.log(prices[pair["right"]][1]); fit=fit_pbd(spread);checked_pbd+=1
            for field in ["rho","sigma2_m","sigma2_r","sigma2_w","finite_variance_share","full_bic","rw_noise_bic","ar_noise_bic"]:
                error=abs(float(fit[field])-float(pair[field]));max_error=max(max_error,error)
                if error>1e-5*max(1.0,abs(float(pair[field]))): violations.append(f"pbd:{pair['pair_id']}:{field}:{error}")
    r0=json.loads((REPO_ARTIFACT/"r0-round26-gate-decomposition.json").read_text())
    if r0["status"]!="PASS": violations.append("round26-gate-decomposition")
    finished=time.time(); report={"schema_version":1,"validator":"independent_r27_raw_db_model_validator","passed":not violations and checked_pairs>0 and (checked_pbd>0 or all(not snap["exact_matching"] for _,snap in p)),"snapshot_count":len(c)+len(p),"checked_leg_count":checked_legs,"checked_pair_count":checked_pairs,"checked_pbd_pair_count":checked_pbd,"max_numeric_error":max_error,"violations":violations,"production_fitter_imported":False,"raw_database_read":True,"runtime":{"argv":os.sys.argv,"pid":os.getpid(),"workers":args.workers,"start_epoch":started,"end_epoch":finished,"wall_seconds":finished-started,"peak_rss_kib":max(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss),"exit_code":0 if not violations else 1}}
    atomic_json(RAW_ROOT/"model-independent-validator.json",report);atomic_json(REPO_ARTIFACT/"model-independent-validator.json",report)
    print(json.dumps({"validator":"model","passed":report["passed"],"pairs":checked_pairs,"pbd_pairs":checked_pbd,"violations":len(violations)}))
    if not report["passed"]: raise SystemExit(1)


if __name__=="__main__": main()
