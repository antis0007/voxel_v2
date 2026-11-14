"""Authored hand planes, block-remeshed wand parts and shared engine export."""
import bpy
import bmesh
from mathutils import Vector
from mathutils.bvhtree import BVHTree
from geometry import PALETTE
from anatomy import skin_weight
from pixel_surface import united, painted

BINDINGS = {}

def combined(collection, name, destination):
    vertices, faces, pigments, bones = [], [], [], []
    materials = list(PALETTE)
    for obj in collection.objects:
        obj.data.calc_loop_triangles()
        offset = len(vertices)
        vertices.extend(obj.matrix_world @ v.co for v in obj.data.vertices)
        for triangle in obj.data.loop_triangles:
            faces.append(tuple(offset+i for i in triangle.vertices))
            mat = obj.data.materials[triangle.material_index].name
            pigments.append(materials.index(mat))
            bones.append(next((i+1 for i,n in enumerate(('Index','Middle','Ring','Little','Thumb'))
                               if obj.name.startswith(n)),0))
    data = bpy.data.meshes.new(name)
    data.from_pydata(vertices, [], faces)
    for mat in materials:
        data.materials.append(bpy.data.materials[mat])
    for face, pigment in zip(data.polygons,pigments):
        face.material_index = pigment
    obj = bpy.data.objects.new(name,data)
    destination.objects.link(obj)
    tree=BVHTree.FromPolygons(vertices,faces,all_triangles=True)
    BINDINGS[name]=(tree,bones)
    return obj, tree, pigments

def apply(obj, modifier):
    bpy.ops.object.select_all(action='DESELECT')
    obj.select_set(True)
    bpy.context.view_layer.objects.active=obj
    bpy.ops.object.modifier_apply(modifier=modifier.name)

def bake(collection, name, destination, depth, union=False):
    obj, source, pigments = combined(collection,name,destination)
    if name in ('hand','sleeve'):
        united(collection,obj,source,pigments,apply)
        painted(obj)
        return obj
    if union:
        remesh=obj.modifiers.new('Unite palm, pads and finger anatomy','REMESH')
        remesh.mode='VOXEL'
        remesh.voxel_size=.0018
        remesh.use_smooth_shade=False
        apply(obj,remesh)
    remesh=obj.modifiers.new('Authored voxel surface','REMESH')
    remesh.mode='BLOCKS'
    remesh.octree_depth=depth
    remesh.scale=.96
    remesh.use_remove_disconnected=False
    remesh.use_smooth_shade=False
    apply(obj,remesh)
    # Blender's remesher does not preserve pigment islands. Transfer from the
    # editable source after the volume union, before merging planar faces.
    for face in obj.data.polygons:
        _,_,index,_=source.find_nearest(face.center)
        face.material_index=pigments[index]
    bm=bmesh.new()
    bm.from_mesh(obj.data)
    bm.normal_update()
    bmesh.ops.dissolve_limit(bm,angle_limit=.001,verts=list(bm.verts),
                            edges=list(bm.edges),delimit={'MATERIAL'},use_dissolve_boundaries=True)
    bmesh.ops.recalc_face_normals(bm,faces=list(bm.faces))
    bm.to_mesh(obj.data)
    bm.free()
    obj.data.update()
    # A single narrow bevel gives block edges a deliberate treatment. It never
    # adds surface texture and leaves the broad voxel planes intact.
    bevel=obj.modifiers.new('One-plane voxel edge', 'BEVEL')
    bevel.width=.0007
    bevel.segments=1
    bevel.limit_method='ANGLE'
    bevel.angle_limit=.3
    apply(obj,bevel)
    return obj

def export(obj, path):
    mesh=obj.data
    mesh.calc_loop_triangles()
    tree=BVHTree.FromPolygons([v.co for v in mesh.vertices],
                             [tuple(t.vertices) for t in mesh.loop_triangles],all_triangles=True)
    materials=list(PALETTE)
    binding_tree,binding_bones=BINDINGS[obj.name]
    pixel_paint=mesh.color_attributes.get('Pixel paint')
    lines=['// Blender-authored voxel surface. Rebuild with tools/blender_artificer/build.py.',
           '(vertices: [']
    def fmt(values):
        return '('+', '.join(f'{v:.6f}' for v in values)+')'
    # Face-level AO is subtle and finite; actual directional light stays dynamic.
    valid_triangles=0
    for tri in mesh.loop_triangles:
        normal=tri.normal
        points=[mesh.vertices[i].co for i in tri.vertices]
        rounded=[Vector(tuple(round(c,6) for c in p)) for p in points]
        if (rounded[1]-rounded[0]).cross(rounded[2]-rounded[0]).length_squared < 1e-18:
            continue
        if normal.length_squared < .5:
            continue
        valid_triangles+=1
        center=sum(points,Vector())/3
        mat=mesh.materials[tri.material_index].name
        color,kind,finish,tint=PALETTE[mat]
        if pixel_paint:
            color=tuple(pixel_paint.data[tri.loops[0]].color_srgb[:3])
        tangent=normal.cross(Vector((0,1,0)))
        if tangent.length < .1:
            tangent=normal.cross(Vector((1,0,0)))
        tangent.normalize()
        bitangent=normal.cross(tangent)
        occluded=0
        for side in (tangent,-tangent,bitangent,-bitangent):
            direction=(normal*.65+side*.76).normalized()
            hit,_,_,distance=tree.ray_cast(center+normal*.0003,direction,.012)
            occluded+=int(hit is not None)
        shade=1-occluded*(0 if mat == 'skin' or pixel_paint else .025)
        color=tuple(c*shade for c in color)
        # Projection stays attached to the object; the shader never applies its
        # five-centimetre world grid to the held skin or leather.
        major=max(range(3),key=lambda axis:abs(normal[axis]))
        axes=[axis for axis in range(3) if axis != major]
        for p in points:
            uv=(p[axes[0]],p[axes[1]])
            bone,weight=0,0.
            if obj.name=='hand':
                _,_,index,_=binding_tree.find_nearest(p)
                bone=binding_bones[index]
                weight=skin_weight(bone,p)
            lines.append(f'    (p: {fmt(p)}, n: {fmt(normal)}, c: {fmt(color)}, '
                         f'uv: {fmt(uv)}, kind: {kind}.0, finish: {finish:.2f}, tint: {tint}, '
                         f'bone: {bone}, weight: {weight:.4f}),')
    lines+= ['])','']
    path.write_text('\n'.join(lines),encoding='utf-8')
    return {'triangles':valid_triangles,'vertices':len(mesh.vertices),
            'bounds':[[min(v.co[a] for v in mesh.vertices),max(v.co[a] for v in mesh.vertices)]
                      for a in range(3)]}
