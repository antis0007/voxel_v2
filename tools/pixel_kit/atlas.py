"""Small, editable pixel clusters. No noise, AI resampling, or baked illumination."""
from pathlib import Path
import struct
import zlib

SIZE = 64
TILES = {
    'wood': (0, 0, 16, 64), 'skin': (16, 0, 24, 32),
    'cloth': (40, 0, 24, 32), 'leather': (16, 32, 24, 16),
    'copper': (40, 32, 16, 16), 'mineral': (56, 32, 8, 16),
    'ceramic': (16, 48, 24, 16), 'dark': (40, 48, 24, 16),
}
RAMPS = {
    'wood': ['302b32','49382f','644535','80573a','a27149'],
    'skin': ['79534c','9e6b56','bd896c','d29e7a','e4b58b'],
    'cloth': ['353849','41465b','505770','60667b','777d8f'],
    'leather': ['543c35','775037','97683e','b78448','c79b5d'],
    'copper': ['563a36','81503b','ae704c','cf9466','e3b785'],
    'mineral': ['29483f','3d7463','65a58b','9cceb0','d2e5c1'],
    'ceramic': ['74614f','9f8969','c0a981','ddc79e','efdeb8'],
    'dark': ['242a31','303741','444b54','565c63','6b7379'],
}

def make(directory):
    pixels = [(255, 0, 255, 255)] * (SIZE * SIZE)
    def rect(material, box, level):
        ox, oy, width, height = TILES[material]
        color = tuple(bytes.fromhex(RAMPS[material][level])) + (255,)
        x0,y0,x1,y1 = box
        for y in range(max(0,y0),min(height,y1)):
            for x in range(max(0,x0),min(width,x1)):
                pixels[(oy+y)*SIZE+ox+x] = color
    for material, (_,_,w,h) in TILES.items():
        rect(material,(0,0,w,h),2)
    # Long connected grain islands with a few forks and a single knot.
    for box, level in [((0,0,3,64),1),((3,0,5,21),3),((4,21,6,44),3),
                       ((3,44,5,64),3),((9,0,11,31),1),((10,31,12,64),1),
                       ((13,3,15,25),3),((13,38,15,60),3),((5,8,6,17),4),
                       ((7,23,10,25),1),((6,25,8,29),1),((8,28,10,30),3),
                       ((1,33,2,49),0),((11,45,12,53),0)]:
        rect('wood',box,level)
    # Skin is deliberately quiet; silhouettes and two broad warmth clusters do the work.
    for box,level in [((0,0,6,32),1),((6,2,9,28),2),((9,3,19,27),3),
                      ((11,5,17,11),4),((18,12,23,25),2),((7,27,19,30),1),
                      ((5,28,9,30),0),((20,0,24,5),2),((7,20,10,23),1)]:
        rect('skin',box,level)
    # One diagonal cloth fold and one smaller compression shadow, not stripes.
    for box,level in [((0,0,4,32),1),((4,0,8,11),3),((8,7,12,15),3),
                      ((12,12,18,19),3),((18,16,24,23),3),((0,12,6,17),0),
                      ((6,16,11,21),1),((11,21,17,25),1),((17,25,24,28),1),
                      ((3,2,6,7),4)]:
        rect('cloth',box,level)
    for box,level in [((0,0,24,2),1),((0,2,24,4),3),((0,13,24,16),1),
                      ((3,5,8,7),3),((12,6,18,8),3),((7,9,13,10),1),
                      ((19,9,23,11),1)]:
        rect('leather',box,level)
    for box,level in [((0,0,16,2),1),((0,2,16,4),4),((0,4,16,6),3),
                      ((0,13,16,16),0),((1,11,15,13),1),((2,6,4,9),1),
                      ((12,7,14,10),3)]:
        rect('copper',box,level)
    for box,level in [((0,0,4,16),1),((4,0,8,16),2),((8,0,21,13),3),
                      ((10,2,18,6),4),((21,0,24,16),2),((8,13,24,16),2),
                      ((5,9,7,12),0),((7,11,10,12),1)]:
        rect('ceramic',box,level)
    for box,level in [((0,0,2,16),0),((2,2,4,13),2),((4,1,7,11),3),
                      ((4,2,6,5),4),((2,13,7,16),1)]:
        rect('mineral',box,level)
    raw = bytes(c for p in pixels for c in p)
    directory.mkdir(parents=True,exist_ok=True)
    (directory/'kit.rgba').write_bytes(raw)
    def chunk(kind,data):
        return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
    scan = b''.join(b'\0'+raw[y*256:(y+1)*256] for y in range(SIZE))
    png = b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',64,64,8,6,0,0,0))
    (directory/'kit.png').write_bytes(png+chunk(b'IDAT',zlib.compress(scan))+chunk(b'IEND',b''))
    (directory/'palette.gpl').write_text('GIMP Palette\nName: Lowered city workshop\nColumns: 5\n#\n'+
        '\n'.join(' '.join(str(v) for v in bytes.fromhex(c))+' '+m+str(i)
                  for m,ramp in RAMPS.items() for i,c in enumerate(ramp))+'\n')
