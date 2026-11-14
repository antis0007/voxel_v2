"""Blender 5: explicitly designed plant meshes, two variants and two LODs.

Run from the repository using Blender --background --python this_file.
No external Python packages. Export is a separate step, so editing the saved
.blend then running export.py does not regenerate or discard the manual edits.
"""
import math
import sys
from pathlib import Path
import bpy
from mathutils import Vector

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
from export import export_library

# sRGB pigments; converted only for Blender's linear material input.
PALETTE = {
    'root': '414c2c', 'grass': '687c43', 'grass_light': '829452',
    'grass_fold': '566c39', 'straw': 'b1a36a',
    'clover': '4e7947', 'clover_light': '719651', 'clover_fold': '42683d',
    'fern': '52754a', 'fern_light': '718d52', 'fern_fold': '405d3c',
    'stem': '63713b', 'petal': 'e2d6a7', 'petal_fold': 'c3bb8c',
    'heart': 'c49946', 'heart_dark': '8c6736',
    'cap': 'af774b', 'cap_top': 'c4915c', 'cap_edge': '97603d',
    'gills': 'bdad88', 'stalk': 'c6bb98',
}

def linear(v):
    return v / 12.92 if v <= .04045 else ((v + .055) / 1.055) ** 2.4

def pigment(name):
    rgb = tuple(int(PALETTE[name][i:i+2], 16) / 255 for i in (0, 2, 4))
    m = bpy.data.materials.new(name)
    m.diffuse_color = (*map(linear, rgb), 1)
    m.use_nodes = True
    m.node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = m.diffuse_color
    m.node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = .87
    m['export_srgb'] = rgb
    return m

class Plant:
    def __init__(self, name, collection):
        self.name, self.collection = name, collection
        self.vertices, self.faces, self.colors = [], [], []

    def face(self, points, color):
        start = len(self.vertices)
        self.vertices.extend(tuple(p) for p in points)
        self.faces.append(tuple(range(start, len(self.vertices))))
        self.colors.append(color)

    def ribbon(self, stations, angle, widths, colors, origin=(0, 0, 0), fold=.012):
        # Each station is (reach, height). A raised midrib gives the leaf a
        # folded cross section; broad connected segments replace triangle spikes.
        forward = Vector((math.cos(angle), math.sin(angle), 0))
        across = Vector((-math.sin(angle), math.cos(angle), 0))
        rows = []
        for (reach, height), width in zip(stations, widths):
            p = Vector(origin) + forward * reach + Vector((0, 0, height))
            rows.append((p - across * width, p + Vector((0, 0, fold)), p + across * width))
        for i in range(len(rows)-1):
            a, b = rows[i:i+2]
            for side in range(2):
                color = colors[min(i, len(colors)-1)]
                if side == 1 and color in ('grass', 'clover', 'fern'):
                    color += '_fold'
                self.face((a[side], b[side], b[side+1], a[side+1]), color)

    def stem(self, a, b, width, color='stem'):
        a, b = Vector(a), Vector(b)
        for axis in (Vector((width, 0, 0)), Vector((0, width, 0))):
            self.face((a-axis, b-axis, b+axis, a+axis), color)

    def finish(self, materials, lod, species, variant):
        mesh = bpy.data.meshes.new(self.name)
        mesh.from_pydata(self.vertices, [], self.faces)
        mesh.update()
        obj = bpy.data.objects.new(self.name, mesh)
        self.collection.objects.link(obj)
        for mat in materials.values():
            mesh.materials.append(mat)
        indices = {name: i for i, name in enumerate(materials)}
        for poly, color in zip(mesh.polygons, self.colors):
            poly.material_index = indices[color]
        obj['species'], obj['variant'], obj['lod'] = species, variant, lod
        obj['root_metres'] = (0., 0., 0.)
        obj['two_sided'] = species != 'mushroom'
        obj['wind'] = species != 'mushroom'
        return obj

def grass(p, variant, lod):
    # Two designed habits: an open bowing tussock and a compact upright fan.
    stems = [(0.0, .29, .20), (1.9, .43, .15), (3.8, .34, .23),
             (5.3, .25, .19), (.8, .37, .11), (3.0, .21, .20)]
    if lod: stems = stems[:4]
    for i, (angle, height, reach) in enumerate(stems):
        angle += variant * .37
        height *= 1.12 if variant else 1
        reach *= .78 if variant else 1
        if lod:
            stations = [(0, -.008), (reach*.35, height*.83), (reach, height)]
            widths = [.009, .023, .002]
        else:
            stations = [(0, -.008), (reach*.38, height*.82), (reach, height)]
            widths = [.009, .027, .002]
        p.ribbon(stations, angle, widths,
                 ['root', 'grass', 'straw' if i in (1, 4) and variant else 'grass_light'],
                 origin=(math.cos(angle)*.024, math.sin(angle)*.024, 0), fold=.003)

