"""Run with Blender 5 --background --python tools/blender/export_viewmodels.py.
Metres, common grip origin; editable lofts, armature and actions, official GLB export.
"""
import bpy, math, json, struct, zlib
from pathlib import Path
from mathutils import Vector, Quaternion
from mathutils.kdtree import KDTree

ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT/'assets/source/blender'
OUT=ROOT/'assets/runtime/viewmodels'
REVIEW=ROOT/'target/wayfarer-review'
for p in (SOURCE,OUT,REVIEW): p.mkdir(parents=True,exist_ok=True)
bpy.ops.object.select_all(action='SELECT'); bpy.ops.object.delete(use_global=False)
scene=bpy.context.scene
scene.unit_settings.system='METRIC'
scene.render.fps=30
PALETTE={'wood':('392c29','644632','98704a'), 'leather':('352d2a','584537','806345'),
 'brass':('61533c','aa8c50','dfc282'), 'ceramic':('8a948b','c3c6ae','e2dfbe'),
 'crystal':('155653','2f9690','82d6c1'), 'skin':('9a6554','c18a6b','dbab84'),
 'cloth':('26393e','3c5355','627371'), 'nails':('b07d66','d2a58b','e1bfa4')}
N=256
raw=bytearray(N*N*4)
for k,(name,colors) in enumerate(PALETTE.items()):
    ox=(k%4)*64; oy=(k//4)*128
    for y in range(128):
        for x in range(64):
            # Large connected paint clusters; never per-pixel random noise.
            level=1
            if name=='wood': level=0 if (x+(y//28)%3)%23<3 else 2 if (x+(y//40))%31<3 else 1
            elif name=='leather': level=0 if (y+x//3)%29<3 else 2 if (y+x//3)%29==4 else 1
            elif name=='brass': level=2 if x<9 else 0 if x>53 else 1
            elif name=='crystal': level=0 if x<16 else 2 if 35<x<47 else 1
            elif name=='cloth': level=0 if x<12 or (x//12+y//32)%7==0 else 2 if 36<x<43 else 1
            elif name=='skin': level=2 if 18<x<45 and 22<y<80 else 1
            else: level=2 if 12<x<47 else 1
            i=((oy+y)*N+ox+x)*4
            raw[i:i+4]=bytes.fromhex(colors[level])+bytes([255])
(OUT/'wayfarer.rgba').write_bytes(raw)
def chunk(t,d): return struct.pack('>I',len(d))+t+d+struct.pack('>I',zlib.crc32(t+d))
png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',N,N,8,6,0,0,0))
png+=chunk(b'IDAT',zlib.compress(b''.join(b'\0'+raw[y*N*4:(y+1)*N*4] for y in range(N))))+chunk(b'IEND',b'')
(OUT/'wayfarer.png').write_bytes(png)
image=bpy.data.images.load(str(OUT/'wayfarer.png')); image.pack()
for name in PALETTE:
    mat=bpy.data.materials.new(name); mat.use_nodes=True
    bs=mat.node_tree.nodes.get('Principled BSDF')
    tex=mat.node_tree.nodes.new('ShaderNodeTexImage'); tex.image=image; tex.interpolation='Closest'
    mat.node_tree.links.new(tex.outputs['Color'],bs.inputs['Base Color'])
    bs.inputs['Roughness'].default_value={'brass':.36,'crystal':.25,'skin':.78}.get(name,.84)
    bs.inputs['Metallic'].default_value=.75 if name=='brass' else 0
    if name=='crystal':
        bs.inputs['Emission Color'].default_value=(.02,.18,.14,1)
        bs.inputs['Emission Strength'].default_value=.28

parts=[]; bones=[]
def bone(name,head,tail,parent=None): bones.append((name,head,tail,parent))
bone('Root',(0,-.15,0),(0,-.10,0))
bone('Forearm',(.22,-.19,-.01),(.083,-.043,0),'Root')
bone('Wrist',(.083,-.043,0),(.031,0,0),'Forearm')
bone('GripSocket',(0,0,0),(0,.05,0),'Wrist')

def loft(name,centers,radii,material,group,joints=None,sides=10):
    """Cross-section loft with authored centre line and elliptical anatomy."""
    vs=[]; faces=[]; centers=[Vector(c) for c in centers]
    for i,(c,r) in enumerate(zip(centers,radii)):
        tangent=(centers[min(i+1,len(centers)-1)]-centers[max(i-1,0)]).normalized()
        ref=Vector((0,1,0)) if abs(tangent.y)<.9 else Vector((1,0,0))
        u=tangent.cross(ref).normalized(); v=tangent.cross(u).normalized()
        for j in range(sides):
            a=math.tau*j/sides
            fold=1+.08*math.sin(a*3+i*.8) if material=='cloth' else 1
            vs.append(c+(u*math.cos(a)*r[0]+v*math.sin(a)*r[1])*fold)
    faces.append(tuple(reversed(range(sides))))
    for i in range(len(centers)-1):
        for j in range(sides):
            a=i*sides+j; b=i*sides+(j+1)%sides
            faces.append((a,b,b+sides,a+sides))
    faces.append(tuple(range((len(centers)-1)*sides,len(centers)*sides)))
    mesh=bpy.data.meshes.new(name); mesh.from_pydata(vs,[],faces); mesh.update()
    obj=bpy.data.objects.new(name,mesh); scene.collection.objects.link(obj)
    obj.data.materials.append(bpy.data.materials[material]); obj['part']=group
    uv=mesh.uv_layers.new(name='Painted atlas')
    k=list(PALETTE).index(material); ox=(k%4)*64; oy=(k//4)*128
    for f in mesh.polygons:
        f.use_smooth=material in ('skin','cloth','leather')
        for li in f.loop_indices:
            idx=mesh.loops[li].vertex_index
            u=(idx%sides)/(sides-1); v=(idx//sides)/(len(centers)-1)
            uv.data[li].uv=((ox+2+u*59)/N,1-(oy+2+v*123)/N)
    if joints:
        for ji in set(j for weights in joints for j,w in weights): obj.vertex_groups.new(name=ji)
        for i,weights in enumerate(joints):
            for j,w in weights: obj.vertex_groups[j].add(list(range(i*sides,(i+1)*sides)),w,'REPLACE')
    parts.append(obj); return obj

def wand(name,cs,rs,mat,sides=10):
    return loft(name,cs,[(r,r) for r in rs],mat,'wand',[[('GripSocket',1)]]*len(cs),sides)

# 42 cm full length, 30 mm shaped grip, distinct shoulder and asymmetric tip.
wand('Dark heartwood spine',[(0,-.105,0),(0,-.09,0),(0,-.045,0),(0,.015,0),(.002,.065,0),(.007,.13,0),(.004,.205,0),(.004,.232,0)], [.009,.012,.014,.014,.012,.010,.008,.012],'wood')
wand('Palm shaped leather grip',[(0,-.075,0),(0,-.068,0),(0,-.03,0),(0,.025,0),(0,.037,0)], [.013,.015,.0155,.0145,.012],'leather',12)
for y,r in [(-.082,.013),(.042,.014),(.207,.014),(.22,.018)]:
    wand('Chased brass ferrule',[(.003,y-.003,0),(.003,y,0),(.003,y+.003,0)],[r*.88,r,r*.88],'brass',12)
wand('Ivory socket',[(.004,.216,0),(.004,.228,0),(.004,.247,0)],[.016,.021,.018],'ceramic',12)
wand('Turquoise focus',[(.004,.234,0),(.006,.248,.002),(.003,.278,.003),(-.003,.311,.001)],[.008,.023,.019,.001],'crystal',7)
for i in range(3):
    a=i*math.tau/3+.4
    cs=[(.004+math.cos(a)*r,y,math.sin(a)*r) for y,r in [(.214,.013),(.237,.025),(.264,.027),(.277,.019)]]
    wand('Retaining claw '+str(i),cs,[.0045,.004,.003,.002],'brass',6)
for a in (.5,3.6):
    cs=[(.004+math.cos(a)*r,y,math.sin(a)*r) for y,r in [(.055,.012),(.12,.0105),(.195,.009),(.215,.014)]]
    wand('Inlaid conduit',cs,[.0014]*4,'brass',5)

# Palm metacarpal mass, wrist taper and padded thenar eminence.
loft('Palm',[(.091,-.048,0),(.074,-.028,0),(.050,-.005,-.002),(.026,.003,-.004),(.018,.003,-.003)],[(.014,.018),(.018,.027),(.020,.040),(.016,.042),(.010,.037)],'skin','arm',[[('Wrist',1)]]*5,12)
loft('Thenar pad',[(.068,-.005,.006),(.052,.016,.009),(.038,.025,.006)],[(.009,.014),(.017,.022),(.009,.012)],'skin','arm',[[('Wrist',1)]]*3,10)
for f,(y,s) in enumerate([(.032,1),(.010,1.04),(-.012,.97),(-.032,.82)]):
    name=['Index','Middle','Ring','Little'][f]
    cs=[(.028,y,-.007),(.006,y,-.023),(-.018,y,-.019),(-.023,y,.002),(-.015,y,.018),(.003,y,.021),(.012,y,.012)]
    cs=[(x,y,z*s) for x,y,z in cs]
    for j in range(3): bone(name+str(j),cs[j*2],cs[j*2+2], 'Wrist' if j==0 else name+str(j-1))
    weights=[[(name+'0',1)],[(name+'0',1)],[(name+'0',.5),(name+'1',.5)],[(name+'1',1)],[(name+'1',.5),(name+'2',.5)],[(name+'2',1)],[(name+'2',1)]]
    loft(name,cs,[(r*s,r*s*.89) for r in [.010,.010,.0095,.009,.008,.007,.0035]],'skin','arm',weights,10)
cs=[(.057,.013,.006),(.049,.032,.020),(.033,.045,.023),(.016,.045,.024),(.002,.036,.021),(-.005,.031,.018),(-.008,.028,.016)]
for j in range(3): bone('Thumb'+str(j),cs[j*2],cs[j*2+2],'Wrist' if j==0 else 'Thumb'+str(j-1))
loft('Opposing thumb',cs,[(r,r*.85) for r in [.012,.012,.011,.010,.009,.007,.003]],'skin','arm',[[('Thumb'+str(min(i//2,2)),1)] for i in range(7)],10)
loft('Wrist skin',[(.083,-.043,0),(.102,-.065,0),(.121,-.084,-.002)],[(.015,.020),(.017,.021),(.020,.023)],'skin','arm',[[('Wrist',1)],[('Wrist',.5),('Forearm',.5)],[('Forearm',1)]],12)
loft('Tailored sleeve',[(.105,-.070,0),(.112,-.079,0),(.126,-.095,-.002),(.143,-.117,-.004),(.162,-.139,-.008),(.190,-.166,-.012),(.235,-.207,-.015)],[(.023,.025),(.024,.027),(.027,.029),(.025,.030),(.029,.031),(.030,.032),(.034,.035)],'cloth','arm',[[('Forearm',1)]]*7,12)
loft('Leather cuff',[(.101,-.067,0),(.109,-.075,0),(.116,-.082,0)],[(.022,.024),(.024,.027),(.025,.027)],'leather','arm',[[('Forearm',1)]]*3,12)

# Join the overlapping anatomical volumes into continuous skin, then transfer
# the authored joint weights. This removes the visible tube/palm insertion seams.
skin=[o for o in parts if o.name in ('Palm','Thenar pad','Wrist skin')]
samples=[]
for obj in skin:
    for vertex in obj.data.vertices:
        samples.append((vertex.co.copy(),[(obj.vertex_groups[g.group].name,g.weight) for g in vertex.groups]))
kd=KDTree(len(samples))
for i,(co,_) in enumerate(samples): kd.insert(co,i)
kd.balance()
bpy.ops.object.select_all(action='DESELECT')
for obj in skin: obj.select_set(True)
bpy.context.view_layer.objects.active=skin[0]
bpy.ops.object.join(); joined=bpy.context.object; joined.name='Continuous anatomical hand'
joined.data.remesh_voxel_size=.0018
bpy.ops.object.voxel_remesh()
smooth=joined.modifiers.new('Palm and web transitions','SMOOTH'); smooth.factor=.7; smooth.iterations=4
bpy.ops.object.modifier_apply(modifier=smooth.name)
dec=joined.modifiers.new('Authored silhouette reduction','DECIMATE'); dec.ratio=.12
bpy.ops.object.modifier_apply(modifier=dec.name)
joined.vertex_groups.clear()
for name,_,_,_ in bones: joined.vertex_groups.new(name=name)
for vertex in joined.data.vertices:
    weights={}
    neighbours=kd.find_n(vertex.co,3)
    total=sum(1/max(distance,.0001) for _,_,distance in neighbours)
    for _,idx,distance in neighbours:
        for name,w in samples[idx][1]: weights[name]=weights.get(name,0)+w/max(distance,.0001)/total
    top=sorted(weights.items(),key=lambda x:-x[1])[:4]; norm=sum(w for _,w in top)
    for name,w in top: joined.vertex_groups[name].add([vertex.index],w/norm,'REPLACE')
while joined.data.uv_layers: joined.data.uv_layers.remove(joined.data.uv_layers[0])
uv=joined.data.uv_layers.new(name='Skin painted islands')
for face in joined.data.polygons:
    face.use_smooth=True
    for li in face.loop_indices:
        co=joined.data.vertices[joined.data.loops[li].vertex_index].co
        uv.data[li].uv=((64+8+((co.x+.03)/.16)*46)/256,1-(128+8+((co.y+.10)/.18)*110)/256)
parts=[o for o in parts if o not in skin]+[joined]
joined['part']='arm'

arm=bpy.data.armatures.new('Wayfarer nineteen joint hand')
rig=bpy.data.objects.new('WayfarerRig',arm); scene.collection.objects.link(rig)
bpy.context.view_layer.objects.active=rig; rig.select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
for name,head,tail,parent in bones:
    b=arm.edit_bones.new(name); b.head=head; b.tail=tail
    if parent: b.parent=arm.edit_bones[parent]
bpy.ops.object.mode_set(mode='OBJECT')
for obj in parts:
    mod=obj.modifiers.new('Anatomical skin','ARMATURE'); mod.object=rig
    obj.parent=rig
rig.animation_data_create()
clips={'idle':(90,.015,0),'equip':(24,-.65,0),'windup':(12,.15,-.035),
       'release':(9,-.12,.025),'recovery':(21,.08,0),'inspect':(90,.12,.12),
       'open':(30,0,.8),'strong_grip':(30,0,-.04),'index_extend':(30,0,.65)}
for name,(duration,wrist,finger) in clips.items():
    action=bpy.data.actions.new(name); action.use_fake_user=True
    rig.animation_data.action=action
    for frame,factor in [(1,0),(duration//2,1),(duration,0 if name not in ('open','strong_grip','index_extend') else 1)]:
        for pb in rig.pose.bones:
            pb.rotation_mode='QUATERNION'; pb.rotation_quaternion=Quaternion()
            if pb.name=='Wrist': pb.rotation_quaternion=Quaternion((0,1,0),wrist*factor)
            if any(pb.name.startswith(s) for s in ('Index','Middle','Ring','Little','Thumb')):
                angle=-finger*factor if name!='index_extend' or pb.name.startswith('Index') else 0
                axis=Vector((0,0,1)) if pb.name.startswith('Thumb') else Vector((0,1,0))
                axis=pb.bone.matrix_local.to_quaternion().inverted()@axis
                pb.rotation_quaternion=Quaternion(axis,angle)
            pb.keyframe_insert(data_path='rotation_quaternion',frame=frame,group=pb.name)
    track=rig.animation_data.nla_tracks.new(); track.name=name
    strip=track.strips.new(name,1,action); track.mute=True
rig.animation_data.action=None
scene.frame_set(1)
for pb in rig.pose.bones: pb.rotation_quaternion=Quaternion()

def export(group,filename):
    bpy.ops.object.select_all(action='DESELECT'); rig.select_set(True)
    for obj in parts: obj.select_set(obj['part']==group)
    bpy.ops.export_scene.gltf(filepath=str(OUT/filename),export_format='GLB',use_selection=True,
        export_animations=True,export_animation_mode='ACTIONS',export_skins=True,export_def_bones=False,
        export_force_sampling=True,export_frame_range=False,export_yup=False)
export('wand','wayfarer_focus.glb'); export('arm','first_person_arms.glb')

# Save both requested editable source files; same rig/assembly keeps socket registration inspectable.
bpy.ops.wm.save_as_mainfile(filepath=str(SOURCE/'wayfarer_focus.blend'))
bpy.ops.wm.save_as_mainfile(filepath=str(SOURCE/'first_person_arms.blend'))
scene.render.engine='CYCLES'; scene.cycles.samples=24
scene.render.resolution_x=1000; scene.render.resolution_y=1000; scene.render.resolution_percentage=100
scene.world.color=(.12,.12,.12)
scene.view_settings.view_transform='AgX'
def area(name,pos,power,size,color):
    d=bpy.data.lights.new(name,'AREA'); d.energy=power; d.shape='DISK'; d.size=size; d.color=color
    o=bpy.data.objects.new(name,d); scene.collection.objects.link(o); o.location=pos
    o.rotation_euler=(Vector((0,.08,0))-o.location).to_track_quat('-Z','Y').to_euler()
area('Softbox',(.3,.45,.55),7,.45,(1,.87,.72))
area('Cool fill',(-.35,.1,.2),3,.4,(.65,.84,1))
area('Rim',(.1,.4,-.35),6,.3,(.9,1,1))
camdata=bpy.data.cameras.new('Review camera'); cam=bpy.data.objects.new('Review camera',camdata)
scene.collection.objects.link(cam); scene.camera=cam; camdata.type='ORTHO'; camdata.ortho_scale=.59
views={'front':((0,.08,.9),(.025,.055,0)), 'side':((.8,.08,.05),(.025,.055,0)),
 'three_quarter':((.45,.32,.75),(.025,.055,0)), 'player':((.32,-.38,.65),(.035,.065,0)),
 'grip_contact':((-.25,.01,.38),(0,0,0)), 'thumb_contact':((.28,.23,.4),(.025,.025,0))}
for name,(position,target) in views.items():
    cam.location=position; cam.rotation_euler=(Vector(target)-cam.location).to_track_quat('-Z','Y').to_euler()
    camdata.ortho_scale=.18 if 'contact' in name else .59
    scene.render.filepath=str(REVIEW/(name+'.png')); bpy.ops.render.render(write_still=True)
cam.location=(.32,-.38,.65); cam.rotation_euler=(Vector((.035,.065,0))-cam.location).to_track_quat('-Z','Y').to_euler()
camdata.ortho_scale=.59
for name in ('open','strong_grip','windup','release','inspect','index_extend'):
    rig.animation_data.action=bpy.data.actions[name]; scene.frame_set(int(clips[name][0]*.5))
    scene.render.filepath=str(REVIEW/(name+'.png')); bpy.ops.render.render(write_still=True)
rig.animation_data.action=None
for pb in rig.pose.bones: pb.rotation_quaternion=Quaternion()
scene.frame_set(1)
bpy.ops.wm.save_as_mainfile(filepath=str(SOURCE/'wayfarer_focus.blend'))
bpy.ops.wm.save_as_mainfile(filepath=str(SOURCE/'first_person_arms.blend'))
(REVIEW/'manifest.json').write_text(json.dumps({'blender':bpy.app.version_string,'joints':len(bones),'objects':len(parts),'clips':list(clips),'units':'metres','coordinate_system':'Blender Z-up retained for native grip space'},indent=2))
