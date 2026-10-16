"""Tests for the air-to-ground scenario checks X1 added
(tools/battery_scenarios/a2g_cluster.py and a2g_catalog.py)."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from battery_scenarios import a2g_catalog, a2g_cluster, a2g_sights  # noqa: E402


def cluster_run(source="CBU87.JT", radius=2200, damage=200, flashes=28, lost_outside=0, farthest=1182, lost_scale=1.0):
    """One `--a2g-run ... --attack cluster` output, built to the splash rule."""
    lines = [
        f"a2g-run: record {source} flags 0x82a052 land-explosion 18 object-explosion 18 water-explosion 18 "
        f"crater 0 splash-radius {radius} splash-percent 100 fuze 100 damage {';'.join([str(damage)] * 5)}",
        "a2g-run: cluster burst at 999916.5 -0.0 788873.1 blast Some(20) ticks 120",
    ]
    for i, distance in enumerate((359, 765, 1357, radius + 400)):
        gap = distance - 16
        lost = int(lost_scale * damage * 0.7 * (1 - gap / radius)) if gap < radius else lost_outside
        inside = "yes" if gap < radius else "no"
        lines.append(
            f"a2g-run: cluster target 0x5000000{i} distance {distance} radius 16 lost {lost} hp {200 - lost}/200 inside {inside}"
        )
    lines.append(f"a2g-run: cluster splash radius {radius} reached 3 hurt 3 killed 0")
    lines.append(
        f"a2g-run: cluster flashes {flashes} deterministic yes first-at-tick 2 last-at-tick 60 farthest {farthest} "
        f"spread 1200 splash {radius}"
    )
    lines += [f"a2g-run: cluster flash n={n} at 1.0 0.0 2.0 start {n} width 50" for n in range(flashes)]
    lines.append("a2g-run: done")
    return "\n".join(lines)


def three(**changes):
    runs = [
        cluster_run("CBU87.JT", 2200),
        cluster_run("MK20.JT", 1600),
        cluster_run("RBK500.JT", 2500, damage=240),
    ]
    if changes:
        runs[0] = cluster_run("CBU87.JT", 2200, **changes)
    return "\n".join(runs)


class ClusterTests(unittest.TestCase):
    def test_a_healthy_run_passes(self):
        self.assertEqual(a2g_cluster.cluster_problems(three()), [])

    def test_a_vehicle_outside_the_splash_that_loses_hit_points_fails(self):
        problems = a2g_cluster.cluster_problems(three(lost_outside=12))
        self.assertTrue(any("beyond the 2200 ft splash" in p for p in problems), problems)

    def test_damage_that_breaks_the_falloff_fails(self):
        problems = a2g_cluster.cluster_problems(three(lost_scale=0.5))
        self.assertTrue(any("the splash rule gives" in p for p in problems), problems)

    def test_the_wrong_number_of_flashes_fails(self):
        self.assertTrue(any("27 bomblet flashes" in p for p in a2g_cluster.cluster_problems(three(flashes=27))))

    def test_flashes_beyond_their_spread_fail(self):
        self.assertTrue(any("flashes reach" in p for p in a2g_cluster.cluster_problems(three(farthest=1300))))

    def test_missing_runs_fail(self):
        self.assertEqual(a2g_cluster.cluster_problems(cluster_run()), ["1 runs finished, want 3"])


def catalog_output(cheat=55, normal=38, empty_normal=0, records=54):
    rows = []
    for pt, placements, n in (("F18.PT", 117, normal), ("A4E.PT", 56, 22), ("C130.PT", 0, empty_normal)):
        rows.append(
            f"ordnance {pt}: 6 source rows, 0 optional weapon rows, {placements} compatible weapon placements, "
            f"0 tank placements; defaults and restart passed; catalog {n} normal, {cheat} Cheat"
        )
    rows.append(
        "ordnance audit: 36 aircraft, 12 recovered weapon stations, 3402 weapon placements, 175 tank placements, "
        f"{records} weapon and 9 tank records; passed"
    )
    return "\n".join(rows)


class ScatterReachTests(unittest.TestCase):
    def test_the_level_reach_at_one_and_a_half_degrees_is_half_of_retails(self):
        speed = 450 * a2g_sights.KNOT
        now = a2g_sights.scatter_reach_ft(5000.0, speed)
        retail = a2g_sights.scatter_reach_ft(5000.0, speed, 3.0)
        self.assertAlmostEqual(now, 595, delta=2)
        self.assertAlmostEqual(retail / now, 2.0, delta=0.05)


def _rocket_run(fall: float, y: float, rocket: str = "yes") -> str:
    return (
        f"a2g-run: sight t=0.017 mode CCIP rocket {rocket} arc 0 glyph 0x20 range 71903.4 max 12000 "
        f"in-range no slant 71082 fall {fall} impact 442126.3 {y} 1362792.3 target-gap 33136.3 "
        "retail-gap - warnings none\na2g-run: done\n"
    )


class RocketShallowTests(unittest.TestCase):
    def test_a_ground_point_past_the_life_passes(self):
        out = _rocket_run(25.97, -0.0) + _rocket_run(41.9, -0.0)
        self.assertEqual(a2g_sights.rocket_shallow_problems(out), [])

    def test_a_point_inside_the_life_or_in_the_air_fails(self):
        self.assertTrue(a2g_sights.rocket_shallow_problems(_rocket_run(15.0, 0.0) + _rocket_run(41.9, 0.0)))
        self.assertTrue(a2g_sights.rocket_shallow_problems(_rocket_run(30.0, 900.0) + _rocket_run(41.9, 0.0)))

    def test_no_impact_fails(self):
        none = "a2g-run: sight t=0.017 mode CCIP no-impact warnings none\na2g-run: done\n"
        self.assertTrue(a2g_sights.rocket_shallow_problems(none + _rocket_run(41.9, 0.0)))


class CatalogTests(unittest.TestCase):
    def problems(self, text):
        # The scenario checks 36 aircraft; the sample lists three.
        return [p for p in a2g_catalog.catalog_problems(text) if "aircraft listed a catalog" not in p]

    def test_a_healthy_audit_passes(self):
        self.assertEqual(self.problems(catalog_output()), [])

    def test_a_normal_catalog_that_lists_the_novelty_bombs_fails(self):
        problems = self.problems(catalog_output(normal=54))
        self.assertTrue(any("less the two novelty bombs" in p for p in problems), problems)

    def test_an_aircraft_with_no_stores_that_lists_some_fails(self):
        self.assertTrue(any("carries nothing" in p for p in self.problems(catalog_output(empty_normal=3))))

    def test_too_few_supported_records_fail(self):
        self.assertTrue(any("supported records" in p for p in self.problems(catalog_output(records=40))))

    def test_cheat_catalogs_of_different_sizes_fail(self):
        text = catalog_output().replace("catalog 22 normal, 55", "catalog 22 normal, 50")
        self.assertTrue(any("Cheat catalogs list" in p for p in self.problems(text)))


if __name__ == "__main__":
    unittest.main()


class ReplayStoresTests(unittest.TestCase):
    """`replay-a2g`'s stores check (slice BP3): a layout loaded, then the released station empty."""

    def rows(self, start: int, after: list[int]) -> list[dict]:
        return [
            {"kind": "stores.layout", "fields": {"count": 1}},
            {"kind": "stores.station", "fields": {"items": start}},
            {"kind": "stores.items", "fields": {"items": after}},
        ]

    def test_a_pack_that_empties_passes(self):
        from battery_scenarios import _replay_a2g

        self.assertEqual(_replay_a2g.stores_problems(self.rows(1, [0]), "pack"), [])

    def test_each_failure_is_named(self):
        from battery_scenarios import _replay_a2g

        self.assertIn("no stores layout", _replay_a2g.stores_problems([], "pack")[0])
        self.assertIn("starts loaded", _replay_a2g.stores_problems(self.rows(0, [0]), "pack")[0])
        self.assertIn("empty", _replay_a2g.stores_problems(self.rows(1, [1]), "pack")[0])
