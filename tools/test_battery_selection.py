import fnmatch
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import battery  # noqa: E402
import battery_selection as bs  # noqa: E402


def sc(name, window=False, lane=None):
    return battery.Scenario(name=name, lane=lane or name.split("-")[0], args=[], window=window)


def plan(changed, scenarios, durations=None, budget=120, jobs=4, **kw):
    return bs.plan_for(changed, scenarios, durations or {}, budget, jobs, **kw)


class MapTests(unittest.TestCase):
    """The reviewed table of source paths and scenario families."""

    def families(self, path):
        rule = bs.rule_for(path)
        self.assertIsNotNone(rule, path)
        return set(rule.families)

    def test_flight_model_selects_the_flight_families(self):
        fams = self.families("crates/tore-sim/src/flight.rs")
        self.assertTrue({"flight-stall", "flight-takeoff", "flight-landing", "flight-maneuvers"} <= fams)

    def test_stall_table_reaches_stall_and_takeoff(self):
        fams = self.families("crates/tore-sim/src/models/mod.rs")
        self.assertTrue({"flight-stall", "flight-takeoff"} <= fams)

    def test_ai_code_selects_the_ai_families(self):
        fams = self.families("crates/tore-sim/src/ai/mission.rs")
        self.assertTrue({"ai-fights", "ai-airfield", "ai-lead", "ai-regression", "ai-orders"} <= fams)
        self.assertIn("ai-airfield", self.families("crates/tore-world/src/ai_wings/orders.rs"))

    def test_airports_select_creator_and_ils_scenarios(self):
        fams = self.families("crates/tore-sim/src/airport.rs")
        self.assertTrue({"airports", "menus-creator", "ai-airfield"} <= fams)
        self.assertIn("airports", self.families("crates/tore-app/src/ils_survey.rs"))

    def test_radio_selects_the_radio_probes(self):
        self.assertIn("radio", self.families("crates/tore-world/src/radio_calls.rs"))
        self.assertIn("radio", self.families("crates/tore-world/src/airfield_radio.rs"))

    def test_menus_and_hud_select_their_snapshots(self):
        self.assertIn("menus-screens", self.families("crates/tore-app/src/menu.rs"))
        self.assertIn("instruments", self.families("crates/tore-app/src/hud.rs"))

    def test_battery_files_select_their_unit_tests(self):
        rule = bs.rule_for("tools/battery_scenarios/ai.py")
        self.assertIn("test_battery_ai", rule.unit_tests)
        self.assertIn("test_battery", bs.rule_for("tools/battery.py").unit_tests)
        self.assertIn("test_replay_checks", bs.rule_for("tools/battery_scenarios/_replay_live.py").unit_tests)

    def test_documentation_and_tests_select_nothing(self):
        for path in (
            "docs/testing/README.md", "README.md", "crates/tore-world/src/world/tick_tests.rs",
            "crates/tore-sim/src/golden_tests/flight.rs", "crates/tore-replay/tests/format.rs", "tools/test_battery.py",
        ):
            self.assertEqual(self.families(path), set(), path)

    def test_first_matching_rule_wins(self):
        # A test file inside a mapped folder stays test-only; its neighbour is mapped.
        self.assertEqual(bs.rule_for("crates/tore-world/src/world/handoff_tests.rs").families, ())
        self.assertNotEqual(bs.rule_for("crates/tore-world/src/world/handoff.rs").families, ())

    def test_windows_only_for_rendering_and_windowed_input(self):
        self.assertTrue(bs.rule_for("crates/tore-app/src/cockpit_renderer.rs").windowed)
        self.assertTrue(bs.rule_for("crates/tore-app/src/terrain.wgsl").windowed)
        self.assertTrue(bs.rule_for("crates/tore-input/src/bindings.rs").windowed)
        self.assertFalse(bs.rule_for("crates/tore-sim/src/flight.rs").windowed)
        self.assertFalse(bs.rule_for("crates/tore-world/src/ai_wings.rs").windowed)

    def test_network_crates_select_the_net_lane(self):
        for path in ("crates/tore-net/src/connection.rs", "crates/tore-session/src/host/mod.rs", "crates/tore-codec/src/lib.rs"):
            self.assertIn("net-fly", self.families(path), path)
        self.assertEqual(
            set(self.families("crates/tore-server/src/run.rs")), {"net-check", "net-fly", "net-discovery", "net-listing"},
        )
        # The host's side of the master (slice I3) runs the master's scenarios, the listing among them.
        self.assertEqual(self.families("crates/tore-net/src/master/rendezvous.rs"), {"net-master"})
        self.assertIn("net-window", self.families("crates/tore-session/src/client/mod.rs"))
        self.assertIn("net-discovery", self.families("crates/tore-net/src/reach.rs"))
        for path in (
            "crates/tore-app/src/net/hosting.rs", "crates/tore-app/src/net/session.rs", "crates/tore-app/src/net/play.rs",
            "crates/tore-app/src/direct_screen/mod.rs", "crates/tore-app/src/lobby_screen/mod.rs", "crates/tore-app/src/widgets/button.rs",
        ):
            self.assertIn("net-window", self.families(path), path)
            self.assertTrue(bs.rule_for(path).windowed, path)
        self.assertIn("net-discovery", self.families("crates/tore-app/src/net/search.rs"))
        self.assertEqual(self.families("crates/tore-app/src/net/hosting_tests.rs"), set())

    def test_net_scenario_files_select_their_unit_tests_and_cheap_scenarios(self):
        rule = bs.rule_for("tools/battery_scenarios/net.py")
        self.assertIn("test_battery_net", rule.unit_tests)
        self.assertEqual(set(rule.families), {"net-check", "net-discovery"})
        self.assertIn("test_battery_net", bs.rule_for("tools/battery.py").unit_tests)

    def test_cargo_files_select_everything(self):
        self.assertEqual(set(bs.rule_for("Cargo.lock").families), set(bs.ALL_FAMILIES))
        self.assertEqual(set(bs.rule_for("crates/tore-sim/Cargo.toml").families), set(bs.ALL_FAMILIES))

    def test_every_rule_and_family_name_is_known(self):
        for rule in bs.RULES:
            for fam in rule.families:
                self.assertIn(fam, bs.FAMILIES, f"{rule.pattern} names an unknown family")

    def test_the_map_covers_the_real_repository(self):
        scenarios = battery.load_scenarios()
        for fam in bs.FAMILIES:
            self.assertTrue(any(bs.family_matches(fam, s.name) for s in scenarios), f"family {fam} matches no scenario")
        orphans = [s.name for s in scenarios if not any(bs.family_matches(f, s.name) for f in bs.FAMILIES)]
        self.assertEqual(orphans[:10], [], "scenarios in no family: add them to a family in tools/battery_selection.py")
        try:
            files = bs.tracked_files(battery.ROOT)
        except (RuntimeError, OSError):
            self.skipTest("no git checkout")
        missing = [f for f in files if f.startswith(("crates/", "tools/")) and bs.rule_for(f) is None]
        self.assertEqual(missing[:10], [], "files with no rule: add them to RULES in tools/battery_selection.py")

    def test_family_exclusions(self):
        self.assertTrue(bs.family_matches("combat-tapes", "replay-tape-f18"))
        self.assertFalse(bs.family_matches("combat-tapes", "replay-tape-bad-empty"))
        self.assertTrue(bs.family_matches("replay-cli", "replay-tape-bad-empty"))


