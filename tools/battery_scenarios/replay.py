"""Lane: mission recording, replay playback, comms, audio, input and everything around flight.

The scenarios live in the `_replay_*.py` modules next to this file (one per
area) so each stays readable. See docs/testing/lane-replay.md.
"""
from __future__ import annotations

from battery import Scenario, Step
from battery_scenarios.flight import check_combat_evidence
from battery_scenarios import (
    _replay_a2g,
    _replay_keys,
    _replay_live,
    _replay_misc,
    _replay_record,
    _replay_script,
    _replay_surface,
    _replay_view,
)


def scenarios() -> list[Scenario]:
    found: list[Scenario] = []
    found += _replay_record.scenarios()
    found += _replay_surface.scenarios()
    found += _replay_a2g.scenarios()
    found += _replay_view.scenarios()
    found += _replay_live.scenarios()
    found += _replay_keys.scenarios()
    found += _replay_script.scenarios()
    found += _replay_misc.scenarios()
    found += _replay_misc.import_scenarios()
    found.append(Scenario(
        name="replay-stores-unfired",
        lane="replay",
        args=["--replay-stores-check"],
        expect=[
            "Replay stores: PASS",
            "Replay stores: F15.PT unfired shapes, live vertices, release origins and 9 count states agree",
            "Replay stores: F16C.PT unfired shapes, live vertices, release origins and 9 count states agree",
            "Replay stores: MI24.PT unfired shapes, live vertices, release origins and 5 count states agree",
            r"Replay stores: F15\.PT hardpoint=4 .*points=\[\[5\.0, -",
            r"Replay stores: F16C\.PT hardpoint=7 .*points=.*-7\.0",
            r"Replay stores: MI24\.PT hardpoint=2 .*points=\[\[[0-9.]+, -",
        ],
        timeout=120,
    ))
    found.append(Scenario(
        name="replay-banner-objects",
        lane="replay",
        args=["--ai-probe-ticks", "3600", "--separation", "5", "--probe-attack", "600:10",
              "--record-mission", "{work}/air.tore-replay", "--no-audio"],
        outputs=["air.tore-replay"],
        then=[
            Step(["--replay-banner-check", "{work}/air.tore-replay"], timeout=120),
            Step(["--a2g-run", "UKR", "QUCOL", "--attack", "release", "--from", "1",
                  "--aircraft", "a4e", "--load", "MK82AIR", "--count", "1",
                  "--altitude", "3000", "--speed", "450", "--seconds", "12",
                  "--record", "{work}/bomb.tore-replay"], timeout=120),
            Step(["--replay-banner-check", "{work}/bomb.tore-replay"], timeout=120),
        ],
        expect=["Replay banners: PASS", "banner: missile", "banner: bomb", "banner: ground"],
        timeout=300,
        notes="Every selectable identity has a banner; includes air combat, falling bombs and scenery without a GPU.",
    ))
    # Not the Mi-24: its combat smoke fails on the base (the AT-2 guidance
    # probe reports NoTarget), unrelated to where its stores hang.
    for aircraft in ("f15", "f16c"):
        found.append(Scenario(
            name=f"replay-stores-release-{aircraft}",
            lane="replay",
            args=["--combat-smoke", "--aircraft", aircraft, "--no-audio"],
            env={"TORE_COMBAT_EVIDENCE": "{work}/evidence"},
            check=check_combat_evidence,
            timeout=300,
            notes="Moved carrier rows: exercise firing and replay each slot's combat tape. "
                  "replay-stores-unfired also checks drawn origins and recorded vertices.",
        ))
    return found
