#!/usr/bin/env python3
"""Rebuild John's Sahara and top-view cutouts. Authoring only: Pillow and numpy.

Sahara has a smooth coloured matte. Fit a local cubic RGB background to flat
pixels, robustly reject sprite pixels, then retain textured outline components.
Never key this sheet as black. Sources are supplied art, never retail media.
"""
import argparse
from collections import deque
import hashlib
import json
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

# Native 1536x1024 coordinates, with clear background around each outline.
SAHARA_SIDE = [
 ('date_palm',(12,85,179,391)), ('date_cluster',(149,92,322,392)),
 ('date_medium',(321,153,466,391)), ('date_small',(465,226,574,392)),
 ('acacia',(563,159,789,393)), ('acacia_small',(785,238,1004,393)),
 ('green_shrub',(1004,265,1159,393)), ('dry_shrub',(1159,278,1279,393)),
 ('bare_tree',(1280,192,1424,393)), ('bare_shrub',(1418,290,1532,393)),
 ('saguaro',(16,398,137,605)), ('agave',(138,454,276,605)),
 ('agave_small',(274,492,374,605)), ('shrub',(375,473,516,605)),
 ('brush',(516,478,643,605)), ('outcrop',(650,455,865,611)),
 ('spire',(865,442,1019,611)), ('boulders',(1019,476,1167,611)),
 ('boulder',(1167,502,1292,611)), ('stones',(1292,512,1422,611)),
 ('small_stones',(1422,527,1528,611)),
]
SAHARA_TOP_BOXES = [
 (10,626,179,811),(178,629,322,811),(326,652,474,811),(477,665,593,811),
 (599,640,791,817),(802,666,990,811),(993,663,1146,814),(1146,672,1278,814),
 (1280,659,1417,816),(1417,700,1532,816),
 (17,824,142,982),(143,837,282,985),(284,862,405,984),(405,852,520,986),
 (520,851,642,986),(674,837,896,999),(896,844,1036,999),(1036,866,1188,993),
 (1188,875,1295,987),(1295,882,1415,985),(1415,886,1526,982),
]
# Coordinates on the 2048-wide review. IDs intentionally match pack_layout.rs.
TOP_BOXES = [
 (25,390,170,548),(185,411,313,548),(320,390,463,548),(190,550,318,665),
 (27,229,186,389),(322,550,459,665),(35,20,239,219),(256,47,407,214),
 (432,73,598,214),(653,14,890,258),(891,21,1116,254),(1110,25,1331,254),
 (682,260,841,415),(844,260,996,416),(997,251,1155,416),(1157,245,1319,416),
 (693,534,876,672),(882,541,1015,672),(1020,556,1110,653),
 (1376,13,1634,254),(1634,25,1801,250),(1800,13,2046,263),
 (1380,249,1576,417),(1575,255,1733,418),(1734,255,1881,418),
 (1881,255,2035,420),(1383,532,1575,674),(1574,554,1700,674),
 (1768,542,2036,676),(331,252,467,389),(1695,578,1771,657),
]


def components(mask, minimum=12, reject_border=False):
    h,w = mask.shape
    seen = np.zeros_like(mask)
    out = np.zeros_like(mask)
    for y,x in zip(*np.nonzero(mask)):
        if seen[y,x]:
            continue
        queue = [(int(y),int(x))]
        seen[y,x] = True
        group = []
        while queue:
            cy,cx = queue.pop()
            group.append((cy,cx))
            for dy,dx in ((0,1),(0,-1),(1,0),(-1,0)):
                ny,nx = cy+dy,cx+dx
                if 0<=ny<h and 0<=nx<w and mask[ny,nx] and not seen[ny,nx]:
                    seen[ny,nx] = True
                    queue.append((ny,nx))
        touches = any(cy in (0,h-1) or cx in (0,w-1) for cy,cx in group)
        if len(group)>=minimum and not (reject_border and touches):
            for cy,cx in group:
                out[cy,cx] = True
    return out


