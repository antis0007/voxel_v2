"""Export the currently open groundcover .blend without rebuilding its meshes."""
import hashlib
import json
from pathlib import Path
import bpy

SPECIES = ['grass','clover','flower','fern','mushroom']

def export_library(output):
    assets=[]
    report={'blender':bpy.app.version_string,'source':bpy.data.filepath,'assets':[]}
    for species in SPECIES:
        for variant in range(2):
            for lod in range(2):
                obj=bpy.data.objects[f'{species}_{variant}_lod{lod}']
                mesh=obj.data
                mesh.calc_loop_triangles()
                vertices=[]
                indices=[]
                unique={}
                for tri in mesh.loop_triangles:
                    color=tuple(mesh.materials[tri.material_index]['export_srgb'])
                    for index in tri.vertices:
                        # Blender Z-up -> engine Y-up, with determinant +1.
                        v=mesh.vertices[index].co
                        n=mesh.polygons[tri.polygon_index].normal
                        vertex=tuple(round(x,6) for x in (v.x,v.z,-v.y,n.x,n.z,-n.y,*color))
                        if vertex not in unique:
                            unique[vertex]=len(vertices);vertices.append(vertex)
                        indices.append(unique[vertex])
                def floats(values): return ','.join(f'{v:.6f}' for v in values)
                vs=','.join(f'(p:({floats(v[:3])}),n:({floats(v[3:6])}),c:({floats(v[6:])}))' for v in vertices)
                double=bool(obj['two_sided'])
                assets.append(f'(vertices:[{vs}],indices:[{",".join(map(str,indices))}],two_sided:{str(double).lower()})')
                report['assets'].append({'name':obj.name,'vertices':len(vertices),'triangles':len(indices)//3*(2 if double else 1),'two_sided':double})
    export=output/'library.ron'
    export.write_text('// Blender-authored groundcover. Use export.py to retain manual edits.\n(assets:[\n'+',\n'.join(assets)+'\n])\n',encoding='utf-8')
    report['sha256']=hashlib.sha256(export.read_bytes()).hexdigest()
    (output/'manifest.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
    print('GROUNDCOVER',json.dumps(report),flush=True)

if __name__=='__main__':
    export_library(Path(bpy.data.filepath).parent)
