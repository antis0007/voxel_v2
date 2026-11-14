"""Author the Artificer tally wand in Blender. No Rust geometry or source overwrite.

Run: blender --background --python tools/blender/finish_artificer_wand.py
Y is the casting axis in Axiom's grip space; export_yup=False is intentional.
"""
import hashlib, json, math, struct, zlib
from pathlib import Path
import bpy
from mathutils import Vector, Matrix

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT/'assets/source/wand_ecosystem/artificer_tally'
RUNTIME = ROOT/'assets/runtime/viewmodels/artificer_tally'
REVIEW = ROOT/'target/artificer-tally'
for folder in (OUT, RUNTIME, REVIEW): folder.mkdir(parents=True, exist_ok=True)
bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene
scene.unit_settings.system = 'METRIC'
scene.render.fps = 30

# Small hand-placed clusters; long empty fields are deliberate. The atlas is
# also the editable pixel source, with every stroke recorded in texture.json.
RAMPS = {'wood':['292930','433b38','625043','876747'],
         'leather':['313535','4b5550','697365','949581'],
         'brass':['614039','95603e','c48d50','e3bc78'],
         'ceramic':['6d7776','9ca99e','c3c8af','dcd8b9'],
         'iron':['242936','3c4651','626c71','8c9590'],
         'crystal':['6b3547','ab5350','df8959','f0c684']}
TILES = {name:(i*40,0,40,128) for i,name in enumerate(RAMPS)}
pixels = bytearray(bytes.fromhex('282b32ff')*(256*256))
masks = bytearray(bytes.fromhex('000000ff')*(256*256))
strokes=[]
def rect(name, box, level):
    strokes.append([name,list(box),level])
    ox,oy,w,h=TILES[name]
    for y in range(box[1],box[3]):
        for x in range(box[0],box[2]):
            at=((oy+y)*256+ox+x)*4
            pixels[at:at+4]=bytes.fromhex(RAMPS[name][level])+b'\xff'
for index,(name,(ox,oy,w,h)) in enumerate(TILES.items()):
    rect(name,(0,0,w,h),1)
    for y in range(h):
        for x in range(w):
            at=((oy+y)*256+ox+x)*4
            masks[at:at+4]=bytes((32+index*32,255 if name=='brass' else 0,255 if name=='crystal' else 0,255))
# Quiet grain follows the taper instead of crossing it in noisy stripes.
for box,level in [((3,20,5,38),0),((4,38,6,52),0),((2,63,3,73),2),
                  ((6,9,8,28),0),((7,28,9,45),0),((8,45,10,68),0),
                  ((17,15,19,40),2),((18,40,20,63),2),((29,8,31,32),2),
                  ((27,32,30,52),2),((31,59,33,86),0)]: rect('wood',box,level)
for box,level in [((5,4,7,24),2),((17,5,19,24),0),((26,7,28,21),2)]:rect('leather',box,level)
for box,level in [((2,0,4,128),2),((18,0,20,128),0),((28,0,30,128),2)]:rect('brass',box,level)
rect('ceramic',(3,0,6,128),2)
rect('ceramic',(25,0,28,128),0)
# The signed tally belongs on a replaceable ceramic cheek, not all surfaces.
for y in (5,9):rect('ceramic',(3,y,6,y+2),0)
rect('iron',(5,0,7,128),2)
for box,level in [((0,0,5,128),0),((12,0,16,128),2),((22,0,24,128),3)]:rect('crystal',box,level)
def png(path, raw):
    def chunk(t,d):return struct.pack('>I',len(d))+t+d+struct.pack('>I',zlib.crc32(t+d))
    rows=b''.join(b'\0'+raw[y*1024:(y+1)*1024] for y in range(256))
    path.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',256,256,8,6,0,0,0))+
                     chunk(b'IDAT',zlib.compress(rows))+chunk(b'IEND',b''))
png(OUT/'albedo.png',pixels);png(OUT/'material_masks.png',masks)
(RUNTIME/'atlas.rgba').write_bytes(pixels)
(OUT/'texture.json').write_text(json.dumps({'size':[256,256],'ramps':RAMPS,'tiles':TILES,'rectangles':strokes,
    'density':'180 texels/metre; nearest sample; shared strips, separately unfolded cap islands',
    'masks':'R material ordinal *32, G metal, B emission eligibility. Runtime uses matching named material classes.'},indent=2))
image=bpy.data.images.load(str(OUT/'albedo.png'));image.pack()
materials={}
for name in RAMPS:
    mat=bpy.data.materials.new(name);mat.use_nodes=True
    bs=mat.node_tree.nodes['Principled BSDF'];bs.inputs['Roughness'].default_value=.58 if name=='brass' else .83
    bs.inputs['Metallic'].default_value=.55 if name=='brass' else 0
    tex=mat.node_tree.nodes.new('ShaderNodeTexImage');tex.image=image;tex.interpolation='Closest'
    mat.node_tree.links.new(tex.outputs['Color'],bs.inputs['Base Color']);materials[name]=mat