class PlanTests(unittest.TestCase):
    def setUp(self):
        self.scenarios = [
            sc("flight-takeoff-f18-ukr-1"), sc("flight-takeoff-rafale-apa-1"), sc("flight-takeoff-f14-bal-1"),
            sc("flight-takeoff-mig21-egy-1"), sc("flight-stall-f18-default"), sc("flight-stall-rafale-default"),
            sc("flight-level-f18-default"), sc("flight-land-f18-ukr-1"),
            sc("ai-fight-1v1-default"), sc("ai-fight-15v15-default"), sc("ai-long-1v1"),
            sc("menus-window-launch-none-f18", window=True), sc("menus-snap-normal"),
            sc("flight-damage-f18-0.5", window=True),
        ]
        self.durations = {s.name: 1.0 for s in self.scenarios}
        self.durations.update({"ai-fight-1v1-default": 30.0, "ai-fight-15v15-default": 300.0, "ai-long-1v1": 100.0})

    def names(self, p):
        return {s.name for s in p.scenarios}

    def test_every_selected_family_keeps_a_scenario(self):
        p = plan(["crates/tore-sim/src/flight.rs"], self.scenarios, self.durations, budget=1)
        for fam in ("flight-stall", "flight-takeoff", "flight-landing", "flight-maneuvers", "ai-fights"):
            self.assertTrue(any(fam in pick.families for pick in p.picks), fam)

    def test_a_tiny_budget_is_exceeded_only_by_the_mandatory_picks(self):
        p = plan(["crates/tore-sim/src/flight.rs"], self.scenarios, self.durations, budget=1)
        self.assertEqual(len(p.picks), 5)  # one per family, nothing more
        self.assertTrue(p.over_budget)  # the AI fight costs 30 s

    def test_the_budget_limits_the_extras(self):
        small = plan(["crates/tore-sim/src/flight.rs"], self.scenarios, self.durations, budget=40, jobs=1)
        big = plan(["crates/tore-sim/src/flight.rs"], self.scenarios, self.durations, budget=1000, jobs=4)
        self.assertLessEqual(len(small.picks), len(big.picks))
        self.assertLessEqual(small.estimate, 40 + 1e-9)
        self.assertNotIn("ai-fight-15v15-default", self.names(small))

    def test_a_long_scenario_never_joins_as_an_extra(self):
        p = plan(["crates/tore-sim/src/ai/mission.rs"], self.scenarios, self.durations, budget=300, jobs=4)
        self.assertNotIn("ai-fight-15v15-default", self.names(p))  # 300 s is over a third of the budget
        self.assertIn("ai-fight-1v1-default", self.names(p))

    def test_preferred_then_second_aircraft_or_theater(self):
        p = plan(["crates/tore-sim/src/flight.rs"], self.scenarios, self.durations, budget=1000)
        chosen = self.names(p)
        self.assertIn("flight-takeoff-f18-ukr-1", chosen)
        self.assertIn("flight-takeoff-rafale-apa-1", chosen)
        self.assertNotIn("flight-takeoff-f14-bal-1", chosen)  # one or two aircraft, not all

    def test_headless_unless_the_change_touches_rendering(self):
        sim = plan(["crates/tore-sim/src/flight.rs"], self.scenarios, self.durations)
        self.assertFalse(any(s.window for s in sim.scenarios))
        render = plan(["crates/tore-app/src/cockpit_renderer.rs"], self.scenarios, self.durations)
        self.assertTrue(render.windows_used)
        self.assertIn("menus-window-launch-none-f18", self.names(render))
        off = plan(["crates/tore-app/src/cockpit_renderer.rs"], self.scenarios, self.durations, windowed="no")
        self.assertFalse(any(s.window for s in off.scenarios))
        forced = plan(["crates/tore-sim/src/flight.rs"], self.scenarios, self.durations, windowed="yes")
        self.assertTrue(forced.windows_used)

    def test_windowed_only_families_are_reported(self):
        p = plan(["crates/tore-sim/src/ejection.rs"], self.scenarios, self.durations)
        self.assertEqual(p.needs_window, ["flight-damage"])
        self.assertEqual(p.picks, [])

    def test_documentation_only_selects_nothing(self):
        p = plan(["docs/ROADMAP.md", "crates/tore-world/src/world/tick_tests.rs"], self.scenarios, self.durations)
        self.assertEqual(p.picks, [])
        self.assertIn("no scenario can be affected", bs.format_plan(p))

    def test_unmapped_file_selects_everything_and_says_so(self):
        p = plan(["somewhere/new.rs"], self.scenarios, self.durations, budget=1000)
        self.assertEqual(p.unmapped, ["somewhere/new.rs"])
        self.assertIn("NOT IN THE MAP", bs.format_plan(p))
        self.assertGreater(len(p.picks), 5)

    def test_battery_files_carry_their_unit_tests(self):
        p = plan(["tools/battery_scenarios/ai.py"], self.scenarios, self.durations)
        self.assertEqual(p.unit_tests, ["test_battery_ai"])

    def test_nothing_changed(self):
        self.assertIn("nothing to select", bs.format_plan(plan([], self.scenarios)))

    def test_per_family_cap(self):
        many = [sc(f"flight-cheat-{chr(97 + i)}-f18") for i in range(20)]  # twenty different kinds
        p = plan(["crates/tore-sim/src/ejection.rs"], many, {}, budget=1000, per_family=5)
        self.assertEqual(len(p.picks), 5)


