"""Three new instrument blockouts for silhouette review, not runtime assets."""
import math
from pathlib import Path
import bpy
from mathutils import Vector, Matrix

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT/'target/magi2000-silhouettes'
OUT.mkdir(parents=True, exist_ok=True)
bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene
scene.unit_settings.system = 'METRIC'
colors = {'wood':'634631', 'leather':'49352F', 'metal':'354D50',
          'copper':'AE7649', 'ceramic':'D5C7A3', 'core':'65B7AF'}
for name, color in colors.items():
    mat = bpy.data.materials.new(name)
    srgb = tuple(int(color[i:i+2],16)/255 for i in (0,2,4))
    rgb = tuple(c/12.92 if c<=.04045 else ((c+.055)/1.055)**2.4 for c in srgb)
    mat.diffuse_color = (*rgb,1)
    mat.use_nodes = True
    bs = mat.node_tree.nodes['Principled BSDF']
    bs.inputs['Base Color'].default_value = (*rgb,1)
    bs.inputs['Roughness'].default_value = .62 if name=='copper' else .85
    bs.inputs['Metallic'].default_value = .4 if name=='copper' else 0

offset = 0
collection = None
def attach(obj, name, material):
    obj.name = name
    for coll in list(obj.users_collection): coll.objects.unlink(obj)
    collection.objects.link(obj)
    obj.location.x += offset
    obj.data.materials.append(bpy.data.materials[material])
    return obj

def box(name, center, dimensions, material, bevel=.002):
    bpy.ops.mesh.primitive_cube_add(size=1, location=center)
    obj = bpy.context.object
    obj.dimensions = dimensions
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    if bevel:
        mod = obj.modifiers.new('Broad constructed edges','BEVEL')
        mod.width = bevel; mod.segments = 1
        bpy.ops.object.modifier_apply(modifier=mod.name)
    return attach(obj,name,material)

def section(name, levels, material, sides=8):
    vertices=[]
    for y,rx,rz,cx in levels:
        vertices.extend((cx+rx*math.cos(math.tau*i/sides+math.pi/8),y,
                         rz*math.sin(math.tau*i/sides+math.pi/8)) for i in range(sides))
    faces=[tuple(reversed(range(sides))),tuple(range((len(levels)-1)*sides,len(levels)*sides))]
    faces.extend((j*sides+i,j*sides+(i+1)%sides,(j+1)*sides+(i+1)%sides,(j+1)*sides+i)
                 for j in range(len(levels)-1) for i in range(sides))
    mesh=bpy.data.meshes.new(name); mesh.from_pydata(vertices,[],faces); mesh.update()
    obj=bpy.data.objects.new(name,mesh); scene.collection.objects.link(obj)
    return attach(obj,name,material)

def beam(name,a,b,width,depth,material):
    a,b=Vector(a),Vector(b)
    obj=box(name,(a+b)/2,(width,(b-a).length+width*.5,depth),material,.001)
    obj.rotation_euler=(b-a).to_track_quat('Y','Z').to_euler()
    return obj

def grip():
    section('Shaped timber grip',[(-.105,.013,.014,0),(-.09,.016,.016,0),
        (-.065,.015,.016,-.002),(-.02,.014,.015,-.003),(.015,.017,.017,0),
        (.034,.014,.014,0)],'wood')
    section('Pommel',[(-.11,.013,.014,0),(-.104,.017,.018,0),(-.093,.017,.018,0),
        (-.089,.014,.015,0)],'copper')
    section('Collar',[(.014,.018,.018,0),(.023,.020,.020,0),(.032,.017,.017,0)],'copper')
    section('Ceramic isolator',[(.032,.014,.014,0),(.04,.020,.020,0),(.061,.020,.020,0),
        (.067,.014,.014,0)],'ceramic')

