//! Explicit metre-based room plans. Bay coordinates describe architecture, not voxels.
//! Validation precedes allocation of a Wing; historical Wing::new stays unchanged.
use super::{Door, Palette, Roof, Windows, Wing, WingSpec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomRole {
    Passage,
    Hall,
    Shrine,
    Library,
    Workshop,
    Store,
    Crypt,
    Living,
}

#[derive(Clone, Debug)]
pub struct Room {
    pub role: RoomRole,
    /// Half-open bay rectangle [x0, z0, x1, z1]. Room identity is its vector index.
    pub bounds: [usize; 4],
}

/// A portal joins the cells on either side of an interior bay edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    X { x: usize, z: usize },
    Z { x: usize, z: usize },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Front,
    Back,
    Left,
    Right,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entrance {
    pub side: Side,
    pub bay: usize,
}

pub struct Plan {
    pub bays: [usize; 2],
    pub bay_metres: f32,
    pub height_metres: f32,
    pub rooms: Vec<Room>,
    pub portals: Vec<Edge>,
    pub entrances: Vec<Entrance>,
    /// Require matched room geometry, roles, portals and entrances across X.
    pub mirror_x: bool,
    pub roof: Roof,
    /// Material roles remain separate from room topology and voxel resolution.
    pub palette: Palette,
}

impl Plan {
    fn cells(&self, voxel_metres: f32) -> Result<Vec<u8>, &'static str> {
        let [nx, nz] = self.bays;
        if nx == 0
            || nz == 0
            || nx > 64
            || nz > 64
            || self.rooms.is_empty()
            || self.rooms.len() > 254
        {
            return Err("plan exceeds bounded room/bay budget");
        }
        if !voxel_metres.is_finite()
            || voxel_metres <= 0.0
            || voxel_metres > 1.0
            || !self.bay_metres.is_finite()
            || self.bay_metres < 3.6
            || self.bay_metres > 32.0
            || !self.height_metres.is_finite()
            || self.height_metres < 3.6
            || self.height_metres > 32.0
        {
            return Err("unsupported scale or insufficient architectural clearance");
        }
        let mut cells = vec![255; nx * nz];
        for (id, room) in self.rooms.iter().enumerate() {
            let [x0, z0, x1, z1] = room.bounds;
            if x0 >= x1 || z0 >= z1 || x1 > nx || z1 > nz {
                return Err("room outside plan");
            }
            if self.mirror_x
                && !self
                    .rooms
                    .iter()
                    .any(|r| r.role == room.role && r.bounds == [nx - x1, z0, nx - x0, z1])
            {
                return Err("room lacks mirrored counterpart");
            }
            for z in z0..z1 {
                for x in x0..x1 {
                    let cell = &mut cells[z * nx + x];
                    if *cell != 255 {
                        return Err("overlapping rooms");
                    }
                    *cell = id as u8;
                }
            }
        }
        if cells.contains(&255) {
            return Err("unassigned interior bay");
        }
        let mut graph = vec![Vec::new(); self.rooms.len()];
        for (i, edge) in self.portals.iter().enumerate() {
            if self.portals[..i].contains(edge) {
                return Err("duplicate portal");
            }
            let (a, b, mirrored) = match *edge {
                Edge::X { x, z } if x > 0 && x < nx && z < nz => {
                    (z * nx + x - 1, z * nx + x, Edge::X { x: nx - x, z })
                }
                Edge::Z { x, z } if z > 0 && z < nz && x < nx => {
                    ((z - 1) * nx + x, z * nx + x, Edge::Z { x: nx - 1 - x, z })
                }
                _ => return Err("portal is not an interior edge"),
            };
            let (a, b) = (cells[a] as usize, cells[b] as usize);
            if a == b {
                return Err("portal must connect distinct rooms");
            }
            if self.mirror_x && !self.portals.contains(&mirrored) {
                return Err("portal lacks mirror");
            }
            graph[a].push(b);
            graph[b].push(a);
        }
        if self.entrances.is_empty() {
            return Err("plan has no entrance");
        }
        let mut roots = Vec::new();
        for (i, entrance) in self.entrances.iter().enumerate() {
            if self.entrances[..i].contains(entrance) {
                return Err("duplicate entrance");
            }
            let Entrance { side, bay } = *entrance;
            let (cell, mirrored) = match side {
                Side::Front if bay < nx => (
                    bay,
                    Entrance {
                        side,
                        bay: nx - 1 - bay,
                    },
                ),
                Side::Back if bay < nx => (
                    (nz - 1) * nx + bay,
                    Entrance {
                        side,
                        bay: nx - 1 - bay,
                    },
                ),
                Side::Left if bay < nz => (
                    bay * nx,
                    Entrance {
                        side: Side::Right,
                        bay,
                    },
                ),
                Side::Right if bay < nz => (
                    bay * nx + nx - 1,
                    Entrance {
                        side: Side::Left,
                        bay,
                    },
                ),
                _ => return Err("entrance outside facade"),
            };
            if self.mirror_x && !self.entrances.contains(&mirrored) {
                return Err("entrance lacks mirror");
            }
            roots.push(cells[cell] as usize);
        }
        // All rooms must belong to one navigable component, even with several exits.
        let mut seen = vec![false; self.rooms.len()];
        let mut stack = vec![roots[0]];
        while let Some(room) = stack.pop() {
            if seen[room] {
                continue;
            }
            seen[room] = true;
            stack.extend(graph[room].iter().copied());
        }
        if seen.contains(&false) {
            return Err("disconnected room graph");
        }
        Ok(cells)
    }

    pub fn validate(&self, voxel_metres: f32) -> Result<(), &'static str> {
        self.cells(voxel_metres).map(|_| ())
    }

    /// Compile one floor into the existing sampler. Keep the Plan for semantic queries.
    /// Caller must sample with the validated scale; arbitrary world-grid phase still
    /// requires native clearance inspection, particularly at one metre resolution.
    pub fn compile(
        &self,
        origin: [f32; 2],
        seed: u32,
        voxel_metres: f32,
    ) -> Result<Wing, &'static str> {
        self.compile_floors(origin, seed, voxel_metres, 1, Windows::None, None, 0.0)
    }

    /// Repeat a room plan around a shared stairwell in its circulation core.
    pub(crate) fn compile_floors(
        &self,
        origin: [f32; 2],
        seed: u32,
        voxel_metres: f32,
        storeys: usize,
        windows: Windows,
        stairs: Option<[usize; 2]>,
        age: f32,
    ) -> Result<Wing, &'static str> {
        let cells = self.cells(voxel_metres)?;
        if !origin.iter().all(|v| v.is_finite()) {
            return Err("non-finite origin");
        }
        let [nx, nz] = self.bays;
        let stair = stairs.map(|[sx, ra]| super::Stair { sx, ra, len: 2 });
        if !(1..=32).contains(&storeys)
            || storeys > 1 && !stair.is_some_and(|st| {
                st.sx < nx - 1 && st.ra < nz - 1
                    && cells[st.ra * nx + st.sx] == cells[st.ra * nx + st.sx + 1]
                    && cells[st.ra * nx + st.sx] == cells[(st.ra + 1) * nx + st.sx]
                    && cells[st.ra * nx + st.sx] == cells[(st.ra + 1) * nx + st.sx + 1]
            })
        {
            return Err("stairwell must fit in a shared room");
        }
        let mut spec = WingSpec::new(origin[0], origin[1], nx, nz, seed);
        spec.cell = self.bay_metres;
        spec.h = self.height_metres;
        spec.open = true;
        spec.roof = self.roof;
        spec.pal = self.palette;
        spec.windows = windows;
        spec.storeys = storeys;
        spec.age = age;
        spec.doors = self
            .entrances
            .iter()
            .map(|e| Door {
                side: match e.side {
                    Side::Front => 0,
                    Side::Back => 1,
                    Side::Left => 2,
                    Side::Right => 3,
                },
                u: (e.bay as f32 + 0.5) * self.bay_metres,
                hw: 1.0,
                height: 2.8,
            })
            .collect();
        let mut wing = Wing::new(spec);
        wing.stair = stair;
        for floor in 0..storeys {
            wing.rooms[floor].clone_from(&cells);
            for edge in &self.portals {
                match *edge {
                    Edge::X { x, z } => wing.door_x[floor][z * (nx + 1) + x] = true,
                    Edge::Z { x, z } => wing.door_z[floor][z * nx + x] = true,
                }
            }
        }
        wing.connected |= storeys > 1;
        wing.planned = true;
        Ok(wing)
    }

    /// A processional sanctuary with paired side chambers, longitudinal loops,
    /// a central approach and a full-width terminal hall. 6-14 rooms, not random BSP.
    pub fn sanctuary(pairs: usize) -> Result<Self, &'static str> {
        if !(2..=6).contains(&pairs) {
            return Err("sanctuary requires 2-6 chamber pairs");
        }
        let nz = pairs + 1;
        let mut rooms = vec![
            Room {
                role: RoomRole::Passage,
                bounds: [1, 0, 2, pairs],
            },
            Room {
                role: RoomRole::Hall,
                bounds: [0, pairs, 3, nz],
            },
        ];
        let mut portals = vec![Edge::Z { x: 1, z: pairs }];
        for z in 0..pairs {
            let role = if z == pairs - 1 {
                RoomRole::Shrine
            } else {
                RoomRole::Library
            };
            rooms.push(Room {
                role,
                bounds: [0, z, 1, z + 1],
            });
            rooms.push(Room {
                role,
                bounds: [2, z, 3, z + 1],
            });
            portals.extend([Edge::X { x: 1, z }, Edge::X { x: 2, z }]);
            if z > 0 {
                portals.extend([Edge::Z { x: 0, z }, Edge::Z { x: 2, z }]);
            }
        }
        Ok(Self {
            bays: [3, nz],
            bay_metres: 4.0,
            height_metres: 4.0,
            rooms,
            portals,
            entrances: vec![Entrance {
                side: Side::Front,
                bay: 1,
            }],
            mirror_x: true,
            roof: Roof::Gable,
            palette: Palette::LIMESTONE,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sanctuary_compiles_connected_symmetric_multiroom_plans() {
        for pairs in 2..=6 {
            for scale in [1.0, 0.5, 0.25, 0.125] {
                let p = Plan::sanctuary(pairs).unwrap();
                let w = p.compile([0.0, 0.0], 42, scale).unwrap();
                assert_eq!(
                    w.rooms[0].iter().copied().max().unwrap() as usize + 1,
                    pairs * 2 + 2
                );
                assert!(w.planned);
                assert_eq!(w.door_x[0].iter().filter(|&&v| v).count(), pairs * 2);
                for edge in &p.portals {
                    let (x, z) = match *edge {
                        Edge::X { x, z } => (x as f32 * 4.0, (z as f32 + 0.5) * 4.0),
                        Edge::Z { x, z } => ((x as f32 + 0.5) * 4.0, z as f32 * 4.0),
                    };
                    for y in [1.25, 2.25] {
                        assert_eq!(
                            w.sample(glam::Vec3::new(x, y, z), scale, -1.0),
                            Some(super::super::B::Air)
                        );
                    }
                }
                // Wall beside a portal still encloses its chamber.
                assert_eq!(
                    w.sample(glam::Vec3::new(4.0, 1.5, 1.05), scale, -1.0),
                    Some(p.palette.wall)
                );
            }
        }
    }
    #[test]
    fn malformed_plans_are_rejected_before_sampling() {
        let mut p = Plan::sanctuary(3).unwrap();
        p.portals.pop();
        assert!(p.validate(0.25).is_err());
        let mut p = Plan::sanctuary(3).unwrap();
        p.rooms[2].bounds = p.rooms[3].bounds;
        assert!(p.validate(0.25).is_err());
        let mut p = Plan::sanctuary(3).unwrap();
        p.portals.clear();
        assert_eq!(p.validate(0.25), Err("disconnected room graph"));
        let p = Plan::sanctuary(3).unwrap();
        assert!(p.validate(f32::NAN).is_err());
        assert!(p.validate(2.0).is_err());
    }
}
