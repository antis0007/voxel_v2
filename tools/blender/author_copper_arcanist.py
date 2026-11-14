"""UV and export the saved Blender artist mesh; never regenerate its geometry."""
import hashlib, json, math, struct, zlib
from pathlib import Path
import bpy
from mathutils import Vector, Matrix, Quaternion

ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT/'assets/source/blender/studies/wand_families_v2.blend'
OUT=ROOT/'target/copper-arcanist-review';OUT.mkdir(parents=True,exist_ok=True)
TEX=ROOT/'assets/source/textures/copper_arcanist';TEX.mkdir(parents=True,exist_ok=True)
source_hash=hashlib.sha256(SOURCE.read_bytes()).hexdigest()
bpy.ops.wm.open_mainfile(filepath=str(SOURCE))
scene=bpy.context.scene
scene.render.fps=30
if bpy.context.object and bpy.context.object.mode!='OBJECT':bpy.ops.object.mode_set(mode='OBJECT')
kit=bpy.data.collections['Copper Arcanist']
parts=[o for o in kit.objects if o.type=='MESH']
for obj in list(scene.objects):
    if obj.name not in kit.objects and obj.type not in ('CAMERA','LIGHT'):
        bpy.data.objects.remove(obj,do_unlink=True)
PALETTE={'wood':['292e35','3d454d','535d65'],
         'brass':['774831','ad7045','d59b61'],
         'ceramic':['a29878','ccc2a0','e4dabb'],
         'crystal':['4d4269','8f74aa','c8b3d9']}
TILES={'wood':(0,0,32,128),'brass':(32,0,32,128),
       'ceramic':(64,0,32,64),'crystal':(96,0,32,64)}
N=128;pixels=[(28,29,34,255)]*(N*N);mask=[(0,0,0,255)]*(N*N)
def rect(name,box,level):
    ox,oy,w,h=TILES[name];color=(*bytes.fromhex(PALETTE[name][level]),255)
    for y in range(max(0,box[1]),min(h,box[3])):
        for x in range(max(0,box[0]),min(w,box[2])):pixels[(oy+y)*N+ox+x]=color
for name,(ox,oy,w,h) in TILES.items():
    rect(name,(0,0,w,h),1)
    ident={'wood':32,'brass':96,'ceramic':160,'crystal':224}[name]
    for y in range(oy,oy+h):
        for x in range(ox,ox+w):mask[y*N+x]=(ident,255 if name=='brass' else 0,255 if name=='crystal' else 0,255)
# Broad connected marks on a mostly quiet field. No randomized/noise texture.
for box,level in [((4,10,6,46),0),((5,46,7,66),0),((20,78,23,117),0),
                   ((13,22,16,51),2),((8,94,11,122),2)]:rect('wood',box,level)
rect('brass',(1,0,4,128),2);rect('brass',(27,0,32,128),0)
rect('ceramic',(0,0,4,64),0);rect('ceramic',(25,0,28,64),2)
rect('crystal',(0,0,9,64),0);rect('crystal',(19,0,24,64),2)
def save_png(path,p):
    raw=bytes(c for v in p for c in v)
    def chunk(t,d):return struct.pack('>I',len(d))+t+d+struct.pack('>I',zlib.crc32(t+d))
    scan=b''.join(b'\0'+raw[y*N*4:(y+1)*N*4] for y in range(N))
    path.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',N,N,8,6,0,0,0))+
                     chunk(b'IDAT',zlib.compress(scan))+chunk(b'IEND',b''))
    return raw
(TEX/'albedo.rgba').write_bytes(save_png(TEX/'albedo.png',pixels))
save_png(TEX/'material_masks.png',mask)
(TEX/'palette.json').write_text(json.dumps({'ramps':PALETTE,'tiles':TILES,
    'masks':'R material ID:32 wood,96 brass,160 ceramic,224 crystal; G metal; B emission eligibility',
    'note':'Emission eligibility is not constant glow. Runtime mask support pending.'},indent=2))
image=bpy.data.images.load(str(TEX/'albedo.png'));image.pack()
materials={}
for name in PALETTE:
    mat=bpy.data.materials.new('Authored '+name);mat.use_nodes=True
    bs=mat.node_tree.nodes['Principled BSDF'];bs.inputs['Roughness'].default_value=.65 if name=='brass' else .85
    tex=mat.node_tree.nodes.new('ShaderNodeTexImage');tex.image=image;tex.interpolation='Closest'
    mat.node_tree.links.new(tex.outputs['Color'],bs.inputs['Base Color']);materials[name]=mat
