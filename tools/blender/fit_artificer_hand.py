"""Fit the saved hand_v3 to the tally wand; preserve the earlier artist source."""
from pathlib import Path
import bpy, bmesh, json, struct, zlib
from mathutils import Vector
ROOT=Path(__file__).resolve().parents[2]
OUT=ROOT/'assets/source/wand_ecosystem/artificer_tally'
RUNTIME=ROOT/'assets/runtime/viewmodels/artificer_tally'
bpy.ops.wm.open_mainfile(filepath=str(ROOT/'assets/source/wand_ecosystem/hand_v3/hand_continuity.blend'))
rig=bpy.data.objects['WayfarerRig'];hand=bpy.data.collections['Plane hand']
thumb=bpy.data.objects['Opposing thumb planes']
for start in range(0,len(thumb.data.vertices),8):
    ring=list(thumb.data.vertices)[start:start+8]
    centre=sum((v.co for v in ring),Vector())/8
    fitted=centre.copy();fitted.z=.009+(centre.z-.009)*.72
    for v in ring:v.co=fitted+(v.co-centre)*.94
# The inherited cuff was a capped solid. Its front disk covered the wrist in
# inspection. Model a real opening and rolled rim while retaining skin weights.
cuff=next(obj for obj in hand.objects if obj.name.startswith('Leather cuff'))
bm=bmesh.new();bm.from_mesh(cuff.data)
deform=bm.verts.layers.deform.verify();uv=bm.loops.layers.uv.active
forearm=cuff.vertex_groups['Forearm'].index
caps=[face for face in bm.faces if len(face.verts)>4]
for cap in caps:
    outer=list(cap.verts);tex=[loop[uv].uv.copy() for loop in cap.loops]
    centre=sum((v.co for v in outer),Vector())/len(outer)
    inner=[]
    for vertex in outer:
        v=bm.verts.new(centre+(vertex.co-centre)*.73);v[deform][forearm]=1.;inner.append(v)
    bm.faces.remove(cap)
    for i in range(len(outer)):
        j=(i+1)%len(outer)
        face=bm.faces.new((outer[i],outer[j],inner[j],inner[i]))
        face.smooth=False
        for loop,coord in zip(face.loops,(tex[i],tex[j],tex[j],tex[i])):loop[uv].uv=coord
bm.normal_update();bm.to_mesh(cuff.data);bm.free();cuff.data.update()
# The cloth had a second hidden end disk behind the cuff. Open that end too,
# and extend the skin just inside the garment so the wrist visibly enters it.
sleeve=next(obj for obj in hand.objects if obj.name.startswith('Tailored sleeve'))
bm=bmesh.new();bm.from_mesh(sleeve.data)
caps=[face for face in bm.faces if len(face.verts)>4]
front=min(caps,key=lambda face:(face.calc_center_median()-Vector((.105,-.070,0))).length)
bm.faces.remove(front);bm.normal_update();bm.to_mesh(sleeve.data);bm.free();sleeve.data.update()
palm=bpy.data.objects['Wrist and metacarpal planes']
underlap=Vector((.130,-.137,-.015)).normalized()*.012
for vertex in list(palm.data.vertices)[:8]:vertex.co+=underlap
paint=bytearray((ROOT/'assets/source/wand_ecosystem/minimal_v2/hand_atlas.rgba').read_bytes())
# Quiet leather cuff: its silhouette carries the folded edge; remove repeated
# diagonal bands that looked like a knurled metal bracelet in native captures.
for y in range(128):
    for x in range(64):
        color='4b4940' if y<8 or y>119 else '7b7862' if 24<=x<30 else '626450'
        at=(y*256+64+x)*4;paint[at:at+4]=bytes.fromhex(color)+b'\xff'
(RUNTIME/'hand_atlas.rgba').write_bytes(paint)
def chunk(t,d):return struct.pack('>I',len(d))+t+d+struct.pack('>I',zlib.crc32(t+d))
rows=b''.join(b'\0'+paint[y*1024:(y+1)*1024] for y in range(256))
(OUT/'hand_albedo.png').write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',256,256,8,6,0,0,0))+
    chunk(b'IDAT',zlib.compress(rows))+chunk(b'IEND',b''))
image=bpy.data.images.load(str(OUT/'hand_albedo.png'));image.pack()
for obj in hand.objects:
    for mat in obj.data.materials:
        for node in mat.node_tree.nodes:
            if node.type=='TEX_IMAGE':node.image=image
for col in bpy.context.scene.collection.children:
    if col.name in ('bone_needle','runic_key','cinder_rod'):col.hide_render=True
bpy.ops.object.select_all(action='DESELECT');rig.hide_set(False);rig.select_set(True);bpy.context.view_layer.objects.active=rig
for obj in hand.objects:
    if obj.type=='MESH':obj.hide_set(False);obj.select_set(True)
bpy.ops.wm.save_as_mainfile(filepath=str(OUT/'fitted_hand.blend'))
bpy.ops.export_scene.gltf(filepath=str(RUNTIME/'hand.glb'),export_format='GLB',use_selection=True,
    export_animations=True,export_animation_mode='ACTIONS',export_skins=True,export_def_bones=False,
    export_force_sampling=True,export_frame_range=False,export_yup=False)
(OUT/'hand_changes.json').write_text(json.dumps({'source':'hand_v3/hand_continuity.blend',
    'changes':['Thumb bridge lowered toward shaft; cross-section reduced 6 percent',
               'Quiet olive leather cuff instead of diagonal bands','Solid cuff caps replaced by open rolled rims',
               'Cloth entrance cap removed; skin extends inside the garment'],
    'clips':'Nine inherited actions; no new finger animation',
    'cuff_texture':{'tile':[64,0,64,128],'edge_rows':[0,8,120,128],'highlight_columns':[24,30],
                    'colors':['4b4940','626450','7b7862']}},indent=2))
print('ARTIFICER_FITTED_HAND_EXPORTED')
