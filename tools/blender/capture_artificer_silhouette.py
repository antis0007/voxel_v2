"""Read-only first-person silhouette capture from the saved Blender asset."""
import math
from pathlib import Path
import bpy
from mathutils import Vector, Matrix
ROOT=Path(__file__).resolve().parents[2]
bpy.ops.wm.open_mainfile(filepath=str(ROOT/'assets/source/wand_ecosystem/artificer_tally/artificer_tally.blend'))
scene=bpy.context.scene;cam=scene.camera
black=bpy.data.materials.new('Black first-person silhouette');black.use_nodes=True;black.node_tree.nodes.clear()
out=black.node_tree.nodes.new('ShaderNodeOutputMaterial');em=black.node_tree.nodes.new('ShaderNodeEmission')
em.inputs[0].default_value=(0,0,0,1);black.node_tree.links.new(em.outputs[0],out.inputs[0])
for obj in scene.objects:
    if obj.type=='MESH':obj.data.materials.clear();obj.data.materials.append(black)
bg=scene.world.node_tree.nodes['Background'];bg.inputs[0].default_value=(1,1,1,1);bg.inputs[1].default_value=1
axis=Vector((-.30,.84,-.45)).normalized();right=Vector((1,0,0));side=(right-axis*right.dot(axis)).normalized()
rotation=Matrix((side,axis,side.cross(axis))).transposed().to_4x4()@Matrix.Rotation(-.18,4,'Y')
presentation=Matrix.Translation((.25,-.245,-.53))@rotation@Matrix.Scale(.84,4)
cam.matrix_world=presentation.inverted();cam.scale=(1,1,1)
cam.data.type='PERSP';cam.data.sensor_fit='VERTICAL';cam.data.angle=math.radians(72)
scene.render.resolution_x=1280;scene.render.resolution_y=720;scene.cycles.samples=8
scene.render.filepath=str(ROOT/'target/artificer-tally/silhouette_fov72.png')
bpy.ops.render.render(write_still=True)
print('ARTIFICER_FIRST_PERSON_SILHOUETTE_CAPTURED')
