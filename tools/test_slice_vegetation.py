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

class Pass5AtlasTests(unittest.TestCase):
    def test_top_opaque_bounds_fill_cells_after_cleanup_and_resampling(self):
        directory=ASSETS/'pass5'
        raw=(directory/'pack.rgba').read_bytes()
        records=json.loads((directory/'sprites.json').read_text())
        for record in records:
            if not (record['name'].startswith('top_') or record['name'].startswith('sahara_top_')):
                continue
            page=record['page']*65536*4
            x=record['tile']%2*128; y=record['tile']//2*128
            opaque=[(dx,dy) for dy in range(128) for dx in range(128)
                    if raw[page+((y+dy)*256+x+dx)*4+3]]
            self.assertTrue(opaque,record['name'])
            self.assertEqual((min(p[0] for p in opaque),min(p[1] for p in opaque),
                              max(p[0] for p in opaque)+1,max(p[1] for p in opaque)+1),
                             (2,2,126,126),record['name'])

    @unittest.skipIf(Image is None,'Pillow needed for authoring tests')
    def test_top_cell_retrims_tips_lost_during_nearest_downsampling(self):
        try:
            from slice_vegetation_pass5 import clean_top, top_cell
        except ImportError:
            self.skipTest('numpy needed for authoring tests')
        source=Image.new('RGBA',(256,256),(0,0,0,255))
        for y in range(40,216):
            for x in range(30,226):source.putpixel((x,y),(35,90,30,255))
        cleaned=clean_top(source)
        self.assertEqual(cleaned.getchannel('A').getbbox(),(0,0,cleaned.width,cleaned.height))
        padded=Image.new('RGBA',(256,256))
        padded.paste(cleaned,(30,40))
        tile=top_cell(padded)
        self.assertEqual(tile.getchannel('A').getbbox(),(0,0,124,124))
        self.assertEqual(tile.getpixel((0,0)),(35,90,30,255))

    @unittest.skipIf(Image is None,'Pillow needed for authoring tests')
    def test_top_outline_cleanup_preserves_crown_bark_and_stone_interior(self):
        try:
            from slice_vegetation_pass5 import clean_top
        except ImportError:
            self.skipTest('numpy needed for authoring tests')
        source=Image.new('RGBA',(48,48),(0,0,0,255))
        # White, antialiased grey and dark rings around a coloured crown.
        for inset,color in [(5,(220,220,220,255)),(6,(110,115,112,255)),
                            (7,(32,32,32,255)),(8,(30,80,38,255))]:
            for y in range(inset,48-inset):
                for x in range(inset,48-inset):source.putpixel((x,y),color)
        for y in range(18,30):
            for x in range(18,24):source.putpixel((x,y),(38,32,28,255))
            for x in range(24,30):source.putpixel((x,y),(210,210,205,255))
        result=clean_top(source,trim=False)
        self.assertTrue(all(result.getpixel((x,24))==(0,0,0,0) for x in range(8)))
        self.assertEqual(result.getpixel((12,24)),(30,80,38,255))
        self.assertEqual(result.getpixel((20,24)),(38,32,28,255))
        self.assertEqual(result.getpixel((26,24)),(210,210,205,255))
        self.assertTrue(all(p[3] in (0,255) and (p[3] or p==(0,0,0,0)) for p in result.getdata()))

    def test_sahara_and_top_atlas_cutouts_and_gutters(self):
        directory=ASSETS/'pass5'
        raw=(directory/'pack.rgba').read_bytes()
        records=json.loads((directory/'sprites.json').read_text())
        self.assertEqual(len(records),73)
        self.assertEqual(len(raw),19*256*256*4)
        for at in range(0,len(raw),4):
            r,g,b,a=raw[at:at+4]
            self.assertIn(a,(0,255))
            if not a:self.assertEqual((r,g,b),(0,0,0))
            x,y=(at//4)%256,((at//4)//256)%256
            if x%128<2 or x%128>=126 or y%128<2 or y%128>=126:self.assertEqual(a,0)
        for i in range(73):
            page=i//4*65536*4; x=i%2*128+2; y=i%4//2*128+2
            self.assertTrue(any(raw[page+((y+dy)*256+x+dx)*4+3]
                                for dy in range(124) for dx in range(124)),records[i]['name'])

    @unittest.skipIf(Image is None,'Pillow needed for authoring tests')
    def test_local_glow_is_removed_without_keying_dark_bark_or_green_leaves(self):
        try:
            import numpy as np
            from slice_vegetation_pass5 import clean_sahara
        except ImportError:
            self.skipTest('numpy needed only for the local matte fit')
        source=Image.new('RGB',(96,128))
        for y in range(128):
            for x in range(96):
                source.putpixel((x,y),(40+x//5+y//6,50+x//8+y//12,20+y//10))
        original=np.array(source)
        # A clean silhouette with dark bark and textured green crown, on glow.
        for y in range(25,75):
            for x in range(20,76):source.putpixel((x,y),(35+(x%5)*4,100+(y%5)*6,25))
        for y in range(75,113):
            for x in range(43,53):source.putpixel((x,y),(55,30,14))
        result=np.array(clean_sahara(source,trim=False))
        changed=(np.array(source)!=original).any(axis=2)
        self.assertFalse((result[~changed,3]>0).any(),'smooth local glow must all be transparent')
        self.assertGreater((result[changed,3]>0).mean(),.90)
        self.assertEqual(tuple(result[95,47]),(55,30,14,255))
