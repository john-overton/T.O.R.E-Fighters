"""Tests for the net lane's own helpers (tools/battery_scenarios/net.py)."""
import re
import socket
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parent))
import battery  # noqa: E402
from battery_scenarios import net  # noqa: E402
from battery_scenarios import net_observe  # noqa: E402

CHECK_REPORT = """Mission: UKR (clear), airborne at 20000 ft, enemy 20 nm away; friendly 4 F18.PT, 2 F14.PT; enemy 4 MIG29.PT, 2 SU27.PT; 12 aircraft
Mission file: /tmp/mission.txt

Planes (the numbers open-planes and --slot use):
   0  F18.PT    friendly wing 1, member 1, experienced
   1  F18.PT    friendly wing 1, member 2, experienced
   2  F18.PT    friendly wing 1, member 3, experienced
   3  F18.PT    friendly wing 1, member 4, experienced
   4  F14.PT    friendly wing 2, member 1, average
   5  F14.PT    friendly wing 2, member 2, average
   6  MIG29.PT  enemy wing 1, member 1, experienced
   7  MIG29.PT  enemy wing 1, member 2, experienced
   8  MIG29.PT  enemy wing 1, member 3, experienced
   9  MIG29.PT  enemy wing 1, member 4, experienced
  10  SU27.PT   enemy wing 2, member 1, ace
  11  SU27.PT   enemy wing 2, member 2, ace

Runways of UKR (the numbers start ground takes):
    0  Zaporizhzhya (5532 ft)

Content manifest: 433 resources, digest 012f3911765f6c35

Content: Fighters Anthology 1.02F, imported by T.O.R.E 0.1.4 (48d62dac)
Content items: 2 aircraft, 1 theater, 1 weapon, the shared data
  aircraft F14.PT 0c68b3b124f7f3b8
  aircraft F18.PT 195dfc20f45d3807
  theater UKR 2ef6db2ac361554f
  weapon AIM9X.JT 69d2821b6ba1980a
  shared data 3fe3336ebde69141
"""

PLAYERS = """id   seat callsign        plane      rtt    loss    margin  repeated  address
1    1    GoneA           0         4 ms    0.0%    4.0 tk         0  [::ffff:127.0.0.1]:57384
2    2    GoneB           2         5 ms    0.0%    5.0 tk         0  [::ffff:127.0.0.1]:33907
0    0    Stay            1         5 ms    0.0%   28.0 tk        28  [::ffff:127.0.0.1]:39149
3    -    Lobbyist        -         5 ms      -       -          0  [::ffff:127.0.0.1]:40000
"""

NET_TSV = (
    "seconds\tkind\tround_trip_ms\tloss_percent\n"
    "0.000\tconnect\t127.0.0.1:27411\tViper\n"
    "0.009\tjoined\t299a11382b378d60\t0\n"
    "0.321\tmission\t0\n"
    "0.954\tseated\t0\t0\t0\n"
    + "".join(f"{i}.0\tstats\t5.0\t0.00\n" for i in range(2, 8))
)


def fake_drive(directory):
    return SimpleNamespace(work=Path(directory), problems=[], problem=lambda text: None)


class GuideMissionTests(unittest.TestCase):
    def test_the_example_is_read_from_the_guide(self):
        mission = net.guide_mission()
        self.assertTrue(mission.startswith("tore-mission 1\n"))
        self.assertIn("wing friendly 1 F18.PT 4 experienced", mission)
        self.assertIn("separation-nm 20", mission)

    def test_the_enemy_can_be_brought_closer(self):
        mission = net.guide_mission(separation_nm=5)
        self.assertIn("separation-nm 5\n", mission)
        self.assertNotIn("separation-nm 20", mission)
        self.assertEqual(mission.count("separation-nm"), 1)

    def test_the_server_files_are_written_with_the_given_settings(self):
        with tempfile.TemporaryDirectory() as d:
            config = net.write_server_files(fake_drive(d), 27500, name="Battery", empty_timeout=7)
            text = config.read_text()
            self.assertIn("port 27500\n", text)
            self.assertIn("mission mission.txt\n", text)
            self.assertIn("name Battery\n", text)
            self.assertIn("empty-timeout 7\n", text)  # one setting once: the default is replaced
            self.assertEqual(text.count("empty-timeout"), 1)
            self.assertIn("after-end quit\n", text)
            self.assertTrue((Path(d) / "mission.txt").read_text().startswith("tore-mission 1"))


