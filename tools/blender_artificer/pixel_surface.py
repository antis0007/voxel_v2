"""Broad authored planes with a small, deliberate pixel-paint palette."""
from math import floor, ceil
import bpy
import bmesh
from mathutils import Vector
from geometry import PALETTE


def united(collection, obj, source, pigments, apply):
    """Exact unions retain the authored knuckles instead of stair remeshing."""
    originals=list(collection.objects)
    def shape(original, target):
        target.from_pydata([original.matrix_world @ v.co for v in original.data.vertices],
                          [],[list(p.vertices) for p in original.data.polygons])
        bm=bmesh.new()
        bm.from_mesh(target)
        bmesh.ops.recalc_face_normals(bm,faces=list(bm.faces))
        bm.to_mesh(target)
        bm.free()
    obj.data.clear_geometry()
    shape(originals[0],obj.data)
    for original in originals[1:]:
        data=bpy.data.meshes.new('union operand')
        shape(original,data)
        operand=bpy.data.objects.new('union operand',data)
        bpy.context.scene.collection.objects.link(operand)
        modifier=obj.modifiers.new('Joined anatomical surface','BOOLEAN')
        modifier.operation='UNION'
        modifier.solver='EXACT'
        modifier.object=operand
        apply(obj,modifier)
        bpy.data.objects.remove(operand,do_unlink=True)
        bpy.data.meshes.remove(data)
    for face in obj.data.polygons:
        _,_,index,_=source.find_nearest(face.center)
        face.material_index=pigments[index]
    bm=bmesh.new()
    bm.from_mesh(obj.data)
    bm.normal_update()
    bmesh.ops.dissolve_limit(bm,angle_limit=.025,verts=list(bm.verts),
                            edges=list(bm.edges),delimit={'MATERIAL'})
    bm.to_mesh(obj.data)
    bm.free()
    obj.data.update()


def clip(poly, axis, edge, keep_above):
    result=[]
    for a,b in zip(poly,poly[1:]+poly[:1]):
        da,db=(a[axis]-edge),(b[axis]-edge)
        inside_a=da>=-1e-8 if keep_above else da<=1e-8
        inside_b=db>=-1e-8 if keep_above else db<=1e-8
        if inside_a:
            result.append(a)
        if inside_a!=inside_b and abs(da-db)>1e-10:
            result.append(a.lerp(b,da/(da-db)))
    return result


def pigment_at(name,material,p):
    base=PALETTE[material][0]
    x,y,z=p
    gain=1.0
    if material=='skin':
        # Connected warm clusters over the knuckles and palm heel. No freckles,
        # random dirt or line drawn around every voxel.
        if x < -.016 and -.012 < y < .061:
            gain=1.075
        elif x > .044 and .002 < y < .036 and z > -.004:
            gain=1.055
        elif y < -.022:
            gain=.95
        if z > .037 and .005 < x < .025 and .029 < y < .048:
            return (.64,.48,.36)
    elif material=='cloth':
        if -.080 < y < -.045 and x > .023:
            gain=1.12
        elif -.142 < y < -.110 and z > .010:
            gain=1.10
        elif -.120 < y < -.055 and x < -.030:
            gain=.86
        if z > .022 and 0 <= floor(x/.012) <= 1 and -8 <= floor(y/.012) <= -6:
            return (.225,.29,.255)
    return tuple(c*gain for c in base)


def painted(obj):
    """Split broad planes only at paint cells; export uses these flat colours."""
    vertices,faces,pigments,colours=[],[],[],[]
    step=.008 if obj.name=='hand' else .012
    for face in obj.data.polygons:
        points=[obj.data.vertices[i].co.copy() for i in face.vertices]
        axes=[i for i in range(3) if i!=max(range(3),key=lambda i:abs(face.normal[i]))]
        u,v=axes
        material=obj.data.materials[face.material_index].name
        for i in range(floor(min(p[u] for p in points)/step),ceil(max(p[u] for p in points)/step)):
            for j in range(floor(min(p[v] for p in points)/step),ceil(max(p[v] for p in points)/step)):
                polygon=points
                for axis,edge,above in [(u,i*step,True),(u,(i+1)*step,False),
                                         (v,j*step,True),(v,(j+1)*step,False)]:
                    if polygon:
                        polygon=clip(polygon,axis,edge,above)
                if len(polygon)<3:
                    continue
                center=sum(polygon,Vector())/len(polygon)
                colour=pigment_at(obj.name,material,center)
                start=len(vertices)
                vertices.extend(polygon)
                faces.append(tuple(range(start,start+len(polygon))))
                pigments.append(face.material_index)
                colours.append(colour)
    data=bpy.data.meshes.new(obj.name+' pixel-painted topology')
    data.from_pydata(vertices,[],faces)
    for mat in obj.data.materials:
        data.materials.append(mat)
    for face,index in zip(data.polygons,pigments):
        face.material_index=index
    paint=data.color_attributes.new(name='Pixel paint',type='FLOAT_COLOR',domain='CORNER')
    for face,colour in zip(data.polygons,colours):
        for i in face.loop_indices:
            paint.data[i].color_srgb=(*colour,1)
    # Keep the paint boundaries, not a triangle grid across quiet solid colours.
    bm=bmesh.new()
    bm.from_mesh(data)
    bmesh.ops.remove_doubles(bm,verts=list(bm.verts),dist=.000001)
    bm.normal_update()
    layer=bm.loops.layers.float_color.get('Pixel paint')
    edges=[]
    for edge in bm.edges:
        if len(edge.link_faces)!=2:
            continue
        a,b=edge.link_faces
        same_colour=sum(abs(x-y) for x,y in zip(a.loops[0][layer],b.loops[0][layer]))<1e-6
        if same_colour and a.material_index==b.material_index and a.normal.dot(b.normal)>.99999:
            edges.append(edge)
    bmesh.ops.dissolve_edges(bm,edges=edges,use_verts=True,use_face_split=False)
    bm.to_mesh(data)
    bm.free()
    data.update()
    obj.data=data
    # The exact same baked pigment drives Blender and the exported game mesh.
    for index in set(pigments):
        mat=data.materials[index]
        if not mat.use_nodes:
            continue
        nodes=mat.node_tree.nodes
        if nodes.get('Pixel paint') is None:
            vertex=nodes.new('ShaderNodeVertexColor')
            vertex.name='Pixel paint'
            vertex.layer_name='Pixel paint'
            mat.node_tree.links.new(vertex.outputs['Color'],nodes['Principled BSDF'].inputs['Base Color'])
