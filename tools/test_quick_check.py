import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import quick_check as qc  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent

GRAPH = {
    "tore-formats": [],
    "tore-input": [],
    "tore-sim": ["tore-formats", "tore-input"],
    "tore-world": ["tore-formats", "tore-input", "tore-sim"],
    "tore-replay": [],
    "tore-app": ["tore-sim", "tore-world", "tore-replay", "tore-formats"],
}


class CrateTests(unittest.TestCase):
    def test_workspace_graph_is_read_from_the_manifests(self):
        crates = qc.workspace_crates(ROOT)
        self.assertIn("tore-sim", crates["tore-world"])
        self.assertIn("tore-formats", crates["tore-sim"])
        self.assertEqual(crates["tore-formats"], [])

    def test_dependents_come_with_a_touched_crate(self):
        self.assertEqual(qc.dependents_closure(GRAPH, {"tore-sim"}), {"tore-sim", "tore-world", "tore-app"})
        self.assertEqual(qc.dependents_closure(GRAPH, {"tore-app"}), {"tore-app"})
        self.assertIn("tore-app", qc.dependents_closure(GRAPH, {"tore-input"}))

    def test_touched_crates(self):
        changed = ["crates/tore-sim/src/flight.rs", "docs/x.md", "tools/battery.py", "crates/nope/src/a.rs"]
        self.assertEqual(qc.touched_crates(changed, GRAPH), {"tore-sim"})

    def test_public_api_lines(self):
        yes = ["+pub fn spin()", "-pub struct Flight {", "+    pub const MAX: f32 = 1.0;", "+pub enum E {", "+pub use a::b;", "+pub async fn f()"]
        no = ["+pub(crate) fn spin()", "+    let pub_x = 1;", "+// pub fn spin()", "+fn private()", "+++ b/crates/x.rs", "+pub(super) struct S;"]
        for line in yes:
            self.assertTrue(qc.API_LINE.match(line), line)
        for line in no:
            self.assertFalse(qc.API_LINE.match(line), line)


class ScopeTests(unittest.TestCase):
    def scope(self, changed, api=False):
        with mock.patch.object(qc, "workspace_crates", return_value=GRAPH), mock.patch.object(qc, "public_api_changed", return_value=api):
            return qc.cargo_scope(ROOT, changed, "BASE", None)

    def test_no_rust_no_cargo_steps(self):
        self.assertIsNone(self.scope(["docs/a.md", "tools/battery.py"]))

    def test_a_leaf_crate_runs_alone(self):
        s = self.scope(["crates/tore-app/src/menu.rs"], api=True)
        self.assertEqual(s.packages, ["tore-app"])
        self.assertFalse(s.workspace)

    def test_a_shared_crate_brings_its_dependents(self):
        s = self.scope(["crates/tore-sim/src/flight.rs"])
        self.assertEqual(s.packages, ["tore-app", "tore-sim", "tore-world"])
        self.assertFalse(s.workspace)

    def test_a_public_api_change_in_a_shared_crate_runs_the_workspace(self):
        s = self.scope(["crates/tore-sim/src/flight.rs"], api=True)
        self.assertTrue(s.workspace)
        self.assertEqual(s.packages, sorted(GRAPH))

    def test_cargo_files_run_the_workspace(self):
        for path in ("Cargo.lock", "Cargo.toml", "crates/tore-app/Cargo.toml", "rust-toolchain.toml"):
            self.assertTrue(self.scope([path]).workspace, path)


class SummaryTests(unittest.TestCase):
    def test_battery_summary(self):
        out = "  chosen 40 of 900 candidate scenarios (headless only), estimated 118 s wall at 4 jobs (budget 120 s), 400 s of scenario time\npass a (1.0s)\n\n40/40 passed. Results: x/summary.md\n"
        self.assertEqual(qc.battery_summary(out), "40/40 passed (headless only, estimated 118 s)")
        self.assertEqual(qc.battery_summary("Nothing to run: no scenario can be affected by these changes.\n"), "no scenario can be affected")

    def test_battery_echo_stops_at_the_first_result(self):
        out = "plan line\nsecond\npass a (1.0s)\nFAIL b (2.0s)\n"
        self.assertEqual(qc.battery_echo(out), "    plan line\n    second")

    def test_guard_summary(self):
        self.assertEqual(qc.guard_summary("== summary\nSAME 52, DIFFERENT 0, MISSING 0\nRESULT: ok\n"), "SAME 52, DIFFERENT 0, MISSING 0")


if __name__ == "__main__":
    unittest.main()
