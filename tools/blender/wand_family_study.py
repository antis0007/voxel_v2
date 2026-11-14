"""Original slender wand studies. Run with Blender; preserves rejected machines."""
import math
from pathlib import Path
import bpy
from mathutils import Vector, Matrix

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT/'target/wand-family-study'
OUT.mkdir(parents=True, exist_ok=True)
bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene
scene.unit_settings.system = 'METRIC'
materials = {}
for name, value in {'bark':'675039','heartwood':'AC8154','fiber':'C0AA77',
                    'blackwood':'383440','brass':'B58048','ceramic':'CCC1A0',
                    'mineral':'CDC6D7','binding':'494356','amber':'B87D44',
                    'violet':'9D80BB'}.items():
    srgb = [int(value[i:i+2],16)/255 for i in (0,2,4)]
    rgb = [v/12.92 if v<=.04045 else ((v+.055)/1.055)**2.4 for v in srgb]
    mat = bpy.data.materials.new(name); mat.use_nodes=True
    bs = mat.node_tree.nodes['Principled BSDF']
    bs.inputs['Base Color'].default_value=(*rgb,1)
    bs.inputs['Roughness'].default_value=.55 if name=='brass' else .82
    mat.diffuse_color=(*rgb,1); materials[name]=mat

family=None
def loft(name, rings, material, sides=6):
    # Each station specifies centre x,y,z and deliberately authored elliptical radii.
    vertices=[]
    for x,y,z,rx,rz in rings:
        vertices.extend((x+rx*math.cos(math.tau*i/sides),y,
                         z+rz*math.sin(math.tau*i/sides)) for i in range(sides))
    faces=[tuple(reversed(range(sides))),tuple(range((len(rings)-1)*sides,len(rings)*sides))]
    faces.extend((j*sides+i,j*sides+(i+1)%sides,(j+1)*sides+(i+1)%sides,(j+1)*sides+i)
                 for j in range(len(rings)-1) for i in range(sides))
    mesh=bpy.data.meshes.new(name);mesh.from_pydata(vertices,[],faces);mesh.update()
    obj=bpy.data.objects.new(name,mesh);family.objects.link(obj)
    obj.data.materials.append(materials[material]);obj['part']='wand';obj['role']=name
    return obj

def band(name,y,radius,material,height=.006,x=0,z=0):
    return loft(name,[(x,y-height/2,z,radius,radius),(x,y+height/2,z,radius,radius)],material,8)

def marker(name,p):
    obj=bpy.data.objects.new(name,None);family.objects.link(obj);obj.location=p
    obj.empty_display_type='ARROWS';obj.empty_display_size=.025

families=[]
for name in ['Thornroot','Copper Arcanist','Astral Splinter']:
    family=bpy.data.collections.new(name);scene.collection.children.link(family);families.append(family)
    marker('grip_origin',(0,0,0));marker('casting_axis',(0,.35,0))
    if name=='Thornroot':
        loft('Carved continuous branch',[(0,-.115,0,.012,.013),(-.003,-.09,0,.016,.014),
            (0,.015,0,.014,.013),(.008,.075,0,.016,.012),(-.014,.18,.003,.011,.011),
            (.004,.29,0,.012,.009),(-.011,.37,0,.009,.008),(-.030,.43,0,.005,.004),
            (-.024,.46,0,.001,.001)],'bark')
        loft('Living side prong',[(.003,.29,0,.010,.008),(.025,.35,.002,.007,.006),
            (.025,.401,.002,.005,.004),(.012,.421,0,.001,.001)],'heartwood')
        loft('Caught amber seed',[(0,.382,0,.003,.003),(.001,.397,0,.009,.007),
            (-.003,.413,0,.004,.004),(-.003,.417,0,.001,.001)],'amber',5)
        for i in range(4): band('Fiber grip binding',-.082+i*.009,.016,'fiber',.006,x=-.002)
        loft('Pruned growth scar',[(-.008,.165,.003,.009,.008),(-.031,.179,.003,.004,.005),
            (-.035,.185,.003,.001,.001)],'bark')
        marker('muzzle',(-.003,.423,0))
    elif name=='Copper Arcanist':
        loft('Tapered blackwood core',[(0,-.12,0,.011,.011),(0,-.108,0,.015,.014),
            (0,-.02,0,.014,.013),(0,.015,0,.012,.012),(0,.18,0,.011,.010),
            (0,.31,0,.008,.008),(0,.348,0,.009,.008)],'blackwood',8)
        band('Pommel cap',-.111,.016,'brass',.012)
        band('Grip stop',.012,.016,'brass',.008)
        loft('Ivory dielectric insert',[(0,.13,0,.012,.011),(0,.184,0,.011,.010)],'ceramic',8)
        for i in range(4): band('Induction winding',.198+i*.012,.012-i*.0005,'brass',.004)
        band('Terminal ferrule',.326,.013,'brass',.016)
        loft('Long conducting tine',[(-.006,.33,0,.009,.008),(-.019,.363,0,.006,.006),
            (-.023,.402,0,.005,.005),(-.011,.435,0,.002,.003)],'brass')
        loft('Short conducting tine',[(.006,.33,0,.009,.008),(.021,.36,0,.005,.006),
            (.020,.39,0,.004,.005),(.012,.404,0,.002,.003)],'brass')
        loft('Faceted casting focus',[(0,.365,0,.003,.004),(.001,.386,0,.010,.008),
            (.003,.413,0,.006,.005),(.003,.436,0,.001,.001)],'violet',5)
        band('Tuning collar',.264,.013,'brass',.007)
        loft('Tuning index',[(.009,.259,0,.005,.005),(.017,.267,0,.004,.005),
            (.013,.275,0,.003,.004)],'brass')
        marker('muzzle',(.003,.436,0))
    else:
        loft('Split mineral spine',[(0,-.13,0,.010,.009),(.005,-.10,0,.014,.011),
            (0,0,0,.014,.010),(-.008,.12,.002,.012,.009),(.003,.255,0,.011,.008),
            (-.014,.36,0,.012,.007),(-.029,.44,0,.008,.005),(-.019,.50,0,.001,.001)],'mineral',5)
        loft('Unequal mineral prong',[(0,.268,0,.009,.007),(.023,.336,0,.006,.005),
            (.025,.397,0,.004,.004),(.018,.421,0,.001,.001)],'mineral',5)
        for y in [-.085,-.065,.045]: band('Dark retaining band',y,.015,'binding',.008)
        loft('Suspended splinter',[(0,.414,0,.002,.002),(.004,.437,.002,.008,.006),
            (.008,.468,0,.004,.004),(.007,.481,0,.001,.001)],'violet',5)
        marker('muzzle',(.007,.484,0))

