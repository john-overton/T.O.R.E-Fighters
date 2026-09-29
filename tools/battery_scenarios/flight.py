"""Lane: takeoff to landing, flight model, weapons, countermeasures, damage."""
from battery import Scenario


def scenarios() -> list[Scenario]:
    return [
        Scenario(name="flight-level-f18", lane="flight", args=["--headless-flight", "1200", "--aircraft", "f18", "--maneuver", "level", "--no-audio"]),
    ]
