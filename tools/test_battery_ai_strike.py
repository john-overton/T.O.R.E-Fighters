"""Tests for the AI strike driver's scenarios (tools/battery_scenarios/ai_strike.py): the output
parsing, the consistency and behaviour checks, the pending markers and the battery selection
family (AI strike round, slice ST-D)."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import battery_selection as bs  # noqa: E402
from battery_scenarios import ai_strike as s  # noqa: E402

SETUP = (
    "ai-strike: setup aircraft=A4E load=MK82.JT station=1 count=4 rounds=4 target=0x50000001 "
    "target_at=999570,7,789571 hp=200 skill={skill} profile={profile} handback=none order=none mode=none"
)


def run(skill=2, profile="dive", named=None, releases=(), impacts=(), kills=(), egress=None, lost=None,
        hp="200/200", extra=()):
    """One driver run's output. `releases` and `impacts` are times; an impact is (t, miss)."""
    name = named if named is not None else "none"
    out = [
        SETUP.format(skill=skill, profile=profile),
        f"ai-strike: PROFILE t=0.000 name={name} requested={profile} note=no-strike-behaviour",
        "ai-strike: start t=0.000 plane=0 range_ft=24304 alt_ft=5000 speed_kt=450",
        "ai-strike: ACTIVITY t=0.008 name=In formation",
        *extra,
    ]
    for n, t in enumerate(releases, 1):
        out.append(
            f"ai-strike: RELEASE t={t:.3f} n={n} weapon=MK82.JT station=1 alt_ft=4900 speed_kt=450 range_ft=13000"
        )
    for n, (t, miss) in enumerate(impacts, 1):
        out.append(f"ai-strike: IMPACT t={t:.3f} n={n} miss_ft={miss} at=1.0,2.0,3.0")
    for t in kills:
        out.append(f"ai-strike: KILL t={t:.3f} victim=0x50000001 category=0x0001 target=yes")
    if egress is not None:
        out.append(f"ai-strike: EGRESS t={egress:.3f} range_ft=6000 alt_ft=5000")
    if lost is not None:
        out.append(f"ai-strike: LOST t={lost:.3f} reason=destroyed")
    out.append(
        f"ai-strike: summary t=60.000 ran_s=60.0 profile={name} releases={len(releases)} impacts={len(impacts)} "
        f"kills={len(kills)} egress={'yes' if egress is not None else 'no'} lost={'yes' if lost is not None else 'no'} "
        f"activity=In formation target_hp={hp} closest_ft=200 store_rounds_left=3"
    )
    if not releases:
        out.append("ai-strike: no release")
    out.append("ai-strike: done")
    return "\n".join(out)


class ParsingTests(unittest.TestCase):
    def test_runs_split_at_done_and_fields_parse(self):
        runs = s.parse_runs(run(skill=1) + "\n" + run(skill=3, releases=[10.0], impacts=[(27.5, 88.5)]))
        self.assertEqual(len(runs), 2)
        self.assertEqual([r.skill for r in runs], [1, 3])
        self.assertEqual(runs[0].setup["load"], "MK82.JT")
        self.assertEqual(runs[1].of("IMPACT")[0]["miss_ft"], "88.5")
        self.assertEqual(runs[1].times("RELEASE"), [10.0])
        self.assertEqual(runs[1].summary["releases"], "1")

    def test_no_release_line_is_its_own_kind(self):
        (parsed,) = s.parse_runs(run())
        self.assertEqual([k for k, _ in parsed.lines].count("no-release"), 1)

    def test_a_quoted_value_keeps_its_spaces(self):
        fields = s.parse_line("ORDER-REPLY", 't=3.025 result=refused message="Wing order unavailable: no valid hostile target"')
        self.assertEqual(fields["result"], "refused")
        self.assertEqual(fields["message"], "Wing order unavailable: no valid hostile target")

    def test_other_output_is_ignored(self):
        text = "a2g-run: fitted station 1\nnoise\n" + run()
        self.assertEqual(len(s.parse_runs(text)), 1)
        self.assertEqual(s.parse_runs("nothing here"), [])

    def test_number_reads_nan_for_junk(self):
        self.assertEqual(s.number("12.5"), 12.5)
        self.assertNotEqual(s.number("nan"), s.number("nan"))
        self.assertNotEqual(s.number(None), s.number(None))


