"""Synthetic matte tests and checks on the John-supplied generated asset only."""
import json
from pathlib import Path
import unittest

try:
    from PIL import Image
    from slice_vegetation import clean
except ImportError:
    Image = None

ASSETS = Path(__file__).resolve().parents[1] / 'crates/tore-app/assets/vegetation'


class VegetationPackTests(unittest.TestCase):
    def test_atlas_has_cutouts_gutters_and_no_generator_fringe(self):
        raw = (ASSETS/'pack.rgba').read_bytes()
        self.assertEqual(len(raw), 8*256*256*4)
        opaque = 0
        for at in range(0,len(raw),4):
            r,g,b,a = raw[at:at+4]
            self.assertIn(a,(0,255))
            if not a:
                self.assertEqual((r,g,b),(0,0,0))
            else:
                opaque += 1
                self.assertFalse(r > 120 and g < 65 and b < 65 and r > 3*max(g,1))
                self.assertFalse(g > 65 and r < 35 and b < 35)
                self.assertFalse(r > 200 and g > 200 and b < 40)
            x,y = (at//4)%256, ((at//4)//256)%256
            if x%128 < 2 or x%128 >= 126 or y%128 < 2 or y%128 >= 126:
                self.assertEqual(a,0)
        self.assertGreater(opaque,100000)
        records = json.loads((ASSETS/'sprites.json').read_text())
        self.assertEqual(len(records),31)
        self.assertEqual(len({(r['page'],r['tile']) for r in records}),31)
        for record in records:
            self.assertGreater(record['radius'],0)

    @unittest.skipIf(Image is None,'Pillow needed only for rebuilding/synthetic slicer tests')
    def test_key_trim_and_fringe_preserve_natural_green_and_orange(self):
        source = Image.new('RGB',(32,24))
        for y in range(4,20):
            for x in range(4,28):
                source.putpixel((x,y),(255,0,0) if x < 7 else (0,255,0) if x > 24 else (255,255,0) if y < 7 else (190,105,25) if x < 16 else (65,110,35))
        result = clean(source)
        colors = list(result.getdata())
        self.assertLess(result.width,source.width)
        self.assertLess(result.height,source.height)
        self.assertIn((190,105,25,255),colors)
        self.assertIn((65,110,35,255),colors)
        self.assertNotIn((255,0,0,255),colors)
        self.assertNotIn((0,255,0,255),colors)
        self.assertNotIn((255,255,0,255),colors)


if __name__ == '__main__':
    unittest.main()
