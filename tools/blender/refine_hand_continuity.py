"""Preserve v2; isolate fuller palm and skin continuity under the cuff."""
from pathlib import Path
import bpy, json
from mathutils import Vector
ROOT=Path(__file__).resolve().parents[2]
OUT=ROOT/'assets/source/wand_ecosystem/hand_v3';OUT.mkdir(parents=True,exist_ok=True)
bpy.ops.wm.open_mainfile(filepath=str(ROOT/'assets/source/wand_ecosystem/minimal_v2/ecosystem_items_r2.blend'))
rig=bpy.data.objects['WayfarerRig'];hand=bpy.data.collections['Plane hand']
changes=[]
for obj in hand.objects:
    if obj.type!='MESH':continue
    if obj.name=='Wrist and metacarpal planes':
        for v in obj.data.vertices:v.co.z=-.002+(v.co.z+.002)*1.5
        # Exposed skin follows the wrist as one volume. Forearm-only skin at the
        # cuff caused a sheared hinge when the renderer redirected that bone.
        for group in obj.vertex_groups:group.remove(list(range(len(obj.data.vertices))))
        obj.vertex_groups['Wrist'].add(list(range(len(obj.data.vertices))),1.,'REPLACE')
        changes.append('Palm depth +50%; exposed wrist entirely Wrist weighted')
    elif 'grip planes' in obj.name or obj.name=='Opposing thumb planes':
        for start in range(0,len(obj.data.vertices),8):
            ring=list(obj.data.vertices)[start:start+8]
            center=sum((v.co for v in ring),Vector())/len(ring)
            for v in ring:v.co=center+(v.co-center)*1.12
        changes.append(obj.name+' section +12%')
    elif obj.name.startswith(('Tailored sleeve','Leather cuff')):
        anchor=Vector((.105,-.070,0));axis=Vector((.130,-.137,-.015)).normalized()
        for v in obj.data.vertices:
            d=v.co-anchor;along=axis*d.dot(axis)
            v.co=anchor+along+(d-along)*1.20
        changes.append(obj.name+' transverse section +20%')
bpy.ops.object.select_all(action='DESELECT');rig.hide_set(False);rig.select_set(True)
bpy.context.view_layer.objects.active=rig
for o in hand.objects:
    if o.type=='MESH':o.hide_set(False);o.select_set(True)
bpy.ops.export_scene.gltf(filepath=str(OUT/'plane_hand.glb'),export_format='GLB',use_selection=True,
    export_animations=True,export_animation_mode='ACTIONS',export_skins=True,export_def_bones=False,
    export_force_sampling=True,export_frame_range=False,export_yup=False)
bpy.ops.wm.save_as_mainfile(filepath=str(OUT/'hand_continuity.blend'))
(OUT/'changes.json').write_text(json.dumps({'changes':changes,'status':'diagnostic; not accepted',
    'animations':'Inherited unchanged; no new motion claim'},indent=2))
