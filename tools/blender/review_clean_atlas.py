"""Matched texture-only comparison on the existing kit; not model approval."""
from pathlib import Path
import hashlib
import json
import bpy

ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT/'assets/viewmodel/pixel_source/workshop_kit.blend'
OUT=ROOT/'target/clean-atlas-review'
OUT.mkdir(parents=True,exist_ok=True)
before=hashlib.sha256(SOURCE.read_bytes()).hexdigest()
bpy.ops.wm.open_mainfile(filepath=str(SOURCE))
scene=bpy.context.scene
scene.cycles.samples=24
scene.render.resolution_x=720
scene.render.resolution_y=800
for obj in scene.objects:
    if obj.type=='LIGHT':
        obj.data.color=(1,1,1)
        obj.data.energy*=.45
image=bpy.data.images.load(str(ROOT/'assets/source/textures/clean_modded_candidate/kit.png'),check_existing=False)
image.pack()
nodes=[n for mat in bpy.data.materials if mat.use_nodes
       for n in mat.node_tree.nodes if n.type=='TEX_IMAGE' and n.image]
for stage in ('current','candidate'):
    if stage=='candidate':
        for node in nodes:
            node.image=image
            node.interpolation='Closest'
    scene.render.filepath=str(OUT/(stage+'.png'))
    bpy.ops.render.render(write_still=True)
bpy.ops.wm.save_as_mainfile(filepath=str(OUT/'texture_comparison_candidate.blend'))
assert hashlib.sha256(SOURCE.read_bytes()).hexdigest()==before
(OUT/'receipt.json').write_text(json.dumps({'source_sha256':before,
    'source_unchanged':True,'geometry':'Existing rejected kit, used only to compare texture mapping',
    'lighting':'Matched white lights','runtime_promoted':False},indent=2))
