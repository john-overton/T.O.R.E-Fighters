from pathlib import Path
import unittest

from setup_dev import HOOKS_DIRECTORY, REQUIRED_HOOKS, repository_root


class SetupTests(unittest.TestCase):
    def setUp(self):
        self.root = repository_root()
        self.assertIsNotNone(self.root, "tests must run inside the repository")

    def test_every_required_hook_is_committed(self):
        for name in REQUIRED_HOOKS:
            self.assertTrue(
                (self.root / HOOKS_DIRECTORY / name).is_file(),
                f"{HOOKS_DIRECTORY}/{name} is missing",
            )

    def test_pre_push_runs_the_checks_ci_runs(self):
        """The hook is only useful while it mirrors the CI job."""
        hook = (self.root / HOOKS_DIRECTORY / "pre-push").read_text(encoding="utf-8")
        for command in (
            "cargo fmt --all -- --check",
            "cargo clippy --workspace --all-targets --locked -- -D warnings",
            "cargo test --workspace --locked",
            "cargo build --workspace --locked",
            "unittest discover -s tools",
            "tools/check_assets.py",
            "tools/check_docs.py",
        ):
            self.assertIn(command, hook, f"pre-push no longer runs: {command}")

    def test_pre_push_fails_the_push_when_a_check_fails(self):
        hook = (self.root / HOOKS_DIRECTORY / "pre-push").read_text(encoding="utf-8")
        self.assertIn("exit 1", hook)

    def test_setup_is_documented_where_a_fresh_clone_will_look(self):
        for relative in ("AGENTS.md", "docs/DEVELOPMENT.md"):
            text = (self.root / relative).read_text(encoding="utf-8")
            self.assertIn("tools/setup_dev.py", text, f"{relative} does not mention setup")


if __name__ == "__main__":
    unittest.main()
