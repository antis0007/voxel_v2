"""Second art pass from the saved Blender study; keep v1 and native evidence."""
import json, math, struct, zlib
from pathlib import Path
import bpy
from mathutils import Vector

ROOT=Path(__file__).resolve().parents[2]
SRC=ROOT/'assets/source/wand_ecosystem/minimal_v1'
OUT=ROOT/'assets/source/wand_ecosystem/minimal_v2';OUT.mkdir(parents=True,exist_ok=True)
REVIEW=ROOT/'target/ecosystem-minimal-v2';REVIEW.mkdir(parents=True,exist_ok=True)
bpy.ops.wm.open_mainfile(filepath=str(SRC/'ecosystem_items.blend'))
scene=bpy.context.scene;rig=bpy.data.objects['WayfarerRig']
def save_rgba(name,raw):
    (OUT/(name+'.rgba')).write_bytes(raw)
    def chunk(t,d):return struct.pack('>I',len(d))+t+d+struct.pack('>I',zlib.crc32(t+d))
    (OUT/(name+'.png')).write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',256,256,8,6,0,0,0))+
        chunk(b'IDAT',zlib.compress(b''.join(b'\0'+raw[y*1024:(y+1)*1024] for y in range(256))))+chunk(b'IEND',b''))
    im=bpy.data.images.load(str(OUT/(name+'.png')));im.pack();return im
paint=bytearray((SRC/'atlas.rgba').read_bytes())
# Carved bone notches, a dark forge line and restrained wear on the key. These
# clusters use the same UV grid as the first pass; none are random speckles.
marks=[(13,21,18,23,'8d8771'),(12,26,17,28,'8d8771'),(11,37,13,44,'ece3bf'),
       (47,21,49,37,'252d38'),(49,39,51,43,'718080'),
       (109,34,112,45,'d5bc77'),(110,46,113,49,'68523d'),
       (13,96,16,103,'66595c'),(15,107,18,119,'272630')]
for x0,y0,x1,y1,col in marks:
    for y in range(y0,y1):
        for x in range(x0,x1):paint[(y*256+x)*4:(y*256+x+1)*4]=bytes.fromhex(col)+b'\xff'
wand_image=save_rgba('atlas',paint)
for mat in bpy.data.materials:
    if mat.use_nodes:
        for n in mat.node_tree.nodes:
            if n.type=='TEX_IMAGE':n.image=wand_image
hand_paint=bytearray((ROOT/'assets/runtime/viewmodels/wayfarer.rgba').read_bytes())
ramps={'skin':['98745f','b59478','d3b293'],'cloth':['293d40','43595a','697b75']}
for name,ox in [('skin',64),('cloth',128)]:
    for y in range(128,256):
        for x in range(64):
            level=0 if x<10 else 2 if 38<=x<49 else 1
            if name=='cloth' and 174<=y<182 and 12<=x<40:level=0
            offset=(y*256+ox+x)*4
            hand_paint[offset:offset+4]=bytes.fromhex(ramps[name][level])+b'\xff'
hand_image=save_rgba('hand_atlas',hand_paint)
(OUT/'texture_sources.json').write_text(json.dumps({'wand_rectangles':marks,'hand_ramps':ramps,
    'status':'Authored clusters; no noise or resampled concept art'},indent=2))
factors={'bone_needle':(.8,.65),'runic_key':(.9,.65),'cinder_rod':(.85,.7)}
for key,(upper,lower) in factors.items():
    col=bpy.data.collections[key];col.hide_viewport=False
    for obj in col.objects:
        if obj.type=='MESH':
            for v in obj.data.vertices:v.co.y*=upper if v.co.y>0 else lower
        else:obj.location.y*=upper if obj.location.y>0 else lower
    tip=list(col['muzzle']);tip[1]*=upper;col['muzzle']=tip

# Skin is an intentional quad cage with broad dorsal/palmar planes and a wrist
# taper. No remeshing, triangulation decimation, or inherited superhero surface.
hand=bpy.data.collections.new('Plane hand');scene.collection.children.link(hand)
with bpy.data.libraries.load(str(ROOT/'assets/source/blender/first_person_arms.blend'),link=False) as (src,dst):
    dst.objects=['Tailored sleeve','Leather cuff']
for obj in dst.objects:
    hand.objects.link(obj);obj.parent=rig
    for mod in obj.modifiers:
        if mod.type=='ARMATURE':mod.object=rig
# Appending the cuff/sleeve also brings a second copy of their rig/actions.
# Keep exactly the source rig's nine actions in this derived export.
for obj in list(bpy.data.objects):
    if obj.type=='ARMATURE' and obj!=rig:bpy.data.objects.remove(obj,do_unlink=True)
for action in list(bpy.data.actions):
    base,separator,suffix=action.name.rpartition('.')
    if separator and suffix.isdigit() and bpy.data.actions.get(base):bpy.data.actions.remove(action)

old_atlas=hand_image
for obj in hand.objects:
    for material in obj.data.materials:
        for n in material.node_tree.nodes:
            if n.type=='TEX_IMAGE':n.image=hand_image
skin=bpy.data.materials.new('skin');skin.use_nodes=True
bs=skin.node_tree.nodes['Principled BSDF'];bs.inputs['Roughness'].default_value=.9
bs.inputs['Specular IOR Level'].default_value=.08
tex=skin.node_tree.nodes.new('ShaderNodeTexImage');tex.image=old_atlas;tex.interpolation='Closest'
skin.node_tree.links.new(tex.outputs['Color'],bs.inputs['Base Color'])

