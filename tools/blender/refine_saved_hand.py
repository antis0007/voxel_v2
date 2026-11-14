"""One-time mesh repair of the saved r5 assembly; never rebuild the wand or rig.
Outputs a new artist source, preserving the canonical source for comparison.
"""
from pathlib import Path
import bpy
from mathutils import Vector
from mathutils.kdtree import KDTree

ROOT = Path(__file__).resolve().parents[2]
bpy.ops.wm.open_mainfile(filepath=str(ROOT/'assets/source/blender/first_person_arms.blend'))
scene = bpy.context.scene
rig = bpy.data.objects['WayfarerRig']
rig.animation_data.action = None
scene.frame_set(1)
for bone in rig.pose.bones:
    bone.rotation_quaternion = (1, 0, 0, 0)
palm = bpy.data.objects['Continuous anatomical hand']
thumb = bpy.data.objects['Opposing thumb']
samples = []
for obj in (palm, thumb):
    for vertex in obj.data.vertices:
        samples.append((vertex.co.copy(), [(obj.vertex_groups[g.group].name, g.weight)
                                          for g in vertex.groups]))
tree = KDTree(len(samples))
for index, (position, _) in enumerate(samples):
    tree.insert(position, index)
tree.balance()

# Exact union preserves the thumb's joint loops away from the root junction.
# Voxel remeshing the entire closed hand would weld touching fingers together.
bpy.ops.object.select_all(action='DESELECT')
palm.select_set(True)
bpy.context.view_layer.objects.active = palm
union = palm.modifiers.new('Continuous thumb web', 'BOOLEAN')
union.operation = 'UNION'
union.solver = 'EXACT'
union.object = thumb
bpy.ops.object.modifier_move_up(modifier=union.name)
bpy.ops.object.modifier_apply(modifier=union.name)
bpy.data.objects.remove(thumb, do_unlink=True)
palm.name = 'Palm and opposing thumb'
palm.vertex_groups.clear()
for bone in rig.data.bones:
    palm.vertex_groups.new(name=bone.name)
for vertex in palm.data.vertices:
    nearest = tree.find_n(vertex.co, 3)
    if nearest[0][2] < 1e-6:
        weights = dict(samples[nearest[0][1]][1])
    else:
        weights = {}
        total = sum(1/max(distance, 1e-5) for _, _, distance in nearest)
        for _, index, distance in nearest:
            for name, weight in samples[index][1]:
                weights[name] = weights.get(name, 0) + weight/max(distance, 1e-5)/total
    top = sorted(weights.items(), key=lambda item: -item[1])[:4]
    total = sum(weight for _, weight in top)
    for name, weight in top:
        palm.vertex_groups[name].add([vertex.index], weight/total, 'REPLACE')
for face in palm.data.polygons:
    face.use_smooth = True

# Existing sleeve loops are edited in place. Cuff and elbow landmarks remain
# unchanged, preserving the renderer's wrist-preserving forearm constraint.
sleeve = bpy.data.objects['Tailored sleeve']
old_centers = [(.105,-.070,0),(.112,-.079,0),(.126,-.095,-.002),
               (.143,-.117,-.004),(.162,-.139,-.008),(.190,-.166,-.012),(.235,-.207,-.015)]
new_centers = [(.105,-.070,0),(.112,-.079,0),(.125,-.092,-.002),
               (.135,-.108,-.003),(.153,-.131,-.007),(.188,-.165,-.012),(.235,-.207,-.015)]
radial_scale = [1, 1, 1.09, .86, 1.07, .98, 1]
assert len(sleeve.data.vertices) == 84
for vertex in sleeve.data.vertices:
    ring = vertex.index//12
    offset = vertex.co-Vector(old_centers[ring])
    vertex.co = Vector(new_centers[ring])+offset*radial_scale[ring]
for face in sleeve.data.polygons:
    face.use_smooth = False

# Preserve clear knuckle planes without the original needle-like fingertips.
for name in ('Index', 'Middle', 'Ring', 'Little'):
    obj = bpy.data.objects[name]
    vertices = obj.data.vertices
    end = list(vertices)[-10:]
    center = sum((v.co for v in end), Vector())/10
    for vertex in end:
        vertex.co = center+(vertex.co-center)*1.28

scene.frame_set(1)
destination = ROOT/'assets/source/blender/wayfarer_artist_r2.blend'
bpy.ops.wm.save_as_mainfile(filepath=str(destination))
review = ROOT/'target/wayfarer-artist-r2'
review.mkdir(parents=True, exist_ok=True)
camera = scene.camera
for name, position, target, size in [
    ('player',(.32,-.38,.65),(.035,.065,0),.59),
    ('thumb_contact',(.28,.23,.4),(.025,.025,0),.18),
    ('side',(.8,.08,.05),(.025,.055,0),.59)]:
    camera.location = position
    camera.rotation_euler = (Vector(target)-camera.location).to_track_quat('-Z','Y').to_euler()
    camera.data.ortho_scale = size
    scene.render.filepath = str(review/(name+'.png'))
    bpy.ops.render.render(write_still=True)
camera.location = (.32,-.38,.65)
camera.rotation_euler = (Vector((.035,.065,0))-camera.location).to_track_quat('-Z','Y').to_euler()
camera.data.ortho_scale = .59
for name in ('open', 'strong_grip', 'inspect'):
    rig.animation_data.action = bpy.data.actions[name]
    scene.frame_set(15 if name != 'inspect' else 45)
    scene.render.filepath = str(review/(name+'.png'))
    bpy.ops.render.render(write_still=True)
print('ARTIST_REPAIR_SAVED', destination)