class NetPlanTests(unittest.TestCase):
    def setUp(self):
        self.scenarios = [
            sc("net-server-check", lane="net"), sc("net-server-fight", lane="net"), sc("net-server-chat", lane="net"),
            sc("net-server-kick", lane="net"), sc("net-discovery", lane="net"),
            sc("net-window-stall", window=True, lane="net"), sc("net-window-host", window=True, lane="net"),
        ]
        self.durations = {
            "net-server-check": 1.0, "net-server-fight": 80.0, "net-server-chat": 25.0, "net-server-kick": 8.0,
            "net-discovery": 6.0, "net-window-stall": 18.0, "net-window-host": 25.0,
        }

    def test_a_session_change_runs_the_cheapest_flying_scenario_and_asks_for_a_window(self):
        p = plan(["crates/tore-session/src/host/mod.rs"], self.scenarios, self.durations)
        names = {s.name for s in p.scenarios}
        self.assertIn("net-server-kick", names)  # the cheapest of the flying family
        self.assertNotIn("net-server-fight", names)  # 80 s is over a third of the budget
        self.assertFalse(any(s.window for s in p.scenarios))
        self.assertEqual(p.needs_window, ["net-window"])

    def test_a_game_side_change_opens_the_window_scenarios(self):
        p = plan(["crates/tore-app/src/net/hosting.rs"], self.scenarios, self.durations)
        self.assertTrue(p.windows_used)
        self.assertTrue(any(s.name.startswith("net-window-") for s in p.scenarios))
        self.assertFalse(any(s.name.startswith("net-server-") for s in p.scenarios))

    def test_a_server_change_alone_runs_headless(self):
        p = plan(["crates/tore-server/src/config.rs"], self.scenarios, self.durations)
        names = {s.name for s in p.scenarios}
        self.assertTrue({"net-server-check", "net-discovery"} <= names)
        self.assertFalse(any(s.window for s in p.scenarios))