def local_matte(image):
    """Local smooth RGB estimate, without colour assumptions about the plant."""
    rgb = np.array(image.convert('RGB'), dtype=np.float64)
    h,w,_ = rgb.shape
    y,x = np.mgrid[-1:1:complex(h), -1:1:complex(w)]
    basis = np.stack([x**i*y**j for i in range(4) for j in range(4-i)],axis=2)
    # Smooth glow changes slowly; textured foliage/trunks do not.
    lo = np.array(image.convert('RGB').filter(ImageFilter.MinFilter(5)),dtype=float)
    hi = np.array(image.convert('RGB').filter(ImageFilter.MaxFilter(5)),dtype=float)
    flat = np.max(hi-lo,axis=2)<9
    border = np.zeros((h,w),dtype=bool)
    border[:4,:]=border[-4:,:]=True
    border[:,:4]=border[:,-4:]=True
    keep = flat.copy()
    # Robust regression discards constant-colour sprite interiors as outliers.
    for _ in range(7):
        fit = keep & ((np.indices((h,w)).sum(axis=0)%3)==0)
        coef = np.linalg.lstsq(basis[fit],rgb[fit],rcond=None)[0]
        bg = basis@coef
        residual = np.max(np.abs(rgb-bg),axis=2)
        keep = flat & ((residual<10) | (border & (residual<18)))
    return rgb, bg


def clean_sahara(image, trim=True):
    rgb,bg = local_matte(image)
    residual = np.max(np.abs(rgb-bg),axis=2)
    # A strict core, grown only through pixels unlike their local background.
    core = residual>=22
    possible = residual>=12
    core = components(core,18)
    mask = core.copy()
    queue = deque(zip(*np.nonzero(core)))
    h,w = mask.shape
    while queue:
        y,x=queue.popleft()
        for dy,dx in ((0,1),(0,-1),(1,0),(-1,0)):
            ny,nx=y+dy,x+dx
            if 0<=ny<h and 0<=nx<w and possible[ny,nx] and not mask[ny,nx]:
                mask[ny,nx]=True
                queue.append((ny,nx))
    # Smooth matte errors can have a large residual but have no pixel-art edge.
    # Retain textured outlines and their enclosed interiors, not those smooth blobs.
    lo=np.array(image.convert('RGB').filter(ImageFilter.MinFilter(3)),dtype=int)
    hi=np.array(image.convert('RGB').filter(ImageFilter.MaxFilter(3)),dtype=int)
    textured=(hi-lo).max(axis=2)>=10
    barrier=textured & mask
    outside=np.zeros_like(mask)
    queue=deque([(y,x) for y in range(h) for x in range(w)
                 if y in (0,h-1) or x in (0,w-1)])
    while queue:
        y,x=queue.popleft()
        if outside[y,x] or barrier[y,x]:continue
        outside[y,x]=True
        for dy,dx in ((0,1),(0,-1),(1,0),(-1,0)):
            ny,nx=y+dy,x+dx
            if 0<=ny<h and 0<=nx<w and not outside[ny,nx]:queue.append((ny,nx))
    mask &= ~outside
    mask=components(mask, max(18,int(mask.sum()*.02)))
    # No semi-transparent matte or smooth glow fringes survive. Edge pixels
    # must differ by at least 12 RGB levels from the fitted local matte.
    rgba=np.zeros((h,w,4),dtype=np.uint8)
    rgba[mask,:3]=rgb[mask].astype(np.uint8)
    rgba[mask,3]=255
    out=Image.fromarray(rgba)
    bounds=out.getbbox()
    if bounds is None:
        raise ValueError('empty Sahara sprite')
    return out.crop(bounds) if trim else out


def clean_top(image, trim=True):
    rgba=np.array(image.convert('RGBA'))
    rgb=rgba[:,:,:3].astype(int)
    mask=(rgba[:,:,3]>=128) & (rgb.max(axis=2)>=24)
    # The outline includes dim grey/black and antialiased pixels, not just white.
    # Limit colour rejection to a four-pixel edge band so bright stone faces and
    # dark interior bark survive. One more pixel removes mixed matte colours.
    alpha=Image.fromarray(mask.astype(np.uint8)*255)
    edge=~(np.array(alpha.filter(ImageFilter.MinFilter(9)))>0)
    chroma=rgb.max(axis=2)-rgb.min(axis=2)
    outline=((rgb.min(axis=2)>155) & (chroma<38)) | ((rgb.max(axis=2)<100) & (chroma<16))
    mask &= ~(edge & outline)
    mask=np.array(Image.fromarray(mask.astype(np.uint8)*255).filter(ImageFilter.MinFilter(3)))>0
    mask=components(mask,max(12,int(mask.sum()*.02)))
    rgba[~mask]=0
    rgba[mask,3]=255
    out=Image.fromarray(rgba)
    bounds=out.getbbox()
    if bounds is None:
        raise ValueError('empty top sprite')
    return out.crop(bounds) if trim else out


