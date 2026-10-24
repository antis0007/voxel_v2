# Greedy meshing (Lysenko; binary greedy meshing)
Source: Mikola Lysenko, "Meshing in a Minecraft Game", 0fps, 2012-06-30 (http://0fps.net/2012/06/30/meshing-in-a-minecraft-game/); binary variant: github.com/cgerikj/binary-greedy-meshing (credits Ethan Gore for V2 core and vertex pulling; "TanTan" credit unverified).
Adopt: Greedy merge of coplanar same-material faces per slice; binary bitmask occupancy and face masks (64-bit words) for bitwise culling and merging; 8-byte quads with vertex pulling for brick meshes.
Reject: Baked AO and float vertex formats; per-face naive or culled-only meshes as the main path; the 8x-optimal bound is a guarantee we do not need.
Why: Bitmask meshing is integer-only and fits 8^3 GPU-resident bricks on an iGPU, but merge rules must respect variable voxel scale and per-cell material and state flags.
