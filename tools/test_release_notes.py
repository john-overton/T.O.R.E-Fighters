import unittest

from release_notes import body, title


NOTES = """# v0.1.1: Replays

**Released:** 2026-09-26 · **Tag:** [v0.1.1](https://github.com/o/r/releases/tag/v0.1.1)

See [replays](../REPLAYS.md), [v0.1.0](v0.1.0.md#known-limitations),
[the map](https://example.com/map.html) and [below](#feedback).
![shot](../images/shot.png "A shot")
"""


class ReleaseNotesTests(unittest.TestCase):
    def test_title_is_the_first_heading(self):
        self.assertEqual(title(NOTES), "v0.1.1: Replays")

    def test_title_requires_a_heading(self):
        with self.assertRaises(ValueError):
            title("No heading\n")

    def test_body_drops_the_title_line(self):
        self.assertTrue(body(NOTES, "v0.1.1").startswith("**Released:**"))

    def test_relative_links_point_at_the_tag(self):
        result = body(NOTES, "v0.1.1", repository="o/r")
        self.assertIn("](https://github.com/o/r/blob/v0.1.1/docs/REPLAYS.md)", result)
        self.assertIn(
            "](https://github.com/o/r/blob/v0.1.1/docs/release/v0.1.0.md#known-limitations)",
            result,
        )
        self.assertIn(
            '![shot](https://github.com/o/r/raw/v0.1.1/docs/images/shot.png "A shot")', result
        )

    def test_absolute_and_fragment_links_are_kept(self):
        result = body(NOTES, "v0.1.1", repository="o/r")
        self.assertIn("](https://example.com/map.html)", result)
        self.assertIn("](https://github.com/o/r/releases/tag/v0.1.1)", result)
        self.assertIn("](#feedback)", result)


if __name__ == "__main__":
    unittest.main()
