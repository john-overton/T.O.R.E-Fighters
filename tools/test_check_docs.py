import unittest

from check_docs import HEADER, HEADER_MARKER, HEADER_PREFIX, apply_header, covered


TITLE = "# Example document\n"
BODY = "First paragraph.\n"


class DocumentHeaderTests(unittest.TestCase):
    def test_header_is_inserted_under_the_title(self):
        result = apply_header(TITLE + "\n" + BODY)
        self.assertEqual(result, f"{TITLE}\n{HEADER}\n\n{BODY}")

    def test_current_header_is_left_alone(self):
        headed = f"{TITLE}\n{HEADER}\n\n{BODY}"
        self.assertIsNone(apply_header(headed))

    def test_outdated_header_is_replaced_not_stacked(self):
        outdated = f"{TITLE}\n{HEADER_MARKER}\n> An earlier wording.\n\n{BODY}"
        result = apply_header(outdated)
        self.assertEqual(result.count(HEADER_MARKER), 1)
        self.assertNotIn("An earlier wording.", result)

    def test_reworded_opening_line_is_replaced_not_stacked(self):
        """Detection matches the prefix, so changing the first line is safe."""
        previous = f"{HEADER_PREFIX} \u2014 an entirely different opening.**"
        outdated = f"{TITLE}\n{previous}\n> An earlier wording.\n\n{BODY}"
        result = apply_header(outdated)
        self.assertEqual(result.count(HEADER_PREFIX), 1)
        self.assertNotIn(previous, result)
        self.assertIn(HEADER, result)
        self.assertIn(BODY, result)

    def test_repeated_rewording_does_not_accumulate_blank_lines(self):
        document = TITLE + "\n" + BODY
        for opening in ("first", "second", "third"):
            previous = f"{HEADER_PREFIX}: {opening}.**"
            document = apply_header(document) or document
            document = document.replace(HEADER.split("\n")[0], previous, 1)
        final = apply_header(document)
        self.assertNotIn("\n\n\n", final)
        self.assertEqual(final, f"{TITLE}\n{HEADER}\n\n{BODY}")

    def test_header_contains_no_links_and_no_em_dash(self):
        self.assertNotIn("](", HEADER)
        self.assertNotIn("\u2014", HEADER)
        self.assertNotIn("user", HEADER.lower())

    def test_banner_above_the_title_keeps_its_place(self):
        banner = "> **Frozen as of 2026-09-15.**\n"
        result = apply_header(banner + "\n" + TITLE + "\n" + BODY)
        self.assertTrue(result.startswith(banner))
        self.assertLess(result.index(banner), result.index(HEADER_MARKER))
        self.assertLess(result.index(TITLE.strip()), result.index(HEADER_MARKER))

    def test_existing_scope_header_is_kept_below(self):
        scope = "> **Measured evidence — research mode.** What was run.\n"
        result = apply_header(TITLE + "\n" + scope + "\n" + BODY)
        self.assertIn(scope, result)
        self.assertLess(result.index(HEADER_MARKER), result.index(scope))

    def test_document_without_a_heading_is_reported(self):
        self.assertIsNone(apply_header(BODY))

    def test_covered_directories_and_exemptions(self):
        self.assertTrue(covered("docs/formats/weapons.md"))
        self.assertTrue(covered("docs/baselines/input.md"))
        self.assertTrue(covered("docs/ROADMAP.md"))
        self.assertFalse(covered("README.md"))
        self.assertFalse(covered("AGENTS.md"))
        self.assertFalse(covered("docs/doc-realignment-2026-09-15.md"))
        self.assertFalse(covered("docs/images/notes.txt"))
        self.assertFalse(covered("USNF-ATF/Docs/progress.md"))


if __name__ == "__main__":
    unittest.main()