class CheckReportTests(unittest.TestCase):
    def test_the_real_report_passes(self):
        self.assertEqual(net.check_report_problems(CHECK_REPORT), [])

    def test_a_missing_plane_a_missing_manifest_and_a_wrong_summary_are_reported(self):
        short = CHECK_REPORT.replace("   5  F14.PT    friendly wing 2, member 2, average\n", "")
        self.assertTrue(any("planes 0 to 11" in p for p in net.check_report_problems(short)))
        self.assertTrue(any("manifest" in p for p in net.check_report_problems(CHECK_REPORT.replace("Content manifest", "Content"))))
        self.assertTrue(any("summary" in p for p in net.check_report_problems(CHECK_REPORT.replace("UKR (clear)", "BAL (clear)"))))
        self.assertTrue(any("runway" in p for p in net.check_report_problems(CHECK_REPORT.replace("Runways of", "Strips of"))))
        # Stage L (slice L3): the import's source, its counts and one line per item.
        no_source = CHECK_REPORT.replace("Content: Fighters", "Contents: Fighters")
        self.assertTrue(any("source" in p for p in net.check_report_problems(no_source)))
        no_items = "".join(line for line in CHECK_REPORT.splitlines(True) if not line.startswith("  aircraft"))
        self.assertTrue(any("aircraft" in p for p in net.check_report_problems(no_items)))


