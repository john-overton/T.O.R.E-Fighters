"""Lane: mission recording, replay playback, comms, audio, input and everything around flight.

The scenarios live in the `_replay_*.py` modules next to this file (one per
area) so each stays readable. See docs/testing/lane-replay.md.
"""
from __future__ import annotations

from battery import Scenario
from battery_scenarios import _replay_misc, _replay_record, _replay_view


def scenarios() -> list[Scenario]:
    found: list[Scenario] = []
    found += _replay_record.scenarios()
    found += _replay_view.scenarios()
    found += _replay_misc.scenarios()
    found += _replay_misc.import_scenarios()
    return found
