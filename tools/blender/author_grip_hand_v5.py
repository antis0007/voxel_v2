"""Blender hand production pass: staggered grip, opposing thumb, gathered sleeve.
Preserves all previous sources. Uses the shared wrist/socket contract for 2.5D wands.
"""
from pathlib import Path
import bpy, math, json, struct, zlib
from mathutils import Vector, Matrix, Quaternion

ROOT=Path(__file__).resolve().parents[2]
OUT=ROOT/'assets/source/wand_ecosystem/hand_v5'
RUNTIME=ROOT/'assets/runtime/viewmodels/hand_v5'
REVIEW=ROOT/'target/hand-v5'
for folder in (OUT,RUNTIME,REVIEW):folder.mkdir(parents=True,exist_ok=True)
bpy.ops.wm.open_mainfile(filepath=str(ROOT/'assets/source/wand_ecosystem/artificer_tally/fitted_hand.blend'))
scene=bpy.context.scene;rig=bpy.data.objects['WayfarerRig']
for obj in list(bpy.data.objects):
    if obj.type=='MESH':bpy.data.objects.remove(obj,do_unlink=True)
hand=bpy.data.collections.new('Hand v5 authored surfaces');scene.collection.children.link(hand)
ramps={'skin':['8a635d','a77a66','b98f73','cba283'],
       'cloth':['25313f','3d4e60','60788a','8e9eaa'],
       'leather':['42353a','674e48','92715c','b39978']}
paint=bytearray(bytes.fromhex('c99f7cff')*256*256);strokes=[]
def rect(box,color):
    strokes.append([list(box),color])
    for y in range(box[1],box[3]):
        for x in range(box[0],box[2]):paint[(y*256+x)*4:(y*256+x+1)*4]=bytes.fromhex(color)+b'\xff'
# Material patches follow hand planes. No per-pixel noise, grunge or global filter.
rect((0,0,256,128),ramps['skin'][2])
for box,level in [((8,20,11,22),3),((10,22,13,24),3),((13,24,15,25),3)]:rect(box,ramps['skin'][level])
# Fingers have separate unfolded strips; selected clusters follow pads/knuckles.
for i in range(5):
    x=i*32
    for box,level in [((x+3,70,x+5,78),3),((x+7,79,x+9,81),1),
                      ((x+4,84,x+6,91),3),((x+8,93,x+10,95),1)]:rect(box,ramps['skin'][level])
rect((0,128,128,256),ramps['cloth'][1])
for box,level in [((7,130,11,168),0),((10,168,15,182),0),((14,182,17,200),0),
                  ((26,135,32,153),2),((31,151,36,164),2),((39,169,44,204),2),
                  ((53,132,57,151),0),((54,151,59,178),0),((71,148,78,169),2),
                  ((77,169,81,188),2),((91,137,96,159),0)]:rect(box,ramps['cloth'][level])
rect((128,128,256,256),ramps['leather'][1])
rect((130,130,222,132),ramps['leather'][2]);rect((130,137,222,139),ramps['leather'][0])
def chunk(t,d):return struct.pack('>I',len(d))+t+d+struct.pack('>I',zlib.crc32(t+d))
png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',256,256,8,6,0,0,0))
png+=chunk(b'IDAT',zlib.compress(b''.join(b'\0'+paint[y*1024:(y+1)*1024] for y in range(256))))+chunk(b'IEND',b'')
(OUT/'albedo.png').write_bytes(png);(RUNTIME/'atlas.rgba').write_bytes(paint)
(OUT/'paint.json').write_text(json.dumps({'ramps':ramps,'rectangles':strokes,'grid':256,'filter':'nearest'},indent=2))
image=bpy.data.images.load(str(OUT/'albedo.png'));image.pack()
materials={}
for name in ramps:
    # Export material names match the runtime finish classes.
    prior=bpy.data.materials.get(name)
    if prior:prior.name='Previous '+name
    mat=bpy.data.materials.new(name);mat.use_nodes=True
    bs=mat.node_tree.nodes['Principled BSDF'];bs.inputs['Roughness'].default_value=.87
    bs.inputs['Specular IOR Level'].default_value=.12
    tex=mat.node_tree.nodes.new('ShaderNodeTexImage');tex.image=image;tex.interpolation='Closest'
    mat.node_tree.links.new(tex.outputs['Color'],bs.inputs['Base Color']);materials[name]=mat
