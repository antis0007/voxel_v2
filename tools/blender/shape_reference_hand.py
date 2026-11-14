"""Crop and pose a CC0 anatomical base for a faceted hand study, not production."""
import bpy, bmesh, math, json
from pathlib import Path
from mathutils import Vector, Quaternion

ROOT=Path(__file__).resolve().parents[2]
OUT=ROOT/'target/hand-grip-study';OUT.mkdir(parents=True,exist_ok=True)
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=str(ROOT/'assets/source/reference/quaternius/standard/Superhero_Male_FullBody.gltf'))
scene=bpy.context.scene
rig=next(o for o in scene.objects if o.type=='ARMATURE');rig.animation_data_clear()
for p in rig.pose.bones:
    p.rotation_mode='QUATERNION';p.rotation_quaternion=Quaternion()
mesh=max((o for o in scene.objects if o.type=='MESH'),key=lambda o:len(o.data.vertices))
for other in scene.objects:
    if other.type=='MESH' and other!=mesh:other.hide_render=True
bpy.context.view_layer.objects.active=mesh
bm=bmesh.new();bm.from_mesh(mesh.data)
# Preserve the authored hand/weights; cut proximal forearm where a cuff can cover it.
bmesh.ops.delete(bm,geom=[v for v in bm.verts if (mesh.matrix_world@v.co).x>-.675],context='VERTS')
bm.to_mesh(mesh.data);bm.free();mesh.data.update()
assert len(mesh.data.vertices)>100,'Hand crop unexpectedly empty'
for p in mesh.data.polygons:p.use_smooth=False
skin=bpy.data.materials.new('Faceted skin');skin.use_nodes=True
skin.node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value=(.59,.34,.20,1)
skin.node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value=.9
skin.node_tree.nodes['Principled BSDF'].inputs['Specular IOR Level'].default_value=.1
mesh.data.materials.clear();mesh.data.materials.append(skin)
for p in mesh.data.polygons:p.material_index=0
# Collapse redundant coplanar/small surfaces before the armature; retain vertex weights.
dec=mesh.modifiers.new('Planar hand simplification','DECIMATE');dec.ratio=.33
bpy.ops.object.modifier_move_up(modifier=dec.name)
bpy.ops.object.modifier_apply(modifier=dec.name)
# Imported glTF split normals otherwise keep the superhero's smooth highlights.
weights=[[(g.group,g.weight) for g in v.groups] for v in mesh.data.vertices]
group_names=[g.name for g in mesh.vertex_groups]
clean=bpy.data.meshes.new('Planar anatomical topology')
clean.from_pydata([tuple(v.co) for v in mesh.data.vertices],[],
                  [tuple(p.vertices) for p in mesh.data.polygons]);clean.update()
mesh.data=clean;clean.materials.append(skin)
for name in group_names:mesh.vertex_groups.new(name=name)
for i,groups in enumerate(weights):
    for group,weight in groups:mesh.vertex_groups[group].add([i],weight,'REPLACE')
def rotate(name,axis,degrees):
    p=rig.pose.bones[name]
    local=p.bone.matrix_local.to_3x3().inverted()@Vector(axis)
    p.rotation_quaternion=Quaternion(local,math.radians(degrees))
for finger,angles in [('index',(-52,-72,-42)),('middle',(-57,-76,-44)),
                      ('ring',(-61,-74,-40)),('pinky',(-64,-70,-35))]:
    for j,a in enumerate(angles,1):rotate(f'{finger}_{j:02}_r',(0,1,0),a)
# Oppose the thumb across the palm rather than simply flexing all digits equally.
p=rig.pose.bones['thumb_01_r'];rest=p.bone.matrix_local.to_quaternion()
p.rotation_quaternion=rest.inverted()@Quaternion((0,0,1),math.radians(-25))@Quaternion((0,1,0),math.radians(30))@rest
rotate('thumb_02_r',(0,1,0),-10)
rotate('thumb_03_r',(0,1,0),-25)
bpy.context.view_layer.update()
center=Vector((-.81,.06,1.42))
bpy.ops.mesh.primitive_cylinder_add(vertices=8,radius=.017,depth=.34,location=(-.83,.06,1.414),rotation=(math.pi/2,0,0))
handle=bpy.context.object;handle.name='Grip diameter reference 34mm'
wood=bpy.data.materials.new('Grip reference');wood.diffuse_color=(.09,.075,.06,1)
handle.data.materials.append(wood)
scene.world=bpy.data.worlds.new('Neutral studio');scene.world.use_nodes=True
scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.18,.18,.18,1)
scene.world.node_tree.nodes['Background'].inputs[1].default_value=.7
scene.render.engine='CYCLES';scene.cycles.samples=24;scene.view_settings.view_transform='AgX'
scene.render.resolution_x=1000;scene.render.resolution_y=800;scene.render.resolution_percentage=100
def aim(o):o.rotation_euler=(center-o.location).to_track_quat('-Z','Y').to_euler()
for off,power in [((.3,-.4,.5),8),((-.3,.3,-.4),5)]:
    bpy.ops.object.light_add(type='AREA',location=center+Vector(off));lamp=bpy.context.object
    lamp.data.energy=power;lamp.data.size=.45;aim(lamp)
bpy.ops.object.camera_add();camera=bpy.context.object;camera.data.type='ORTHO';camera.data.ortho_scale=.38;scene.camera=camera
for label,off in [('back',(-.2,.2,.7)),('palm',(-.15,-.3,-.7)),('thumb',(-.3,-.7,.1))]:
    camera.location=center+Vector(off);aim(camera)
    scene.render.filepath=str(OUT/(label+'.png'));bpy.ops.render.render(write_still=True)
scene['provenance']='CC0 Quaternius Universal Base Characters Standard; source license retained under assets/source/reference/quaternius.'
scene['status']='Anatomical grip study only. Contact, thumb opposition, style and integration not accepted.'
bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/'assets/source/blender/studies/faceted_hand_grip.blend'))
(OUT/'manifest.json').write_text(json.dumps({'vertices':len(mesh.data.vertices),'faces':len(mesh.data.polygons),
    'source':'CC0 Quaternius Superhero_Male_FullBody.gltf','rendered':['back','palm','thumb'],
    'status':'Grip study, not native acceptance'},indent=2))
