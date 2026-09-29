#!/usr/bin/env python3
"""Print the tore-app command an AI lane fuzz seed runs (tools/battery_scenarios/_ai_fuzz.py).

    python3 tools/_ai_fuzz_cmd.py 42
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from battery_scenarios import _ai_fuzz  # noqa: E402

for seed in (int(a) for a in sys.argv[1:]):
    args, ticks, _ = _ai_fuzz.config(seed)
    print(" ".join(["target/debug/tore-app", "--ai-probe-ticks", str(ticks), *args, "--no-audio"]))