class ParsingTests(unittest.TestCase):
    def test_the_players_table_gives_ids_seats_and_planes(self):
        rows = {r["callsign"]: r for r in net.players_table(PLAYERS)}
        self.assertEqual(rows["GoneA"], {"id": 1, "seat": "1", "callsign": "GoneA", "plane": "0"})
        self.assertEqual(rows["Stay"]["id"], 0)
        self.assertEqual(rows["Lobbyist"]["seat"], "-")
        self.assertEqual(len(rows), 4)

    def test_a_good_net_log_passes_and_each_gap_is_named(self):
        self.assertEqual(net.net_log_problems(NET_TSV), [])
        self.assertEqual(net.net_log_problems(""), ["net log has no header line"])
        no_seat = NET_TSV.replace("seated", "x")
        self.assertIn("net log has no `seated` line", net.net_log_problems(no_seat))
        few = "".join(NET_TSV.splitlines(True)[:7])
        self.assertTrue(any("only 2 stats" in p for p in net.net_log_problems(few)))
        wide = NET_TSV + "9.0\tstats\t5.0\t0.00\textra\n"
        self.assertTrue(any("columns" in p for p in net.net_log_problems(wide)))

    def test_the_figures_lines_are_found_per_player(self):
        line = "figures seat 0 Bot1 plane 0: round trip 5 ms, loss 0.0%, arrival spread 1.0 ms"
        self.assertEqual(net.figures_problems(line, ["Bot1"]), [])
        self.assertEqual(net.figures_problems(line, ["Bot1", "Bot2"]), ["the server log has no figures line for Bot2"])

    def test_the_scores_lines_are_checked_per_bot(self):
        good = (
            "Bot1: scores: players ranked by kills: 1 Bot1 (friendly) 0/0 0.00, 2 Bot2 (friendly) 0/0 0.00; "
            "sides 0/0 to 0/0; 0:58 left\n"
            "Bot2: scores: players ranked by kills: 1 Bot1 (friendly) 0/0 0.00, 2 Bot2 (friendly) 0/0 0.00; "
            "sides 0/0 to 0/0; 0:58 left\n"
            "Bot1: scores: players ranked by kills: 1 Bot1 (friendly) 1/0 1.00, 2 Bot2 (friendly) 0/0 0.00; "
            "sides 1/0 to 0/0; 0:00 left\n"
            "Bot1: Mission ended: the time limit.\n"
            "Bot2: scores: players ranked by kills: 1 Bot1 (friendly) 1/0 1.00, 2 Bot2 (friendly) 0/0 0.00; "
            "sides 1/0 to 0/0; 0:00 left\n"
            "Bot2: Mission ended: the time limit.\n"
        )
        self.assertEqual(net.scores_problems(good, ["Bot1", "Bot2"]), [])
        self.assertEqual(net.scores_problems(good, ["Bot1", "Bot3"])[-1], "Bot3 printed no scores")
        late = good.replace("Bot2: Mission ended: the time limit.\n", "") + "Bot2: Mission ended: the time limit.\n"
        self.assertEqual(net.scores_problems(late, ["Bot1", "Bot2"]), [])
        no_final = good.replace("0:00 left", "0:01 left")
        self.assertIn("Bot1's last scores are not the final ones (0:00 left)", net.scores_problems(no_final, ["Bot1"]))
        after = good + "Bot1: scores: players ranked by kills: 1 Bot1 0/0 0.00; sides 0/0 to 0/0; 0:00 left\n"
        self.assertIn("Bot1's final scores came after the end", net.scores_problems(after, ["Bot1"]))
        alone = good.replace(", 2 Bot2 (friendly) 0/0 0.00", "")
        self.assertIn("no scores line of Bot1's lists every player", net.scores_problems(alone, ["Bot1", "Bot2"]))

    def test_the_results_line_is_checked_per_bot(self):
        row = "0 Blue alive 0k, 1 AI alive 0k, 2 AI dead 1k, 6 Red alive 0k, 7 AI alive 0k"
        good = (
            f"Blue: results: 5 aircraft, 2 flown by players: {row}; a draw\n"
            "Blue: Mission ended: the time limit.\n"
            f"Red: results: 5 aircraft, 2 flown by players: {row}; a draw\n"
            "Red: Mission ended: the time limit.\n"
        )
        self.assertEqual(net.results_problems(good, ["Blue", "Red"]), [])
        winner = good.replace("a draw", "the friendly side wins")
        self.assertEqual(net.results_problems(winner, ["Blue", "Red"]), [])
        self.assertEqual(
            net.results_problems(good, ["Blue", "Hawk"]),
            ["Blue's results list no row for Hawk", "Hawk printed 0 results lines, not one"],
        )
        twice = good + good.splitlines()[0] + "\n"
        self.assertEqual(net.results_problems(twice, ["Blue"]), ["Blue printed 2 results lines, not one"])
        no_ai = good.replace("1 AI alive 0k, 2 AI dead 1k, ", "").replace("7 AI alive 0k", "8 Red2 alive 0k")
        self.assertIn("Blue's results list no AI aircraft", net.results_problems(no_ai, ["Blue", "Red"]))
        no_winner = good.replace("; a draw", "")
        self.assertIn("Blue's results name no winner of the final scores", net.results_problems(no_winner, ["Blue", "Red"]))
        late = "Blue: Mission ended: the time limit.\n" + good.splitlines()[0] + "\n"
        self.assertIn("Blue's results came after the mission's end", net.results_problems(late, ["Blue"]))
        few = good.replace("5 aircraft, 2 flown", "2 aircraft, 2 flown")
        self.assertIn("Blue's results list only 2 aircraft, none of them the AI's", net.results_problems(few, ["Blue", "Red"]))

    def test_the_pvp_end_follows_the_kills(self):
        line = "{who}: scores: players ranked by kills: 1 Blue (friendly) {b}/{bl} 0.00, 2 Red (enemy) {r}/{rl} 0.00; " \
            "sides {b}/{bl} to {r}/{rl}; {left} left; ends at 1 kill in all{end}\n"
        flying = line.format(who="Blue", b=0, bl=0, r=0, rl=0, left="1:30", end="")
        won = flying + line.format(who="Red", b=2, bl=0, r=0, rl=1, left="1:02", end="; the friendly side wins") + \
            "Red: Mission ended: the kill limit.\n"
        self.assertEqual(net.pvp_end_problems(won, ["Blue", "Red"]), [])
        # Two bots that shot each other down on one pass finish level, a draw.
        both = flying + line.format(who="Red", b=2, bl=1, r=2, rl=1, left="1:02", end="; a draw") + \
            "Red: Mission ended: the kill limit.\n"
        self.assertEqual(net.pvp_end_problems(both, ["Blue", "Red"]), [])
        # A time limit with nobody shot down is no longer enough (the scripted pilot lands gun kills).
        draw = flying + line.format(who="Blue", b=0, bl=0, r=0, rl=0, left="0:00", end="; a draw") + \
            "Blue: Mission ended: the time limit.\n"
        problems = net.pvp_end_problems(draw, ["Blue", "Red"])
        self.assertIn("nobody shot anyone down: the time limit ended the mission, not the kill limit", problems)
        self.assertIn("no player scored a kill", problems)
        # A draw with one scorer, a limit that ended it without a winner line and no end at all.
        lone = flying + line.format(who="Blue", b=1, bl=0, r=0, rl=0, left="1:00", end="; a draw") + \
            "Blue: Mission ended: the kill limit.\n"
        self.assertIn("fewer than two players who scored", net.pvp_end_problems(lone, ["Blue", "Red"])[0])
        bare = flying + line.format(who="Blue", b=1, bl=0, r=0, rl=0, left="1:00", end="") + \
            "Blue: Mission ended: the kill limit.\n"
        self.assertIn("name no winner", net.pvp_end_problems(bare, ["Blue", "Red"])[0])
        self.assertEqual(net.pvp_end_problems(flying, ["Blue", "Red"])[0], "the kill limit did not end the mission")
        co_op = won.replace("(enemy)", "(friendly)").replace("ends at 1 kill in all", "")
        self.assertEqual(
            net.pvp_end_problems(co_op, ["Blue", "Red"]),
            ["no scores line puts a player on the enemy side", "no scores line names the kill limit (1 kill in all)"],
        )
        self.assertEqual(net.pvp_end_problems("", ["Blue"]), ["no bot printed scores"])

    def test_a_hunt_needs_the_bots_own_kill_and_its_sides_win(self):
        line = "Hunter: scores: players ranked by kills: 1 Hunter (friendly) {k}/0 0.40; sides {k}/0 to 0/0; " \
            "ends at 1 kill in all{end}\n"
        good = line.format(k=0, end="") + line.format(k=1, end="; the friendly side wins") + \
            "Hunter: Mission ended: the kill limit.\n"
        self.assertEqual(net.hunt_problems(good, "Hunter"), [])
        timed = line.format(k=0, end="; a draw") + "Hunter: Mission ended: the time limit.\n"
        problems = net.hunt_problems(timed, "Hunter")
        self.assertIn("the kill limit did not end the mission: the bot shot nothing down in time", problems)
        self.assertTrue(any("has no kill" in p for p in problems), problems)
        self.assertEqual(net.hunt_problems("", "Hunter"), ["Hunter printed no scores"])

    def test_the_pvp_mission_can_be_made_peaceful_and_its_enemies_dummies(self):
        mission = net.guide_mission(separation_nm=5)
        peaceful = net.weapons_hold(mission)
        self.assertIn("preset hold\n", peaceful)
        self.assertNotIn("preset free", peaceful)
        self.assertNotIn("objective ", peaceful)
        self.assertIn("wing enemy 1 MIG29.PT 4 experienced", peaceful, "the wings themselves are as they were")
        dummies = net.dummy_enemies(mission)
        self.assertIn("wing enemy 1 MIG29.PT 4 dummy\n", dummies)
        self.assertIn("wing enemy 2 SU27.PT 2 dummy\n", dummies)
        self.assertIn("wing friendly 1 F18.PT 4 experienced\n", dummies, "the friendly wings keep their skill")

    def test_a_revival_is_read_from_the_bots_lines(self):
        good = (
            "Phoenix: seat 0, plane 0, at tick 3\n"
            "Phoenix: ejected\n"
            "Phoenix: revival: Press Enter to fly again\n"
            "Phoenix: spawned plane 12 in Friendly wing 1, member 4\n"
            "Phoenix: seat 0, plane 12, at tick 1200\n"
        )
        self.assertEqual(net.revive_problems(good, "Phoenix", 12), [])
        self.assertEqual(
            net.revive_problems(good.replace("Phoenix: ejected\n", ""), "Phoenix", 12), ["Phoenix never ejected"]
        )
        never = good.replace("plane 12, at tick 1200", "plane 0, at tick 1200")
        self.assertEqual(net.revive_problems(never, "Phoenix", 12), ["Phoenix was not seated again in plane 12"])
        lines = good.splitlines(keepends=True)
        early = lines[0] + lines[4] + lines[1] + lines[2] + lines[3]
        self.assertIn(
            "Phoenix's ejection, revival, new plane and seating came out of order",
            net.revive_problems(early, "Phoenix", 12),
        )
        self.assertIn("Phoenix was not told of plane 13", net.revive_problems(good, "Phoenix", 13))

    def test_an_away_and_back_is_read_from_the_bots_lines(self):
        good = (
            "Viper: seat 0, plane 0, at tick 3\n"
            "Viper: away: the AI flies plane 0\n"
            "Viper: observing from tick 1210, 0 s behind\n"
            "Viper: back at the controls\n"
            "Viper: observing ended\n"
            "Viper: seat 1, plane 0, at tick 1930\n"
        )
        self.assertEqual(net.away_problems(good, "Viper", 0), [])
        self.assertEqual(
            net.away_problems(good.replace("Viper: away: the AI flies plane 0\n", ""), "Viper", 0),
            ["the AI never flew Viper's plane 0"],
        )
        never = good.replace("plane 0, at tick 1930", "plane 1, at tick 1930")
        self.assertEqual(net.away_problems(never, "Viper", 0), ["Viper was not seated again in plane 0"])
        unwatched = good.replace("Viper: observing from tick 1210, 0 s behind\n", "")
        self.assertEqual(net.away_problems(unwatched, "Viper", 0), ["Viper never watched its plane while away"])
        self.assertEqual(
            net.away_problems("", "Viper", 0),
            [
                "Viper was never seated in plane 0",
                "the AI never flew Viper's plane 0",
                "Viper never watched its plane while away",
                "Viper never asked for its plane back",
                "Viper was not seated again in plane 0",
            ],
        )

    def test_a_rejoin_is_read_from_the_bots_lines(self):
        good = (
            "Viper: rejoining with its token (session 00000000deadbeef)\n"
            "Viper: joined\n"
            "Viper: Welcome back, Viper: your aircraft is waiting.\n"
            "Viper: seat 0, plane 0, at tick 812\n"
        )
        self.assertEqual(net.rejoin_problems(good, "Viper", 0), [])
        self.assertEqual(
            net.rejoin_problems(good.replace("plane 0, at", "plane 2, at"), "Viper", 0),
            ["Viper was not seated in plane 0 again"],
        )
        self.assertEqual(
            net.rejoin_problems(good.replace("Viper: Welcome back, Viper: your aircraft is waiting.\n", ""), "Viper", 0),
            ["Viper was not welcomed back with its aircraft waiting"],
        )
        swapped = (
            "Viper: seat 0, plane 0, at tick 812\n"
            "Viper: rejoining with its token (session 00000000deadbeef)\n"
            "Viper: Welcome back, Viper: your aircraft is waiting.\n"
        )
        self.assertEqual(
            net.rejoin_problems(swapped, "Viper", 0),
            ["Viper's token, welcome and seating came out of order"],
        )
        self.assertEqual(
            net.rejoin_problems("", "Viper", 0),
            [
                "Viper never sent its token",
                "Viper was not welcomed back with its aircraft waiting",
                "Viper was not seated in plane 0 again",
            ],
        )

    def test_bad_network_words_are_recognised(self):
        import re

        for text in ("The connection ended: a protocol error.", "no packets for 5 seconds", "Bot1: refused: full", "x fault y"):
            self.assertTrue(re.search(net.NET_BAD, text), text)
        self.assertFalse(re.search(net.NET_BAD, "Bot1: The connection ended: the player left."))

    def test_a_port_in_use_is_noticed(self):
        with socket.socket(socket.AF_INET6, socket.SOCK_DGRAM) as held:
            held.bind(("::", 0))
            self.assertTrue(net.port_in_use(held.getsockname()[1]))
        self.assertFalse(net.port_in_use(battery.free_port()))

    def test_fresh_data_clears_what_a_profile_copy_brought(self):
        with tempfile.TemporaryDirectory() as d:
            data = Path(d)
            (data / "logs").mkdir()
            (data / "logs" / "net-old.tsv").write_text("old")
            (data / "replays").mkdir()
            (data / "network-v1.conf").write_text("old")
            (data / "keep.pack").write_text("import")
            net.fresh_data(SimpleNamespace(data=data))
            self.assertEqual([p.name for p in data.iterdir()], ["keep.pack"])