parts=[]
section=[(.7,1),(-.7,1),(-1,.65),(-1,-.65),(-.7,-1),(.7,-1),(1,-.65),(1,.65)]
def loft(name,rings,material):
    verts=[(x+rx*a,y,z+rz*b) for x,y,z,rx,rz in rings for a,b in section]
    faces=[tuple(reversed(range(8))),tuple(range((len(rings)-1)*8,len(rings)*8))]
    faces += [(j*8+i,j*8+(i+1)%8,(j+1)*8+(i+1)%8,(j+1)*8+i) for j in range(len(rings)-1) for i in range(8)]
    mesh=bpy.data.meshes.new(name);mesh.from_pydata(verts,[],faces);mesh.update()
    obj=bpy.data.objects.new(name,mesh);scene.collection.objects.link(obj)
    obj.data.materials.append(materials[material]);obj['functional_role']=name;parts.append(obj)
    uv=mesh.uv_layers.new(name='Pixel strips and caps')
    ox,oy,w,h=TILES[material]
    distances=[0.0]
    for a,b in zip(rings,rings[1:]):distances.append(distances[-1]+(Vector(a[:3])-Vector(b[:3])).length)
    # One pixel scale across all components; cap islands live below the strips.
    rx=max(r[3] for r in rings);rz=max(r[4] for r in rings)
    perimeter=[0.0]
    for a,b in zip(section,section[1:]+section[:1]):
        perimeter.append(perimeter[-1]+math.hypot((a[0]-b[0])*rx,(a[1]-b[1])*rz)*180)
    for face in mesh.polygons:
        cap=face.index<2
        side=(face.index-2)%8
        for li in face.loop_indices:
            vi=mesh.loops[li].vertex_index;ring=vi//8;corner=vi%8
            if cap:
                a,b=section[corner];px=ox+20+a*rx*180;py=oy+112+b*rz*180
            else:
                px=ox+2+perimeter[side if corner==side else side+1]
                strip_y=48 if material=='ceramic' and not name.startswith('08 ') else 2
                py=oy+strip_y+distances[ring]*180
            assert ox<=px<ox+w and oy<=py<oy+h,(name,px,py)
            uv.data[li].uv=(px/256,1-py/256)
    return obj
def band(name,y,rx,rz,material,height=.006,x=0,z=0):
    return loft(name,[(x,y-height/2,z,rx*.91,rz*.91),(x,y-height*.3,z,rx,rz),
                      (x,y+height*.3,z,rx,rz),(x,y+height/2,z,rx*.91,rz*.91)],material)

loft('01 Continuous carved heartwood',[(0,-.112,0,.010,.011),(-.003,-.098,0,.017,.015),
     (-.003,-.065,0,.018,.015),(0,-.018,0,.014,.013),(0,.025,0,.013,.012),
     (.004,.065,0,.013,.011),(.003,.14,0,.011,.010),(-.002,.21,0,.010,.009),
     (-.003,.273,0,.008,.008)],'wood')
band('02 Grounded heel',-.105,.014,.014,'iron',.012,x=-.002)
band('03 Heel joining seam',-.096,.017,.015,'brass',.004,x=-.003)
loft('04 Recessed grip wrap',[(0,-.04,0,.015,.014),(0,-.034,0,.016,.014),
     (0,.014,0,.014,.013),(0,.021,0,.013,.012)],'leather')
band('05 Thumb stop',.026,.016,.014,'brass',.008)
band('06 Coil foot',.176,.012,.011,'iron',.009)
for i in range(3):band('07 Copper winding '+str(i+1),.185+i*.008,.012-i*.0003,.011-i*.0003,'brass',.003)
loft('08 Tapered ceramic socket', [(-.003,.235,0,.010,.010),(-.004,.248,0,.015,.013),
     (-.005,.273,0,.016,.014),(-.006,.290,0,.013,.012)],'ceramic')
band('09 Socket retaining hoop',.25,.017,.015,'brass',.006,x=-.004)
loft('10 Dark inner electrode',[(-.006,.282,0,.010,.009),(-.006,.31,0,.008,.007)],'iron')
# One asymmetric ceramic beak and a contact behind it leave the exit open.
# No paired tines: the first render read too much like a candle in a fork.
loft('11 Swept dielectric cheek',[(-.015,.275,-.002,.006,.010),(-.019,.300,-.002,.006,.009),
     (-.015,.321,-.002,.004,.006),(-.009,.327,-.002,.002,.003)],'ceramic')
loft('12 Rear conductive contact',[(-.004,.279,-.012,.007,.004),(-.003,.312,-.014,.005,.003),
     (-.002,.326,-.009,.003,.002)],'brass')
loft('13 Exposed side contact',[(.006,.279,.002,.004,.006),(.009,.299,.002,.003,.006),
     (.005,.310,.002,.002,.003)],'brass')
