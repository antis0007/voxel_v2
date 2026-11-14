"""Blender entry point: editable source, shared atlas, native meshes, studio render."""
import sys
import json
from pathlib import Path
import bpy
from mathutils import Vector

HERE=Path(__file__).resolve().parent
sys.path.insert(0,str(HERE))
ROOT=HERE.parent.parent
OUT=ROOT/'assets/viewmodel'
SOURCE=OUT/'pixel_source'
import atlas
import mesh
import forms

bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
atlas.make(OUT)
image=bpy.data.images.load(str(OUT/'kit.png'),check_existing=False)
for name in atlas.TILES:
    material=bpy.data.materials.new(name)
    material.use_nodes=True
    bsdf=material.node_tree.nodes.get('Principled BSDF')
    bsdf.inputs['Roughness'].default_value=.8 if name!='copper' else .48
    texture=material.node_tree.nodes.new('ShaderNodeTexImage')
    texture.image=image; texture.interpolation='Closest'
    material.node_tree.links.new(texture.outputs['Color'],bsdf.inputs['Base Color'])
forms.hand(); forms.sleeve()
for i in range(6): forms.shaft(i); forms.head(i)
stats={g:mesh.export(g,OUT/(g+'.ron')) for g in mesh.PARTS}
(SOURCE/'mesh_stats.json').write_text(json.dumps(stats,indent=2)+'\n')
# Keep the same attachment contract; each grip joint pivots within its own pad.
rig='''// Shared with the editable Blender source in pixel_source/workshop_kit.blend.
(rest:(offset:(0.25,-0.245,0.53),axis:(-0.30,0.84,0.45),roll:-0.18),
aim:(offset:(0.16,-0.215,0.60),axis:(-0.12,0.55,0.82),roll:0.02),
inspect:(offset:(0.13,-0.18,0.53),axis:(-0.70,0.69,0.18),roll:0.55),
interact:(offset:(0.25,-0.30,0.60),axis:(-0.25,0.58,0.77),roll:-0.22),
windup_offset:(0.012,0.018,-0.045),release_offset:(-0.018,-0.012,0.075),
wrist:(0.082,-0.040,-0.004),elbow:(0.46,-0.58,0.20),head_height:0.235,
bones:(
(pivot:(0.028,0.039,-0.010),axis:(0.0,1.0,0.0),angle:-0.22),
(pivot:(0.030,0.016,-0.010),axis:(0.0,1.0,0.0),angle:-0.20),
(pivot:(0.030,-0.006,-0.010),axis:(0.0,1.0,0.0),angle:-0.18),
(pivot:(0.030,-0.025,-0.010),axis:(0.0,1.0,0.0),angle:-0.16),
(pivot:(0.050,0.052,0.018),axis:(0.0,0.0,1.0),angle:-0.18)))
'''
(OUT/'rig.ron').write_text(rig)
for group,objects in mesh.PARTS.items():
    collection=bpy.data.collections.new(group)
    bpy.context.scene.collection.children.link(collection)
    for obj in objects:
        for old in list(obj.users_collection): old.objects.unlink(obj)
        collection.objects.link(obj)
        if group.startswith('head'): obj.location.y=.235
        if group=='sleeve':
            obj.location=(.082,-.040,-.004)
            obj.rotation_euler.z=.70
        if group.startswith(('shaft','head')) and not group.endswith('_0'):
            obj.hide_render=True; obj.hide_set(True)
image.pack()
# Editable articulation matches the runtime's five compact grip joints.
data=bpy.data.armatures.new('Grip joints')
arm=bpy.data.objects.new('Grip joints',data)
bpy.context.scene.collection.objects.link(arm)
bpy.ops.object.select_all(action='DESELECT')
arm.select_set(True);bpy.context.view_layer.objects.active=arm
bpy.ops.object.mode_set(mode='EDIT')
root=data.edit_bones.new('Wrist');root.head=(.082,-.040,-.004);root.tail=(.045,.025,0)
joints=[('Index',(.028,.039,-.010)),('Middle',(.030,.016,-.010)),
        ('Ring',(.030,-.006,-.010)),('Little',(.030,-.025,-.010)),
        ('Thumb',(.050,.052,.018))]
for name,pivot in joints:
    bone=data.edit_bones.new(name);bone.head=pivot
    bone.tail=Vector(pivot)+Vector((-.025,0,.016));bone.parent=root
bpy.ops.object.mode_set(mode='OBJECT')
for obj in mesh.PARTS['hand']:
    group=obj.vertex_groups.new(name='Wrist')
    ids=list(range(len(obj.data.vertices)));group.add(ids,.6 if obj['bone'] else 1.,'REPLACE')
    if obj['bone']:
        group=obj.vertex_groups.new(name=joints[obj['bone']-1][0]);group.add(ids,.4,'REPLACE')
    modifier=obj.modifiers.new('Grip articulation','ARMATURE');modifier.object=arm
arm.show_in_front=True;arm.hide_render=True;arm.hide_set(True)
scene=bpy.context.scene
scene.render.engine='CYCLES'; scene.cycles.samples=32
scene.world.color=(.16,.16,.16)
scene.view_settings.view_transform='Standard'
scene.view_settings.look='None'
for name,loc,energy,size,color in [
    ('Warm key',(-.4,.6,.7),22,.6,(1,.84,.65)),
    ('Cool fill',(.5,.1,.5),8,.5,(.63,.77,1)),
    ('Edge',(-.3,.2,-.4),15,.4,(.8,.9,1))]:
    data=bpy.data.lights.new(name,'AREA'); data.energy=energy; data.shape='DISK'; data.size=size
    data.color=color
    obj=bpy.data.objects.new(name,data);scene.collection.objects.link(obj);obj.location=loc
    obj.rotation_euler=(Vector((0,.1,0))-obj.location).to_track_quat('-Z','Y').to_euler()
cam_data=bpy.data.cameras.new('Concept view'); cam=bpy.data.objects.new('Concept view',cam_data)
scene.collection.objects.link(cam);cam.location=(.28,.20,.85)
cam.rotation_euler=(Vector((.025,.11,0))-cam.location).to_track_quat('-Z','Y').to_euler()
cam_data.type='ORTHO';cam_data.ortho_scale=.56;scene.camera=cam
scene.render.resolution_x=900;scene.render.resolution_y=1000;scene.render.resolution_percentage=100
scene.render.image_settings.file_format='PNG'
scene.render.filepath=str(ROOT/'target/pixel-kit-reset-20261026/blender.png')
bpy.ops.wm.save_as_mainfile(filepath=str(SOURCE/'workshop_kit.blend'))
bpy.ops.render.render(write_still=True)
print('PIXEL_KIT',json.dumps(stats),'DEFAULT_TRIANGLES',sum(stats[x] for x in ('hand','sleeve','shaft_0','head_0')))