class AwayWatchTests(unittest.TestCase):
    GAME = (
        "Network: seated in plane 0\n"
        "Network: away for the idle-ai seconds; the AI flies the plane\n"
        "Observer screen: watching the mission\n"
        "Observer screen: watching the player's own aircraft\n"
        "Network: a flight input; taking the aircraft back from the AI\n"
        "Observer screen: back to the flight\n"
        "Network: seated in plane 0\n"
        "Network: away for the idle-ai seconds; the AI flies the plane\n"
        "Observer screen: watching the mission\n"
        "Observer screen: watching the player's own aircraft\n"
        "Network: Stop Watching; taking the aircraft back from the AI\n"
        "Observer screen: back to the flight\n"
        "Network: seated in plane 0\n"
    )
    SERVER = (
        "seat 0 Viper: Viper is away: the AI flies plane 0\n"
        "Viper is back: takes plane 0 from the AI\n"
        "Viper is away: the AI flies plane 0\n"
        "Viper is back: takes plane 0 from the AI\n"
    )

    def test_two_handoffs_and_two_returns_pass(self):
        self.assertEqual(net_observe.away_watch_problems(self.GAME, self.SERVER), [])

    def test_each_missing_step_is_named(self):
        problems = net_observe.away_watch_problems(
            self.GAME.replace("Network: a flight input; taking the aircraft back from the AI\n", ""), self.SERVER
        )
        self.assertEqual(len(problems), 1)
        self.assertIn("a flight input taking the plane back", problems[0])
        problems = net_observe.away_watch_problems(self.GAME + "Could not show the mission: x\n", self.SERVER)
        self.assertEqual(len(problems), 1)
        self.assertIn("failing to open", problems[0])
        problems = net_observe.away_watch_problems(self.GAME + "The AI lost your aircraft while you were away.\n", self.SERVER)
        self.assertEqual(len(problems), 1)
        self.assertIn("lost the aircraft", problems[0])
        self.assertEqual(len(net_observe.away_watch_problems("", "")), 8)
        once = self.SERVER.split("Viper is away", 2)[0] + "Viper is away: the AI flies plane 0\n"
        self.assertGreaterEqual(len(net_observe.away_watch_problems(self.GAME, once)), 1)

    def test_the_scenario_is_listed_with_a_window(self):
        scenario = next(s for s in net_observe.scenarios() if s.name == "net-window-away-watch")
        self.assertTrue(scenario.window and callable(scenario.driver))
        self.assertTrue(set(scenario.uses) <= {"server", "bot"})
        # Every picture the script takes is one the driver looks for.
        for name in net_observe.AWAY_PICTURES:
            self.assertIn(f"SHOTS/{name}.ppm", net_observe.AWAY_SCRIPT)
        self.assertEqual(net_observe.AWAY_SCRIPT.count("shot SHOTS/"), len(net_observe.AWAY_PICTURES))