section=[(.66,1),(-.66,1),(-1,.55),(-1,-.55),(-.66,-1),(.66,-1),(1,-.55),(1,.55)]
parts=[]
def mesh_part(name,vertices,faces,weights,material,uvs):
    data=bpy.data.meshes.new(name);data.from_pydata(vertices,[],faces);data.update()
    obj=bpy.data.objects.new(name,data);hand.objects.link(obj);obj.parent=rig;data.materials.append(materials[material])
    for bone in rig.data.bones:obj.vertex_groups.new(name=bone.name)
    for i,groups in enumerate(weights):
        for bone,weight in groups:obj.vertex_groups[bone].add([i],weight,'REPLACE')
    mod=obj.modifiers.new('Shared hand rig','ARMATURE');mod.object=rig
    uv=data.uv_layers.new(name='Authored pixel islands')
    for face,coords in zip(data.polygons,uvs):
        face.use_smooth=False
        for li,xy in zip(face.loop_indices,coords):uv.data[li].uv=(xy[0]/256,1-xy[1]/256)
    parts.append(obj);return obj
def loft(name,stations,groups,material,tile=(0,0),palm=False,cap_start=True,cap_end=True):
    vertices=[];weights=[];lengths=[0.0]
    for i,(centre,a,b,ra,rb) in enumerate(stations):
        vertices.extend(Vector(centre)+Vector(a)*u*ra+Vector(b)*v*rb for u,v in section)
        weights.extend([groups[i]]*8)
        if i:lengths.append(lengths[-1]+(Vector(centre)-Vector(stations[i-1][0])).length)
    faces=[];uvs=[]
    for j in range(len(stations)-1):
        for i in range(8):
            face=(j*8+i,j*8+(i+1)%8,(j+1)*8+(i+1)%8,(j+1)*8+i);faces.append(face)
            if palm:uvs.append([(3+vertices[k].x*180,4+(vertices[k].y+.10)*180) for k in face])
            else:
                circumference=sum(math.hypot((a[0]-b[0])*stations[j][3],(a[1]-b[1])*stations[j][4])
                                  for a,b in zip(section,section[1:]+section[:1]))
                width=circumference*180
                u0=tile[0]+2+i/8*width;u1=tile[0]+2+(i+1)/8*width
                v0=tile[1]+2+lengths[j]*180;v1=tile[1]+2+lengths[j+1]*180
                uvs.append([(u0,v0),(u1,v0),(u1,v1),(u0,v1)])
    for use,ring,reverse in [(cap_start,0,True),(cap_end,len(stations)-1,False)]:
        if use:
            face=list(range(ring*8,ring*8+8));face=face[::-1] if reverse else face;faces.append(face)
            uvs.append([(tile[0]+16+section[k%8][0]*3,tile[1]+53+section[k%8][1]*3) for k in face])
    # Palm sections run from wrist toward knuckles, opposite their Y/Z frame.
    # Reverse both topology and UV loops so the renderer sees outward surfaces.
    travel=Vector(stations[1][0])-Vector(stations[0][0])
    if Vector(stations[0][1]).cross(Vector(stations[0][2])).dot(travel)<0:
        faces=[list(reversed(face)) for face in faces]
        uvs=[list(reversed(coords)) for coords in uvs]
    return mesh_part(name,vertices,faces,weights,material,uvs)

palm=[]
for x,y,ry,rz,ax in [(.126,-.093,.022,.022,(.726,.688,0)),(.105,-.070,.022,.022,(.726,.688,0)),
                  (.082,-.037,.026,.021,(.45,.893,0)),(.060,-.011,.034,.022,(.16,.987,0)),
                  (.040,.001,.038,.020,(0,1,0)),(.026,.001,.035,.014,(0,1,0))]:
    palm.append(((x,y,-.002),ax,(0,0,1),ry,rz))
