"""Isolated anatomical reference review; never writes production viewmodels."""
import bpy, json, math
from pathlib import Path
from mathutils import Vector, Quaternion

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / 'assets/source/reference/quaternius/standard'
OUT = ROOT / 'target/quaternius-hand-review'
OUT.mkdir(parents=True, exist_ok=True)
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=str(SOURCE / 'Superhero_Male_FullBody.gltf'))
rig = next(o for o in bpy.context.scene.objects if o.type == 'ARMATURE')
rig.animation_data_clear()
hand = rig.data.bones['hand_r']
center = rig.matrix_world @ hand.head_local
points = []
meshes = []
for obj in list(bpy.context.scene.objects):
    if obj.type != 'MESH':
        continue
    groups = {g.index for g in obj.vertex_groups if g.name.endswith('_r') and
              any(k in g.name for k in ('hand', 'thumb', 'index', 'middle', 'ring', 'pinky'))}
    selected = [v for v in obj.data.vertices if sum(g.weight for g in v.groups if g.group in groups) > .5]
    if not selected:
        obj.hide_render = True
        continue
    points.extend(obj.matrix_world @ v.co for v in selected)
    meshes.append(obj)
    # Keep topology and skin intact; the camera crops to the hand.
    mat = bpy.data.materials.new('Neutral clay')
    mat.diffuse_color = (.45,.45,.45,1)
    mat.use_nodes = True
    mat.node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (.45,.45,.45,1)
    mat.node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = .85
    obj.data.materials.clear()
    obj.data.materials.append(mat)
    for p in obj.data.polygons:
        p.material_index = 0
low = Vector(tuple(min(p[i] for p in points) for i in range(3)))
high = Vector(tuple(max(p[i] for p in points) for i in range(3)))
center = (low + high) / 2
span = (high - low).length
scene = bpy.context.scene
scene.render.engine = 'CYCLES'
scene.cycles.samples = 24
scene.render.resolution_x = 1000
scene.render.resolution_y = 800
scene.render.resolution_percentage = 100
scene.world = bpy.data.worlds.new('Gray studio')
scene.world.use_nodes = True
scene.world.node_tree.nodes['Background'].inputs[0].default_value = (.14,.14,.14,1)
scene.world.node_tree.nodes['Background'].inputs[1].default_value = .5
scene.view_settings.view_transform = 'AgX'
def aim(obj, target):
    obj.rotation_euler = (target-obj.location).to_track_quat('-Z','Y').to_euler()
bpy.ops.object.camera_add(location=center + Vector((0,-span*2,span*.6)))
camera = bpy.context.object
camera.data.type = 'ORTHO'
camera.data.ortho_scale = span*1.3
scene.camera = camera
aim(camera,center)
for offset,energy,size in [((1,-2,3),350,2),((-2,1,1),180,2)]:
    bpy.ops.object.light_add(type='AREA',location=center+Vector(offset))
    light=bpy.context.object
    light.data.energy=energy
    light.data.shape='DISK'
    light.data.size=size
    aim(light,center)
report = {'source': str(SOURCE/'Superhero_Male_FullBody.gltf'),
          'hand_bounds': [list(low),list(high)], 'hand_weighted_vertices':len(points),
          'bones': {b.name:{'head':list(b.head_local),'tail':list(b.tail_local)}
                    for b in rig.data.bones if b.name.endswith('_r') and
                    any(k in b.name for k in ('hand','thumb','index','middle','ring','pinky'))},
          'meshes': {o.name: {'vertices':len(o.data.vertices),'polygons':len(o.data.polygons)} for o in meshes},
          'pose_note':'Curl is diagnostic, not an authored production grip.'}
(OUT/'inspection.json').write_text(json.dumps(report,indent=2))
for view,offset in [('open_palm',(0,-.5,-2)),('open_back',(0,.5,2))]:
    camera.location=center+Vector(offset)*span
    aim(camera,center)
    scene.render.filepath=str(OUT/(view+'.png'))
    bpy.ops.render.render(write_still=True)
# Bones use a local Y longitudinal axis. Derive flexion axis from the common
# palm normal and each rest matrix, so finger rotation stays anatomically coherent.
for finger in ('index','middle','ring','pinky'):
    for joint,angle in [(1,55),(2,70),(3,40)]:
        bone=rig.pose.bones[f'{finger}_{joint:02}_r']
        local_axis=bone.bone.matrix_local.to_3x3().inverted() @ Vector((0,1,0))
        bone.rotation_mode='QUATERNION'
        bone.rotation_quaternion=Quaternion(local_axis,math.radians(angle))
bpy.context.view_layer.update()
camera.location=center+Vector((-.4,-1.5,-1.5))*span
aim(camera,center)
scene.render.filepath=str(OUT/'curl_diagnostic.png')
bpy.ops.render.render(write_still=True)
bpy.ops.wm.save_as_mainfile(filepath=str(OUT/'hand_assessment.blend'))