def top_cell(sprite):
    """Trim again after downsampling: nearest sampling can miss a thin tip."""
    tile = sprite.resize((124,124),Image.Resampling.NEAREST)
    for _ in range(124):
        bounds = tile.getchannel('A').getbbox()
        if bounds == (0,0,124,124):
            return tile
        if bounds is None:
            raise ValueError('empty top cell')
        tile = tile.crop(bounds).resize((124,124),Image.Resampling.NEAREST)
    raise ValueError('top cell bounds did not converge')


def build(sahara, top, output):
    source=Image.open(sahara).convert('RGBA')
    overhead=Image.open(top).convert('RGBA')
    output.mkdir(parents=True,exist_ok=True)
    sprites=[]
    for name,box in SAHARA_SIDE:
        sprite=clean_sahara(source.crop(box))
        if name in ('date_palm','date_cluster','date_medium','date_small','acacia','acacia_small'):
            data=np.array(sprite); mask=components(data[:,:,3]>0,max(30,int((data[:,:,3]>0).sum()*.05)))
            data[~mask]=0; sprite=Image.fromarray(data); sprite=sprite.crop(sprite.getbbox())
        sprites.append(('sahara_'+name,sprite))
    for (name,_),box in zip(SAHARA_SIDE,SAHARA_TOP_BOXES):
        sprites.append(('sahara_top_'+name,clean_sahara(source.crop(box))))
    from slice_vegetation import SPRITES
    for (name,_),box in zip(SPRITES,TOP_BOXES):
        scaled=tuple(round(v*overhead.width/2048) for v in box)
        sprites.append(('top_'+name,clean_top(overhead.crop(scaled))))
    pages=[Image.new('RGBA',(256,256)) for _ in range((len(sprites)+3)//4)]
    records=[]
    review=Image.new('RGB',(8*160,((len(sprites)+7)//8)*160),(85,95,110))
    for i,(name,sprite) in enumerate(sprites):
        sprite.save(output/(name+'.png'))
        tile=(top_cell(sprite) if name.startswith('top_') or name.startswith('sahara_top_')
              else sprite.resize((124,124),Image.Resampling.NEAREST))
        pages[i//4].paste(tile,((i%2)*128+2,(i%4//2)*128+2))
        records.append(dict(name=name,page=i//4,tile=i%4,radius=sprite.width/sprite.height/2))
        thumb=sprite.copy();thumb.thumbnail((148,148),Image.Resampling.NEAREST)
        review.paste(thumb,((i%8)*160+6,(i//8)*160+6),thumb)
    review.save(output/'review.png')
    for i,p in enumerate(pages):p.save(output/f'page-{i}.png')
    (output/'pack.rgba').write_bytes(b''.join(p.tobytes() for p in pages))
    (output/'sprites.json').write_text(json.dumps(records,indent=2)+'\n')
    layout='// Generated from John-supplied art by tools/slice_vegetation_pass5.py.\n'
    layout+='const EXTRA: &[(&str, f32)] = &[\n'
    layout+=''.join(f'    ("{r["name"]}", {r["radius"]:.6f}),\n' for r in records)
    (output/'layout.rs').write_text(layout+'];\n')
    print('source SHA256',*[hashlib.sha256(p.read_bytes()).hexdigest() for p in (sahara,top)])
    print(len(sprites),'sprites;',len(pages),'pages')


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('sahara',type=Path);parser.add_argument('top',type=Path);parser.add_argument('output',type=Path)
    args=parser.parse_args();build(args.sahara,args.top,args.output)
