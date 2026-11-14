"""Original ID/metal/emission-eligibility atlas; not consumed by runtime yet."""
import json, struct, zlib
from pathlib import Path

root=Path(__file__).resolve().parents[2]
out=root/'assets/source/wand_ecosystem/minimal_v2'
names=['bone','iron','cord','brass','charcoal','ember','leather','rune']
raw=bytearray(256*256*4)
for i,name in enumerate(names):
    for y in range((i//4)*64,(i//4+1)*64):
        for x in range((i%4)*32,(i%4+1)*32):
            p=(y*256+x)*4
            raw[p:p+4]=bytes([(i+1)*28,255 if name in ('iron','brass') else 0,
                             255 if name in ('ember','rune') else 0,255])
def chunk(t,d):return struct.pack('>I',len(d))+t+d+struct.pack('>I',zlib.crc32(t+d))
(out/'material_masks.png').write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',256,256,8,6,0,0,0))+
    chunk(b'IDAT',zlib.compress(b''.join(b'\0'+raw[y*1024:(y+1)*1024] for y in range(256))))+chunk(b'IEND',b''))
(out/'material_masks.json').write_text(json.dumps({'R':{n:(i+1)*28 for i,n in enumerate(names)},
    'G':'metal eligibility','B':'emission eligibility, not permanent emission',
    'runtime':'Not wired to the current material-name renderer'},indent=2))
print('ECOSYSTEM_MATERIAL_MASKS_SAVED')
