"""Seeded random AI probe configurations for the AI lane (docs/testing/lane-ai.md).

Each seed in SEEDS draws one whole configuration with `random.Random(seed)`:
theater (base or layout variant), fight size, the player's, friendly AI and
enemy aircraft, enemy skill, mission, separation, geometry, flight adapter,
ground or airborne start, wing orders, controlled threats and faults at random
ticks, the attack script or a passive leader, and a short or long run. The
same seed always gives the same command, so a failing scenario name
(`ai-fuzz-0042`) is enough to reproduce it:

    python3 tools/battery.py --scenario ai-fuzz-0042
    python3 tools/_ai_fuzz_cmd.py 42        # prints the tore-app command

The AI lane runs DEFAULT_COUNT seeds by default; `TORE_AI_FUZZ=all` runs them
all (python3 tools/battery.py --lane ai --scenario 'ai-fuzz-*').
"""
from __future__ import annotations

import os
import random

from ._strips import ground_airport

AIRCRAFT = ["f18", "rafale", "f14", "a4e", "x31", "mig29", "su27", "mig21", "su25", "mig23", "su35", "f22", "f22n", "faxx"]
BASE_THEATERS = ["APA", "BAL", "CUB", "EGY", "FRA", "GRE", "IRA", "KURILE", "LFA", "NSK", "PGU", "SPA", "TVIET", "UKR", "VLA", "WTA"]
VARIANTS = ["~APAF", "~BAL0", "~BAL3", "~BALF", "~CUBF", "~EGY1", "~EGY5", "~EGYF", "~FRA0", "~FRA4", "~FRA9", "~FRAF",
            "~GREF", "~IRAF", "~KURILE", "~LFAF", "~NSKF", "~PGUF", "~SPAF", "~TVIET", "~UKR1", "~UKR4", "~UKR8", "~UKRF",
            "~VLA1", "~VLA5", "~VLAF", "~WTAF"]
SKILLS = ["novice", "average", "experienced", "ace"]
MISSIONS = ["free", "cap", "intercept", "escort", "self-defense", "hold"]
ORDERS = ["bug-out", "land-selected", "attack-on-contact", "engage-my-target"]
THREATS = ["hit", "gun", "aaa"]

# The fixed seed list; results are repeatable because nothing else is random.
SEEDS = list(range(1, 401))
DEFAULT_COUNT = 60


def config(seed: int) -> tuple[list[str], int, dict]:
    """The probe arguments, tick count and a readable summary for one seed."""
    r = random.Random(seed)
    ground = r.random() < 0.25
    theater = r.choice(BASE_THEATERS) if ground else r.choice(BASE_THEATERS + VARIANTS)
    friendly, enemy = r.randint(1, 15), r.randint(1, 15)
    player, wing, foe = r.choice(AIRCRAFT), r.choice(AIRCRAFT), r.choice(AIRCRAFT)
    big = friendly + enemy
    long = r.random() < 0.15
    ticks = r.choice([3600, 6000, 9600, 14400]) if not long else 36000
    if big > 20:
        ticks = min(ticks, 14400)
    args = ["--theater", theater, "--probe-fight", f"{friendly}:{enemy}", "--aircraft", player,
            "--probe-friendly-aircraft", wing, "--probe-enemy-aircraft", foe,
            "--probe-enemy-skill", r.choice(SKILLS), "--ai-mission", r.choice(MISSIONS),
            "--probe-geometry", r.choice(["head", "side", "rear"])]
    # The draw is unchanged; a short strip (no ground start there, John
    # 2026-09-30) or a Redfor field (slice AL1) moves to the next airport that
    # is one, and a theater with none for Blue flies an airborne start.
    airport = ground_airport(theater, r.randint(1, 3)) if ground else None
    ground = airport is not None
    if ground:
        args += ["--ground-start", str(airport), "--maneuver", "takeoff", "--separation", r.choice(["20", "50", "100"])]
    else:
        args += ["--separation", r.choice(["1", "2", "5", "10", "20", "50"]),
                 "--probe-flight-model", r.choice(["legacy", "researched"])]
    if r.random() < 0.6:
        args += ["--probe-attack", f"{r.randint(1, 20) * 60}:{r.choice([5, 10, 20])}"]
        if r.random() < 0.2:
            args += ["--probe-guns"]
    if r.random() < 0.15:
        args += ["--probe-ai-guns-only"]
    for _ in range(r.choice([0, 0, 1, 2, 3])):
        args += ["--probe-wing-order", f"{r.randint(1, ticks // 60 - 1) * 60}:{r.choice(ORDERS)}"]
    for _ in range(r.choice([0, 0, 0, 1, 2])):
        args += ["--probe-threat", f"{r.randint(1, ticks // 60 - 1) * 60}:{r.choice(THREATS)}"]
    for _ in range(r.choice([0, 0, 0, 1, 2])):
        args += ["--probe-fault", f"{r.randint(1, ticks // 60 - 1) * 60}:{r.randint(0, 44)}"]
    summary = {"seed": seed, "theater": theater, "fight": f"{friendly}:{enemy}", "ground": ground, "ticks": ticks}
    return args, ticks, summary


def selected_seeds() -> list[int]:
    return SEEDS if os.environ.get("TORE_AI_FUZZ") == "all" else SEEDS[:DEFAULT_COUNT]
