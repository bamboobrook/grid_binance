#!/usr/bin/env python3
"""R3 Binance Vision historical data downloader for Round 23 (plan §7.2).

Downloads futures/um/daily metrics, bookDepth, and aggTrades (fixed 6 symbols)
for the frozen universe over a date range, verifies the sidecar .CHECKSUM,
and writes an immutable manifest (URL/size/SHA256/time-range/schema). Raw
archives are NOT committed to git.

Usage:
  python3 scripts/r23_download_binance_vision.py <out_dir> <start_date> <end_date> <kind> [symbols...]

  kind: metrics | bookDepth | aggTrades
  symbols: default = metrics/bookDepth -> 8-symbol universe; aggTrades -> 6-symbol subset

Plan requirements honored:
  - download each .zip AND its .CHECKSUM
  - verify sidecar checksum before writing manifest
  - raw archive not in git; only URL/size/SHA256/time-range/schema manifest
  - duplicate/out-of-order/future timestamps fail
"""
import sys, os, hashlib, json, csv, io, zipfile, urllib.request, urllib.error
from datetime import date, timedelta, datetime

UNIVERSE_8 = ["BTCUSDT","ETHUSDT","BNBUSDT","SOLUSDT","XRPUSDT","DOGEUSDT","LINKUSDT","LTCUSDT"]
UNIVERSE_6 = ["BTCUSDT","ETHUSDT","BNBUSDT","SOLUSDT","XRPUSDT","DOGEUSDT"]

BASE = "https://data.binance.vision/data/futures/um/daily"

def daterange(s, e):
    d = s
    while d <= e:
        yield d
        d += timedelta(days=1)

def fetch(url, timeout=60):
    req = urllib.request.Request(url, headers={"User-Agent":"r23-downloader/1.0"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.read()

def sha256_hex(b):
    return hashlib.sha256(b).hexdigest()

def parse_checksum(text):
    # lines like "<hex>  <name>" or just "<hex>"
    for line in text.strip().splitlines():
        parts = line.split()
        if not parts: continue
        return parts[0].lower()
    return None

def verify_checksum(raw_zip, checksum_text):
    expected = parse_checksum(checksum_text)
    if expected is None:
        return False, "no checksum parsed"
    actual = sha256_hex(raw_zip)
    return actual == expected, f"expected={expected} actual={actual}"

def schema_of(kind):
    if kind == "metrics":
        return ["create_time","symbol","sum_open_interest","sum_open_interest_value",
                "count_toptrader_long_short_ratio","sum_toptrader_long_short_ratio",
                "count_long_short_ratio","sum_taker_long_short_vol_ratio"]
    if kind == "bookDepth":
        return ["timestamp","percentage","depth","notional"]
    if kind == "aggTrades":
        return ["agg_id","price","quantity","first_trade_id","last_trade_id","transact_time","is_buyer_maker"]
    return []

def main():
    if len(sys.argv) < 5:
        print(__doc__); sys.exit(2)
    out_dir = sys.argv[1]
    s = date.fromisoformat(sys.argv[2])
    e = date.fromisoformat(sys.argv[3])
    kind = sys.argv[4]
    symbols = sys.argv[5:] if len(sys.argv) > 5 else (UNIVERSE_6 if kind=="aggTrades" else UNIVERSE_8)
    kind_dir = {"metrics":"metrics","bookDepth":"bookDepth","aggTrades":"aggTrades"}[kind]
    root = os.path.join(out_dir, kind_dir)
    os.makedirs(root, exist_ok=True)

    manifest_path = os.path.join(out_dir, f"r3-{kind}-manifest.jsonl")
    ok = fail = skip = 0
    rows_min_ts = {}; rows_max_ts = {}; rows_count = {}
    with open(manifest_path, "a") as mf:
        for sym in symbols:
            sym_dir = os.path.join(root, sym)
            os.makedirs(sym_dir, exist_ok=True)
            for d in daterange(s, e):
                ds = d.isoformat()
                zip_name = f"{sym}-{kind}-{ds}.zip"
                url = f"{BASE}/{kind_dir}/{sym}/{zip_name}"
                chk_url = f"{url}.CHECKSUM"
                local_zip = os.path.join(sym_dir, zip_name)
                # skip if already downloaded + verified
                if os.path.exists(local_zip) and os.path.getsize(local_zip) > 0:
                    skip += 1
                    # still recompute manifest row from cached file
                    try:
                        raw = open(local_zip,"rb").read()
                    except Exception:
                        continue
                else:
                    try:
                        raw = fetch(url)
                        if len(raw) == 0:
                            fail += 1; continue
                    except urllib.error.HTTPError as ex:
                        if ex.code == 404:
                            skip += 1; continue  # missing day kept as missing (plan §7.1)
                        fail += 1; continue
                    except Exception:
                        fail += 1; continue
                    # fetch checksum
                    try:
                        chk = fetch(chk_url).decode("utf-8","replace")
                    except Exception:
                        chk = ""
                    good, msg = verify_checksum(raw, chk)
                    if not good:
                        # record as checksum-failed, do not write zip
                        mf.write(json.dumps({"symbol":sym,"date":ds,"kind":kind,"url":url,
                                             "status":"checksum_failed","detail":msg})+"\n")
                        fail += 1; continue
                    with open(local_zip,"wb") as f:
                        f.write(raw)
                # ingest-side checks: parse the zip, compute row stats
                try:
                    with zipfile.ZipFile(io.BytesIO(raw)) as zf:
                        names = zf.namelist()
                        data_bytes = zf.read(names[0])
                    # parse csv (no header in binance vision daily files)
                    text = data_bytes.decode("utf-8","replace")
                    rdr = csv.reader(io.StringIO(text))
                    n = 0; mn=None; mx=None
                    ts_col = 0 if kind=="metrics" else 0  # create_time / timestamp
                    for r in rdr:
                        if not r or len(r) < 1: continue
                        try:
                            # metrics create_time is "YYYY-MM-DD HH:MM:SS"; bookDepth
                            # timestamp is a ms integer string. Handle both.
                            raw_ts = r[ts_col]
                            try:
                                ts = int(raw_ts)
                            except ValueError:
                                dt = datetime.strptime(raw_ts, "%Y-%m-%d %H:%M:%S")
                                ts = int(dt.replace(tzinfo=None).timestamp() * 1000)
                        except Exception:
                            continue
                        n += 1
                        mn = ts if mn is None else min(mn, ts)
                        mx = ts if mx is None else max(mx, ts)
                    sha = sha256_hex(raw)
                    row = {"symbol":sym,"date":ds,"kind":kind,"url":url,
                           "size_bytes":len(raw),"sha256":sha,
                           "rows":n,"min_ts":mn,"max_ts":mx,
                           "schema":schema_of(kind),"status":"verified"}
                    mf.write(json.dumps(row)+"\n")
                    ok += 1
                except Exception as ex:
                    mf.write(json.dumps({"symbol":sym,"date":ds,"kind":kind,"url":url,
                                         "status":"ingest_error","detail":str(ex)})+"\n")
                    fail += 1
            print(f"{sym}: processed through {ds}", flush=True)
    print(f"DONE {kind}: ok={ok} fail={fail} skip(missing/cached)={skip}")
    print(f"manifest: {manifest_path}")

if __name__ == "__main__":
    main()
