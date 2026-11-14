"""Independent Blender GLB re-import plus clay and lighting review."""
import bpy, json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
out=ROOT/'target/wayfarer-review'
report={}
for name in ('wayfarer_focus','first_person_arms'):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(ROOT/f'assets/runtime/viewmodels/{name}.glb'))
    rigs=[o for o in bpy.data.objects if o.type=='ARMATURE']
    shapes={b.custom_shape for r in rigs for b in r.pose.bones if b.custom_shape}
    meshes=[o for o in bpy.data.objects if o.type=='MESH' and o not in shapes]
    assert meshes and rigs
    for obj in meshes:
        # Blender collapses fully rigid, single-joint skins to bone parenting.
        assert obj.data.uv_layers
        assert obj.vertex_groups or obj.parent_type=='BONE', obj.name
        assert all(all(abs(x)<10 for x in v.co) for v in obj.data.vertices)
    report[name]={'meshes':len(meshes),'vertices':sum(len(o.data.vertices) for o in meshes),
        'joints':len(rigs[0].data.bones),'actions':[a.name for a in bpy.data.actions]}
    assert len(rigs[0].data.bones)==19
(out/'roundtrip.json').write_text(json.dumps(report,indent=2))
bpy.ops.wm.open_mainfile(filepath=str(ROOT/'assets/source/blender/first_person_arms.blend'))
scene=bpy.context.scene
scene.render.filepath=str(out/'neutral_textured.png'); bpy.ops.render.render(write_still=True)
clay=bpy.data.materials.new('Review clay'); clay.diffuse_color=(.35,.35,.35,1)
scene.view_layers[0].material_override=clay
scene.render.filepath=str(out/'clay.png'); bpy.ops.render.render(write_still=True)
scene.view_layers[0].material_override=None
for light in bpy.data.lights:
    light.energy*=.35
    light.color=(1,.39,.12) if light.name=='Softbox' else (.2,.35,.6)
scene.world.color=(.015,.015,.015)
scene.render.filepath=str(out/'torch.png'); bpy.ops.render.render(write_still=True)