loft('Palm wrist and metacarpal arch',palm,[[('Forearm',1)]]+[[('Wrist',1)]]*(len(palm)-1),'skin',palm=True)
chains={}
for index,(name,y,size,lean) in enumerate([('Index',.032,1,0),('Middle',.011,1.04,-.002),
                                       ('Ring',-.010,.96,-.003),('Little',-.029,.82,-.004)]):
    centres=[(.034,y,-.007),(.014,y+.003,-.021),(-.009,y+.001,-.023),
             (-.022,y-.004,-.012),(-.023,y-.008,.004),(-.011,y-.012,.017),(.009,y-.015+lean,.015)]
    if name=='Little':centres=[(x*.88,yy,z*.9) for x,yy,z in centres]
    chains[name]=centres;stations=[]
    for i,c in enumerate(centres):
        tangent=(Vector(centres[min(i+1,6)])-Vector(centres[max(i-1,0)])).normalized()
        a=Vector((0,1,0));b=tangent.cross(a).normalized();a=b.cross(tangent).normalized()
        radius=[.0105,.010,.0095,.009,.0085,.0075,.006][i]*size
        stations.append((c,a,b,radius,radius*.82))
    groups=[[(name+'0',1)],[(name+'0',1)],[(name+'0',.4),(name+'1',.6)],[(name+'1',1)],
            [(name+'1',.4),(name+'2',.6)],[(name+'2',1)],[(name+'2',1)]]
    loft(name+' staggered grip',stations,groups,'skin',(index*32,64))
# A saddle at the palm, a sloping proximal phalanx and an opposing broad pad.
centres=[(.075,-.014,.003),(.057,.009,.013),(.039,.027,.022),(.021,.028,.023),(.006,.019,.019),(-.003,.010,.015)]
chains['Thumb']=centres;stations=[]
for i,c in enumerate(centres):
    tangent=(Vector(centres[min(i+1,5)])-Vector(centres[max(i-1,0)])).normalized()
    b=tangent.cross(Vector((0,0,1))).normalized();a=b.cross(tangent).normalized()
    r=[.010,.012,.0105,.0095,.008,.006][i]
    stations.append((c,a,b,r*.8,r))
loft('Opposing thumb and thenar saddle',stations,[[('Thumb0',1)],[('Thumb0',1)],[('Thumb1',1)],
     [('Thumb1',1)],[('Thumb2',1)],[('Thumb2',1)]],'skin',(128,64),cap_start=False)

wrist=Vector((.105,-.070,0));axis=Vector((.130,-.137,-.015)).normalized()
a=axis.cross(Vector((0,0,1))).normalized();b=axis.cross(a).normalized()
stations=[]
for d,ra,rb,fold in [(.009,.025,.026,0),(.022,.033,.031,.002),(.035,.030,.029,-.002),
                     (.052,.039,.037,.003),(.072,.037,.035,-.001),(.105,.043,.040,.001),
                     (.145,.046,.042,0),(.190,.047,.044,0)]:
    stations.append((wrist+axis*d+b*fold,a,b,ra,rb))
loft('Gathered navy workshop sleeve',stations,[[('Forearm',1)]]*len(stations),'cloth',(0,128),cap_start=False)
# Proper hollow cuff: outside, inside and annular rims are editable quads.
verts=[]
for d,ra,rb in [(-.003,.024,.025),(.011,.026,.027),(-.003,.020,.021),(.011,.022,.023)]:
    centre=wrist+axis*d;verts.extend(centre+a*u*ra+b*v*rb for u,v in section)
faces=[]
for r0,r1 in [(0,1),(3,2),(2,0),(1,3)]:
    faces.extend((r0*8+i,r0*8+(i+1)%8,r1*8+(i+1)%8,r1*8+i) for i in range(8))
