#!/usr/bin/env python3
import json, subprocess, sys
EXEC = ["docker","exec","-i","grid-binance-postgres-1","psql","-U","postgres","-d","grid_binance"]
READ = ["docker","exec","-i","grid-binance-postgres-1","psql","-U","postgres","-d","grid_binance","-qAt","-c"]
SRC="mp_margin_v2_lp_conservative_20260626"; NEW_ID="mp_smoke_50u_ltc_btc_20260626"; SYMBOLS={"LTCUSDT","BTCUSDT"}

def read(sql):
    r=subprocess.run(READ+[sql],capture_output=True,text=True); 
    if r.returncode: print("READ ERR:",r.stderr[:500]); sys.exit(1)
    return r.stdout.strip()
def execsql(sql):
    r=subprocess.run(EXEC,input=sql,capture_output=True,text=True)
    print("exec:",r.stdout.strip())
    if r.returncode: print("EXEC ERR:",r.stderr[:500]); sys.exit(1)

cfg = json.loads(read(f"select config from martingale_portfolios where portfolio_id='{SRC}';"))
pcfg = cfg["portfolio_config"]
strats = [s for s in pcfg["strategies"] if s.get("symbol") in SYMBOLS]
assert len(strats)==4, f"got {len(strats)}"
for s in strats: s["portfolio_weight_pct"]="25"
pcfg["strategies"]=strats; pcfg["risk_limits"]["max_global_budget_quote"]=None; cfg["portfolio_config"]=pcfg
rs = {"source":"smoke_50u_clone_conservative_ltc_btc","strategy_count":4,"enabled_strategy_count":4,
      "candidate_count":2,"distinct_symbol_count":2,"symbols":sorted(SYMBOLS),"max_leverage":10,
      "total_weight_pct":"100","capital_model":{"first_order_quote":"order notional",
      "futures_margin":"notional / leverage","returns_drawdown_denominator":"planned margin capital"}}
cfg_s=json.dumps(cfg).replace("'","''"); rs_s=json.dumps(rs).replace("'","''")
execsql(f"""INSERT INTO martingale_portfolios
(portfolio_id,owner,name,status,source_task_id,market,direction,risk_profile,total_weight_pct,config,risk_summary)
VALUES ('{NEW_ID}','flyingkid2022@outlook.com','Smoke 50U LTC+BTC (parity validation)','pending_confirmation','smoke','usd_m_futures','long_short','smoke',100,'{cfg_s}'::jsonb,'{rs_s}'::jsonb)
ON CONFLICT (portfolio_id) DO UPDATE SET config=excluded.config,risk_summary=excluded.risk_summary,updated_at=now();
""")
print("verify:", read(f"select count(*)||' strats, sum='||sum((s->>'portfolio_weight_pct')::numeric) from martingale_portfolios, jsonb_array_elements((config->'portfolio_config')->'strategies') s where portfolio_id='{NEW_ID}';"))
