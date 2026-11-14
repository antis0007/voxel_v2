"""Authored pixel clusters and three minimal, rigged Blender item studies.

Run in Blender. These are review assets, never replacements for the live kit.
Geometry stations and icon polygons are explicit artist inputs, not random seeds.
"""
import json, math, struct, zlib
from pathlib import Path
import bpy
from mathutils import Vector, Matrix

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT/'assets/source/wand_ecosystem/minimal_v1'
REVIEW = ROOT/'target/ecosystem-minimal-v1'
OUT.mkdir(parents=True, exist_ok=True); REVIEW.mkdir(parents=True, exist_ok=True)
PALETTE = {
    'bone': ['8d8771', 'c8c3a2', 'ece3bf'],
    'iron': ['252d38', '435463', '718080'],
    'cord': ['59363c', '98514b', 'cc7961'],
    'brass': ['68523d', 'a38a54', 'd5bc77'],
    'charcoal': ['272630', '444047', '66595c'],
    'ember': ['8e3d3c', 'db6949', 'f6b967'],
    'leather': ['333d39', '596053', '888c6e'],
    'rune': ['364b54', '639b96', 'b4d2b1'],
}

def png(path, width, height, pixels):
    raw = bytes(c for p in pixels for c in p)
    def chunk(t,d): return struct.pack('>I',len(d))+t+d+struct.pack('>I',zlib.crc32(t+d))
    path.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',width,height,8,6,0,0,0))+
        chunk(b'IDAT',zlib.compress(b''.join(b'\0'+raw[y*width*4:(y+1)*width*4] for y in range(height))))+chunk(b'IEND',b''))
    return raw

def color(name, level): return (*bytes.fromhex(PALETTE[name][level]),255)

