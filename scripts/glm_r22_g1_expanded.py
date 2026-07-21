#!/usr/bin/env python3
"""R22 G1 EXPANDED: 100+ configs on continuous prequential account.

Verifier requires multiplier 1.0-5.0, fo_quote 5-200, max_legs 3-6, entry_z 0.5-3.0.
"""
import sys, json, time, os
sys.path.insert(0, 'scripts')
from glm_r22_r3_prequential import run_prequential

start_t = time.time()
results = []
# Expanded grid
for mult in [1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0]:
    for fo in [5, 10, 20, 30, 50, 80, 120, 200]:
        for ml in [3, 4, 5, 6]:
            for ez in [0.5, 1.0, 1.5, 2.0, 3.0]:
                cfg = {'budget': 500.0, 'entry_z': ez, 'so_step': 0.5,
                       'exit_z': 0.5, 'group_fo_quote': float(fo),
                       'multiplier': mult, 'max_legs': ml}
                r = run_prequential(cfg)
                r['config'] = cfg
                results.append(r)
                if r['total_return_pct'] > 0:
                    print(f"POSITIVE: m={mult} fo={fo} ml={ml} ez={ez}: ret={r['total_return_pct']:.1f}% dd={r['max_equity_dd_pct']:.1f}%")

elapsed = time.time() - start_t
results.sort(key=lambda r: -r['total_return_pct'])
positive = [r for r in results if r['total_return_pct'] > 0]
print(f"\nelapsed: {elapsed:.1f}s, configs: {len(results)}, positive: {len(positive)}")
print(f"TOP 10:")
for r in results[:10]:
    c = r['config']
    print(f"  m={c['multiplier']} fo={c['group_fo_quote']} ml={c['max_legs']} ez={c['entry_z']}: "
          f"ret={r['total_return_pct']:.1f}% dd={r['max_equity_dd_pct']:.1f}% "
          f"pos={r['positive_blocks']}/12 so={r['had_so']}")

os.makedirs('docs/superpowers/artifacts/glm-martingale-core-round22/g1/gates', exist_ok=True)
summary = {
    'phase': 'R22 G1 expanded prequential sweep',
    'elapsed_s': round(elapsed, 1),
    'total_configs': len(results),
    'positive_configs': len(positive),
    'total_replays': len(results),
    'top10': [{'config': r['config'], 'total_return_pct': r['total_return_pct'],
               'max_equity_dd_pct': r['max_equity_dd_pct'],
               'positive_blocks': r['positive_blocks'], 'had_so': r['had_so']} for r in results[:10]],
    'grid': {'mult': [1.0,1.5,2.0,2.5,3.0,3.5,4.0,5.0],
             'fo': [5,10,20,30,50,80,120,200],
             'max_legs': [3,4,5,6], 'entry_z': [0.5,1.0,1.5,2.0,3.0]},
}
with open('docs/superpowers/artifacts/glm-martingale-core-round22/g1/gates/g1.json', 'w') as f:
    json.dump(summary, f, indent=2, sort_keys=True)
print(f"wrote g1.json ({len(results)} configs)")
