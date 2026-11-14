"""Package A only: verify Blender export, metres, skinning and clip interchange."""
import bpy
from pathlib import Path
from math import sin,cos,pi

ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT/'assets/source/blender'
RUNTIME=ROOT/'assets/runtime/viewmodels'
SOURCE.mkdir(parents=True,exist_ok=True);RUNTIME.mkdir(parents=True,exist_ok=True)
bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
verts=[]
for z,r in [(0,.018),(.018,.020),(.090,.016),(.110,.012)]:
    verts.extend((r*cos(i*pi/4),r*.85*sin(i*pi/4),z) for i in range(8))
faces=[tuple(reversed(range(8))),tuple(range(24,32))]
faces.extend((j*8+i,j*8+(i+1)%8,(j+1)*8+(i+1)%8,(j+1)*8+i)
             for j in range(3) for i in range(8))
data=bpy.data.meshes.new('Measured taper');data.from_pydata(verts,[],faces);data.update()
obj=bpy.data.objects.new('PipelineProbe',data);bpy.context.collection.objects.link(obj)
uv=data.uv_layers.new(name='UVMap')
for face in data.polygons:
    for loop in face.loop_indices:
        p=data.vertices[data.loops[loop].vertex_index].co
        uv.data[loop].uv=(p.x/.05+.5,p.z/.11)
mat=bpy.data.materials.new('Probe ceramic');mat.diffuse_color=(.27,.55,.49,1)
mat.use_nodes=True;mat.node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value=mat.diffuse_color
data.materials.append(mat)
rigdata=bpy.data.armatures.new('Probe skeleton');rig=bpy.data.objects.new('ProbeRig',rigdata)
bpy.context.collection.objects.link(rig);bpy.context.view_layer.objects.active=rig;rig.select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
root=rigdata.edit_bones.new('Grip');root.head=(0,0,0);root.tail=(0,0,.11)
bpy.ops.object.mode_set(mode='OBJECT')
group=obj.vertex_groups.new(name='Grip');group.add(list(range(32)),1,'REPLACE')
modifier=obj.modifiers.new('Skin','ARMATURE');modifier.object=rig
obj.parent=rig
bone=rig.pose.bones['Grip'];bone.rotation_mode='XYZ'
for frame,angle in [(1,0),(16,.25),(31,0)]:
    bone.rotation_euler.y=angle;bone.keyframe_insert(data_path='rotation_euler',frame=frame)
rig.animation_data.action.name='ProbeFlex'
bpy.context.scene.render.fps=30;bpy.context.scene.frame_end=31;bpy.context.scene.frame_set(1)
bpy.ops.wm.save_as_mainfile(filepath=str(SOURCE/'pipeline_probe.blend'))
bpy.ops.object.select_all(action='DESELECT');obj.select_set(True);rig.select_set(True)
bpy.ops.export_scene.gltf(filepath=str(RUNTIME/'pipeline_probe.glb'),export_format='GLB',
    use_selection=True,export_yup=True,export_skins=True,export_animations=True,
    export_animation_mode='ACTIONS',export_force_sampling=True,export_tangents=True)
print('BLENDER_PROBE exported Blender',bpy.app.version_string,'metres=0.110 Blender +Z -> glTF +Y')