for obj in parts:
    old=obj.data.materials[0].name
    name={'blackwood':'wood','brass':'brass','ceramic':'ceramic','violet':'crystal'}[old]
    obj.data.materials.clear();obj.data.materials.append(materials[name])
    mesh=obj.data;uv=mesh.uv_layers.new(name='Authored strip islands')
    ymin=min(v.co.y for v in mesh.vertices);ymax=max(v.co.y for v in mesh.vertices)
    # First cap retains the section's vertex count. Unwrap around the section,
    # duplicated seam at the last face; no reversed seam across the whole tile.
    sides=len(mesh.polygons[0].vertices);ox,oy,w,h=TILES[name]
    length=max(ymax-ymin,1e-5);paint_h=min(h-4,max(4,round(length*240)))
    for face in mesh.polygons:
        face.use_smooth=False
        indices=[mesh.loops[li].vertex_index for li in face.loop_indices]
        seam=0 in [i%sides for i in indices] and sides-1 in [i%sides for i in indices]
        cap=len(set(i//sides for i in indices))==1
        for li in face.loop_indices:
            idx=mesh.loops[li].vertex_index;v=mesh.vertices[idx].co
            if cap:
                px=ox+w/2;py=oy+h/2
            else:
                u=idx%sides
                if seam and u==0:u=sides
                px=ox+2+u/sides*(w-4)
                py=oy+2+(v.y-ymin)/length*paint_h
            uv.data[li].uv=(px/N,1-py/N)
# Reuse the existing attachment/clip contract, not its rejected geometry.
with bpy.data.libraries.load(str(ROOT/'assets/source/blender/first_person_arms.blend'),link=False) as (src,dst):
    dst.objects=['WayfarerRig']
rig=dst.objects[0];scene.collection.objects.link(rig)
for obj in parts:
    obj.parent=rig;group=obj.vertex_groups.new(name='GripSocket')
    group.add(list(range(len(obj.data.vertices))),1,'REPLACE')
    mod=obj.modifiers.new('Rigid grip attachment','ARMATURE');mod.object=rig
scene.frame_set(1)
for pb in rig.pose.bones:pb.rotation_quaternion=Quaternion()
for name,mat in materials.items():
    prior=bpy.data.materials.get(name)
    if prior and prior!=mat:prior.name='Unused study '+name
    mat.name=name
scene.render.engine='CYCLES';scene.cycles.samples=24
scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.25,.25,.25,1)
scene.world.node_tree.nodes['Background'].inputs[1].default_value=.7
scene.view_settings.view_transform='Standard'
data=bpy.data.lights.new('Broad neutral key','AREA');obj=bpy.data.objects.new('Broad neutral key',data)
scene.collection.objects.link(obj);obj.location=(.3,.5,.8);data.energy=10;data.size=.8
obj.rotation_euler=(Vector((0,.2,0))-obj.location).to_track_quat('-Z','Y').to_euler()
camera=scene.camera
def view(position):
    camera.location=position;f=(Vector((0,.15,0))-camera.location).normalized()
    r=f.cross(Vector((0,1,0))).normalized();camera.rotation_euler=Matrix((r,r.cross(f),-f)).transposed().to_euler()
def render(label):scene.render.filepath=str(OUT/(label+'.png'));bpy.ops.render.render(write_still=True)
camera.data.type='ORTHO';camera.data.ortho_scale=.65
scene.render.resolution_x=720;scene.render.resolution_y=900
for label,pos in [('front',(0,.15,2)),('side',(2,.15,0)),('three_quarter',(.8,.3,1.6))]:view(pos);render(label)
view((.8,.3,1.6));scene['status']='Textured Copper study, native review pending. GUI focus shortened to72percent.'
saved=ROOT/'assets/source/blender/copper_arcanist_artist.blend'
bpy.ops.wm.save_as_mainfile(filepath=str(saved))
bpy.ops.object.select_all(action='DESELECT');rig.select_set(True);bpy.context.view_layer.objects.active=rig
for obj in parts:obj.select_set(True)
bpy.ops.export_scene.gltf(filepath=str(OUT/'copper_arcanist.glb'),export_format='GLB',use_selection=True,
    export_animations=True,export_animation_mode='ACTIONS',export_skins=True,export_def_bones=False,
    export_force_sampling=True,export_frame_range=False,export_yup=False)
assert source_hash==hashlib.sha256(SOURCE.read_bytes()).hexdigest()
(OUT/'manifest.json').write_text(json.dumps({'artist_input':str(SOURCE),'input_sha256':source_hash,
    'source':str(saved),'source_sha256':hashlib.sha256(saved.read_bytes()).hexdigest(),
    'blender':bpy.app.version_string,'meshes':len(parts),'joints':len(rig.data.bones),
    'status':'Isolated review asset. Existing clip contract reused; new animation/hand and WGPU review pending.'},indent=2))
print('COPPER_ARTIST_EXPORT_SAVED')