class ConsistencyTests(unittest.TestCase):
    def check(self, text):
        (parsed,) = s.parse_runs(text)
        return s.consistency_problems(parsed, "run")

    def test_a_run_that_released_nothing_is_consistent(self):
        self.assertEqual(self.check(run()), [])

    def test_a_full_strike_is_consistent(self):
        text = run(named="dive", releases=[10.0, 10.5], impacts=[(27.0, 40), (27.5, 60)], kills=[27.5], egress=33.0)
        self.assertEqual(self.check(text), [])

    def test_the_summary_must_count_the_lines(self):
        text = run(releases=[10.0], impacts=[(27.0, 40)]).replace("releases=1", "releases=2")
        self.assertTrue(any("releases=2" in p for p in self.check(text)))

    def test_an_impact_needs_a_release_before_it(self):
        text = run(releases=[30.0], impacts=[(27.0, 40)])
        self.assertTrue(any("impact before the first release" in p for p in self.check(text)))

    def test_egress_needs_a_release_before_it(self):
        text = run(egress=12.0)
        self.assertTrue(any("egress before" in p for p in self.check(text)))

    def test_losing_the_striker_is_reported_in_the_summary(self):
        self.assertEqual(self.check(run(lost=22.0)), [])
        text = run(lost=22.0).replace("lost=yes", "lost=no")
        self.assertTrue(any("lost=no" in p for p in self.check(text)))

    def test_silence_about_a_missing_release_is_a_problem(self):
        text = run().replace("ai-strike: no release\n", "")
        self.assertTrue(any("does not say so" in p for p in self.check(text)))

    def test_events_out_of_time_order_are_a_problem(self):
        text = run(releases=[20.0], impacts=[(21.0, 5)], egress=10.5)
        self.assertTrue(any("out of order" in p or "egress before" in p for p in self.check(text)))


class DriverTests(unittest.TestCase):
    def test_three_honest_runs_pass(self):
        text = "\n".join([run(2, "dive"), run(0, "popup"), run(3, "none")])
        self.assertEqual(s.driver_problems(text, 3, ["dive", "popup", None]), [])

    def test_a_run_that_names_the_forced_profile_passes(self):
        text = run(2, "dive", named="dive", releases=[10.0], impacts=[(27.0, 30)], egress=33.0)
        self.assertEqual(s.driver_problems(text, 1, ["dive"]), [])

    def test_a_wrong_profile_or_count_fails(self):
        self.assertTrue(any("2 runs" in p for p in s.driver_problems(run() + "\n" + run(), 3, [None] * 3)))
        text = run(2, "dive", named="popup")
        self.assertTrue(any("PROFILE name=popup" in p for p in s.driver_problems(text, 1, ["dive"])))
        self.assertTrue(any("asked popup" in p for p in s.driver_problems(run(2, "dive"), 1, ["popup"])))


def handback_run(trigger=True, mode="LVL", release=13.3, back=30.0, miss=359.5, designate="0x50000000"):
    lines = [
        SETUP.format(skill=1, profile="none"),
        "ai-strike: PROFILE t=0.000 name=none requested=none note=no-strike-behaviour",
        "ai-strike: start t=0.000 plane=0 range_ft=24304 alt_ft=5000 speed_kt=450",
        f"ai-strike: DESIGNATE t=0.017 target={designate} T-80",
    ]
    releases = []
    if trigger:
        lines.append(f"ai-strike: RELEASE t={release} n=1 weapon=MK82.JT station=1 alt_ft=5000 speed_kt=449 range_ft=13371")
        lines.append(f"ai-strike: IMPACT t=29.942 n=1 miss_ft={miss} at=1.0,0.0,2.0")
        releases = [release]
    lines.append(f"ai-strike: HANDBACK t={back} designated=none selected=1 mode={mode}")
    lines.append(f"ai-strike: ACTIVITY t={back + 0.008} name=In formation")
    lines.append(
        f"ai-strike: summary t=40.0 ran_s=40.0 profile=none releases={len(releases)} impacts={len(releases)} kills=0 "
        f"egress=no lost=no activity=In formation target_hp=154/200 closest_ft=23 store_rounds_left=3"
    )
    if not releases:
        lines.append("ai-strike: no release")
    lines.append("ai-strike: done")
    return "\n".join(lines)