def clover(p, variant, lod):
    centers = [(-.10, -.045, .11), (.085, -.025, .15), (.0, .105, .09), (.13, .11, .075)]
    if lod: centers = centers[:3]
    for i, center in enumerate(centers):
        c = Vector(center) * (1.1 if variant else 1)
        if not lod: p.stem((c.x*.35, c.y*.35, -.006), c, .003)
        for j in range(3):
            angle = j*math.tau/3 + i*.63 + variant*.4
            out = Vector((math.cos(angle), math.sin(angle), .12))
            across = Vector((-math.sin(angle), math.cos(angle), 0))
            length = .083 if i != 2 else .07
            # Broad paired lobes and a shallow notch, with a raised center vein.
            contour = [c, c+out*length*.35-across*.036,
                       c+out*length*.78-across*.041, c+out*length-across*.019,
                       c+out*length*.88, c+out*length+across*.019,
                       c+out*length*.78+across*.041,c+out*length*.35+across*.036]
            ridge=c+out*length*.53+Vector((0,0,.007))
            if lod:
                p.face(contour, 'clover_light' if (i+j)%3 == 0 else 'clover')
            else:
                p.face((contour[0],contour[1],contour[2],contour[3],contour[4],ridge), 'clover_light')
                p.face((ridge,contour[4],contour[5],contour[6],contour[7],contour[0]), 'clover')

def flower(p, variant, lod):
    for i, (x, y, height) in enumerate([(-.045, -.02, .38), (.095, .04, .29)] if variant else [(0, 0, .43)]):
        c = Vector((x+.025, y, height))
        p.stem((x, y, -.006), c, .005)
        for side in (-1, 1):
            p.ribbon([(0, .09), (.09, .14), (.15, .145)], side*1.5+.2,
                     [.002, .025, .001], ['grass', 'grass_light'], origin=(x,y,0), fold=.003)
        for j in range(6):
            a = j*math.tau/6 + variant*.4
            out = Vector((math.cos(a), math.sin(a), 0))
            across = Vector((-math.sin(a), math.cos(a), 0))
            inner = c + out*.019
            outer = c + out*.080 + Vector((0,0,.014))
            p.face((inner-across*.012, outer-out*.014-across*.024,
                    outer-across*.014,outer+across*.014,
                    outer-out*.014+across*.024,inner+across*.012),
                   'petal' if j%3 else 'petal_fold')
        heart = [c+Vector((math.cos(j*math.tau/6)*.025, math.sin(j*math.tau/6)*.025, .009)) for j in range(6)]
        p.face(heart, 'heart')
        if not lod:
            for j in range(6):
                p.face((c-Vector((0,0,.013)), heart[(j+1)%6], heart[j]), 'heart_dark')

def fern(p, variant, lod):
    fronds = [(0.2,.36,.27), (1.8,.41,.35), (3.3,.35,.30), (4.9,.29,.23)]
    for i, (a, reach, height) in enumerate(fronds):
        a += variant*.3
        f = Vector((math.cos(a), math.sin(a), 0))
        side = Vector((-math.sin(a), math.cos(a), 0))
        ts = [.20,.43,.66,.84] if not lod else [.30,.65]
        # The rachis follows the same curve as every pinna attachment.
        path=[Vector((0,0,-.006))]+[f*(reach*t)+Vector((0,0,height*math.sin(t*1.65))) for t in ts]+[f*reach+Vector((0,0,height*.9))]
        for a0,b0 in zip(path,path[1:]):
            p.stem(a0,b0,.004,'fern_fold')
        for t in ts:
            root = f*(reach*t) + Vector((0,0,height*math.sin(t*1.65)))
            for sign in (-1,1):
                length = .125*(1.15-t)
                tip = root + side*(sign*length) + f*.045 + Vector((0,0,-.012))
                width = f*.032
                p.face((root, (root+tip)*.5-width, tip-width*.35,tip+width*.35,(root+tip)*.5+width),
                       'fern_light' if (i+int(t*10))%3 == 0 else 'fern')
        p.face((f*reach*.77+Vector((0,0,height*.95))-side*.02,
                f*reach+Vector((0,0,height*.9)),
                f*reach*.77+Vector((0,0,height*.95))+side*.02), 'fern_light')

