"""The airports a Quick Mission ground start never uses: short strips and
the other side's fields.

John decided on 2026-09-30 that short strips (a runway under 2,000 ft, the 22
strips of about 1,074 ft in Cuba, the Falklands, Pakistan, Panama and the
Persian Gulf) are no ground start for the player or any wing. Since slice AL1
(John, 2026-10-10) each airport takes its runway's layout side, and the
player's Blue wing starts only at a Blue or neutral field. The AI probe's
`--ground-start N` (N counts the theater's airports, 1 up) refuses both, so a
scenario that wants a takeoff picks one with `ground_airport`.

The tables are what `TORE_AIRPORT_PROBE=1 tore-app --quick-mission --snapshot
X.ppm --theater CODE` lists as `short_strip=true` and `allegiance=Some(Hostile)`
(docs/testing/lane-ai.md, "Short strips"; `--airport-allegiance` prints the
offered fields by name). The developer `~` layouts are in neither table. The
Falklands (whose one Blue field, San Carlos, is a short strip) and North
Vietnam have no Blue field at all, so no Blue ground start there.
"""
from __future__ import annotations

# Base theater -> the airport numbers of its short strips.
SHORT_STRIPS = {
    "APA": {2, 3, 4, 5, 6},
    "CUB": {13, 14, 15, 16, 17, 18},
    "LFA": {2, 3},
    "PGU": {9, 10, 13, 14, 15},
    "SPA": {8, 9, 13, 15},
}

# Base theater -> the airport numbers of its Redfor fields: the enemy's, for
# the player's Blue wing (slice AL1). No retail field is neutral.
ENEMY_FIELDS = {
    "APA": {5, 6, 9, 10, 11, 12, 13, 14, 16, 17, 18, 19, 20},
    "BAL": {1, 13, 14, 15, 16, 17, 18, 22, 23, 27, 28, 29, 30},
    "CUB": set(range(2, 24)),
    "EGY": {1, 2, 3, 4, 5, 6, 7, 8, 9, 18, 19, 20, 21},
    "FRA": {2, 3, 5, 6, 7, 8, 9, 10, 18, 19, 20, 21, 22, 23, 28, 29, 30, 31, 32},
    "GRE": {7, 8, 10, 11, 12, 16, 17},
    "IRA": {2, 4, 5, 6, 7, 10, 11, 12, 17, 18, 19, 20, 21},
    "KURILE": {2, 3, 4},
    "LFA": {1, 2, 4, 5},
    "NSK": {1, 2, 3, 4, 5, 6, 7, 8, 20, 21, 22},
    "PGU": {4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15, 16, 20, 21},
    "SPA": {1, 5, 6, 7, 13, 14, 15, 20, 21},
    "TVIET": set(range(1, 11)),
    "UKR": {1, 3, 4, 5, 6, 7, 10, 13, 14},
    "VLA": {1, 2, 3, 4, 5, 14, 15, 16, 17, 19, 20, 21, 26},
    "WTA": {1, 2, 7, 12},
}

# Base theater -> its number of airports.
AIRPORT_COUNTS = {
    "APA": 20, "BAL": 34, "CUB": 23, "EGY": 25, "FRA": 32, "GRE": 17, "IRA": 21, "KURILE": 4,
    "LFA": 5, "NSK": 22, "PGU": 21, "SPA": 21, "TVIET": 10, "UKR": 14, "VLA": 26, "WTA": 16,
}


def is_short_strip(theater: str, airport: int) -> bool:
    return airport in SHORT_STRIPS.get(theater, ())


def is_enemy_field(theater: str, airport: int) -> bool:
    """A Redfor field, where the player's Blue wing never starts."""
    return airport in ENEMY_FIELDS.get(theater, ())


def is_ground_start(theater: str, airport: int) -> bool:
    return not is_short_strip(theater, airport) and not is_enemy_field(theater, airport)


def ground_airport(theater: str, wanted: int) -> int | None:
    """`wanted` if Blue may start there, else the next airport up that Blue
    may; None when there is none (every one from `wanted` up is a short strip
    or the enemy's). A developer `~` layout keeps `wanted`."""
    last = AIRPORT_COUNTS.get(theater)
    if last is None:
        return wanted
    for airport in range(wanted, last + 1):
        if is_ground_start(theater, airport):
            return airport
    return None
