"""Quiet, low-resolution material candidate. Does not overwrite the live atlas.

Run with Blender --background --python tools/pixel_kit/clean_atlas.py.
The tile layout matches atlas.py; each material has three authored shades.
"""
import json
import struct
import zlib
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).parent))
from atlas import TILES

OUT = ROOT / 'assets/source/textures/clean_modded_candidate'
OUT.mkdir(parents=True, exist_ok=True)
RAMPS = {
    'wood': ['503a2b', '765337', '946d46'],
    'skin': ['a77357', 'cb9670', 'dfb08a'],
    'cloth': ['344953', '49616a', '617880'],
    'leather': ['4e3830', '705040', '906a4c'],
    'copper': ['774831', 'ad7045', 'd59b61'],
    'mineral': ['35655e', '65a28a', 'aed6b1'],
    'ceramic': ['a29878', 'ccc2a0', 'e4dabb'],
    'dark': ['292e35', '3d454d', '535d65'],
}
pixels = [(0, 0, 0, 255)] * 4096
def rect(name, box, shade):
    ox, oy, width, height = TILES[name]
    color = (*bytes.fromhex(RAMPS[name][shade]), 255)
    x0, y0, x1, y1 = box
    for y in range(max(y0, 0), min(y1, height)):
        for x in range(max(x0, 0), min(x1, width)):
            pixels[(oy+y)*64+ox+x] = color
for name, (_, _, width, height) in TILES.items():
    rect(name, (0, 0, width, height), 1)
# Few broad grain marks, with staggered ends. No pinstripes or random pixels.
for box, shade in [((2,3,4,20),0), ((3,20,5,30),0),
                   ((10,35,12,58),0), ((11,10,14,27),2),
                   ((3,44,6,61),2), ((6,5,8,24),0),
                   ((7,24,9,33),0), ((8,40,10,58),2)]:
    rect('wood',box,shade)
# Anatomical shading must come from planes, not a repeating painted highlight.
rect('skin',(0,0,4,32),0)
rect('skin',(15,4,21,12),2)
# A single compression fold. Leave most of the cloth field uninterrupted.
rect('cloth',(0,23,8,27),0)
rect('cloth',(8,25,16,29),0)
rect('cloth',(2,20,10,23),2)
rect('leather',(0,13,24,16),0)
rect('leather',(0,0,24,2),2)
# Copper reads from one light edge and a broad shaded lower face.
rect('copper',(0,0,16,3),2)
rect('copper',(0,12,16,16),0)
rect('ceramic',(0,0,4,16),0)
rect('ceramic',(14,0,20,16),2)
rect('mineral',(0,0,2,16),0)
rect('mineral',(5,0,8,12),2)
rect('dark',(0,13,24,16),0)

def png(path, data, width, height):
    def chunk(kind, payload):
        return struct.pack('>I', len(payload))+kind+payload+struct.pack('>I',zlib.crc32(kind+payload))
    raw = bytes(c for pixel in data for c in pixel)
    scan = b''.join(b'\0'+raw[y*width*4:(y+1)*width*4] for y in range(height))
    path.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',width,height,8,6,0,0,0))+
                     chunk(b'IDAT',zlib.compress(scan))+chunk(b'IEND',b''))
png(OUT/'kit.png',pixels,64,64)
(OUT/'kit.rgba').write_bytes(bytes(c for pixel in pixels for c in pixel))
(OUT/'palette.json').write_text(json.dumps({'palette':RAMPS,'tiles':TILES,
    'status':'candidate; requires mapped model and native review',
    'direction':'clean modded Minecraft; three shades per material; nearest filtering; no noise'},indent=2))
# Direct comparison of real source pixels, nearest-neighbour enlarged.
import bpy
old = bpy.data.images.load(str(ROOT/'assets/viewmodel/kit.png'),check_existing=False) if (ROOT/'assets/viewmodel/kit.png').exists() else None
if old:
    old_pixels=list(old.pixels[:])
    old_width, old_height=old.size[:]
    # Blender stores image rows bottom-up and in scene-linear floating point.
    # Use the original PNG through packed raw source instead when available.
    raw_path=ROOT/'assets/viewmodel/kit.rgba'
    if raw_path.exists() and raw_path.stat().st_size==16384:
        raw=raw_path.read_bytes()
        previous=[tuple(raw[i:i+4]) for i in range(0,len(raw),4)]
        board=[]
        for y in range(512):
            for x in range(1040):
                board.append(previous[(y//8)*64+x//8] if x<512 else
                    pixels[(y//8)*64+(x-528)//8] if x>=528 else (28,30,34,255))
        png(OUT/'comparison_current_left_candidate_right.png',board,1040,512)
print('Clean atlas saved:',OUT)