loft('14 Amber casting mineral',[(-.005,.300,0,.004,.004),(-.003,.314,0,.010,.009),
     (.002,.331,0,.007,.006),(.006,.338,0,.003,.003)],'crystal')

with bpy.data.libraries.load(str(ROOT/'assets/source/wand_ecosystem/hand_v3/hand_continuity.blend'),link=False) as (src,dst):
    dst.objects=['WayfarerRig']
rig=dst.objects[0];scene.collection.objects.link(rig)
for obj in parts:
    obj.parent=rig;group=obj.vertex_groups.new(name='GripSocket');group.add(list(range(len(obj.data.vertices))),1,'REPLACE')
    mod=obj.modifiers.new('Grip attachment','ARMATURE');mod.object=rig
for name,loc in [('grip',(0,0,0)),('muzzle',(.006,.338,0)),('casting_axis',(.006,.40,0))]:
    obj=bpy.data.objects.new(name,None);scene.collection.objects.link(obj);obj.location=loc;obj['units']='metres'
scene.frame_set(1)
rig.animation_data.action=bpy.data.actions.get('idle')
scene.render.engine='CYCLES';scene.cycles.samples=16
scene.world=bpy.data.worlds.new('Neutral studio');scene.world.use_nodes=True
scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.18,.20,.23,1)
scene.world.node_tree.nodes['Background'].inputs[1].default_value=.7
scene.view_settings.view_transform='Standard'
for name,pos,power,size in [('Key',(.3,.5,1.0),28,.8),('Fill',(-.5,.1,.5),12,.7)]:
    light=bpy.data.lights.new(name,'AREA');light.energy=power;light.shape='DISK';light.size=size
    ob=bpy.data.objects.new(name,light);scene.collection.objects.link(ob);ob.location=pos
    ob.rotation_euler=(Vector((0,.12,0))-ob.location).to_track_quat('-Z','Y').to_euler()
cam=bpy.data.objects.new('Review camera',bpy.data.cameras.new('Review camera'));scene.collection.objects.link(cam);scene.camera=cam
cam.data.type='ORTHO';cam.data.ortho_scale=.53
def view(pos):
    cam.location=pos;f=(Vector((0,.12,0))-cam.location).normalized();r=f.cross(Vector((0,1,0))).normalized()
    cam.rotation_euler=Matrix((r,r.cross(f),-f)).transposed().to_euler()
def render(name):
    scene.render.filepath=str(REVIEW/(name+'.png'));bpy.ops.render.render(write_still=True)
scene.render.resolution_x=400;scene.render.resolution_y=600;scene.render.resolution_percentage=100
for name,pos in [('front',(0,.12,2)),('side',(2,.12,0)),('three_quarter',(.9,.25,1.7))]:view(pos);render(name)
view((.9,.25,1.7))
scene['art_direction']='Artificer tally wand: carved heartwood, replaceable dielectric socket, signed coil, compact amber focus.'
scene['muzzle']=[.006,.338,0]
bpy.ops.wm.save_as_mainfile(filepath=str(OUT/'artificer_tally.blend'))
bpy.ops.object.select_all(action='DESELECT');rig.select_set(True);bpy.context.view_layer.objects.active=rig
for obj in parts:obj.select_set(True)
bpy.ops.export_scene.gltf(filepath=str(RUNTIME/'wand.glb'),export_format='GLB',use_selection=True,
    export_animations=True,export_animation_mode='ACTIONS',export_skins=True,export_def_bones=False,
    export_force_sampling=True,export_frame_range=False,export_yup=False)
# A genuine black silhouette render, rather than a shaded concept thumbnail.
black=bpy.data.materials.new('Black silhouette');black.use_nodes=True;black.node_tree.nodes.clear()
out=black.node_tree.nodes.new('ShaderNodeOutputMaterial');em=black.node_tree.nodes.new('ShaderNodeEmission')
em.inputs[0].default_value=(0,0,0,1);black.node_tree.links.new(em.outputs[0],out.inputs[0])
for obj in parts:obj.data.materials[0]=black
bg=scene.world.node_tree.nodes['Background'];bg.inputs[0].default_value=(1,1,1,1);bg.inputs[1].default_value=1
scene.render.resolution_x=96;scene.render.resolution_y=96;view((0,.12,2));render('silhouette_96')
(REVIEW/'manifest.json').write_text(json.dumps({'blender':bpy.app.version_string,'parts':len(parts),
    'length_m':.450,'focus_width_m':.038,'muzzle':[.006,.338,0],
    'source':str(OUT/'artificer_tally.blend'),'export':str(RUNTIME/'wand.glb'),
    'sha256':hashlib.sha256((RUNTIME/'wand.glb').read_bytes()).hexdigest(),
    'animation':'Existing nine hand-compatible clips retained; no new finger animation claimed.'},indent=2))
print('ARTIFICER_TALLY_BLENDER_EXPORT_COMPLETE')
