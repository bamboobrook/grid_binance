#!/usr/bin/env python3
"""R2 binding gate evidence: read binding-probes.json and record for validator."""
import json
import os

ART = "docs/superpowers/artifacts/glm-martingale-core-round16"


def main():
    probes = json.load(open(os.path.join(ART, "r2", "binding-probes.json")))
    families = [{"name": f["name"], "bound": f["bound"]} for f in probes["families"]]
    d = os.path.join(ART, "r2", "gates")
    os.makedirs(d, exist_ok=True)
    json.dump({"families": families, "all_bound": probes["all_bound"]},
              open(os.path.join(d, "d_h_c_params_bind_config_event_order_hash.json"), "w"), indent=2)
    print(f"R2 gate: all_bound={probes['all_bound']}, families={len(families)}")


if __name__ == "__main__":
    main()