uvs=[[(130+(k%8)*4,130+(k//8)*3) for k in face] for face in faces]
mesh_part('Rolled open leather cuff',verts,faces,[[('Forearm',1)]]*len(verts),'leather',uvs)

# Retain the wrist/socket motion shared with held objects. Adjust only finger
# tracks, with differentiated relaxation/pressure rather than identical bars.
for action in list(bpy.data.actions):
    if '.' in action.name:continue
    rig.animation_data.action=action
    start,end=map(int,action.frame_range)
    if end<=start:continue
    for frame in range(start,end+1):
        phase=(frame-start)/(end-start);pulse=math.sin(math.pi*phase)**2
        for finger_index,name in enumerate(('Index','Middle','Ring','Little','Thumb')):
            pressure={'idle':.006*math.sin(math.tau*phase+finger_index*.7),'windup':-.025*pulse,
                      'release':.014*pulse,'recovery':.009*pulse,'inspect':.025*pulse,
                      'open':.23*phase,'strong_grip':-.025*phase,'index_extend':.16*phase if name=='Index' else 0}.get(action.name,0)
            for j in range(3):
                pb=rig.pose.bones[name+str(j)];world_axis=Vector((0,0,1)) if name=='Thumb' else Vector((0,1,0))
                local=pb.bone.matrix_local.to_quaternion().inverted()@world_axis
                pb.rotation_mode='QUATERNION';pb.rotation_quaternion=Quaternion(local,pressure*(.65+.15*j)*(1-finger_index*.05))
                pb.keyframe_insert(data_path='rotation_quaternion',frame=frame,group=pb.name)
rig.animation_data.action=None;scene.frame_set(1)
for pb in rig.pose.bones:pb.rotation_quaternion=Quaternion()
bpy.ops.object.select_all(action='DESELECT');rig.hide_set(False);rig.select_set(True);bpy.context.view_layer.objects.active=rig
for obj in parts:obj.select_set(True)
bpy.ops.export_scene.gltf(filepath=str(RUNTIME/'hand.glb'),export_format='GLB',use_selection=True,
    export_animations=True,export_animation_mode='ACTIONS',export_skins=True,export_def_bones=False,
    export_force_sampling=True,export_frame_range=False,export_yup=False)

# A reference shaft is for grip review only; runtime export above contains hand only.
with bpy.data.libraries.load(str(ROOT/'assets/source/wand_ecosystem/artificer_tally/artificer_tally.blend'),link=False) as (src,dst):
    dst.objects=[name for name in src.objects if name[:2].isdigit()]
for obj in dst.objects:
    scene.collection.objects.link(obj);obj.parent=rig
    for mod in obj.modifiers:
        if mod.type=='ARMATURE':mod.object=rig
for obj in list(bpy.data.objects):
    if obj.type=='ARMATURE' and obj!=rig:bpy.data.objects.remove(obj,do_unlink=True)
scene.render.engine='CYCLES';scene.cycles.samples=16;scene.render.resolution_x=900;scene.render.resolution_y=700
scene.render.resolution_percentage=100;scene.view_settings.view_transform='Standard'
scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.16,.18,.22,1)
scene.world.node_tree.nodes['Background'].inputs[1].default_value=.65
cam=scene.camera;cam.data.type='ORTHO';cam.data.ortho_scale=.25
for obj in scene.objects:
    if obj.type=='LIGHT':obj.hide_render=True
data=bpy.data.lights.new('Hand neutral softbox','AREA');light=bpy.data.objects.new('Hand neutral softbox',data)
scene.collection.objects.link(light);light.location=(.1,.3,.5);data.energy=8;data.size=.5
light.rotation_euler=(Vector((.04,-.03,0))-light.location).to_track_quat('-Z','Y').to_euler()
for name,pos in [('grip_front',(.14,.09,.6)),('grip_side',(-.3,.12,.36))]:
    cam.location=pos;forward=(Vector((.045,-.025,0))-cam.location).normalized();right=forward.cross(Vector((0,1,0))).normalized()
    cam.rotation_euler=Matrix((right,right.cross(forward),-forward)).transposed().to_euler()
    scene.render.filepath=str(REVIEW/(name+'.png'));bpy.ops.render.render(write_still=True)
bpy.ops.wm.save_as_mainfile(filepath=str(OUT/'hand_v5.blend'))
(OUT/'manifest.json').write_text(json.dumps({'source':'hand_v5.blend','runtime':'assets/runtime/viewmodels/hand_v5/hand.glb',
    'blender':bpy.app.version_string,'grip_contract':'origin 0, shaft +Y, width 28-32mm, depth 24mm',
    'changes':['Staggered curled fingers','Thenar saddle and opposing thumb','Hollow cuff and gathered sleeve',
               'Individual finger pressure/relaxation; unchanged wrist/socket tracks'],
    'reference':'NEW art bible; civic_salvage_user_reference.png; not copying generated pixels into the hand',
    'status':'Native review required'},indent=2))
print('HAND_V5_EXPORTED_AND_RENDERED')