scene.world=bpy.data.worlds.new('White review');scene.world.use_nodes=True
scene.world.node_tree.nodes['Background'].inputs[0].default_value=(1,1,1,1)
scene.world.node_tree.nodes['Background'].inputs[1].default_value=.8
scene.render.engine='CYCLES';scene.cycles.samples=16
scene.render.resolution_x=1024;scene.render.resolution_y=1024;scene.render.resolution_percentage=100
scene.view_settings.view_transform='Standard'
data=bpy.data.cameras.new('Review');camera=bpy.data.objects.new('Review',data)
scene.collection.objects.link(camera);scene.camera=camera;data.type='ORTHO';data.ortho_scale=.76
def view(position,target=(0,.18,0)):
    camera.location=position;f=(Vector(target)-camera.location).normalized()
    right=f.cross(Vector((0,1,0))).normalized();up=right.cross(f).normalized()
    camera.rotation_euler=Matrix((right,up,-f)).transposed().to_euler()
def render(name):
    scene.render.filepath=str(OUT/(name+'.png'));bpy.ops.render.render(write_still=True)
black=bpy.data.materials.new('Black silhouette');black.use_nodes=True
nodes=black.node_tree.nodes;nodes.clear();out=nodes.new('ShaderNodeOutputMaterial');em=nodes.new('ShaderNodeEmission')
em.inputs[0].default_value=(0,0,0,1);black.node_tree.links.new(em.outputs[0],out.inputs[0])
for i,c in enumerate(families):
    for obj in c.objects: obj.location.x+=(i-1)*.19
view((0,.18,2))
scene.view_layers[0].material_override=black
render('family_front_silhouettes')
scene.render.resolution_x=192;scene.render.resolution_y=192
render('family_inventory_silhouettes')
scene.render.resolution_x=1024;scene.render.resolution_y=1024
scene.view_layers[0].material_override=None
view((.4,.26,1.7));render('family_three_quarter_clay')
for i,c in enumerate(families):
    for obj in c.objects: obj.location.x-=(i-1)*.19
    c.hide_render=True
for c in families:
    c.hide_render=False
    for label,pos in [('front',(0,.18,2)),('side',(2,.18,0)),('three_quarter',(.8,.28,1.6))]:
        view(pos);render(c.name.lower().replace(' ','_')+'_'+label)
    c.hide_render=True
for i,c in enumerate(families):
    c.hide_render=False
    for obj in c.objects: obj.location.x+=(i-1)*.19
view((0,.18,2))
scene['acceptance']='Silhouette studies only. No textures, rig or native acceptance.'
bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/'assets/source/blender/studies/wand_families_v1.blend'))
print('WAND_FAMILY_STUDY_SAVED')
