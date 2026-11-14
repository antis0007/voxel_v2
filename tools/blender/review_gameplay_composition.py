"""Review a saved artist source at native viewmodel projection; never export/promote.

blender --background --python-exit-code 1 --python tools/blender/review_gameplay_composition.py -- SOURCE.blend OUTPUT
No source geometry, rig or material is saved over. Review outputs are isolated.
"""
import bpy, sys, math, hashlib, json
from pathlib import Path
from mathutils import Vector, Matrix, Quaternion

args=sys.argv[sys.argv.index('--')+1:]
if len(args)!=2: raise SystemExit('Expected SOURCE.blend OUTPUT')
source,output=map(lambda s:Path(s).resolve(),args)
output.mkdir(parents=True,exist_ok=True)
sha=hashlib.sha256(source.read_bytes()).hexdigest()
bpy.ops.wm.open_mainfile(filepath=str(source))
scene=bpy.context.scene
scene.render.engine='CYCLES'; scene.cycles.samples=32
scene.render.resolution_x=1280; scene.render.resolution_y=720
scene.render.resolution_percentage=100
scene.view_settings.view_transform='AgX'
scene.render.film_transparent=False
for obj in scene.objects:
    if obj.type=='LIGHT': obj.hide_render=True
scene.world.use_nodes=True
scene.world.node_tree.nodes.get('Background').inputs['Color'].default_value=(.13,.13,.13,1)
scene.world.node_tree.nodes.get('Background').inputs['Strength'].default_value=.6

def area(name,location,energy,size):
    data=bpy.data.lights.new(name,'AREA');data.energy=energy;data.shape='DISK';data.size=size
    obj=bpy.data.objects.new(name,data);scene.collection.objects.link(obj);obj.location=location
    obj.rotation_euler=(Vector((0,.08,0))-obj.location).to_track_quat('-Z','Y').to_euler()
area('Review broad key',(.35,.35,.45),6,.45)
area('Review broad fill',(-.30,.10,.2),2,.35)
area('Review separation',(.2,.35,-.3),4,.3)
data=bpy.data.cameras.new('Native 72 degree view');cam=bpy.data.objects.new('Native 72 degree view',data)
scene.collection.objects.link(cam);scene.camera=cam
data.clip_start=.005;data.clip_end=100

def native(offset,axis,roll):
    # Native basis: camera right +X, up +Y, forward -Z. Geometry remains untouched.
    a=Vector((axis[0],axis[1],-axis[2])).normalized()
    side=(Vector((1,0,0))-a*a.x).normalized()
    rotation=Matrix((side,a,side.cross(a))).transposed()@Quaternion((0,1,0),roll).to_matrix()
    placement=Matrix.Translation(Vector((offset[0],offset[1],-offset[2])))@rotation.to_4x4()@Matrix.Scale(.84,4)
    inverse=placement.inverted()
    cam.location=inverse.translation;cam.rotation_euler=inverse.to_quaternion().to_euler()
    cam.scale=(1,1,1);data.type='PERSP';data.sensor_fit='VERTICAL';data.sensor_height=24
    data.lens=12/math.tan(math.radians(36))

def render(name):
    scene.render.filepath=str(output/(name+'.png'));bpy.ops.render.render(write_still=True)

native((.25,-.245,.53),(-.30,.84,.45),-.18)
render('gameplay_neutral')
for label,roll in [('oblique_positive',.55),('back_of_hand',1.0),('oblique_negative',-.75)]:
    native((.25,-.245,.53),(-.30,.84,.45),roll)
    render(label)
native((.25,-.245,.53),(-.30,.84,.45),-.18)
clay=bpy.data.materials.new('Review neutral clay');clay.use_nodes=True
shader=clay.node_tree.nodes.get('Principled BSDF')
shader.inputs['Base Color'].default_value=(.38,.38,.38,1);shader.inputs['Roughness'].default_value=.8
scene.view_layers[0].material_override=clay
render('gameplay_clay')
scene.view_layers[0].material_override=None
native((.13,-.18,.53),(-.70,.69,.18),.55)
render('inspect_neutral')

# Plain silhouette makes depth, grip, finger rhythm and sleeve termination legible.
silhouette=bpy.data.materials.new('Review black silhouette');silhouette.use_nodes=True
nodes=silhouette.node_tree.nodes;nodes.clear()
emission=nodes.new('ShaderNodeEmission');emission.inputs['Color'].default_value=(.005,.005,.005,1)
surface=nodes.new('ShaderNodeOutputMaterial');silhouette.node_tree.links.new(emission.outputs[0],surface.inputs['Surface'])
scene.view_layers[0].material_override=silhouette
scene.world.node_tree.nodes.get('Background').inputs['Color'].default_value=(.8,.8,.8,1)
scene.world.node_tree.nodes.get('Background').inputs['Strength'].default_value=1
native((.25,-.245,.53),(-.30,.84,.45),-.18)
render('gameplay_silhouette')
scene.view_layers[0].material_override=clay
data.type='ORTHO';data.ortho_scale=.19
cam.location=(.24,.20,.4)
cam.rotation_euler=(Vector((.025,.01,0))-cam.location).to_track_quat('-Z','Y').to_euler()
render('grip_clay')
assert hashlib.sha256(source.read_bytes()).hexdigest()==sha,'Artist source changed during read-only review'
(output/'review.json').write_text(json.dumps({'source':str(source),'source_sha256':sha,
    'blender':bpy.app.version_string,'resolution':[1280,720],'vertical_fov':72,
    'scope':'Saved mesh review. No native lighting, runtime elbow constraint or procedural ornaments. No promotion.',
    'views':['gameplay_neutral','oblique_positive','back_of_hand','oblique_negative',
        'gameplay_clay','inspect_neutral','gameplay_silhouette','grip_clay']},indent=2))
print('SOURCE_UNCHANGED',sha)
