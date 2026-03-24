use glam::IVec3;
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MatterState {
    Vacuum,
    Solid,
    Powder,
    Liquid,
    Gas,
    Plasma,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollisionKind {
    Passable,
    Solid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderKind {
    Invisible,
    Opaque,
    Cutout,
    Liquid,
}

#[derive(Clone, Copy, Debug)]
pub struct FluidTraits {
    pub viscosity: f32,
    pub compressibility: f32,
    pub pressure_response: f32,
}

impl FluidTraits {
    pub const NONE: Self = Self {
        viscosity: 0.0,
        compressibility: 0.0,
        pressure_response: 0.0,
    };
}

#[derive(Clone, Copy, Debug)]
pub struct MaterialPhysicsTraits {
    pub matter_state: MatterState,
    pub collision: CollisionKind,
    pub density: f32,
    pub hardness: f32,
    pub friction: f32,
    pub restitution: f32,
    pub fluid: FluidTraits,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TintPattern {
    None,
    SumXZParity,
    DiffXZParity,
    AbsSumXZParity,
    SumXZParityPlusYParity,
}

#[derive(Clone, Copy, Debug)]
pub struct ColorModel {
    pub base: [f32; 3],
    pub base_tint: f32,
    pub parity_tint: f32,
    pub vertical_tint: f32,
    pub pattern: TintPattern,
}

impl ColorModel {
    pub const fn flat(base: [f32; 3]) -> Self {
        Self {
            base,
            base_tint: 1.0,
            parity_tint: 0.0,
            vertical_tint: 0.0,
            pattern: TintPattern::None,
        }
    }

    pub const fn tinted(
        base: [f32; 3],
        base_tint: f32,
        parity_tint: f32,
        vertical_tint: f32,
        pattern: TintPattern,
    ) -> Self {
        Self {
            base,
            base_tint,
            parity_tint,
            vertical_tint,
            pattern,
        }
    }

    pub fn sample(self, p: IVec3) -> [f32; 3] {
        let parity = match self.pattern {
            TintPattern::None => 0.0,
            TintPattern::SumXZParity => ((p.x + p.z).abs() & 1) as f32,
            TintPattern::DiffXZParity => ((p.x - p.z).abs() & 1) as f32,
            TintPattern::AbsSumXZParity => ((p.x.abs() + p.z.abs()) & 1) as f32,
            TintPattern::SumXZParityPlusYParity => ((p.x + p.z).abs() & 1) as f32,
        };
        let y_parity = if matches!(self.pattern, TintPattern::SumXZParityPlusYParity) {
            (p.y.abs() & 1) as f32
        } else {
            0.0
        };
        let tint = self.base_tint + self.parity_tint * parity + self.vertical_tint * y_parity;
        [
            self.base[0] * tint,
            self.base[1] * tint,
            self.base[2] * tint,
        ]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MaterialRenderTraits {
    pub kind: RenderKind,
    pub occludes_light: bool,
    pub color: ColorModel,
    pub surface: SurfaceDetail,
}

#[derive(Clone, Copy, Debug)]
pub struct SurfaceDetail {
    pub roughness: f32,
    pub specular: f32,
    pub detail_scale: f32,
    pub detail_strength: f32,
    pub ao_strength: f32,
}

impl SurfaceDetail {
    pub const fn new(
        roughness: f32,
        specular: f32,
        detail_scale: f32,
        detail_strength: f32,
        ao_strength: f32,
    ) -> Self {
        Self {
            roughness,
            specular,
            detail_scale,
            detail_strength,
            ao_strength,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MaterialTags {
    pub selectable: bool,
    pub hotbar_order: Option<u8>,
    pub contributes_to_heightmap: bool,
    pub render_in_lod_decor: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct MaterialDef {
    pub id: BlockId,
    pub display_name: &'static str,
    pub render: MaterialRenderTraits,
    pub physics: MaterialPhysicsTraits,
    pub tags: MaterialTags,
}

macro_rules! define_materials {
    (
        $(
            $id:ident {
                display: $display:literal,
                render_kind: $render_kind:expr,
                occludes: $occludes:expr,
                color: $color:expr,
                surface: $surface:expr,
                matter: $matter:expr,
                collision: $collision:expr,
                density: $density:expr,
                hardness: $hardness:expr,
                friction: $friction:expr,
                restitution: $restitution:expr,
                fluid: $fluid:expr,
                selectable: $selectable:expr,
                hotbar: $hotbar:expr,
                heightmap: $heightmap:expr,
                lod_decor: $lod_decor:expr
            }
        ),+ $(,)?
    ) => {
        #[repr(u8)]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        pub enum BlockId {
            $( $id, )+
        }

        pub const ALL_BLOCK_IDS: &[BlockId] = &[ $(BlockId::$id),+ ];

        pub const MATERIAL_TABLE: [MaterialDef; define_materials!(@count $($id),+)] = [
            $(
                MaterialDef {
                    id: BlockId::$id,
                    display_name: $display,
                    render: MaterialRenderTraits {
                        kind: $render_kind,
                        occludes_light: $occludes,
                        color: $color,
                        surface: $surface,
                    },
                    physics: MaterialPhysicsTraits {
                        matter_state: $matter,
                        collision: $collision,
                        density: $density,
                        hardness: $hardness,
                        friction: $friction,
                        restitution: $restitution,
                        fluid: $fluid,
                    },
                    tags: MaterialTags {
                        selectable: $selectable,
                        hotbar_order: $hotbar,
                        contributes_to_heightmap: $heightmap,
                        render_in_lod_decor: $lod_decor,
                    },
                },
            )+
        ];
    };
    (@count $($x:ident),+) => {
        <[()]>::len(&[$(define_materials!(@replace $x ())),+])
    };
    (@replace $_t:tt $sub:expr) => {
        $sub
    };
}

define_materials! {
    Air {
        display: "Air",
        render_kind: RenderKind::Invisible,
        occludes: false,
        color: ColorModel::flat([0.0, 0.0, 0.0]),
        surface: SurfaceDetail::new(1.0, 0.0, 0.0, 0.0, 0.0),
        matter: MatterState::Vacuum,
        collision: CollisionKind::Passable,
        density: 0.0,
        hardness: 0.0,
        friction: 0.0,
        restitution: 0.0,
        fluid: FluidTraits::NONE,
        selectable: false,
        hotbar: None,
        heightmap: false,
        lod_decor: false
    },
    Grass {
        display: "Grass",
        render_kind: RenderKind::Opaque,
        occludes: true,
        color: ColorModel::tinted([0.20, 0.78, 0.24], 0.90, 0.10, 0.0, TintPattern::SumXZParity),
        surface: SurfaceDetail::new(0.88, 0.05, 0.14, 0.22, 0.42),
        matter: MatterState::Solid,
        collision: CollisionKind::Solid,
        density: 1500.0,
        hardness: 0.50,
        friction: 0.82,
        restitution: 0.05,
        fluid: FluidTraits::NONE,
        selectable: true,
        hotbar: Some(1),
        heightmap: true,
        lod_decor: false
    },
    Dirt {
        display: "Dirt",
        render_kind: RenderKind::Opaque,
        occludes: true,
        color: ColorModel::tinted([0.47, 0.31, 0.18], 0.92, 0.08, 0.0, TintPattern::DiffXZParity),
        surface: SurfaceDetail::new(0.95, 0.03, 0.12, 0.18, 0.48),
        matter: MatterState::Powder,
        collision: CollisionKind::Solid,
        density: 1650.0,
        hardness: 0.58,
        friction: 0.86,
        restitution: 0.03,
        fluid: FluidTraits::NONE,
        selectable: true,
        hotbar: Some(2),
        heightmap: true,
        lod_decor: false
    },
    Stone {
        display: "Stone",
        render_kind: RenderKind::Opaque,
        occludes: true,
        color: ColorModel::tinted(
            [0.44, 0.44, 0.46],
            0.85,
            0.15,
            0.0,
            TintPattern::AbsSumXZParity,
        ),
        surface: SurfaceDetail::new(0.62, 0.18, 0.24, 0.42, 0.58),
        matter: MatterState::Solid,
        collision: CollisionKind::Solid,
        density: 2700.0,
        hardness: 0.95,
        friction: 0.78,
        restitution: 0.07,
        fluid: FluidTraits::NONE,
        selectable: true,
        hotbar: Some(3),
        heightmap: true,
        lod_decor: false
    },
    Sand {
        display: "Sand",
        render_kind: RenderKind::Opaque,
        occludes: true,
        color: ColorModel::tinted([0.82, 0.76, 0.48], 0.92, 0.08, 0.0, TintPattern::SumXZParity),
        surface: SurfaceDetail::new(0.84, 0.07, 0.10, 0.16, 0.38),
        matter: MatterState::Powder,
        collision: CollisionKind::Solid,
        density: 1700.0,
        hardness: 0.35,
        friction: 0.74,
        restitution: 0.02,
        fluid: FluidTraits::NONE,
        selectable: true,
        hotbar: Some(4),
        heightmap: true,
        lod_decor: false
    },
    Sandstone {
        display: "Sandstone",
        render_kind: RenderKind::Opaque,
        occludes: true,
        color: ColorModel::tinted([0.74, 0.66, 0.44], 0.90, 0.10, 0.0, TintPattern::DiffXZParity),
        surface: SurfaceDetail::new(0.72, 0.10, 0.14, 0.24, 0.44),
        matter: MatterState::Solid,
        collision: CollisionKind::Solid,
        density: 2200.0,
        hardness: 0.74,
        friction: 0.76,
        restitution: 0.04,
        fluid: FluidTraits::NONE,
        selectable: true,
        hotbar: None,
        heightmap: true,
        lod_decor: false
    },
    SnowGrass {
        display: "Snow Grass",
        render_kind: RenderKind::Opaque,
        occludes: true,
        color: ColorModel::tinted([0.85, 0.90, 0.92], 0.94, 0.06, 0.0, TintPattern::SumXZParity),
        surface: SurfaceDetail::new(0.54, 0.12, 0.18, 0.16, 0.36),
        matter: MatterState::Solid,
        collision: CollisionKind::Solid,
        density: 1400.0,
        hardness: 0.52,
        friction: 0.60,
        restitution: 0.05,
        fluid: FluidTraits::NONE,
        selectable: true,
        hotbar: None,
        heightmap: true,
        lod_decor: false
    },
    Water {
        display: "Water",
        render_kind: RenderKind::Liquid,
        occludes: false,
        color: ColorModel::tinted([0.18, 0.38, 0.72], 0.88, 0.12, 0.0, TintPattern::SumXZParity),
        surface: SurfaceDetail::new(0.10, 0.42, 0.34, 0.10, 0.10),
        matter: MatterState::Liquid,
        collision: CollisionKind::Passable,
        density: 1000.0,
        hardness: 0.0,
        friction: 0.02,
        restitution: 0.0,
        fluid: FluidTraits {
            viscosity: 1.0,
            compressibility: 0.03,
            pressure_response: 1.0,
        },
        selectable: false,
        hotbar: None,
        heightmap: false,
        lod_decor: false
    },
    Log {
        display: "Log",
        render_kind: RenderKind::Opaque,
        occludes: true,
        color: ColorModel::flat([0.46, 0.33, 0.18]),
        surface: SurfaceDetail::new(0.74, 0.06, 0.22, 0.26, 0.42),
        matter: MatterState::Solid,
        collision: CollisionKind::Solid,
        density: 680.0,
        hardness: 0.42,
        friction: 0.70,
        restitution: 0.06,
        fluid: FluidTraits::NONE,
        selectable: true,
        hotbar: None,
        heightmap: false,
        lod_decor: true
    },
    Torch {
        display: "Torch",
        render_kind: RenderKind::Opaque,
        occludes: false,
        color: ColorModel::flat([0.40, 0.25, 0.08]),
        surface: SurfaceDetail::new(0.82, 0.06, 0.18, -0.22, 0.18),
        matter: MatterState::Solid,
        collision: CollisionKind::Passable,
        density: 420.0,
        hardness: 0.12,
        friction: 0.62,
        restitution: 0.03,
        fluid: FluidTraits::NONE,
        selectable: true,
        hotbar: Some(0),
        heightmap: false,
        lod_decor: false
    },
    TorchWallNorth {
        display: "Torch",
        render_kind: RenderKind::Opaque,
        occludes: false,
        color: ColorModel::flat([0.40, 0.25, 0.08]),
        surface: SurfaceDetail::new(0.82, 0.06, 0.18, -0.22, 0.18),
        matter: MatterState::Solid,
        collision: CollisionKind::Passable,
        density: 420.0,
        hardness: 0.12,
        friction: 0.62,
        restitution: 0.03,
        fluid: FluidTraits::NONE,
        selectable: false,
        hotbar: None,
        heightmap: false,
        lod_decor: false
    },
    TorchWallSouth {
        display: "Torch",
        render_kind: RenderKind::Opaque,
        occludes: false,
        color: ColorModel::flat([0.40, 0.25, 0.08]),
        surface: SurfaceDetail::new(0.82, 0.06, 0.18, -0.22, 0.18),
        matter: MatterState::Solid,
        collision: CollisionKind::Passable,
        density: 420.0,
        hardness: 0.12,
        friction: 0.62,
        restitution: 0.03,
        fluid: FluidTraits::NONE,
        selectable: false,
        hotbar: None,
        heightmap: false,
        lod_decor: false
    },
    TorchWallWest {
        display: "Torch",
        render_kind: RenderKind::Opaque,
        occludes: false,
        color: ColorModel::flat([0.40, 0.25, 0.08]),
        surface: SurfaceDetail::new(0.82, 0.06, 0.18, -0.22, 0.18),
        matter: MatterState::Solid,
        collision: CollisionKind::Passable,
        density: 420.0,
        hardness: 0.12,
        friction: 0.62,
        restitution: 0.03,
        fluid: FluidTraits::NONE,
        selectable: false,
        hotbar: None,
        heightmap: false,
        lod_decor: false
    },
    TorchWallEast {
        display: "Torch",
        render_kind: RenderKind::Opaque,
        occludes: false,
        color: ColorModel::flat([0.40, 0.25, 0.08]),
        surface: SurfaceDetail::new(0.82, 0.06, 0.18, -0.22, 0.18),
        matter: MatterState::Solid,
        collision: CollisionKind::Passable,
        density: 420.0,
        hardness: 0.12,
        friction: 0.62,
        restitution: 0.03,
        fluid: FluidTraits::NONE,
        selectable: false,
        hotbar: None,
        heightmap: false,
        lod_decor: false
    },
    Leaves {
        display: "Leaves",
        render_kind: RenderKind::Cutout,
        occludes: true,
        color: ColorModel::tinted([0.18, 0.56, 0.20], 0.88, 0.12, 0.0, TintPattern::DiffXZParity),
        surface: SurfaceDetail::new(0.92, 0.04, 0.16, 0.24, 0.30),
        matter: MatterState::Solid,
        collision: CollisionKind::Passable,
        density: 120.0,
        hardness: 0.10,
        friction: 0.30,
        restitution: 0.02,
        fluid: FluidTraits::NONE,
        selectable: true,
        hotbar: None,
        heightmap: false,
        lod_decor: true
    },
    Bush {
        display: "Bush",
        render_kind: RenderKind::Cutout,
        occludes: true,
        color: ColorModel::tinted(
            [0.22, 0.58, 0.17],
            0.82,
            0.10,
            0.08,
            TintPattern::SumXZParityPlusYParity,
        ),
        surface: SurfaceDetail::new(0.90, 0.05, 0.18, 0.30, 0.34),
        matter: MatterState::Solid,
        collision: CollisionKind::Passable,
        density: 180.0,
        hardness: 0.08,
        friction: 0.34,
        restitution: 0.03,
        fluid: FluidTraits::NONE,
        selectable: true,
        hotbar: None,
        heightmap: false,
        lod_decor: true
    },
    Gravel {
        display: "Gravel",
        render_kind: RenderKind::Opaque,
        occludes: true,
        color: ColorModel::flat([0.52, 0.52, 0.50]),
        surface: SurfaceDetail::new(0.78, 0.08, 0.18, 0.28, 0.46),
        matter: MatterState::Powder,
        collision: CollisionKind::Solid,
        density: 1800.0,
        hardness: 0.48,
        friction: 0.80,
        restitution: 0.02,
        fluid: FluidTraits::NONE,
        selectable: true,
        hotbar: None,
        heightmap: true,
        lod_decor: false
    },
    Clay {
        display: "Clay",
        render_kind: RenderKind::Opaque,
        occludes: true,
        color: ColorModel::tinted([0.62, 0.56, 0.52], 0.92, 0.08, 0.0, TintPattern::DiffXZParity),
        surface: SurfaceDetail::new(0.82, 0.06, 0.16, 0.20, 0.50),
        matter: MatterState::Powder,
        collision: CollisionKind::Solid,
        density: 1900.0,
        hardness: 0.64,
        friction: 0.84,
        restitution: 0.03,
        fluid: FluidTraits::NONE,
        selectable: true,
        hotbar: Some(5),
        heightmap: true,
        lod_decor: false
    },
    Mud {
        display: "Mud",
        render_kind: RenderKind::Opaque,
        occludes: true,
        color: ColorModel::tinted(
            [0.36, 0.29, 0.20],
            0.90,
            0.10,
            0.0,
            TintPattern::AbsSumXZParity,
        ),
        surface: SurfaceDetail::new(0.96, 0.03, 0.12, 0.18, 0.56),
        matter: MatterState::Powder,
        collision: CollisionKind::Solid,
        density: 1850.0,
        hardness: 0.50,
        friction: 0.93,
        restitution: 0.01,
        fluid: FluidTraits::NONE,
        selectable: true,
        hotbar: Some(6),
        heightmap: true,
        lod_decor: false
    },
    Basalt {
        display: "Basalt",
        render_kind: RenderKind::Opaque,
        occludes: true,
        color: ColorModel::tinted(
            [0.20, 0.21, 0.24],
            0.88,
            0.12,
            0.0,
            TintPattern::AbsSumXZParity,
        ),
        surface: SurfaceDetail::new(0.46, 0.22, 0.28, 0.48, 0.60),
        matter: MatterState::Solid,
        collision: CollisionKind::Solid,
        density: 3000.0,
        hardness: 1.10,
        friction: 0.82,
        restitution: 0.05,
        fluid: FluidTraits::NONE,
        selectable: true,
        hotbar: Some(7),
        heightmap: true,
        lod_decor: false
    },
    Ice {
        display: "Ice",
        render_kind: RenderKind::Opaque,
        occludes: true,
        color: ColorModel::tinted([0.62, 0.78, 0.94], 0.93, 0.07, 0.0, TintPattern::SumXZParity),
        surface: SurfaceDetail::new(0.18, 0.28, 0.24, 0.18, 0.24),
        matter: MatterState::Solid,
        collision: CollisionKind::Solid,
        density: 917.0,
        hardness: 0.30,
        friction: 0.04,
        restitution: 0.02,
        fluid: FluidTraits::NONE,
        selectable: true,
        hotbar: Some(8),
        heightmap: true,
        lod_decor: false
    },
    Lava {
        display: "Lava",
        render_kind: RenderKind::Liquid,
        occludes: false,
        color: ColorModel::tinted([0.92, 0.32, 0.08], 0.88, 0.12, 0.0, TintPattern::DiffXZParity),
        surface: SurfaceDetail::new(0.20, 0.48, 0.30, 0.14, 0.08),
        matter: MatterState::Liquid,
        collision: CollisionKind::Passable,
        density: 3100.0,
        hardness: 0.0,
        friction: 0.01,
        restitution: 0.0,
        fluid: FluidTraits {
            viscosity: 12.0,
            compressibility: 0.004,
            pressure_response: 0.55,
        },
        selectable: true,
        hotbar: None,
        heightmap: false,
        lod_decor: false
    }
}

static HOTBAR_DEFAULTS: LazyLock<Vec<BlockId>> = LazyLock::new(|| {
    let mut slots = MATERIAL_TABLE
        .iter()
        .filter_map(|def| def.tags.hotbar_order.map(|order| (order, def.id)))
        .collect::<Vec<_>>();
    slots.sort_by_key(|(order, _)| *order);
    slots.into_iter().map(|(_, id)| id).collect()
});

impl BlockId {
    pub const COUNT: usize = MATERIAL_TABLE.len();

    pub fn from_u32(raw: u32) -> Self {
        ALL_BLOCK_IDS
            .get(raw as usize)
            .copied()
            .unwrap_or(BlockId::Air)
    }

    pub const fn as_u32(self) -> u32 {
        self as u32
    }

    pub fn material(self) -> &'static MaterialDef {
        &MATERIAL_TABLE[self as usize]
    }

    pub fn display_name(self) -> &'static str {
        self.material().display_name
    }

    pub fn color(self, p: IVec3) -> [f32; 3] {
        self.material().render.color.sample(p)
    }

    pub fn surface(self) -> SurfaceDetail {
        self.material().render.surface
    }

    pub fn is_solid(self) -> bool {
        matches!(self.material().physics.collision, CollisionKind::Solid)
    }

    pub fn is_opaque(self) -> bool {
        self.material().render.occludes_light
    }

    pub fn is_empty(self) -> bool {
        matches!(self.material().render.kind, RenderKind::Invisible)
    }

    pub fn is_liquid(self) -> bool {
        matches!(self.material().render.kind, RenderKind::Liquid)
            || matches!(self.material().physics.matter_state, MatterState::Liquid)
    }

    pub fn contributes_to_heightmap(self) -> bool {
        self.material().tags.contributes_to_heightmap
    }

    pub fn render_in_lod_decor(self) -> bool {
        self.material().tags.render_in_lod_decor
    }

    pub fn is_torch(self) -> bool {
        matches!(
            self,
            BlockId::Torch
                | BlockId::TorchWallNorth
                | BlockId::TorchWallSouth
                | BlockId::TorchWallWest
                | BlockId::TorchWallEast
        )
    }

    pub fn is_ray_pickable(self) -> bool {
        self.is_solid() || self.is_torch()
    }

    pub fn torch_support_offset(self) -> Option<IVec3> {
        match self {
            BlockId::Torch => Some(IVec3::new(0, -1, 0)),
            BlockId::TorchWallNorth => Some(IVec3::new(0, 0, 1)),
            BlockId::TorchWallSouth => Some(IVec3::new(0, 0, -1)),
            BlockId::TorchWallWest => Some(IVec3::new(1, 0, 0)),
            BlockId::TorchWallEast => Some(IVec3::new(-1, 0, 0)),
            _ => None,
        }
    }

    pub fn torch_from_attachment(normal: IVec3) -> Option<Self> {
        match normal {
            IVec3 { x: 0, y: 1, z: 0 } => Some(BlockId::Torch),
            IVec3 { x: 0, y: 0, z: -1 } => Some(BlockId::TorchWallNorth),
            IVec3 { x: 0, y: 0, z: 1 } => Some(BlockId::TorchWallSouth),
            IVec3 { x: -1, y: 0, z: 0 } => Some(BlockId::TorchWallWest),
            IVec3 { x: 1, y: 0, z: 0 } => Some(BlockId::TorchWallEast),
            _ => None,
        }
    }

    pub fn hotbar_defaults() -> &'static [BlockId] {
        hotbar_defaults()
    }
}

pub fn material_defs() -> &'static [MaterialDef] {
    &MATERIAL_TABLE
}

pub fn hotbar_defaults() -> &'static [BlockId] {
    HOTBAR_DEFAULTS.as_slice()
}

pub fn hotbar_block(slot: usize) -> Option<BlockId> {
    hotbar_defaults().get(slot).copied()
}

pub fn cycle_hotbar_block(current: BlockId, dir: i32) -> BlockId {
    let blocks = hotbar_defaults();
    if blocks.is_empty() {
        return current;
    }
    let current_index = blocks
        .iter()
        .position(|block| *block == current)
        .unwrap_or(0);
    let next = (current_index as i32 + dir).rem_euclid(blocks.len() as i32) as usize;
    blocks[next]
}

#[derive(Clone, Copy)]
pub struct TerrainGenerationIds {
    pub block_ids0: [u32; 4],
    pub block_ids1: [u32; 4],
    pub block_ids2: [u32; 4],
}

pub const fn terrain_generation_ids() -> TerrainGenerationIds {
    TerrainGenerationIds {
        block_ids0: [
            BlockId::Air.as_u32(),
            BlockId::Grass.as_u32(),
            BlockId::Dirt.as_u32(),
            BlockId::Stone.as_u32(),
        ],
        block_ids1: [
            BlockId::Sand.as_u32(),
            BlockId::Sandstone.as_u32(),
            BlockId::SnowGrass.as_u32(),
            BlockId::Water.as_u32(),
        ],
        block_ids2: [
            BlockId::Log.as_u32(),
            BlockId::Leaves.as_u32(),
            BlockId::Bush.as_u32(),
            BlockId::Gravel.as_u32(),
        ],
    }
}