def mushroom(p, variant, lod):
    mushrooms = [(-.065,0,.17,.082), (.075,.045,.105,.06), (.02,-.075,.085,.048)]
    for x,y,height,r in mushrooms[:2 if lod else 3]:
        if variant: height *= 1.25; r *= .85
        c=Vector((x,y,height))
        sides=6 if lod else 8
        rings=[]
        for radius,z in [(r*.55, -.035), (r, -.012), (r*.72, .025), (r*.24, .038)]:
            rings.append([c+Vector((math.cos(j*math.tau/sides)*radius,math.sin(j*math.tau/sides)*radius,z)) for j in range(sides)])
        for j in range(6):
            a,b=j*math.tau/6,(j+1)*math.tau/6
            p.face(((x+math.cos(a)*.012,y+math.sin(a)*.012,-.006),
                    (x+math.cos(b)*.012,y+math.sin(b)*.012,-.006),
                    (x+math.cos(b)*.012,y+math.sin(b)*.012,height-.025),
                    (x+math.cos(a)*.012,y+math.sin(a)*.012,height-.025)), 'stalk')
        for row, color in enumerate(['cap_edge','cap','cap_top']):
            for j in range(sides):
                p.face((rings[row][j],rings[row][(j+1)%sides],rings[row+1][(j+1)%sides],rings[row+1][j]), color)
        p.face(rings[-1], 'cap_top')
        p.face(list(reversed(rings[0])), 'gills')

def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    mats = {name: pigment(name) for name in PALETTE}
    full = bpy.data.collections.new('PLANTS - editable metre-scale meshes')
    low = bpy.data.collections.new('LOD1 - reduced leaf counts')
    scene.collection.children.link(full)
    scene.collection.children.link(low)
    authors = [('grass',grass), ('clover',clover), ('flower',flower), ('fern',fern), ('mushroom',mushroom)]
    for species_idx, (species, author) in enumerate(authors):
        for variant in range(2):
            for lod in range(2):
                p = Plant(f'{species}_{variant}_lod{lod}', full if lod == 0 else low)
                author(p, variant, lod)
                obj = p.finish(mats, lod, species, variant)
                obj.location = ((species_idx-2)*.90, variant*.88, 0)
                obj.hide_render = bool(lod)
    low.hide_viewport = True
    low.hide_render = True
    studio = bpy.data.collections.new('STUDIO - not exported')
    scene.collection.children.link(studio)
    # Matte individual plinths make the contact and species scales legible.
    floor_mat = bpy.data.materials.new('studio clay')
    floor_mat.diffuse_color=(.15,.18,.16,1)
    for species_idx in range(5):
        for variant in range(2):
            bpy.ops.mesh.primitive_cube_add(size=1, location=((species_idx-2)*.9,variant*.88,-.025))
            obj=bpy.context.object
            obj.name='Display base'
            obj.scale=(.80,.78,.04)
            obj.data.materials.append(floor_mat)
            for c in list(obj.users_collection): c.objects.unlink(obj)
            studio.objects.link(obj)
    def camera(name, pos, target, scale):
        data=bpy.data.cameras.new(name)
        obj=bpy.data.objects.new(name,data)
        studio.objects.link(obj)
        obj.location=pos
        obj.rotation_euler=(Vector(target)-obj.location).to_track_quat('-Z','Y').to_euler()
        data.type='ORTHO';data.ortho_scale=scale
        return obj
    scene.camera=camera('Library three-quarter',(1.6,-5.0,4.6),(0,.45,.12),5.0)
    camera('Library side',(0,-6,.8),(0,.45,.12),5.0)
    for name,pos,power,size,color in [('Soft daylight',(-2,-3,5),450,4,(1,.94,.83)),('Sky fill',(2,3,4),250,3,(.77,.87,1))]:
        data=bpy.data.lights.new(name,'AREA'); data.energy=power;data.size=size;data.color=color
        obj=bpy.data.objects.new(name,data);studio.objects.link(obj);obj.location=pos
        obj.rotation_euler=(Vector((0,.4,0))-obj.location).to_track_quat('-Z','Y').to_euler()
    scene.world=bpy.data.worlds.new('Neutral sky')
    scene.world.use_nodes=True
    scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.15,.18,.21,1)
    scene.world.node_tree.nodes['Background'].inputs[1].default_value=.55
    scene.render.engine='CYCLES';scene.cycles.samples=32;scene.cycles.use_denoising=True
    scene.render.resolution_x=1800;scene.render.resolution_y=1000;scene.render.resolution_percentage=100
    scene.view_settings.view_transform='AgX'
    scene['art_direction']='Folded leaves, connected roots, five readable families; sRGB pigment palette, no random surface noise.'
    output=ROOT/'assets/groundcover'
    output.mkdir(parents=True,exist_ok=True)
    evidence=ROOT/'target/art-overhaul-20261026'
    evidence.mkdir(parents=True,exist_ok=True)
    scene.render.filepath=str(evidence/'blender-library.png')
    # Camera material view is immediately useful when opening the source file.
    for area in bpy.context.screen.areas:
        if area.type=='VIEW_3D':
            area.spaces.active.region_3d.view_perspective='CAMERA'
            area.spaces.active.shading.color_type='MATERIAL'
    bpy.ops.wm.save_as_mainfile(filepath=str(output/'groundcover.blend'))
    export_library(output)
    bpy.ops.render.render(write_still=True)

if __name__=='__main__': main()