class VariantTests(unittest.TestCase):
    def test_shapes(self):
        self.assertEqual(bs.variant_key("flight-takeoff-rafale-apa-10")[:3], ("flight-takeoff-<ac>-<th>-<n>", "rafale", "apa"))
        self.assertEqual(bs.variant_key("flight-land-f18-bal3v-1")[:3], ("flight-land-<ac>-<th>-<n>", "f18", "bal"))
        self.assertEqual(bs.variant_key("ai-fight-15v15-default")[0], "ai-fight-<n>v<n>-default")
        self.assertEqual(bs.variant_key("flight-fault07-f18")[0], "flight-fault<n>-<ac>")

    def test_ranking_prefers_f18_ukraine_then_something_new(self):
        group = [
            (sc("flight-takeoff-f18-ukr-2"), "f18", "ukr", 2, 1.0),
            (sc("flight-takeoff-f18-ukr-1"), "f18", "ukr", 1, 1.0),
            (sc("flight-takeoff-rafale-apa-1"), "rafale", "apa", 1, 1.0),
            (sc("flight-takeoff-f14-bal-1"), "f14", "bal", 1, 1.0),
        ]
        ranked = bs._rank_variants(group, 1)
        self.assertEqual([(s.name, t) for s, t in ranked], [("flight-takeoff-f18-ukr-1", 0), ("flight-takeoff-rafale-apa-1", 1)])


class BudgetMathTests(unittest.TestCase):
    def test_makespan(self):
        self.assertEqual(bs.makespan([], 4), 0.0)
        self.assertEqual(bs.makespan([5, 5, 5, 5], 4), 5)
        self.assertEqual(bs.makespan([5, 5, 5, 5], 2), 10)
        self.assertEqual(bs.makespan([9, 3, 3, 3], 2), 9)  # the long one alone on a slot
        self.assertEqual(bs.makespan([1.0] * 10, 1), 10)

    def test_windowed_runs_also_fit_the_window_limit(self):
        picks = [bs.Pick(sc("menus-window-a", window=True), 10.0, [], 0) for _ in range(6)]
        self.assertEqual(bs.estimate_wall(picks, jobs=6, windows=3), 20.0)
        self.assertEqual(bs.estimate_wall(picks, jobs=6, windows=6), 10.0)


class DurationTests(unittest.TestCase):
    def write(self, root, stamp, rows):
        d = root / stamp
        d.mkdir()
        (d / "results.json").write_text(json.dumps([{"name": n, "seconds": s} for n, s in rows.items()]))

    def test_newest_full_run_wins_and_gaps_come_from_newer_partials(self):
        names = [f"s{i}" for i in range(10)]
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.write(root, "20260101-000000-old-full", {n: 100.0 for n in names})
            self.write(root, "20260102-000000-full", {n: 2.0 for n in names if n != "s9"})
            self.write(root, "20260103-000000-partial", {"s1": 50.0, "s9": 7.0})
            got = bs.load_durations(root, names)
        self.assertEqual(got["s1"], 2.0)  # the full run, not the newer partial
        self.assertEqual(got["s9"], 7.0)  # the gap, from the newer partial
        self.assertEqual(got["s0"], 2.0)

    def test_missing_folder_and_bad_files_are_fine(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "20260101-000000").mkdir()
            (root / "20260101-000000" / "results.json").write_text("not json")
            self.assertEqual(bs.load_durations(root, ["a"]), {})
        self.assertEqual(bs.load_durations(Path("/nonexistent-battery-dir"), ["a"]), {})


if __name__ == "__main__":
    unittest.main()
