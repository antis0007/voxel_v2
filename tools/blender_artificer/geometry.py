"""Editable Blender surfaces and the held kit's limited pigment palette."""
import bpy
from math import cos, sin, pi
from mathutils import Vector

PALETTE = {
    'skin': ((.51, .375, .295), 54, 0, 0),
    'nail': ((.63, .475, .38), 54, 0, 0),
    'cloth': ((.17, .255, .235), 54, .15, 0),
    'cuff': ((.40, .40, .30), 54, .15, 0),
    'seam': ((.115, .165, .155), 54, .15, 0),
    'thread': ((.39, .405, .32), 54, .15, 0),
    'leather': ((.235, .155, .095), 55, .1, 0),
    'wrap': ((.82, .82, .82), 55, .1, 1),
    'wood': ((.265, .19, .115), 55, 0, 0),
    'wood_edge': ((.36, .265, .16), 55, 0, 0),
    'brass': ((.48, .365, .20), 55, .4, 0),
    'brass_edge': ((.65, .515, .30), 55, .4, 0),
    'patina': ((.16, .29, .245), 55, .4, 0),
    'iron': ((.105, .14, .145), 55, .4, 0),
    'bone': ((.65, .605, .485), 55, .2, 0),
    'crystal': ((.25, .435, .465), 55, .8, 0),
    'obsidian': ((.085, .10, .125), 55, .6, 0),
    'starwood': ((.16, .185, .20), 55, 0, 0),
    'jewel': ((.48, .61, .58), 55, .8, 2),
    'jade': ((.29, .51, .43), 55, .65, 0),
}

def material(name):
    mat = bpy.data.materials.get(name)
    if mat:
        return mat
    rgb, kind, finish, tint = PALETTE[name]
    mat = bpy.data.materials.new(name)
    mat.diffuse_color = (*rgb, 1)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get('Principled BSDF')
    preview = tuple(c * t for c, t in zip(rgb, (.36, .20, .10))) if tint == 1 else rgb
    preview = tuple(c/12.92 if c <= .04045 else ((c+.055)/1.055)**2.4 for c in preview)
    bsdf.inputs['Base Color'].default_value = (*preview, 1)
    bsdf.inputs['Roughness'].default_value = .4 if kind == 55 and finish >= .4 else .84
    bsdf.inputs['Metallic'].default_value = .65 if finish == .4 else 0
    return mat

def mesh(name, vertices, faces, pigment, collection):
    data = bpy.data.meshes.new(name)
    data.from_pydata(vertices, [], faces)
    data.update()
    obj = bpy.data.objects.new(name, data)
    collection.objects.link(obj)
    data.materials.append(material(pigment))
    return obj

def loft(name, profiles, pigment, collection, sides=24, phase=0):
    """Cross sections follow a curved spine; all source parts remain editable."""
    vertices, faces = [], []
    for i, (point, rx, rz) in enumerate(profiles):
        p = Vector(point)
        tangent = (Vector(profiles[min(i+1, len(profiles)-1)][0]) -
                   Vector(profiles[max(i-1, 0)][0])).normalized()
        reference = Vector((1, 0, 0)) if abs(tangent.x) < .9 else Vector((0, 1, 0))
        across = (reference - tangent * reference.dot(tangent)).normalized()
        depth = across.cross(tangent).normalized()
        for j in range(sides):
            a = 2*pi*j/sides + phase
            vertices.append(p + across*cos(a)*rx + depth*sin(a)*rz)
    for i in range(len(profiles)-1):
        for j in range(sides):
            k = (j+1) % sides
            faces.append((i*sides+j, (i+1)*sides+j, (i+1)*sides+k, i*sides+k))
    faces.append(tuple(range(sides)))
    faces.append(tuple((len(profiles)-1)*sides+j for j in reversed(range(sides))))
    return mesh(name, vertices, faces, pigment, collection)

def ellipsoid(name, center, radii, pigment, collection):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=24, ring_count=16, location=center)
    obj = bpy.context.object
    obj.name = name
    obj.scale = radii
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    for c in list(obj.users_collection):
        c.objects.unlink(obj)
    collection.objects.link(obj)
    obj.data.materials.append(material(pigment))
    return obj

def tube(name, points, radii, pigment, collection, sides=12):
    if isinstance(radii, (float, int)):
        radii = [radii]*len(points)
    return loft(name, [(p, r, r*.92) for p, r in zip(points, radii)],
                pigment, collection, sides)

def band(name, y, radius, width, pigment, collection, ratio=1):
    return loft(name, [((0, y-width/2, 0), radius*.93, radius*.93*ratio),
                      ((0, y-width*.30, 0), radius, radius*ratio),
                      ((0, y+width*.30, 0), radius, radius*ratio),
                      ((0, y+width/2, 0), radius*.93, radius*.93*ratio)],
                pigment, collection)

def gem(name, center, radii, pigment, collection, sides=6):
    x, y, z = center
    rx, ry, rz = radii
    return loft(name, [((x, y-ry, z), rx*.18, rz*.18),
                      ((x, y-ry*.32, z), rx*.82, rz*.82),
                      ((x, y+ry*.08, z), rx, rz),
                      ((x, y+ry*.62, z), rx*.57, rz*.57),
                      ((x, y+ry, z), .0005, .0005)],
                pigment, collection, sides, pi/6)