for label, offset in [('A • spark lantern',-.16),('B • induction ring',0),('C • offset coil',.16)]:
    collection=bpy.data.collections.new(label); scene.collection.children.link(collection)
    grip()
    if label.startswith('A'):
        section('Dark stem',[(.059,.013,.014,0),(.09,.016,.017,-.002),
            (.119,.025,.021,0),(.132,.035,.027,0)],'metal')
        section('Lower socket',[(.122,.027,.022,0),(.133,.038,.030,0),
            (.145,.038,.030,0)],'copper')
        section('Lantern shoulder',[(.238,.039,.031,0),(.248,.035,.028,0),
            (.270,.025,.023,-.003),(.275,.025,.023,-.003)],'metal')
        section('Cap',[(.274,.023,.021,-.003),(.282,.026,.023,-.003),
            (.289,.022,.020,-.003)],'copper')
        for x in (-.032,.032):
            for z in (-.023,.023):
                beam('Structural rib',(x,.140,z),(x,.241,z),.011,.012,'metal')
        for y in (.15,.233):
            box('Window frame',(0,y,.030),(.072,.009,.008),'copper',.001)
        box('Back shell',(0,.193,-.026),(.059,.085,.009),'metal')
        box('Mineral cartridge',(0,.192,.003),(.031,.066,.028),'core',.005)
        for y in (.153,.232):
            box('Cartridge seat',(0,y,.003),(.036,.012,.031),'ceramic')
        for a,b in [((-.018,.072,0),(-.046,.103,0)),((-.046,.103,0),(-.046,.219,0)),
                    ((-.046,.219,0),(-.032,.228,0))]:
            beam('Power return',a,b,.007,.008,'copper')
    elif label.startswith('B'):
        box('Ring foot',(0,.095,0),(.032,.060,.030),'metal',.004)
        radius=.069; cy=.199
        for i in range(12):
            a=math.tau*i/12; b=math.tau*(i+1)/12
            beam('Induction segment',(math.cos(a)*radius,cy+math.sin(a)*radius,0),
                (math.cos(b)*radius,cy+math.sin(b)*radius,0),.019,.024,
                'ceramic' if i in (3,8) else 'copper' if i%3 else 'metal')
        box('Top suspension',(0,.246,0),(.011,.030,.012),'copper')
        box('Suspended cell',(0,.203,0),(.025,.050,.024),'core',.004)
        for y in (.175,.23): box('Cell cap',(0,y,0),(.030,.008,.029),'metal')
    else:
        beam('Offset body',(0,.062,0),(-.023,.114,0),.034,.032,'metal')
        beam('Offset neck',(-.023,.114,0),(-.015,.186,0),.035,.034,'metal')
        box('Coil housing',(.015,.216,0),(.105,.070,.052),'metal',.006)
        for x in (.035,.057):
            box('Copper coil', (x,.216,0),(.011,.079,.060),'copper',.005)
        box('Recessed window',(-.017,.216,.029),(.040,.049,.010),'copper')
        box('Flux cell',(-.017,.216,.035),(.026,.034,.005),'core',.001)
        box('Dielectric neck',(-.025,.164,0),(.038,.016,.037),'ceramic',.003)

scene.render.engine='CYCLES'; scene.cycles.samples=32
scene.render.resolution_x=1500; scene.render.resolution_y=1100
scene.render.resolution_percentage=100
scene.world=bpy.data.worlds.new('Neutral studio')
scene.world.color=(.28,.28,.28)
scene.view_settings.view_transform='AgX'
def light(name,location,power,size):
    data=bpy.data.lights.new(name,'AREA'); data.energy=power; data.shape='DISK'; data.size=size
    obj=bpy.data.objects.new(name,data); scene.collection.objects.link(obj); obj.location=location
    obj.rotation_euler=(Vector((0,.10,0))-obj.location).to_track_quat('-Z','Y').to_euler()
light('Large key',(-.4,.5,.8),25,.6); light('Fill',(.5,.05,.4),8,.5)
data=bpy.data.cameras.new('Study camera'); camera=bpy.data.objects.new('Study camera',data)
scene.collection.objects.link(camera); scene.camera=camera; data.type='ORTHO'; data.ortho_scale=.62
def camera_view(position):
    camera.location=position
    forward=(Vector((0,.09,0))-camera.location).normalized()
    right=forward.cross(Vector((0,1,0))).normalized()
    up=right.cross(forward).normalized()
    camera.rotation_euler=Matrix((right,up,-forward)).transposed().to_euler()
camera_view((.22,.16,1.4))
bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/'assets/source/blender/studies/magi2000_silhouettes.blend'))
scene.render.filepath=str(OUT/'three_quarter.png'); bpy.ops.render.render(write_still=True)
clay=bpy.data.materials.new('Neutral clay'); clay.diffuse_color=(.45,.45,.45,1)
scene.view_layers[0].material_override=clay
scene.render.filepath=str(OUT/'clay.png'); bpy.ops.render.render(write_still=True)
black=bpy.data.materials.new('Silhouette'); black.use_nodes=True
bs=black.node_tree.nodes.get('Principled BSDF'); bs.inputs['Base Color'].default_value=(0,0,0,1)
bs.inputs['Roughness'].default_value=1; bs.inputs['Specular IOR Level'].default_value=0
scene.view_layers[0].material_override=black
scene.world.use_nodes=True
scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.8,.8,.8,1)
for label,z in [('front',1.4),('rear',-1.4)]:
    camera_view((0,.09,z))
    scene.render.filepath=str(OUT/(label+'_silhouettes.png')); bpy.ops.render.render(write_still=True)
print('THREE_NEW_SILHOUETTES_RENDERED')
