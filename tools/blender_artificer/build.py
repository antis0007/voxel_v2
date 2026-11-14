"""Run using Blender --background --python tools/blender_artificer/build.py."""
import sys
import json
import os
from pathlib import Path
import bpy
from mathutils import Vector, Matrix

HERE=Path(__file__).resolve().parent
sys.path.insert(0,str(HERE))
from geometry import material, PALETTE
from anatomy import hand, sleeve, WRIST
from wands import shaft, head
from bake import bake, export
from rig import add_hand_rig

ROOT=HERE.parents[1]
OUTPUT=ROOT/'assets/viewmodel'
EVIDENCE=ROOT/os.environ.get('VOXEL_ART_EVIDENCE','target/artificer-restart-20261026')
EVIDENCE.mkdir(parents=True,exist_ok=True)
OUTPUT.joinpath('source').mkdir(exist_ok=True)
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
for c in list(bpy.data.collections):
    bpy.data.collections.remove(c)
for name in PALETTE:
    material(name)

def collection(name,parent):
    c=bpy.data.collections.new(name)
    parent.children.link(c)
    return c

scene=bpy.context.scene
source=collection('SOURCE - editable anatomical surfaces',scene.collection)
game=collection('GAME - baked voxel meshes',scene.collection)
report={'blender':bpy.app.version_string,'pipeline':'authored joined anatomy and pixel-painted planes; BLOCKS wand parts',
        'assets':{}}
parts={}
for name, author, depth, union in [
    ('hand',hand,4,True),('sleeve',sleeve,5,True),
    *[(f'shaft_{i}',lambda c,i=i:shaft(c,i),6,True) for i in range(6)],
    *[(f'head_{i}',lambda c,i=i:head(c,i),4 if i==0 else 5,True) for i in range(6)]]:
    c=collection(name,source)
    author(c)
    bpy.context.view_layer.update()
    if name.startswith('shaft_'):
        for part in c.objects:
            for vertex in part.data.vertices:
                world=part.matrix_world @ vertex.co
                if world.y > .11:
                    world.y=.11+(world.y-.11)*.73
                    vertex.co=part.matrix_world.inverted() @ world
    obj=bake(c,name,game,depth,union)
    parts[name]=obj
    report['assets'][name]=export(obj,OUTPUT/(name+'.ron'))
    print('ARTIFICER',name,report['assets'][name],flush=True)
    c.hide_render=True
    c.hide_viewport=True
    obj.hide_render=True
    obj.hide_set(True)

# Keep local export coordinates; assembly transforms are only a Blender preview.
add_hand_rig(parts['hand'],game)
for name in ('hand','sleeve','shaft_0','head_0'):
    parts[name].hide_render=False
    parts[name].hide_set(False)
parts['sleeve'].location=WRIST
parts['head_0'].location=(0,.2794,0)

studio=collection('STUDIO - lights and cameras',scene.collection)
def light(name,location,energy,color,size):
    data=bpy.data.lights.new(name,'AREA')
    data.energy=energy
    data.color=color
    data.shape='DISK'
    data.size=size
    obj=bpy.data.objects.new(name,data)
    studio.objects.link(obj)
    obj.location=location
    obj.rotation_euler=(Vector((0,.10,0))-obj.location).to_track_quat('-Z','Y').to_euler()
light('Warm key',(-.45,.65,.8),40,(1,.86,.70),.65)
light('Cool fill',(.65,.25,.45),17,(.68,.80,1),.6)
light('Edge light',(-.25,.12,-.5),32,(.83,1,.91),.4)
camera_data=bpy.data.cameras.new('Grip and construction')
camera=bpy.data.objects.new('Grip and construction',camera_data)
studio.objects.link(camera)
camera.location=(.58,.24,.90)
back=(camera.location-Vector((0,.06,0))).normalized()
right=Vector((0,1,0)).cross(back).normalized()
up=back.cross(right)
camera.rotation_euler=Matrix((right,up,back)).transposed().to_euler()
camera_data.type='ORTHO'
camera_data.ortho_scale=.85
scene.camera=camera
scene.world.color=(.15,.17,.18)
scene.world.use_nodes=True
scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.11,.13,.14,1)
scene.world.node_tree.nodes['Background'].inputs[1].default_value=.4
scene.render.engine='CYCLES'
scene.cycles.samples=32
scene.cycles.use_denoising=True
scene.render.resolution_x=1100
scene.render.resolution_y=1400
scene.render.resolution_percentage=100
scene.view_settings.view_transform='AgX'
scene.render.image_settings.file_format='PNG'
scene.render.filepath=str(EVIDENCE/'blender-studio.png')
scene['art_direction']='Minecraft modded style: compact faceted grip, pixel paint, bent cloth, timber and aged brass.'
scene['export']='Local metre coordinates; engine assembles cached meshes at the wrist and head socket.'
bpy.ops.wm.save_as_mainfile(filepath=str(OUTPUT/'source/artificer.blend'))
report['blend']=str(OUTPUT/'source/artificer.blend')
report['default_triangles']=sum(report['assets'][k]['triangles'] for k in ('hand','sleeve','shaft_0','head_0'))
(EVIDENCE/'authoring.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
bpy.ops.render.render(write_still=True)
print('ARTIFICER COMPLETE',report['default_triangles'],flush=True)
