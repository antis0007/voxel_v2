use glam::{Vec2, Vec3};

pub const RADIUS_M: f32 = 1_500.0;

pub struct RegionalSurvey {
    pub settlements: Vec<super::Settlement>,
    pub roads: Vec<Vec<Vec2>>,
    pub cities: Vec<Vec2>,
}

pub fn survey(cfg: &crate::WorldSampleConfig, center: Vec3) -> RegionalSurvey {
    RegionalSurvey {
        settlements: super::settlements_near(cfg, center, RADIUS_M),
        roads: super::roads_near(cfg, center, RADIUS_M),
        cities: crate::ruins::near(cfg, center, RADIUS_M).into_iter()
            .filter(|district| district.is_city()).map(|district| district.centre).collect(),
    }
}
