"""Synthetic material authoring tests and original pack integrity."""
import json
from pathlib import Path
import unittest

try:
    from PIL import Image
    from pack_vegetation_materials import encode, material
except ImportError:
    Image = None

ASSETS = Path(__file__).resolve().parents[1] / 'crates/tore-app/assets/vegetation'


class VegetationPackTests(unittest.TestCase):
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
