"""An editable five-finger armature matching the engine's compact hand rig."""
import bpy
from mathutils import Vector
from bake import BINDINGS
from anatomy import PIVOTS, WRIST, skin_weight

NAMES=['Index','Middle','Ring','Little','Thumb']

def add_hand_rig(hand, collection):
    data=bpy.data.armatures.new('Hand grip skeleton')
    arm=bpy.data.objects.new('Hand grip skeleton',data)
    collection.objects.link(arm)
    bpy.ops.object.select_all(action='DESELECT')
    arm.select_set(True)
    bpy.context.view_layer.objects.active=arm
    bpy.ops.object.mode_set(mode='EDIT')
    root=data.edit_bones.new('Palm')
    root.head=WRIST
    root.tail=(.027,.053,-.010)
    for name,pivot in zip(NAMES,PIVOTS):
        bone=data.edit_bones.new(name)
        bone.head=pivot
        bone.tail=Vector(pivot)+Vector((0,.018,0))
        bone.parent=root
    bpy.ops.object.mode_set(mode='OBJECT')
    groups=[hand.vertex_groups.new(name=n) for n in ['Palm']+NAMES]
    source,bones=BINDINGS['hand']
    for vertex in hand.data.vertices:
        p=vertex.co
        _,_,index,_=source.find_nearest(p)
        bone=bones[index]
        weight=skin_weight(bone,p)
        groups[0].add([vertex.index],1-weight,'REPLACE')
        if bone:
            groups[bone].add([vertex.index],weight,'REPLACE')
    modifier=hand.modifiers.new('Grip articulation','ARMATURE')
    modifier.object=arm
    arm.show_in_front=True
    arm.hide_render=True
    arm.hide_set(True)