def polygon(pixels, width, points, rgba):
    # Scan-convert at pixel centres: original integer-grid sprite authoring.
    for y in range(len(pixels)//width):
        for x in range(width):
            inside=False; j=len(points)-1
            for i,(ax,ay) in enumerate(points):
                bx,by=points[j]
                if (ay>y+.5)!=(by>y+.5) and x+.5<(bx-ax)*(y+.5-ay)/(by-ay)+ax: inside=not inside
                j=i
            if inside: pixels[y*width+x]=rgba

# Each mark is a deliberate contiguous shape on the final 32px grid.
ICONS = {
 'bone_needle': [
  ('iron',0,[(4,26),(8,29),(13,23),(20,17),(25,10),(27,3),(23,7),(18,14),(10,21)]),
  ('bone',0,[(7,25),(10,26),(19,17),(24,10),(26,4),(21,10),(17,15)]),
  ('bone',1,[(8,24),(10,24),(19,15),(23,10),(25,5),(19,12)]),
  ('bone',2,[(9,22),(11,22),(19,14),(23,8),(20,11)]),
  ('cord',0,[(9,19),(14,22),(12,25),(7,22)]),
  ('cord',1,[(9,19),(14,22),(13,23),(8,21)]),
  ('cord',2,[(10,19),(13,21),(12,22),(9,20)]),
  ('cord',1,[(13,22),(16,24),(16,27),(14,26),(14,24)]),
 ],
 'runic_key': [
  ('iron',0,[(4,26),(7,29),(20,16),(29,8),(28,4),(24,2),(19,3),(15,7),(16,12)]),
  ('brass',0,[(5,26),(7,27),(19,15),(27,8),(26,5),(23,3),(19,4),(16,7),(17,12)]),
  ('brass',1,[(7,25),(9,25),(20,14),(27,7),(23,3),(19,4),(16,7),(18,11)]),
  ('brass',2,[(17,7),(20,4),(23,4),(26,7),(25,8),(22,5),(20,5),(18,8)]),
  ('transparent',0,[(19,8),(22,5),(25,8),(22,11)]),
  ('iron',0,[(12,21),(16,25),(18,23),(16,21),(19,18),(17,16)]),
  ('brass',1,[(13,21),(16,23),(17,22),(15,20),(18,18),(17,17)]),
  ('leather',1,[(5,26),(7,27),(12,22),(10,20)]),
  ('leather',2,[(6,25),(8,26),(9,25),(7,24)]),
  ('rune',1,[(20,13),(22,11),(23,12),(21,14)]),
 ],
 'cinder_rod': [
  ('charcoal',0,[(4,26),(8,29),(13,22),(19,17),(25,10),(27,5),(25,3),(21,5),(18,11),(12,18)]),
  ('charcoal',1,[(6,25),(8,27),(13,21),(19,16),(24,10),(25,5),(22,6),(19,12),(13,18)]),
  ('charcoal',2,[(7,24),(8,25),(14,19),(14,17),(12,19)]),
  ('charcoal',2,[(16,16),(17,17),(23,10),(24,6),(22,8)]),
  ('ember',0,[(17,13),(19,14),(22,11),(23,8),(21,8),(20,11)]),
  ('ember',1,[(18,13),(19,14),(21,11),(22,8),(21,9),(20,12)]),
  ('ember',2,[(20,11),(21,10),(21,12),(20,13)]),
  ('ember',0,[(12,18),(14,18),(16,16),(15,15)]),
  ('ember',1,[(13,18),(14,18),(16,16),(15,16)]),
 ],
}
icons={}
for name,marks in ICONS.items():
    p=[(0,0,0,0)]*1024
    for material,level,points in marks: polygon(p,32,points,(0,0,0,0) if material=='transparent' else color(material,level))
    icons[name]=p; png(OUT/(name+'_32.png'),32,32,p)
(OUT/'icon_sources.json').write_text(json.dumps({'size':32,'palette':PALETTE,'polygons':ICONS},indent=2))
sheet=[(31,35,44,255)]*(320*120)
for i,p in enumerate(icons.values()):
    for y in range(32):
        for x in range(32):
            if not p[y*32+x][3]: continue
            for dy in range(3):
                for dx in range(3):sheet[(12+y*3+dy)*320+8+i*104+x*3+dx]=p[y*32+x]
png(REVIEW/'icons_3x.png',320,120,sheet)

# Top-left 128px region is used only by the candidate wand. Original skin,
# cloth and nail atlas tiles occupy the lower half and remain byte-identical.
raw=(ROOT/'assets/runtime/viewmodels/wayfarer.rgba').read_bytes()
atlas=[tuple(raw[i:i+4]) for i in range(0,len(raw),4)]
tiles={}
for i,name in enumerate(PALETTE):
    ox=(i%4)*32; oy=(i//4)*64; tiles[name]=(ox,oy)
    for y in range(64):
        for x in range(32):
            level=1
            if name in ('bone','iron','brass'): level=0 if x<5 else 2 if 23<=x<28 else 1
            if name=='charcoal': level=0 if 4<=x<7 and 8<=y<36 or 18<=x<22 and 38<=y<60 else 1
            if name=='cord': level=0 if y%16<3 else 1
            if name=='ember': level=2 if 12<=x<19 else 0 if x<5 or x>=27 else 1
            if name=='leather': level=0 if x<5 else 2 if x>26 else 1
            atlas[(oy+y)*256+ox+x]=color(name,level)
(OUT/'atlas.rgba').write_bytes(png(OUT/'atlas.png',256,256,atlas))
(OUT/'palette.json').write_text(json.dumps({'ramps':PALETTE,'tiles':tiles,'sampling':'nearest; 128 texels/metre longitudinal target'},indent=2))

bpy.ops.wm.read_factory_settings(use_empty=True)
scene=bpy.context.scene; scene.unit_settings.system='METRIC'; scene.render.fps=30
im=bpy.data.images.load(str(OUT/'atlas.png'));im.pack()
mats={}
for name in PALETTE:
    mat=bpy.data.materials.new(name);mat.use_nodes=True
    tex=mat.node_tree.nodes.new('ShaderNodeTexImage');tex.image=im;tex.interpolation='Closest'
    bs=mat.node_tree.nodes['Principled BSDF'];mat.node_tree.links.new(tex.outputs['Color'],bs.inputs['Base Color'])
    bs.inputs['Roughness'].default_value=.8;bs.inputs['Specular IOR Level'].default_value=.15
    mats[name]=mat
with bpy.data.libraries.load(str(ROOT/'assets/source/blender/first_person_arms.blend'),link=False) as (src,dst): dst.objects=['WayfarerRig']
rig=dst.objects[0];scene.collection.objects.link(rig);scene.frame_set(1)
family=None

def mesh(name,vertices,faces,material):
    data=bpy.data.meshes.new(name);data.from_pydata(vertices,[],faces);data.update()
    obj=bpy.data.objects.new(name,data);family.objects.link(obj);data.materials.append(mats[material])
    obj['functional_role']=name;obj.parent=rig
    group=obj.vertex_groups.new(name='GripSocket');group.add(list(range(len(vertices))),1,'REPLACE')
    obj.modifiers.new('Grip attachment','ARMATURE').object=rig
    uv=data.uv_layers.new(name='Pixel islands');ox,oy=tiles[material]
    # Per-face strips keep the same pixel scale along the wand and avoid
    # stretching long faces across an arbitrary full-resolution square.
    for face in data.polygons:
        axis=0 if abs(face.normal.z)>.5 else 2
        for li in face.loop_indices:
            v=data.vertices[data.loops[li].vertex_index].co
            px=ox+16+max(-13,min(13,v[axis]*128))
            py=oy+2+max(0,min(59,(v.y+.12)*100))
            uv.data[li].uv=(px/256,1-py/256)
    return obj

def loft(name,stations,material,sides=6):
    vs=[(x+rx*math.cos(math.tau*i/sides),y,z+rz*math.sin(math.tau*i/sides))
        for x,y,z,rx,rz in stations for i in range(sides)]
    fs=[tuple(reversed(range(sides))),tuple(range(len(vs)-sides,len(vs)))]
    fs += [(j*sides+i,j*sides+(i+1)%sides,(j+1)*sides+(i+1)%sides,(j+1)*sides+i)
           for j in range(len(stations)-1) for i in range(sides)]
    return mesh(name,vs,fs,material)

def flat(name,points,depth,material):
    n=len(points);vs=[(x,y,z) for z in (-depth/2,depth/2) for x,y in points]
    fs=[tuple(reversed(range(n))),tuple(range(n,2*n))]+[(i,(i+1)%n,(i+1)%n+n,i+n) for i in range(n)]
    return mesh(name,vs,fs,material)

def band(name,y,r,material,height=.012):return loft(name,[(0,y-height/2,0,r,r*.85),(0,y+height/2,0,r,r*.85)],material,6)
families=[]
for key in ICONS:
    family=bpy.data.collections.new(key);scene.collection.children.link(family);families.append(family)
    if key=='bone_needle':
        loft('Continuous curved bone',[(0,-.115,0,.013,.012),(0,-.09,0,.020,.015),
            (0,.03,0,.018,.014),(-.006,.14,0,.014,.012),(-.026,.28,0,.012,.009),
            (-.038,.39,0,.009,.007),(-.027,.46,0,.004,.003),(-.013,.495,0,.0007,.0007)],'bone')
        band('Basal grip ferrule',-.095,.021,'iron',.026)
        band('Red retaining cord',.025,.020,'cord',.020)
        flat('Short cord tail',[(.014,.023),(.028,.006),(.024,-.023),(.019,-.013),(.020,.004),(.011,.015)],.006,'cord')
        tip=(-.013,.495,0)
    elif key=='runic_key':
        flat('Forged key spine',[(-.014,-.11),(.014,-.11),(.011,.27),(-.011,.27)],.020,'iron')
        band('Worn leather grip',-.045,.019,'leather',.105)
        # Open diamond bow assembled as four genuine frame members, no filled disc.
        outer=[(0,.39),(.052,.329),(0,.265),(-.043,.329)]
        inner=[(0,.365),(.030,.329),(0,.291),(-.024,.329)]
        for i in range(4):flat('Open bow segment '+str(i),[outer[i],outer[(i+1)%4],inner[(i+1)%4],inner[i]],.017,'brass')
        flat('Unequal ward teeth',[(.009,.09),(.044,.09),(.044,.108),(.025,.108),(.025,.144),(.050,.144),(.050,.164),(.009,.164)],.018,'brass')
        band('Inscribed neck',.251,.015,'rune',.018)
        tip=(0,.39,0)
    else:
        loft('Charred continuous stave',[(0,-.12,0,.014,.013),(0,-.09,0,.019,.016),
            (0,.03,0,.016,.014),(.008,.16,0,.019,.015),(-.010,.29,0,.018,.016),
            (.003,.405,0,.022,.017),(-.003,.449,0,.014,.013)],'charcoal',6)
        # Emission is restricted to two long fissures, not a large crystal/battery.
        obj=flat('Upper ember fissure',[(-.003,.28),(.003,.28),(.008,.32),(.004,.36),(.008,.395),(.003,.415),(-.001,.39),(-.001,.36),(.002,.32)],.001,'ember')
        obj.location.z=.0155
        obj=flat('Lower ember fissure',[(.006,.15),(.011,.15),(.014,.18),(.009,.21),(.004,.21),(.009,.18)],.001,'ember');obj.location.z=.014
        tip=(-.003,.449,0)
    marker=bpy.data.objects.new('muzzle_'+key,None);family.objects.link(marker);marker.location=tip;marker.empty_display_type='ARROWS';marker.empty_display_size=.02
    family['muzzle']=tip

scene.world=bpy.data.worlds.new('Neutral');scene.world.use_nodes=True
scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.21,.24,.28,1)
scene.world.node_tree.nodes['Background'].inputs[1].default_value=.7
scene.render.engine='CYCLES';scene.cycles.samples=16;scene.view_settings.view_transform='Standard'
light=bpy.data.lights.new('Soft key','AREA');light.energy=12;light.size=1
obj=bpy.data.objects.new('Soft key',light);scene.collection.objects.link(obj);obj.location=(-.4,.8,1)
obj.rotation_euler=(Vector((0,.18,0))-obj.location).to_track_quat('-Z','Y').to_euler()
camdata=bpy.data.cameras.new('Review camera');cam=bpy.data.objects.new('Review camera',camdata);scene.collection.objects.link(cam);scene.camera=cam
camdata.type='ORTHO';camdata.ortho_scale=.72
scene.render.resolution_x=400;scene.render.resolution_y=600;scene.render.resolution_percentage=100
def camera(position):
    cam.location=position;f=(Vector((0,.18,0))-cam.location).normalized();r=f.cross(Vector((0,1,0))).normalized()
    cam.rotation_euler=Matrix((r,r.cross(f),-f)).transposed().to_euler()
manifest=[]
for family in families:
    for other in families:other.hide_render=other!=family;other.hide_viewport=other!=family
    parts=[o for o in family.objects if o.type=='MESH']
    for label,pos in [('front',(0,.18,2)),('side',(2,.18,0)),('three_quarter',(.8,.3,1.8))]:
        camera(pos);scene.render.filepath=str(REVIEW/(family.name+'_'+label+'.png'));bpy.ops.render.render(write_still=True)
    bpy.ops.object.select_all(action='DESELECT');rig.select_set(True);bpy.context.view_layer.objects.active=rig
    for o in parts:o.select_set(True)
    bpy.ops.export_scene.gltf(filepath=str(OUT/(family.name+'.glb')),export_format='GLB',use_selection=True,
        export_animations=True,export_animation_mode='ACTIONS',export_skins=True,export_def_bones=False,
        export_force_sampling=True,export_frame_range=False,export_yup=False)
    manifest.append({'name':family.name,'triangles':sum(sum(len(p.vertices)-2 for p in o.data.polygons) for o in parts),'muzzle':list(family['muzzle'])})
for other in families:other.hide_render=other.name!='runic_key';other.hide_viewport=other.name!='runic_key'
camera((.8,.3,1.8))
bpy.ops.object.select_all(action='DESELECT')
for o in bpy.data.collections['runic_key'].objects:
    if o.type=='MESH':o.select_set(True);bpy.context.view_layer.objects.active=o
for area in bpy.context.screen.areas:
    if area.type=='VIEW_3D':area.spaces.active.region_3d.view_perspective='CAMERA'
scene['review_status']='Original minimal studies. Hand/animation inherited. Native review required.'
bpy.ops.wm.save_as_mainfile(filepath=str(OUT/'ecosystem_items.blend'))
(OUT/'manifest.json').write_text(json.dumps({'blender':bpy.app.version_string,'items':manifest,'status':scene['review_status']},indent=2))
print('ECOSYSTEM_ITEMS_EXPORTED',manifest)
