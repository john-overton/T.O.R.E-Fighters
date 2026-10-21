"""Synthetic material authoring tests and original pack integrity."""
import json
from pathlib import Path
import unittest

try:
    from PIL import Image
    from pack_vegetation_materials import defringe, edge_audit, encode, material
except ImportError:
    Image = None

ASSETS = Path(__file__).resolve().parents[1] / 'crates/tore-app/assets/vegetation'


class VegetationPackTests(unittest.TestCase):
    @unittest.skipIf(Image is None, 'Pillow required for authoring only')
    def test_white_matte_is_darkened_without_eroding_thin_branches(self):
        for kind in ('dead', 'dry_bush', 'dry_scrub', 'acacia', 'birch',
                     'fern', 'yucca', 'stone'):
            for alpha in (128, 200, 255):
                with self.subTest(kind=kind, alpha=alpha):
                    source = Image.new('RGBA', (8, 8))
                    for x in range(2, 6):
                        source.putpixel((x, 4), (96, 96, 96, 255))
                    source.putpixel((3, 3), (245, 240, 238, alpha))
                    before = source.tobytes()
                    self.assertEqual(len(edge_audit(source, kind)[1]), 1)
                    fixed = defringe(source, kind)
                    self.assertEqual(fixed.getpixel((3, 3)), (96, 96, 96, alpha))
                    self.assertEqual(edge_audit(fixed, kind), ([], {}))
                    self.assertEqual(source.tobytes(), before)
                    self.assertEqual(fixed.getchannel('A').tobytes(),
                                     source.getchannel('A').tobytes())
                    self.assertEqual(defringe(fixed, kind).tobytes(), fixed.tobytes())

    @unittest.skipIf(Image is None, 'Pillow required for authoring only')
    def test_defringe_preserves_highlights_and_clean_art_byte_for_byte(self):
        for kind, highlight, dark in (
                ('dead', (250, 150, 60, 255), (96, 50, 20, 255)),
                ('birch', (245, 245, 245, 255), (220, 220, 220, 255)),
                ('boulder', (233, 227, 221, 255), (96, 96, 96, 255))):
            source = Image.new('RGBA', (8, 8))
            source.paste(dark, (2, 2, 6, 6))
            source.putpixel((3, 2), highlight)
            # A bright interior pixel is never a silhouette fringe.
            source.putpixel((4, 4), (255, 255, 255, 255))
            self.assertFalse(edge_audit(source, kind)[1])
            self.assertEqual(defringe(source, kind).tobytes(), source.tobytes())

    @unittest.skipIf(Image is None, 'Pillow required for authoring only')
    def test_encode_audits_the_sampled_art_and_keeps_aspect(self):
        source = Image.new('RGBA', (80, 80))
        source.paste((96, 96, 96, 255), (0, 2, 80, 80))
        source.putpixel((40, 1), (250, 250, 250, 255))
        source.putpixel((20, 0), (96, 96, 96, 255))
        audit = {}
        cell, aspect = encode(source, 'dead', audit)
        self.assertGreater(audit['white_matte_before'], 0)
        self.assertEqual(audit['white_matte_after'], 0)
        self.assertEqual(aspect, 1)
        self.assertTrue(all(p == 255 or p // 32 == 1
                            for p in cell.get_flattened_data()))

    def test_all_original_pairs_have_bounded_materials_and_gutters(self):
        raw = (ASSETS / 'pack.materials').read_bytes()
        records = json.loads((ASSETS / 'sprites.json').read_text())
        self.assertEqual(len(records), 54)
        self.assertEqual(len(raw), 27 * 65536)
        self.assertEqual(len({r['name'] for r in records}), 54)
        self.assertEqual({r['variant'] for r in records}, {1, 2, 3})
        for r in records:
            self.assertGreater(r['radius'], 0)
            self.assertGreater(r['top_aspect'], 0)
            for slot in (r['side'], r['top']):
                opaque = []
                for y in range(128):
                    for x in range(128):
                        p = raw[slot // 4 * 65536 + (slot % 4 // 2 * 128 + y) * 256 + slot % 2 * 128 + x]
                        self.assertTrue(p == 255 or p < 128)
                        if x < 2 or y < 2 or x >= 126 or y >= 126:
                            self.assertEqual(p, 255)
                        if p != 255:
                            opaque.append((x, y))
                self.assertTrue(opaque, r['name'])
                self.assertLess(len(opaque), 124 * 124, r['name'])
                self.assertEqual((min(y for _, y in opaque), max(y for _, y in opaque)), (2, 125))

    @unittest.skipIf(Image is None, 'Pillow required for authoring only')
    def test_material_ids_keep_wood_separate_from_leaves_and_alpha(self):
        source = Image.new('RGBA', (24, 32))
        for y in range(4, 24):
            for x in range(4, 20):
                source.putpixel((x, y), (160, 160, 160, 255))
        for y in range(24, 30):
            for x in range(10, 14):
                source.putpixel((x, y), (180, 80, 20, 255))
        encoded, aspect = encode(source, 'broadleaf')
        values = set(encoded.get_flattened_data())
        self.assertIn(255, values)
        self.assertEqual({p // 32 for p in values if p != 255}, {0, 1})
        self.assertAlmostEqual(aspect, 16 / 26)
        self.assertEqual(material('stone', (180, 80, 20)), 2)
        self.assertEqual(material('dry_scrub', (160, 160, 160)), 3)

    @unittest.skipIf(Image is None, 'Pillow required for authoring only')
    def test_empty_cutout_rejected_and_shading_changes_without_changing_material(self):
        with self.assertRaises(ValueError):
            encode(Image.new('RGBA', (16, 16)), 'fern')
        dark, _ = encode(Image.new('RGBA', (16, 16), (40, 40, 40, 255)), 'fern')
        light, _ = encode(Image.new('RGBA', (16, 16), (200, 200, 200, 255)), 'fern')
        self.assertLess(dark.getpixel((64, 64)), light.getpixel((64, 64)))
        self.assertLess(light.getpixel((64, 64)), 32)


if __name__ == '__main__':
    unittest.main()