class HandbackTests(unittest.TestCase):
    def test_a_release_then_a_handoff_passes(self):
        self.assertEqual(s.handback_problems(handback_run() + "\n" + handback_run(trigger=False, back=6.0)), [])

    def test_a_release_after_the_handoff_fails(self):
        text = handback_run(release=31.0) + "\n" + handback_run(trigger=False)
        self.assertTrue(any("after the handback" in p for p in s.handback_problems(text)))

    def test_no_release_by_the_human_fails(self):
        text = handback_run(trigger=False) + "\n" + handback_run(trigger=False)
        self.assertTrue(any("released nothing" in p for p in s.handback_problems(text)))

    def test_the_wrong_mode_or_a_far_bomb_fails(self):
        text = handback_run(mode="CCIP") + "\n" + handback_run(trigger=False)
        self.assertTrue(any("handed back in CCIP" in p for p in s.handback_problems(text)))
        text = handback_run(miss=4000) + "\n" + handback_run(trigger=False)
        self.assertTrue(any("4000 ft from the target" in p for p in s.handback_problems(text)))

    def test_an_undesignated_target_fails(self):
        text = handback_run(designate="none") + "\n" + handback_run(trigger=False)
        self.assertTrue(any("designated nothing" in p for p in s.handback_problems(text)))


def order_run(result="refused", designated="0x50000000", extra=True):
    lines = [
        SETUP.format(skill=1, profile="none").replace("order=none", "order=engage-my-target"),
        "ai-strike: PROFILE t=0.000 name=none requested=none note=no-strike-behaviour",
        "ai-strike: start t=0.000 plane=1 range_ft=24611 alt_ft=5000 speed_kt=450",
        "ai-strike: DESIGNATE t=0.017 target=0x50000000 T-80",
        f"ai-strike: ORDER t=3.017 kind=engage-my-target designated={designated}",
    ]
    if extra:
        lines.append(f'ai-strike: ORDER-REPLY t=3.025 result={result} message="Wing order unavailable: no valid hostile target"')
    lines += [
        "ai-strike: summary t=20.0 ran_s=20.0 profile=none releases=0 impacts=0 kills=0 egress=no lost=no "
        "activity=In formation target_hp=200/200 closest_ft=8902 store_rounds_left=4",
        "ai-strike: no release",
        "ai-strike: done",
    ]
    return "\n".join(lines)


class OrderTests(unittest.TestCase):
    def test_a_refusal_or_an_acceptance_passes(self):
        self.assertEqual(s.order_problems(order_run("refused")), [])
        self.assertEqual(s.order_problems(order_run("given")), [])

    def test_no_reply_or_nothing_designated_fails(self):
        self.assertTrue(any("no reply" in p for p in s.order_problems(order_run(extra=False))))
        self.assertTrue(any("nothing designated" in p for p in s.order_problems(order_run(designated="none"))))

    def test_a_strange_reply_fails(self):
        self.assertTrue(any("order reply weird" in p for p in s.order_problems(order_run("weird"))))


class StrikeTests(unittest.TestCase):
    def good(self, skill=2, profile="dive", miss=100.0, **kw):
        return run(skill, profile, named=profile, releases=[10.0], impacts=[(27.0, miss)], egress=33.0, **kw)

    def wanted(self, skill=2, profile="dive", **extra):
        return [dict(label="r", profile=profile, skill=skill, miss=s.MISS_FT_MAX[skill], **extra)]

    def test_a_good_strike_passes(self):
        self.assertEqual(s.strike_problems(self.good(), self.wanted()), [])

    def test_the_driver_as_it_is_today_fails_the_behaviour_scenarios(self):
        problems = s.strike_problems(run(), self.wanted())
        self.assertTrue(any("no RELEASE" in p for p in problems), problems)
        self.assertTrue(any("flew PROFILE none" in p for p in problems), problems)

    def test_a_miss_beyond_the_skills_limit_fails(self):
        too_far = s.MISS_FT_MAX[3] + 1
        self.assertTrue(any("from the target" in p for p in s.strike_problems(self.good(3, miss=too_far), self.wanted(3))))
        self.assertEqual(s.strike_problems(self.good(0, miss=s.MISS_FT_MAX[3] + 1), self.wanted(0)), [])

    def test_a_lost_striker_or_no_egress_fails(self):
        lost = run(2, "dive", named="dive", releases=[10.0], impacts=[(27.0, 50)], egress=33.0, lost=40.0)
        self.assertTrue(any("was lost" in p for p in s.strike_problems(lost, self.wanted())))
        self.assertEqual(s.strike_problems(lost, self.wanted(may_lose=True)), [])
        no_egress = run(2, "dive", named="dive", releases=[10.0], impacts=[(27.0, 50)])
        self.assertTrue(any("no EGRESS" in p for p in s.strike_problems(no_egress, self.wanted())))

    def test_a_hurt_target_is_required_where_asked(self):
        wanted = [dict(label="r", profile="dive", skill=2, miss=None, hit=True)]
        hurt = self.good(hp="120/200")
        self.assertEqual(s.strike_problems(hurt, wanted), [])
        self.assertTrue(any("unhurt" in p for p in s.strike_problems(self.good(), wanted)))
        self.assertEqual(s.strike_problems(self.good(hp="gone/200"), wanted), [])

    def test_the_run_count_must_match(self):
        self.assertTrue(any("1 runs" in p for p in s.strike_problems(self.good(), self.wanted() * 2)))


