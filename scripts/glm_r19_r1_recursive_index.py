#!/usr/bin/env python3
"""Round 19 R1: recursive canonical fingerprint index (plan §3).

MUST recursively scan ALL rounds, not just the most recent:
  docs/superpowers/artifacts/glm-martingale-core-round*/**/*.json
  docs/superpowers/artifacts/glm-martingale-core-round*/**/*.jsonl
  docs/superpowers/reports/*round*.md
  docs/superpowers/plans/*round*.md

This fixes the R18 bug where scan_historical() only scanned R17 (audit §5.1).
"""
import glob
import hashlib
import json
import os
import re

ART_ROOT = "docs/superpowers/artifacts"
HIST_INDEX = "docs/superpowers/artifacts/glm-martingale-core-round19/historical-fingerprint-index.json"


def sha256_str(s):
    return hashlib.sha256(s.encode()).hexdigest()


def extract_fingerprints_from_registry(path):
    """Read a registry JSONL and extract (fingerprint, window, budget, fold, status)."""
    rows = []
    try:
        with open(path) as fh:
            for ln in fh:
                ln = ln.strip()
                if not ln:
                    continue
                try:
                    r = json.loads(ln)
                except json.JSONDecodeError:
                    continue
                fp = r.get("fingerprint_sha256") or r.get("effective_config_hash") or r.get("resolved_config_hash")
                if not fp:
                    continue
                rows.append({
                    "fingerprint_sha256": fp,
                    "window": r.get("window", ""),
                    "budget": r.get("budget", ""),
                    "fold": r.get("fold", ""),
                    "seed": r.get("seed", ""),
                    "status": r.get("status", ""),
                    "family": r.get("family", ""),
                    "source_row": "registry",
                })
    except (OSError, json.JSONDecodeError):
        pass
    return rows


def extract_fingerprints_from_checkpoint(path):
    """Read a checkpoint JSON (g1/g2-checkpoint.json) and extract done keys."""
    rows = []
    try:
        d = json.load(open(path))
        done = d.get("done", {})
        if isinstance(done, dict):
            for key, rec in done.items():
                if not isinstance(rec, dict):
                    continue
                fp = rec.get("effective_config_hash") or rec.get("resolved_config_hash")
                if not fp:
                    continue
                rows.append({
                    "fingerprint_sha256": fp,
                    "window": rec.get("window", key.split("|")[1] if "|" in key else ""),
                    "budget": rec.get("budget", key.split("|")[-1] if "|" in key else ""),
                    "fold": rec.get("fold", ""),
                    "seed": rec.get("seed", ""),
                    "status": rec.get("status", ""),
                    "family": rec.get("family", ""),
                    "source_row": "checkpoint",
                })
        configs = d.get("configs", [])
        for i, c in enumerate(configs):
            if isinstance(c, dict):
                fp = sha256_str(json.dumps(c, sort_keys=True))
                rows.append({
                    "fingerprint_sha256": fp, "window": "", "budget": "", "fold": "",
                    "seed": "", "status": "config-only", "family": c.get("family", ""),
                    "source_row": f"checkpoint_config_{i}",
                })
    except (OSError, json.JSONDecodeError):
        pass
    return rows


def extract_fingerprints_from_json(path):
    """Generic JSON: look for hash fields."""
    rows = []
    try:
        d = json.load(open(path))
    except (OSError, json.JSONDecodeError):
        return rows
    fingerprints = set()
    def walk(obj):
        if isinstance(obj, dict):
            for k, v in obj.items():
                if k in ("fingerprint_sha256", "effective_config_hash", "resolved_config_hash",
                         "train_selection_commit_sha256", "fit_sha256", "config_sha256") and isinstance(v, str) and len(v) >= 16:
                    fingerprints.add((k, v))
                else:
                    walk(v)
        elif isinstance(obj, list):
            for x in obj:
                walk(x)
    walk(d)
    for k, fp in fingerprints:
        rows.append({"fingerprint_sha256": fp, "source_row": f"json:{k}", "window": "", "budget": "", "fold": "", "status": "", "family": ""})
    return rows


def main():
    rounds_found = []
    excluded_keys = set()
    by_source = {}

    # 1. Recursively scan all round artifact dirs
    round_dirs = sorted(glob.glob(f"{ART_ROOT}/glm-martingale-core-round*/"))
    for rd in round_dirs:
        rname = os.path.basename(rd.rstrip("/")).replace("glm-martingale-core-", "")
        rounds_found.append(rname)
        # JSONL registries
        for jsonl in glob.glob(f"{rd}**/*.jsonl", recursive=True):
            for row in extract_fingerprints_from_registry(jsonl):
                key = f"{rname}:{row['fingerprint_sha256']}|{row.get('window','')}|{row.get('budget','')}"
                excluded_keys.add(key)
                by_source.setdefault(rname, 0)
                by_source[rname] += 1
        # Checkpoint + generic JSON
        for jf in glob.glob(f"{rd}**/*.json", recursive=True):
            if "checkpoint" in jf:
                rows = extract_fingerprints_from_checkpoint(jf)
            else:
                rows = extract_fingerprints_from_json(jf)
            for row in rows:
                key = f"{rname}:{row['fingerprint_sha256']}|{row.get('window','')}|{row.get('budget','')}"
                excluded_keys.add(key)
                by_source.setdefault(rname, 0)
                by_source[rname] += 1

    # 2. Scan reports + plans (text content hashes for never-repeat)
    for txt in (glob.glob("docs/superpowers/reports/*round*.md") + glob.glob("docs/superpowers/plans/*round*.md")):
        try:
            with open(txt, "rb") as fh:
                h = hashlib.sha256(fh.read()).hexdigest()
            excluded_keys.add(f"text:{os.path.basename(txt)}:{h}")
        except OSError:
            pass

    index = {
        "phase": "R1 recursive canonical fingerprint index",
        "rounds_scanned": rounds_found,
        "rounds_count": len(rounds_found),
        "excluded_keys_count": len(excluded_keys),
        "by_round": by_source,
        "excluded_keys_sample": sorted(excluded_keys)[:10],
        "scope": "All Round 1-18 fingerprints are dedup-excluded. Round 19 may use a different cycle_topology / family / fit_contract / resolved config so its fingerprints cannot collide with these. Round 18 M1 exact configs may be re-run as repair_replay (plan §0.5) but must record old fingerprint -> new fingerprint mapping.",
        "forbidden_repeat_families_audit_r18_section_7": [
            "Round 18 M1 with static [1,1] directions (the bug): may not be repeated as a result",
            "Round 1-17 independent-symbol Sobol configs with old HTF gate",
            "R14/15 dual-state SO scale, partial TP, minigrid, depth TP exact family",
            "pure trend/breakout/funding carry/pair-neutral/stat-arb as standalone PnL engine",
        ],
    }
    os.makedirs(os.path.dirname(HIST_INDEX), exist_ok=True)
    with open(HIST_INDEX, "w") as fh:
        json.dump(index, fh, indent=2, sort_keys=True)
    print(f"rounds scanned: {len(rounds_found)} -> {rounds_found}")
    print(f"excluded keys: {len(excluded_keys)}")
    print(f"by round: {by_source}")
    print(f"wrote {HIST_INDEX}")


if __name__ == "__main__":
    main()