class ContentReportTests(unittest.TestCase):
    """The item lines net-content-builds compares between `tore-server --check` and `tore-bot --content-report`."""

    def test_item_lines_are_read_and_nothing_else(self):
        text = (
            "Content manifest: 1405 resources, digest 0011223344556677\n\n"
            "Content: Fighters Anthology 1.02F, imported by T.O.R.E 0.1.4 (48d62dac)\n"
            "Content items: 1 aircraft, 1 theater, 1 weapon, the shared data\n"
            "  aircraft F18.PT 0123456789abcdef\n"
            "  theater UKR fedcba9876543210\n"
            "  weapon AIM9X.JT 00000000000000ff\n"
            "  shared data 1111111111111111\n"
            "   0  F18.PT    friendly wing 1, member 1, average\n"
        )
        self.assertEqual(
            net.content_item_lines(text),
            [
                "aircraft F18.PT 0123456789abcdef",
                "theater UKR fedcba9876543210",
                "weapon AIM9X.JT 00000000000000ff",
                "shared data 1111111111111111",
            ],
        )


PILOTS = """Pilot1: Lost contact with the host. Moving the game to Pilot2...
Pilot2: Lost contact with the host. Moving the game to Pilot1...
Pilot3: Lost contact with the host. Moving the game to Pilot1...
Pilot1: migrate: lost the host
Pilot1: migrate: taking the game over
Pilot1: host: took the game over at tick 3026: replayed 0 ticks in 0 ms, 4 players expected back
Pilot1: host: Pilot2 resumed 450 ms after the takeover, flying
Pilot1: host: Pilot3 resumed 466 ms after the takeover, flying
Pilot2: The game moved to Pilot1.
Pilot3: The game moved to Pilot1.
Pilot1: The game moved to Pilot1.
Pilot1: host: live at tick 3547, 2838 ms after the takeover, 521 ticks fast-forwarded
Pilot1: migrate: snapshots again 2900 ms after the loss was noticed
Pilot2: migrate: snapshots again 2400 ms after the loss was noticed
Pilot3: migrate: snapshots again 2500 ms after the loss was noticed
Pilot1: host: world: tick 3026, 0 missiles in flight, 2 aircraft kills, 3 players
Pilot1: host: world: tick 6768, 0 missiles in flight, 4 aircraft kills, 3 players
Pilot1: results: 12 aircraft, 4 flown by players: 0 Lead dead 0k, 1 Pilot1 alive 0k, 4 AI ejected 0k, 5 AI alive 1k, 7 AI alive 1k, 10 AI alive 1k, 11 AI alive 1k
"""
BEFORE = {"who": "Lead", "tick": 3007, "missiles": 1, "kills": 2, "players": 4}
PILOT_NAMES = ["Pilot1", "Pilot2", "Pilot3"]


