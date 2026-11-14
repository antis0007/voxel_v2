"""Read-only source review: neutral turnarounds and two silhouette scales."""
import math
from pathlib import Path
import bpy
from mathutils import Vector, Matrix

ROOT=Path(__file__).resolve().parents[2]
OUT=ROOT/'target/ecosystem-minimal-v2';OUT.mkdir(parents=True,exist_ok=True)
bpy.ops.wm.open_mainfile(filepath=str(ROOT/'assets/source/wand_ecosystem/minimal_v2/ecosystem_items_r2.blend'))
scene=bpy.context.scene;cam=scene.camera
families=[bpy.data.collections[k] for k in ('bone_needle','runic_key','cinder_rod')]
bpy.data.collections['Plane hand'].hide_render=True
scene.cycles.samples=8
def view(position):
    cam.location=position;f=(Vector((0,.15,0))-cam.location).normalized();r=f.cross(Vector((0,1,0))).normalized()
    cam.rotation_euler=Matrix((r,r.cross(f),-f)).transposed().to_euler()
def render(name):
    scene.render.filepath=str(OUT/(name+'.png'));bpy.ops.render.render(write_still=True)
black=bpy.data.materials.new('Silhouette black');black.use_nodes=True
black.node_tree.nodes.clear();out=black.node_tree.nodes.new('ShaderNodeOutputMaterial');em=black.node_tree.nodes.new('ShaderNodeEmission')
em.inputs[0].default_value=(0,0,0,1);black.node_tree.links.new(em.outputs[0],out.inputs[0])
for family in families:
    for other in families:other.hide_render=other!=family
    cam.data.type='ORTHO';cam.data.ortho_scale=.58
    scene.render.resolution_x=400;scene.render.resolution_y=600
    for label,pos in [('front',(0,.15,2)),('side',(2,.15,0)),('three_quarter',(.8,.3,1.8))]:
        view(pos);render(family.name+'_'+label)
    original=[]
    for obj in family.objects:
        if obj.type=='MESH':original.append((obj,obj.data.materials[0]));obj.data.materials[0]=black
    bg=scene.world.node_tree.nodes['Background'];prior=tuple(bg.inputs[0].default_value);strength=bg.inputs[1].default_value
    bg.inputs[0].default_value=(1,1,1,1);bg.inputs[1].default_value=1
    scene.render.resolution_x=96;scene.render.resolution_y=96;cam.data.ortho_scale=.58
    view((0,.15,2));render(family.name+'_silhouette_96')
    axis=Vector((-.30,.84,-.45)).normalized();right=Vector((1,0,0));side=(right-axis*right.dot(axis)).normalized()
    rotation=Matrix((side,axis,side.cross(axis))).transposed().to_4x4()@Matrix.Rotation(-.18,4,'Y')
    presentation=Matrix.Translation((.25,-.245,-.53))@rotation@Matrix.Scale(.84,4)
    cam.matrix_world=presentation.inverted();cam.scale=(1,1,1)
    cam.data.type='PERSP';cam.data.sensor_fit='VERTICAL';cam.data.angle=math.radians(72)
    scene.render.resolution_x=1280;scene.render.resolution_y=720;render(family.name+'_silhouette_fov72')
    for obj,mat in original:obj.data.materials[0]=mat
    bg.inputs[0].default_value=prior;bg.inputs[1].default_value=strength
print('ECOSYSTEM_TURNAROUNDS_AND_SILHOUETTES_CAPTURED')
