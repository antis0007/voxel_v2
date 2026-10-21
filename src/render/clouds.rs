use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CloudClipmapRing {
    pub level: u8,
    pub span_m: f32,
    pub brick_world_size_m: f32,
    pub brick_resolution: u32,
}

impl CloudClipmapRing {
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

}