class ScenarioTests(unittest.TestCase):
    def setUp(self):
        self.scenarios = {sc.name: sc for sc in s.scenarios()}

    def test_the_driver_scenarios_are_live_and_the_behaviour_ones_pending(self):
        live = {"ai-strike-driver", "ai-strike-handback", "ai-strike-order"}
        for name, sc in self.scenarios.items():
            if name in live:
                self.assertEqual(sc.known_failure, "", name)
            else:
                self.assertIn("ST-T", sc.known_failure, name)

    def test_the_plan_s_scenarios_exist(self):
        want = {"ai-strike-bombs", "ai-strike-rockets", "ai-strike-maverick", "ai-strike-jdam", "ai-sead-harm", "ai-strike-defended"}
        self.assertTrue(want <= set(self.scenarios))

    def test_every_scenario_flies_the_ai(self):
        for name, sc in self.scenarios.items():
            for args in [sc.args] + [step.args for step in sc.then]:
                self.assertIn("--pilot", args, name)
                self.assertEqual(args[args.index("--pilot") + 1], "ai", name)
                self.assertEqual(args[0], "--a2g-run", name)

    def test_the_bomb_scenario_forces_each_pass_once_and_covers_every_skill(self):
        sc = self.scenarios["ai-strike-bombs"]
        argsets = [sc.args] + [step.args for step in sc.then]
        profiles = [a[a.index("--profile") + 1] for a in argsets]
        skills = {int(a[a.index("--skill") + 1]) for a in argsets}
        self.assertEqual(sorted(set(profiles), key=profiles.index)[:5], ["straight", "popup", "fasthigh", "dive", "level"])
        self.assertEqual(skills, {0, 1, 2, 3})
        self.assertEqual(len(argsets), len(s.BOMB_RUNS))

    def test_the_names_are_in_the_selection_family(self):
        patterns = bs.FAMILIES["ai-strike"][1]
        import fnmatch
        for name in self.scenarios:
            self.assertTrue(any(fnmatch.fnmatch(name, p) for p in patterns), name)

    def test_the_checks_accept_an_honest_run_of_the_driver_scenarios(self):
        self.assertEqual(self.scenarios["ai-strike-driver"].check(
            "\n".join([run(2, "dive"), run(0, "popup"), run(3, "none")])), [])
        self.assertEqual(self.scenarios["ai-strike-handback"].check(
            handback_run() + "\n" + handback_run(trigger=False, back=6.0)), [])
        self.assertEqual(self.scenarios["ai-strike-order"].check(order_run()), [])

    def test_the_pending_checks_fail_on_the_driver_as_it_is_today(self):
        today = "\n".join(run(skill, profile) for skill, profile in s.BOMB_RUNS)
        self.assertNotEqual(self.scenarios["ai-strike-bombs"].check(today), [])


class SelectionTests(unittest.TestCase):
    def families(self, path):
        return set(bs.rule_for(path).families)

    def test_the_driver_source_selects_the_strike_and_a2g_families(self):
        self.assertEqual(self.families("crates/tore-app/src/a2g_run/ai.rs"), {"a2g", "ai-strike"})
        self.assertEqual(self.families("crates/tore-app/src/a2g_run/level.rs"), {"a2g"})
        self.assertEqual(self.families("crates/tore-app/src/a2g_run.rs"), {"a2g"})

    def test_the_scenario_file_and_its_tests_select_the_unit_tests(self):
        rule = bs.rule_for("tools/battery_scenarios/ai_strike.py")
        self.assertEqual(set(rule.families), {"ai-strike"})
        self.assertIn("test_battery_ai_strike", rule.unit_tests)
        self.assertEqual(bs.rule_for("tools/test_battery_ai_strike.py").families, ())

    def test_the_family_names_only_strike_scenarios(self):
        description, patterns = bs.FAMILIES["ai-strike"]
        self.assertIn("ai-strike-*", patterns)
        self.assertIn("ai-sead-*", patterns)
        self.assertTrue(description)


if __name__ == "__main__":
    unittest.main()
