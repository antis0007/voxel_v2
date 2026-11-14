"""Deliberate polygon silhouettes and metre-consistent nearest-filtered UV islands."""
import bpy
import bmesh
from mathutils import Vector
from atlas import TILES

PARTS = {}
FINISH = {'wood':0.,'skin':0.,'cloth':.2,'leather':.1,'copper':.4,
          'ceramic':.2,'mineral':.6,'dark':.15}

def shape(name, outline, front, back, material, group, bone=0, bevel=.002,
          plane='xy', bulge=0.):
    """Extruded designed outline; bevels are structural edge planes, never a remesh."""
    count=len(outline)
    verts=[(x,y,z) for z in (back,front) for x,y in outline]
    faces=[tuple(reversed(range(count)))]
    if bulge:
        verts.append((sum(p[0] for p in outline)/count,
                      sum(p[1] for p in outline)/count,front+bulge))
        faces += [(i+count,(i+1)%count+count,count*2) for i in range(count)]
    else:
        faces.append(tuple(range(count,count*2)))
    faces += [(i,(i+1)%count,(i+1)%count+count,i+count) for i in range(count)]
    if plane=='xz': verts=[(x,z,y) for x,y,z in verts]
    mesh=bpy.data.meshes.new(name)
    mesh.from_pydata(verts,[],faces)
    mesh.update()
    obj=bpy.data.objects.new(name,mesh)
    bpy.context.collection.objects.link(obj)
    obj.data.materials.append(bpy.data.materials[material])
    bm=bmesh.new(); bm.from_mesh(mesh)
    bmesh.ops.recalc_face_normals(bm,faces=list(bm.faces))
    bm.to_mesh(mesh); bm.free()
    if bevel:
        bpy.ops.object.select_all(action='DESELECT')
        obj.select_set(True); bpy.context.view_layer.objects.active=obj
        mod=obj.modifiers.new('Single cut edge','BEVEL')
        mod.width=bevel; mod.segments=1
        bpy.ops.object.modifier_apply(modifier=mod.name)
    uv=obj.data.uv_layers.new(name='Pixel atlas')
    ox,oy,w,h=TILES[material]
    # 4.5 mm per texel on the hand; 5 mm cloth, 5.5 mm wood.
    pitch={'wood':.0055,'cloth':.005,'skin':.0045}.get(material,.0045)
    bounds=[(min(v.co[a] for v in obj.data.vertices),max(v.co[a] for v in obj.data.vertices))
            for a in range(3)]
    for face in obj.data.polygons:
        major=max(range(3),key=lambda a:abs(face.normal[a]))
        axes=(0,1) if major==2 else ((2,1) if major==0 else (0,2))
        for loop in face.loop_indices:
            p=obj.data.vertices[obj.data.loops[loop].vertex_index].co
            a,b=axes
            u=(p[a]-(bounds[a][0]+bounds[a][1])*.5)/pitch+w*.5
            v=-(p[b]-(bounds[b][0]+bounds[b][1])*.5)/pitch+h*.5
            uv.data[loop].uv=((ox+max(.5,min(w-.5,u)))/64,
                              1-(oy+max(.5,min(h-.5,v)))/64)
    obj['part']=group; obj['bone']=bone; obj['material_family']=material
    PARTS.setdefault(group,[]).append(obj)
    return obj

def box(name, x,y,w,h,front,back,material,group,bevel=.0015,bone=0):
    return shape(name,[(x-w/2,y-h/2),(x+w/2,y-h/2),(x+w/2,y+h/2),(x-w/2,y+h/2)],
                 front,back,material,group,bone,bevel)

def export(group,path):
    lines=['// Authored in tools/pixel_kit. UV.x >= 2 selects the 64px sRGB atlas.','(vertices: [']
    triangles=0
    def fmt(v): return '('+', '.join(f'{x:.6f}' for x in v)+')'
    for obj in PARTS[group]:
        mesh=obj.data; mesh.calc_loop_triangles()
        mat=obj['material_family']; bone=obj['bone']
        kind=54 if group in ('hand','sleeve') else 55
        tint=1 if mat=='leather' and group.startswith('shaft_') else 0
        for tri in mesh.loop_triangles:
            triangles+=1
            for loop in tri.loops:
                p=mesh.vertices[mesh.loops[loop].vertex_index].co
                uv=mesh.uv_layers.active.data[loop].uv
                # Blender's origin is bottom-left; uploaded RGBA is top-left.
                at=(uv.x+2,1-uv.y)
                weight=.4 if bone else 0.
                lines.append(f'(p:{fmt(p)}, n:{fmt(tri.normal)}, c:(1.0,1.0,1.0), '
                    f'uv:{fmt(at)}, kind:{kind}.0, finish:{FINISH[mat]:.2f}, tint:{tint}, '
                    f'bone:{bone}, weight:{weight:.2f}),')
    path.write_text('\n'.join(lines)+ '\n])\n',encoding='utf-8')
    return triangles
