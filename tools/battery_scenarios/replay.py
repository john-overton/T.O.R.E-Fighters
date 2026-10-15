"""Lane: mission recording, replay playback, comms, audio, input and everything around flight.

The scenarios live in the `_replay_*.py` modules next to this file (one per
area) so each stays readable. See docs/testing/lane-replay.md.
"""
from __future__ import annotations

from battery import Scenario, Step
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
        expect=["Replay stores: PASS"],
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
    return found
