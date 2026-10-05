"""Tests for the net lane's own helpers (tools/battery_scenarios/net.py)."""
import socket
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parent))
import battery  # noqa: E402
from battery_scenarios import net  # noqa: E402

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


class ScenarioListTests(unittest.TestCase):
    def test_every_scenario_is_a_driver_in_the_net_lane(self):
        scenarios = net.scenarios()
        self.assertGreaterEqual(len(scenarios), 7)
        for s in scenarios:
            self.assertEqual(s.lane, "net", s.name)
            self.assertTrue(callable(s.driver), s.name)
            self.assertTrue(s.name.startswith(("net-server-", "net-discovery", "net-window-", "net-master-")), s.name)
            self.assertTrue(set(s.uses) <= {"server", "bot"}, s.name)
            self.assertEqual(s.window, s.name.startswith("net-window-"), f"{s.name}: only the window scenarios open one")

    def test_the_battery_loads_them(self):
        names = [s.name for s in battery.load_scenarios() if s.lane == "net"]
        self.assertIn("net-server-fight", names)
        self.assertIn("net-window-host", names)


if __name__ == "__main__":
    unittest.main()
