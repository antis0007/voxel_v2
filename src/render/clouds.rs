use std::collections::HashMap;

pub const CLOUD_BRICK_RESOLUTION: u32 = 32;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CloudClipmapRing {
    pub level: u8,
    pub span_m: f32,
    pub brick_world_size_m: f32,
    pub brick_resolution: u32,
}

impl CloudClipmapRing {
    pub fn brick_extent_m(self) -> f32 {
        self.brick_world_size_m.max(1.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CloudBrickCoord {
    pub level: u8,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CloudBrickPageMeta {
    pub resident: bool,
    pub occupancy: f32,
    pub min_density: f32,
    pub max_density: f32,
}

#[derive(Debug, Default)]
pub struct CloudStorageScaffold {
    rings: Vec<CloudClipmapRing>,
    pages: HashMap<CloudBrickCoord, CloudBrickPageMeta>,
}

impl CloudStorageScaffold {
    pub fn new(rings: Vec<CloudClipmapRing>) -> Self {
        Self {
            rings,
            pages: HashMap::new(),
        }
    }

    pub fn default_rings() -> Vec<CloudClipmapRing> {
        vec![
            CloudClipmapRing {
                level: 0,
                span_m: 24_000.0,
                brick_world_size_m: 250.0,
                brick_resolution: CLOUD_BRICK_RESOLUTION,
            },
            CloudClipmapRing {
                level: 1,
                span_m: 96_000.0,
                brick_world_size_m: 750.0,
                brick_resolution: CLOUD_BRICK_RESOLUTION,
            },
            CloudClipmapRing {
                level: 2,
                span_m: 288_000.0,
                brick_world_size_m: 2_250.0,
                brick_resolution: CLOUD_BRICK_RESOLUTION,
            },
        ]
    }

    pub fn rings(&self) -> &[CloudClipmapRing] {
        &self.rings
    }

    pub fn page(&self, coord: CloudBrickCoord) -> Option<&CloudBrickPageMeta> {
        self.pages.get(&coord)
    }

    pub fn upsert_page(&mut self, coord: CloudBrickCoord, meta: CloudBrickPageMeta) {
        self.pages.insert(coord, meta);
    }

    pub fn resident_pages(&self) -> impl Iterator<Item = (&CloudBrickCoord, &CloudBrickPageMeta)> {
        self.pages.iter().filter(|(_, meta)| meta.resident)
    }
}
