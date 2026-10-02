"""Tests for the flight lane's output checks (tools/battery_scenarios/flight.py)."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from battery_scenarios import flight  # noqa: E402

EXTREMES = (
    "extremes: samples=7200 non_finite=0 max_speed_kt=450.0 max_g=7.64 min_g=1.05 min_altitude_ft=4326.6 "
    "max_altitude_ft=9207.4 max_pitch_rate_dps=17.4 max_roll_rate_dps=0.0 fuel_start_lb=11220.0 fuel_end_lb=11000.0 "
    "fuel_rise_lb=0.000 dead_stick_gain_ft=0.000 max_dead_stick_step_ft=0.0000 max_energy_rate_fps=65.0 "
    "energy_rate_over_thrust=0.192 speed_over_envelope_top=0.604 max_blackout=0.146 max_redout=0.000\n"
)
TAKEOFF = (
    "loadout: empty_lb=23050 internal_fuel_lb=11220 fuel_lb=11220 carried_lb=7568 gross_lb=41838 max_takeoff_lb=49224\n"
    "takeoff_complete=true airport_ground_ft=0\n"
    "envelope: altitude_ft=8.0 g1_kt=118.5..705.1 g2_kt=154.1..693.3 g3_kt=198.5..681.4 loading=0.8 landing_limit_kt=195.5 stall_scale=1.347 stall_kt=119.7 min_level_flaps_kt=148.4\n"
    "liftoff: tick=1305 speed_kt=147.0 distance_ft=1376 rotation_kt=103.1\n"
    "ticks=1617 speed_kt=162.436 altitude_ft=100.306 fuel_lb=11220.000 crashed=false\n" + EXTREMES
)
LANDING = (
    "landing_start: position=[1.0] runway_length_ft=5532 anchored=true\n"
    "landing: touchdown=true touchdown_tick=11742 touchdown_forward_kt=136.8 touchdown_side_fps=-7.7 "
    "touchdown_sink_fps=2.5 touchdown_pitch_deg=7.6 touchdown_bank_deg=0.4 landing_score=100 "
    "stopped=true stop_tick=13600 runway_left_ft=669 left_runway=false left_runway_kt=0.0 unsafe=none "
    "bounces=0 crashed=false max_cross_ft=32\n" + EXTREMES
)


class ExtremesTests(unittest.TestCase):
    def test_clean_line_passes(self):
        self.assertEqual(flight.extremes_problems(EXTREMES), [])

    def test_missing_line(self):
        self.assertTrue(flight.extremes_problems("ticks=5\n"))

    def test_each_impossible_state_is_named(self):
        for old, new, word in [
            ("non_finite=0", "non_finite=3", "non-finite"),
            ("fuel_rise_lb=0.000", "fuel_rise_lb=5.000", "fuel rose"),
            ("dead_stick_gain_ft=0.000", "dead_stick_gain_ft=90.000", "engine off"),
            ("max_speed_kt=450.0", "max_speed_kt=1500.0", "speed"),
            ("max_g=7.64", "max_g=30.00", "load"),
            ("energy_rate_over_thrust=0.192", "energy_rate_over_thrust=2.5", "thrust"),
            ("speed_over_envelope_top=0.604", "speed_over_envelope_top=1.5", "top speed"),
            ("max_blackout=0.146", "max_blackout=1.500", "outside 0..1"),
        ]:
            problems = flight.extremes_problems(EXTREMES.replace(old, new))
            self.assertTrue(any(word in p for p in problems), (old, problems))

    def test_veil_without_g_is_flagged(self):
        text = EXTREMES.replace("max_g=7.64", "max_g=2.00")
        self.assertTrue(any("greyed" in p for p in flight.extremes_problems(text)))

    def test_terrain_figures(self):
        text = EXTREMES.strip() + " min_agl_ft=-80.0 under_ground_ticks=12\n"
        self.assertTrue(any("below the surface" in p for p in flight.extremes_problems(text)))


class TakeoffAndLandingTests(unittest.TestCase):
    def test_clean_takeoff(self):
        self.assertEqual(flight.check_takeoff(TAKEOFF), [])

    def test_liftoff_band(self):
        env = "envelope: altitude_ft=8.0 g1_kt=118.5..705.1 g2_kt=154.1..693.3 g3_kt=198.5..681.4 loading=0.8 landing_limit_kt=195.5 stall_scale=1.347 stall_kt=119.7 min_level_flaps_kt=148.4\n"
        ok = env + "liftoff: tick=1305 speed_kt=147.0 distance_ft=1376 rotation_kt=103.1\n"
        self.assertEqual(flight.liftoff_problems(ok), [])
        self.assertTrue(flight.liftoff_problems(ok.replace("speed_kt=147.0", "speed_kt=60.0")))
        self.assertTrue(flight.liftoff_problems(ok.replace("speed_kt=147.0", "speed_kt=200.0")))
        self.assertTrue(flight.liftoff_problems(ok.replace("speed_kt=147.0", "speed_kt=125.0")))
        self.assertTrue(flight.liftoff_problems("liftoff: none\n"))

    def test_speed_targets_and_recorded_deviations(self):
        self.assertIsNone(flight._in_target(140, (135, 150), None))
        self.assertIsNone(flight._in_target(97, (125, 140), 97.0))
        self.assertTrue(flight._in_target(97, (125, 140), None))
        self.assertTrue(flight._in_target(110, (125, 140), 97.0))
        self.assertEqual(set(flight.APPROACH_TARGET) | {"x31"}, set(flight.AIRCRAFT))

    def test_overweight_and_stuck(self):
        text = TAKEOFF.replace("gross_lb=41838", "gross_lb=50000")
        self.assertTrue(any("maximum takeoff weight" in p for p in flight.check_takeoff(text)))
        self.assertTrue(any("never reached" in p for p in flight.check_takeoff(TAKEOFF.replace("takeoff_complete=true", "x"))))

    def test_clean_landing(self):
        self.assertEqual(flight.check_landing(LANDING), [])

    def test_landing_problems_on_a_long_runway(self):
        for old, new, word in [
            ("crashed=false", "crashed=true", "crashed"),
            ("stopped=true", "stopped=false", "stop"),
            ("runway_left_ft=669", "runway_left_ft=-50", "off the end"),
            ("touchdown_sink_fps=2.5", "touchdown_sink_fps=20.0", "hard"),
            ("left_runway=false", "left_runway=true", "runway surface"),
        ]:
            problems = flight.check_landing(LANDING.replace(old, new))
            self.assertTrue(any(word in p for p in problems), (old, problems))

    def test_a_short_strip_may_overrun(self):
        text = LANDING.replace("runway_length_ft=5532", "runway_length_ft=1074").replace("runway_left_ft=669", "runway_left_ft=-900")
        self.assertEqual(flight.check_landing(text), [])

    def test_unsafe_variants(self):
        gear = LANDING.replace("crashed=false", "crashed=true").replace("unsafe=none", "unsafe=unsafe_gear_up=true")
        self.assertEqual(flight.check_landing_gear_up(gear), [])
        self.assertTrue(flight.check_landing_gear_up(LANDING))
        off = LANDING.replace("crashed=false", "crashed=true").replace("unsafe=none", "unsafe=not_landable=true")
        self.assertEqual(flight.check_landing_off_runway(off), [])
        self.assertTrue(flight.check_landing_off_runway(LANDING))


class WeaponTests(unittest.TestCase):
    def test_slot_accounting(self):
        check = flight.check_slot("f18", 2)
        ok = "Combat probe: F/A-18D shots=1 hits=1 kills=1 active=0 ammo=[570, 1, 4, 4, 2]"
        self.assertEqual(check(ok), [])
        self.assertTrue(check(ok.replace("ammo=[570, 1,", "ammo=[570, 0,")))
        self.assertTrue(check(ok.replace("[570, 1, 4", "[570, 1, 3")))
        self.assertTrue(check(ok.replace("ammo=[570, 1,", "ammo=[570, 3,")))
        self.assertTrue(check(ok.replace("shots=1", "shots=0")))

    def test_surface_weapons_must_be_refused(self):
        check = flight.check_slot("f18", 3)
        self.assertEqual(check("Combat probe: F/A-18D shots=0 hits=0 kills=0 active=0 ammo=[570, 2, 4, 4, 2]"), [])
        self.assertTrue(check("Combat probe: F/A-18D shots=1 hits=0 kills=0 active=0 ammo=[570, 2, 3, 4, 2]"))

    def test_countermeasure_counts(self):
        line = (
            "Countermeasure preview: ticks=240 flares=6 burning=6 puffs=210 chaff=5 "
            "carried_chaff=25 carried_flares=27 capacity_chaff=30 capacity_flares=30"
        )
        self.assertEqual(flight.check_countermeasures(line), [])
        self.assertTrue(flight.check_countermeasures(line.replace("carried_chaff=25", "carried_chaff=30")))

    def test_combat_smoke(self):
        self.assertTrue(flight.check_combat_smoke("combat smoke X FAIL"))
        self.assertEqual(flight.check_combat_smoke("PASS\n" * 9), [])
        self.assertTrue(flight.check_wreck_contacts("PASS\n" * 9))
        self.assertEqual(flight.check_wreck_contacts("PASS\n" * 9 + "wreck collision: weapons=2 hits=2 second_kills=0 PASS"), [])


class SpinTests(unittest.TestCase):
    RECOVERED = (
        "spin_recovery: entered_tick=3 recovery_started_tick=483 recovered_tick=714 revolutions_during_recovery=0.52 "
        "revolutions_total=1.99 altitude_lost_ft=785 crashed=false\n" + EXTREMES
    )

    def test_recovery(self):
        self.assertEqual(flight.check_spin_recovery("f18")(self.RECOVERED), [])
        self.assertTrue(flight.check_spin_recovery("f18")(self.RECOVERED.replace("recovered_tick=714", "recovered_tick=never")))
        self.assertTrue(flight.check_spin_recovery("f18")(self.RECOVERED.replace("entered_tick=3", "entered_tick=never")))

    def test_spin_immune_aircraft(self):
        text = self.RECOVERED.replace("entered_tick=3", "entered_tick=never")
        self.assertEqual(flight.check_spin_recovery("x31")(text), [])
        self.assertTrue(flight.check_spin_recovery("x31")(self.RECOVERED))


class LossTests(unittest.TestCase):
    LOST = "ticks=100 speed_kt=0.000 altitude_ft=0.000 fuel_lb=1.0 crashed=true\n"

    def test_edge_short_flight_has_no_loss(self):
        ok = "ticks=24000 speed_kt=400.0 altitude_ft=20000.0 fuel_lb=9000.0 crashed=false\nfinal_position: x=1 z=2 heading_deg=3\n" + EXTREMES
        self.assertEqual(flight.check_edge(ok + "loss: cause=none\n"), [])
        self.assertTrue(flight.check_edge(ok + "loss: cause=out of bounds\n"))

    def test_edge_distance_is_from_the_nearest_point_of_the_rectangle(self):
        width = 207 * 8192
        self.assertEqual(flight._edge_nm("UKR", 1000, 1000), 0)
        self.assertAlmostEqual(flight._edge_nm("UKR", -6076.11549 * 105, 1000), 105)
        self.assertAlmostEqual(flight._edge_nm("UKR", width + 6076.11549 * 3, 1000), 3)
        # Past a corner it is the straight distance to the corner.
        self.assertAlmostEqual(flight._edge_nm("UKR", -6076.11549 * 3, -6076.11549 * 4), 5)

    def test_edge_lost_needs_the_cause_and_about_105_nm(self):
        def out(nm, cause="out of bounds", crashed="true"):
            x = -6076.11549 * nm
            return (
                f"ticks=100 speed_kt=0.0 altitude_ft=0.0 fuel_lb=1.0 crashed={crashed}\n"
                f"final_position: x={x} z=100000.0 heading_deg=3\n" + EXTREMES + f"loss: cause={cause}\n"
            )

        self.assertEqual(flight.check_edge_lost(out(106), "UKR"), [])
        self.assertTrue(flight.check_edge_lost(out(106, cause="none"), "UKR"))
        self.assertTrue(flight.check_edge_lost(out(80), "UKR"))
        self.assertTrue(flight.check_edge_lost(out(106, crashed="false"), "UKR"))

    def test_overspeed_loss_and_dive(self):
        self.assertEqual(flight.check_overspeed_loss(self.LOST + "loss: cause=overspeed\noverspeed_ticks=840\n"), [])
        self.assertTrue(flight.check_overspeed_loss(self.LOST + "loss: cause=none\n"))
        self.assertEqual(flight.check_dive(self.LOST + EXTREMES + "loss: cause=none\n"), [])
        fast = EXTREMES.replace("speed_over_envelope_top=0.604", "speed_over_envelope_top=1.7")
        self.assertEqual(flight.check_dive(self.LOST + fast + "loss: cause=overspeed\n"), [])
        for tick in (1, 600, 719, 721, 1201):
            self.assertTrue(flight.check_overspeed_loss(self.LOST + f"loss: cause=overspeed\noverspeed_ticks={tick}\n"))

    def test_invulnerable_overspeed_is_not_a_loss(self):
        kept = "ticks=600 speed_kt=900.0 altitude_ft=20000.0 fuel_lb=1.0 crashed=false\nloss: cause=none\n"
        self.assertEqual(flight.check_overspeed_invulnerable(kept), [])
        self.assertTrue(flight.check_overspeed_invulnerable(self.LOST + "loss: cause=overspeed\n"))

    def test_combat_g_stays_near_the_imported_value(self):
        line = "extremes: samples=600 non_finite=0 max_speed_kt=450.0 max_g={} min_g=1.05 min_altitude_ft=4326.6 max_altitude_ft=9207.4 max_pitch_rate_dps=17.4 max_roll_rate_dps=0.0 fuel_start_lb=11220.0 fuel_end_lb=11000.0 fuel_rise_lb=0.000 dead_stick_gain_ft=0.000 max_dead_stick_step_ft=0.0000 max_energy_rate_fps=65.0 energy_rate_over_thrust=0.192 speed_over_envelope_top=0.604 max_blackout=0.146 max_redout=0.000\n"
        self.assertEqual(flight.check_combat_g(line.format("7.64"), "f18"), [])
        self.assertTrue(flight.check_combat_g(line.format("5.10"), "f18"))
        self.assertEqual(set(flight.PULL_G_RETAIL), set(flight.AIRCRAFT))

    def test_ground_sensor_checks(self):
        roll = "gear_pulled=true ground_sensor_refusals={} gear={}\nticks=9000 speed_kt=300.0 altitude_ft=8000.0 fuel_lb=1.0 crashed={}\n"
        self.assertEqual(flight.check_belly_early(roll.format(1, "1.00", "false") + EXTREMES, "f18"), [])
        self.assertTrue(flight.check_belly_early(roll.format(0, "1.00", "false") + EXTREMES, "f18"))
        self.assertTrue(flight.check_belly_early(roll.format(1, "0.00", "false") + EXTREMES, "f22"))
        self.assertTrue(flight.check_belly_early(roll.format(1, "1.00", "true") + EXTREMES, "f22"))
        air = "takeoff_complete=true airport_ground_ft=0\ngear_pulled=true ground_sensor_refusals={} gear={}\n"
        self.assertEqual(flight.check_belly_airborne(air.format(0, "0.55") + EXTREMES), [])
        self.assertTrue(flight.check_belly_airborne(air.format(1, "0.55") + EXTREMES))
        self.assertTrue(flight.check_belly_airborne(air.format(0, "1.00") + EXTREMES))

    def test_the_debrief_cause_suffix_still_parses(self):
        line = "AI probe debrief: Lost Ok elapsed=10s player[Dead damage=100% kills=[] ff=0 a2a=0/0 dmg=0 gun=0/0 enemy_aam=0/0 enemy_gun=0/0 cause=overspeed] wingman[-]"
        self.assertTrue(flight.PLAYER_LINE.search(line))


class ScenarioListTests(unittest.TestCase):
    def test_names_are_unique_and_prefixed(self):
        names = [s.name for s in flight.scenarios()]
        self.assertEqual(len(names), len(set(names)))
        self.assertTrue(all(n.startswith("flight-") for n in names))

    def test_every_aircraft_has_every_family(self):
        names = {s.name for s in flight.scenarios()}
        for ac in flight.AIRCRAFT:
            for family in ("takeoff", "spinrecover", "stallrecover", "climb", "devices", "combatsmoke", "autopilot"):
                self.assertTrue(any(n.startswith(f"flight-{family}-{ac}") for n in names), (family, ac))


if __name__ == "__main__":
    unittest.main()