def part(name,stations,groups):
    # A station supplies centre, two cross-section axes and half dimensions.
    section=[(.7,1),(-.7,1),(-1,.6),(-1,-.6),(-.7,-1),(.7,-1),(1,-.6),(1,.6)]
    verts=[]
    for center,a,b,ra,rb in stations:
        verts.extend(Vector(center)+Vector(a)*u*ra+Vector(b)*v*rb for u,v in section)
    faces=[tuple(reversed(range(8))),tuple(range(len(verts)-8,len(verts)))]
    faces += [(j*8+i,j*8+(i+1)%8,(j+1)*8+(i+1)%8,(j+1)*8+i) for j in range(len(stations)-1) for i in range(8)]
    data=bpy.data.meshes.new(name);data.from_pydata(verts,[],faces);data.update()
    obj=bpy.data.objects.new(name,data);hand.objects.link(obj);obj.parent=rig;data.materials.append(skin)
    for b in rig.data.bones:obj.vertex_groups.new(name=b.name)
    for i,weights in enumerate(groups):
        for bone,w in weights:obj.vertex_groups[bone].add(list(range(i*8,i*8+8)),w,'REPLACE')
    obj.modifiers.new('Authored finger weights','ARMATURE').object=rig
    uv=data.uv_layers.new(name='Skin planes')
    for face in data.polygons:
        for li in face.loop_indices:
            v=data.vertices[data.loops[li].vertex_index].co
            # Keep inside the original skin tile; broad low-contrast skin ramp.
            uv.data[li].uv=((86+v.z*160)/256,1-(182+v.y*250)/256)
    return obj

palm=[]
for x,y,ry,rz in [(.114,-.079,.017,.017),(.095,-.056,.020,.018),(.074,-.025,.030,.017),
                  (.050,-.003,.039,.016),(.029,.001,.038,.014),(.021,.001,.034,.012)]:
    palm.append(((x,y,-.002),(0,1,0),(0,0,1),ry,rz))
part('Wrist and metacarpal planes',palm,[[('Forearm',1)],[('Wrist',.65),('Forearm',.35)]]+[[('Wrist',1)]]*4)
for name,y,size in [('Index',.030,1),('Middle',.010,1.02),('Ring',-.010,.96),('Little',-.028,.82)]:
    centers=[(.029,y,-.006),(.007,y,-.022),(-.017,y,-.020),(-.024,y,-.002),(-.018,y,.015),(.003,y,.024),(.021,y,.007)]
    stations=[]
    for i,c in enumerate(centers):
        tangent=(Vector(centers[min(i+1,6)])-Vector(centers[max(i-1,0)])).normalized()
        n=tangent.cross(Vector((0,1,0))).normalized()
        r=[.009,.009,.0085,.008,.0075,.0068,.0038][i]*size
        stations.append((c,(0,1,0),n,r,r*.8))
    part(name+' grip planes',stations,[[(name+'0',1)],[(name+'0',1)],[(name+'0',.5),(name+'1',.5)],
        [(name+'1',1)],[(name+'1',.5),(name+'2',.5)],[(name+'2',1)],[(name+'2',1)]])
centers=[(.061,.009,.009),(.049,.028,.021),(.033,.040,.026),(.017,.039,.026),(.004,.031,.023),(-.004,.024,.018)]
stations=[]
for i,c in enumerate(centers):
    tangent=(Vector(centers[min(i+1,5)])-Vector(centers[max(i-1,0)])).normalized()
    a=Vector((0,0,1));b=tangent.cross(a).normalized();a=b.cross(tangent).normalized()
    r=[.014,.012,.0105,.009,.008,.0045][i]
    stations.append((c,a,b,r*.75,r))
part('Opposing thumb planes',stations,[[('Thumb0',1)],[('Thumb0',1)],[('Thumb1',1)],
    [('Thumb1',1)],[('Thumb2',1)],[('Thumb2',1)]])

# Shift selected sleeve rings to make a single readable compression fold above
# the cuff. Existing skinning and cuff attachment remain intact.
sleeve=next(o for o in hand.objects if o.name.startswith('Tailored sleeve'))
for v in sleeve.data.vertices:
    if -.125<v.co.y<-.093:v.co.z+=.006*(1-abs(v.co.y+.109)/.016)
for p in sleeve.data.polygons:p.use_smooth=False

def export(col,name):
    bpy.ops.object.select_all(action='DESELECT');rig.select_set(True);bpy.context.view_layer.objects.active=rig
    for o in col.objects:
        if o.type=='MESH':o.select_set(True)
    bpy.ops.export_scene.gltf(filepath=str(OUT/(name+'.glb')),export_format='GLB',use_selection=True,
        export_animations=True,export_animation_mode='ACTIONS',export_skins=True,export_def_bones=False,
        export_force_sampling=True,export_frame_range=False,export_yup=False)
for key in factors:export(bpy.data.collections[key],key)
export(hand,'plane_hand')
for key in factors:
    col=bpy.data.collections[key];col.hide_render=key!='runic_key';col.hide_viewport=key!='runic_key'
scene['review_status']='Shorter item proportions and authored quad hand. Native acceptance pending.'
bpy.ops.wm.save_as_mainfile(filepath=str(OUT/'ecosystem_items_r2.blend'))
scene.render.resolution_x=600;scene.render.resolution_y=800
scene.render.filepath=str(REVIEW/'hand_and_key.png');bpy.ops.render.render(write_still=True)
manifest={'blender':bpy.app.version_string,'items':{k:list(bpy.data.collections[k]['muzzle']) for k in factors},
    'hand_vertices':sum(len(o.data.vertices) for o in hand.objects),'status':scene['review_status']}
(OUT/'manifest.json').write_text(json.dumps(manifest,indent=2))
print('ECOSYSTEM_R2_EXPORTED',manifest)