class MigrationTests(unittest.TestCase):
    def test_world_lines_and_results_are_read(self):
        text = "Lead: host: world: tick 120, 3 missiles in flight, 1 aircraft kills, 4 players\n" + PILOTS
        self.assertEqual(net.world_lines(text, "Lead"), [{"who": "Lead", "tick": 120, "missiles": 3, "kills": 1, "players": 4}])
        self.assertEqual([w["tick"] for w in net.world_lines(text)], [120, 3026, 6768])
        self.assertEqual(net.results_kills(PILOTS, "Pilot1"), 4)
        self.assertIsNone(net.results_kills(PILOTS, "Pilot2"))
        self.assertEqual(net.snapshots_again(PILOTS, "Pilot2"), [2400])

    def test_a_clean_migration_has_no_problems(self):
        self.assertEqual(net.migrate_problems(PILOTS, PILOT_NAMES, BEFORE, net.SNAPSHOTS_AGAIN_MS), [])

    def test_each_failure_is_named(self):
        def problems(text, **kw):
            return net.migrate_problems(text, PILOT_NAMES, BEFORE, net.SNAPSHOTS_AGAIN_MS, **kw)

        self.assertTrue(any("0 games took" in p for p in problems(PILOTS.replace("migrate: taking the game over", "x"))))
        twice = PILOTS + "Pilot2: migrate: taking the game over\n"
        self.assertTrue(any("2 games took" in p for p in problems(twice)))
        slow = PILOTS.replace("Pilot3: migrate: snapshots again 2500", "Pilot3: migrate: snapshots again 4900")
        self.assertTrue(any("Pilot3's snapshots came again after 4900" in p for p in problems(slow)))
        self.assertTrue(any("Pilot2 was never told" in p for p in problems(PILOTS.replace("Pilot2: The game moved", "x"))))
        lost = PILOTS.replace("2 aircraft kills", "1 aircraft kills")
        self.assertTrue(any("holds 1 kills" in p for p in problems(lost)))
        short = PILOTS.replace("5 AI alive 1k, 7 AI alive 1k, 10 AI alive 1k, ", "")
        self.assertTrue(any("Results hold 1 aircraft kills" in p for p in problems(short)))
        self.assertTrue(any("never saw Pilot3 resume" in p for p in problems(PILOTS.replace("Pilot3 resumed", "x"))))
        stuck = PILOTS.replace("tick 6768", "tick 3100")
        self.assertTrue(any("did not carry on" in p for p in problems(stuck)))
        self.assertEqual(problems(PILOTS.replace("Pilot3 resumed", "x"), handover=True), [])

    def test_a_slow_fast_forward_is_judged_by_how_closely_the_pilots_follow_the_host(self):
        # 831 ticks at 4.7 ms: the 5 second target is not judged, but the pilots must follow the host going live.
        slow = PILOTS.replace("live at tick 3547, 2838 ms after the takeover, 521 ticks", "live at tick 3547, 5426 ms after the takeover, 831 ticks")
        slow = re.sub(r"snapshots again \d+ ms", "snapshots again 6018 ms", slow)
        self.assertEqual(net.migrate_problems(slow, PILOT_NAMES, BEFORE, net.SNAPSHOTS_AGAIN_MS), [])
        late = slow.replace("Pilot3: migrate: snapshots again 6018", "Pilot3: migrate: snapshots again 9000")
        self.assertTrue(any("Pilot3's snapshots came again after 9000" in p and "cost 4.7 ms a tick" in p
                            for p in net.migrate_problems(late, PILOT_NAMES, BEFORE, net.SNAPSHOTS_AGAIN_MS)))
        # The same lateness at a fast tick is a failure of the target.
        fast = PILOTS.replace("2838 ms after the takeover, 521 ticks", "1900 ms after the takeover, 521 ticks").replace(
            "Pilot3: migrate: snapshots again 2500", "Pilot3: migrate: snapshots again 4000"
        )
        self.assertTrue(any("over 3500 ms" in p for p in net.migrate_problems(fast, PILOT_NAMES, BEFORE, net.SNAPSHOTS_AGAIN_MS)))


class ScenarioListTests(unittest.TestCase):
    def test_every_scenario_is_a_driver_in_the_net_lane(self):
        scenarios = net.scenarios()
        self.assertGreaterEqual(len(scenarios), 7)
        for s in scenarios:
            self.assertEqual(s.lane, "net", s.name)
            self.assertTrue(callable(s.driver), s.name)
            self.assertTrue(
                s.name.startswith(
                    ("net-server-", "net-discovery", "net-convert-", "net-window-", "net-master-", "net-content-",
                     "net-migrate-", "net-reach-")
                ),
                s.name,
            )
            self.assertTrue(set(s.uses) <= {"server", "bot"}, s.name)
            self.assertEqual(s.window, s.name.startswith("net-window-"), f"{s.name}: only the window scenarios open one")

    def test_the_battery_loads_them(self):
        names = [s.name for s in battery.load_scenarios() if s.lane == "net"]
        self.assertIn("net-server-fight", names)
        self.assertIn("net-window-host", names)


if __name__ == "__main__":
    unittest.main()
