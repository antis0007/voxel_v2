"""Export an edited .blend without rebuilding or saving over its geometry.

blender --background --python tools/blender/export_saved_viewmodel.py -- SOURCE OUT
The Wayfarer runtime uses Blender coordinates directly (Y along the wand).
"""
import hashlib
import json
import sys
from pathlib import Path

import bpy


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


args = sys.argv[sys.argv.index('--') + 1:]
if len(args) != 2:
    raise SystemExit('Expected SOURCE.blend OUTPUT_DIRECTORY after --')
source, output = (Path(arg).resolve() for arg in args)
if not source.is_file() or source.suffix != '.blend':
    raise SystemExit('Source must be a saved Blender document')
output.mkdir(parents=True, exist_ok=True)
source_hash = digest(source)
bpy.ops.wm.open_mainfile(filepath=str(source))
scene = bpy.context.scene
rigs = [obj for obj in scene.objects if obj.type == 'ARMATURE']
if len(rigs) != 1:
    raise RuntimeError('Expected one shared armature')
rig = rigs[0]
if not 18 <= len(rig.data.bones) <= 64:
    raise RuntimeError('Expected articulated hand and socket rig')
meshes = [obj for obj in scene.objects if obj.type == 'MESH']
report = {'source': str(source), 'source_sha256': source_hash,
          'blender': bpy.app.version_string, 'joints': len(rig.data.bones),
          'coordinates': 'Blender coordinates retained; metres', 'exports': {}}
for group, filename in [('wand', 'wayfarer_focus.glb'), ('arm', 'first_person_arms.glb')]:
    selected = [obj for obj in meshes if obj.get('part') == group]
    if not selected:
        raise RuntimeError(f'No meshes tagged part={group}')
    for obj in selected:
        if not obj.data.uv_layers or not obj.data.materials:
            raise RuntimeError(f'{obj.name}: missing UV or material')
        if obj.find_armature() != rig:
            raise RuntimeError(f'{obj.name}: missing shared skin')
    bpy.ops.object.select_all(action='DESELECT')
    rig.select_set(True)
    bpy.context.view_layer.objects.active = rig
    for obj in selected:
        obj.select_set(True)
    destination = output / filename
    bpy.ops.export_scene.gltf(
        filepath=str(destination), export_format='GLB', use_selection=True,
        export_animations=True, export_animation_mode='ACTIONS',
        export_skins=True, export_def_bones=False, export_force_sampling=True,
        export_frame_range=False, export_yup=False)
    report['exports'][filename] = {'sha256': digest(destination),
                                  'objects': [obj.name for obj in selected]}
if scene.camera:
    scene.render.filepath = str(output / 'player.png')
    bpy.ops.render.render(write_still=True)
if digest(source) != source_hash:
    raise RuntimeError('Export must never modify the artist source')
(output / 'export_manifest.json').write_text(json.dumps(report, indent=2))
print('SAVED_SOURCE_EXPORT_OK', source_hash)
