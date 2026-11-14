"""Isolated Minecraft-directed shape studies; no production export."""
import bpy, math
from pathlib import Path
from mathutils import Vector, Matrix
ROOT=Path(__file__).resolve().parents[2]
OUT=ROOT/'target/chunky-wand-ideas';OUT.mkdir(parents=True,exist_ok=True)
bpy.ops.wm.read_factory_settings(use_empty=True)
scene=bpy.context.scene
palette={'wood':'735035','cut':'a77c4a','leather':'49372e','copper':'b67b49',
         'copper_dark':'714831','cream':'d4c69c','stone':'666475','edge':'9993a5',
         'rune':'95c4ba','amber':'d6a152','dark':'303b40'}
mats={}
for name,color in palette.items():
    m=bpy.data.materials.new(name);m.use_nodes=True
    rgb=[int(color[i:i+2],16)/255 for i in (0,2,4)]
    rgb=[c/12.92 if c<=.04045 else ((c+.055)/1.055)**2.4 for c in rgb]
    m.diffuse_color=(*rgb,1)
    bs=m.node_tree.nodes['Principled BSDF'];bs.inputs['Base Color'].default_value=(*rgb,1)
    bs.inputs['Roughness'].default_value=.85
    mats[name]=m
families=[]
def box(name,p,size,mat):
    bpy.ops.mesh.primitive_cube_add(size=1,location=p)
    o=bpy.context.object;o.name=name;o.scale=size
    bpy.ops.object.transform_apply(location=False,rotation=False,scale=True)
    o.data.materials.append(mats[mat]);family.objects.link(o)
    for c in list(o.users_collection):
        if c!=family:c.objects.unlink(o)
    return o
def stem(name,rings,mat):
    vs=[]
    for x,y,w,d in rings:vs.extend([(x-w/2,y,-d/2),(x+w/2,y,-d/2),(x+w/2,y,d/2),(x-w/2,y,d/2)])
    fs=[(3,2,1,0),tuple(range(len(vs)-4,len(vs)))]
    for j in range(len(rings)-1):
        for i in range(4):fs.append((j*4+i,j*4+(i+1)%4,(j+1)*4+(i+1)%4,(j+1)*4+i))
    mesh=bpy.data.meshes.new(name);mesh.from_pydata(vs,[],fs);mesh.update()
    o=bpy.data.objects.new(name,mesh);family.objects.link(o);o.data.materials.append(mats[mat]);return o
for idx,name in enumerate(['Thornroot','Copper Arcanist','Astral Splinter']):
    family=bpy.data.collections.new(name);scene.collection.children.link(family);families.append(family)
    if idx==0:
        stem('Carved branch',[(0,-.12,.036,.032),(0,0,.033,.03),(-.012,.13,.029,.027),
              (.008,.25,.038,.03),(.008,.33,.026,.025)],'wood')
        box('Worn grip',(0,-.065,0),(.038,.10,.035),'leather')
        for y in [-.10,-.06,-.02]:box('Binding',(0,y,0),(.041,.009,.038),'cut')
        stem('Carved amber seat',[(.008,.285,.036,.032),(.004,.310,.047,.037),
             (.008,.332,.039,.033)],'wood')
        stem('Seated amber',[(.008,.322,.032,.03),(.008,.347,.032,.03),(.008,.363,.016,.018)],'amber')
        box('Carved face',(-.003,.16,.014),(.012,.067,.002),'cut')
    elif idx==1:
        stem('Square tapered core',[(0,-.12,.033,.031),(0,0,.031,.029),(0,.29,.025,.025)],'wood')
        box('Leather handle',(0,-.062,0),(.036,.105,.034),'leather')
        for y in [-.116,-.005]:box('Copper grip collar',(0,y,0),(.041,.012,.039),'copper')
        box('Ceramic section',(0,.12,0),(.034,.08,.032),'cream')
        box('Copper conductor',(0,.12,.017),(.008,.075,.003),'copper')
        box('Closed emitter',(0,.295,0),(.061,.086,.048),'copper_dark')
        box('Front copper frame',(0,.295,.026),(.055,.074,.006),'copper')
        box('Ceramic aperture',(0,.300,.030),(.035,.047,.004),'cream')
        box('Inset casting face',(0,.306,.0325),(.019,.027,.002),'rune')
        box('Emitter cap',(0,.341,0),(.066,.012,.052),'copper')
    else:
        stem('Carved slate spine',[(0,-.12,.034,.029),(0,0,.032,.028),(-.008,.18,.039,.030),
             (-.008,.29,.055,.034),(.003,.35,.035,.032)],'stone')
        box('Grip',(0,-.064,0),(.037,.09,.032),'dark')
        for y in [-.11,-.02]:box('Stone grip band',(0,y,0),(.043,.014,.036),'edge')
        box('Carved rune field',(-.008,.276,.0171),(.031,.055,.0004),'stone')
        box('Rune vertical',(-.008,.276,.0174),(.006,.042,.0004),'rune')
        box('Rune cross',(-.008,.282,.0174),(.024,.006,.0004),'rune')
        box('Stepped shoulder',(-.030,.236,0),(.018,.037,.033),'stone')
    for o in family.objects:o.location.x+=(idx-1)*.18
world=bpy.data.worlds.new('Neutral');scene.world=world;world.use_nodes=True
world.node_tree.nodes['Background'].inputs[0].default_value=(.22,.22,.22,1)
world.node_tree.nodes['Background'].inputs[1].default_value=.45
scene.render.engine='CYCLES';scene.cycles.samples=32
scene.view_settings.view_transform='Standard';scene.view_settings.look='None'
for p,power,size in [((-.5,.6,1),10,1),((.6,.2,.5),4,.8)]:
    bpy.ops.object.light_add(type='AREA',location=p);o=bpy.context.object;o.data.energy=power;o.data.size=size
    o.rotation_euler=(Vector((0,.1,0))-o.location).to_track_quat('-Z','Y').to_euler()
bpy.ops.object.camera_add(location=(.20,.20,1.5));cam=bpy.context.object
forward=(Vector((0,.12,0))-cam.location).normalized()
right=forward.cross(Vector((0,1,0))).normalized();up=right.cross(forward).normalized()
cam.rotation_euler=Matrix((right,up,-forward)).transposed().to_euler()
cam.data.type='ORTHO';cam.data.ortho_scale=.68;scene.camera=cam
scene.render.resolution_x=1100;scene.render.resolution_y=1000;scene.render.resolution_percentage=100
scene.render.image_settings.file_format='PNG'
scene.render.filepath=str(OUT/'three_shapes.png');bpy.ops.render.render(write_still=True)
black=bpy.data.materials.new('Silhouette');black.use_nodes=True
black.node_tree.nodes.clear()
em=black.node_tree.nodes.new('ShaderNodeEmission');em.inputs[0].default_value=(0,0,0,1)
output=black.node_tree.nodes.new('ShaderNodeOutputMaterial');black.node_tree.links.new(em.outputs[0],output.inputs[0])
scene.view_layers[0].material_override=black
scene.render.resolution_x=330;scene.render.resolution_y=300
scene.render.filepath=str(OUT/'small_silhouettes.png');bpy.ops.render.render(write_still=True)
scene.view_layers[0].material_override=None
scene.render.resolution_x=1100;scene.render.resolution_y=1000
scene['status']='Alternative shape study; untextured, unrigged, not runtime-approved'
bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/'assets/source/blender/studies/chunky_wand_ideas.blend'))
