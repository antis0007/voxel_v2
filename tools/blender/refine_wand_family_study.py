"""Historical v1-to-v2 revision. Refuses to overwrite the later GUI artist edit."""
import math
from pathlib import Path
import bpy
from mathutils import Vector, Matrix, Quaternion

ROOT=Path(__file__).resolve().parents[2]
DEST=ROOT/'assets/source/blender/studies/wand_families_v2.blend'
if DEST.exists():
    raise SystemExit('v2 now contains GUI artist edits. Choose a new destination before regenerating.')
OUT=ROOT/'target/wand-family-study-v2';OUT.mkdir(parents=True,exist_ok=True)
bpy.ops.wm.open_mainfile(filepath=str(ROOT/'assets/source/blender/studies/wand_families_v1.blend'))
scene=bpy.context.scene;camera=scene.camera
black=bpy.data.materials.new('Black silhouette');black.use_nodes=True;black.use_fake_user=True
nodes=black.node_tree.nodes;nodes.clear();out=nodes.new('ShaderNodeOutputMaterial');em=nodes.new('ShaderNodeEmission')
em.inputs[0].default_value=(0,0,0,1);black.node_tree.links.new(em.outputs[0],out.inputs[0])
# Natural family: a small closed growth pocket, not a long manufactured fork.
for v in bpy.data.objects['Living side prong'].data.vertices:
    v.co.y=.352+(v.co.y-.29)*.42
    v.co.x*=.68
seed=bpy.data.objects['Caught amber seed']
for v in seed.data.vertices:
    v.co.y-=.031;v.co.x-=.009
# Ancient family: a short broken lateral flake below a displaced terminal.
for v in bpy.data.objects['Unequal mineral prong'].data.vertices:
    v.co.y=.270+(v.co.y-.268)*.34
    v.co.x*=.60
for v in bpy.data.objects['Suspended splinter'].data.vertices:
    v.co.x-=.027;v.co.y+=.052
for o in bpy.data.collections['Astral Splinter'].objects:
    if o.type=='EMPTY' and o.name.startswith('muzzle'):o.location=(-.020+.19,.536,0)
for o in bpy.data.collections['Thornroot'].objects:
    if o.type=='EMPTY' and o.name.startswith('muzzle'):o.location=(-.024-.19,.46,0)
scene.view_layers[0].material_override=bpy.data.materials['Black silhouette']
def view(position,target=(0,.19,0)):
    camera.location=position;f=(Vector(target)-camera.location).normalized()
    right=f.cross(Vector((0,1,0))).normalized();up=right.cross(f).normalized()
    camera.rotation_euler=Matrix((right,up,-f)).transposed().to_euler()
def render(name):
    scene.render.filepath=str(OUT/(name+'.png'));bpy.ops.render.render(write_still=True)
view((0,.19,2));render('family_front_silhouettes')
scene.render.resolution_x=192;scene.render.resolution_y=192
render('family_inventory_silhouettes')
scene.render.resolution_x=1024;scene.render.resolution_y=1024
scene.view_layers[0].material_override=None
view((.4,.26,1.7));render('family_three_quarter')
families=[bpy.data.collections[n] for n in ['Thornroot','Copper Arcanist','Astral Splinter']]
for i,c in enumerate(families):
    for o in c.objects:o.location.x-=(i-1)*.19
    c.hide_render=True
for c in families:
    c.hide_render=False
    # Match current native presentation/FOV. No arm: measures wand projection only.
    a=Vector((-.30,.84,-.45)).normalized()
    side=(Vector((1,0,0))-a*a.x).normalized()
    rot=Matrix((side,a,side.cross(a))).transposed()@Quaternion((0,1,0),-.18).to_matrix()
    placement=Matrix.Translation(Vector((.25,-.245,-.53)))@rot.to_4x4()@Matrix.Scale(.84,4)
    inv=placement.inverted();camera.location=inv.translation;camera.rotation_euler=inv.to_quaternion().to_euler()
    camera.data.type='PERSP';camera.data.sensor_fit='VERTICAL';camera.data.sensor_height=24
    camera.data.lens=12/math.tan(math.radians(36));camera.data.clip_start=.005
    scene.render.resolution_x=1280;scene.render.resolution_y=720
    scene.view_layers[0].material_override=bpy.data.materials['Black silhouette']
    render(c.name.lower().replace(' ','_')+'_projection_silhouette')
    scene.view_layers[0].material_override=None
    c.hide_render=True
for i,c in enumerate(families):
    c.hide_render=False
    for o in c.objects:o.location.x+=(i-1)*.19
camera.data.type='ORTHO';camera.data.ortho_scale=.76
scene.render.resolution_x=1024;scene.render.resolution_y=1024;view((0,.19,2))
scene['acceptance']='Revised silhouette study; no UV, rig, textures or WGPU acceptance.'
bpy.ops.wm.save_as_mainfile(filepath=str(DEST))
print('WAND_FAMILY_REVISION_SAVED')
