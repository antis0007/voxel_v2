//! Bounded, opt-in cameras in generated sites. Never creates or edits a site.
use super::*;

pub fn place_review_view(cfg: &WorldSampleConfig, name: &str) -> Option<(Vec3, Vec3)> {
    if crate::fernwake::active(cfg) {
        let pair = match name {
            "fernwake-cistern" => Some((Vec3::new(316.5,3.2,48.5),Vec3::new(322.0,4.5,40.0))),
            "fernwake-reservoir" => Some((Vec3::new(313.0,7.0,36.5),Vec3::new(322.0,1.0,43.5))),
            "fernwake-receiver" => Some((Vec3::new(328.5,6.0,60.5),Vec3::new(321.0,-1.0,53.0))),
            "fernwake-borough" => Some((Vec3::new(391.5,3.0,44.5),Vec3::new(424.0,2.0,57.0))),
            "fernwake-orchard" => Some((Vec3::new(443.5,3.0,50.5),Vec3::new(468.0,6.0,40.0))),
            "fernwake-crossing" => Some((Vec3::new(487.5,2.0,48.5),Vec3::new(530.0,5.0,48.5))),
            "fernwake-freight" => Some((Vec3::new(541.5,2.0,48.5),Vec3::new(568.0,3.0,48.5))),
            "fernwake-service" => Some((Vec3::new(554.5,-6.0,85.5),Vec3::new(591.5,-6.0,85.5))),
            "fernwake-receiving" => Some((Vec3::new(574.5,2.0,48.5),Vec3::new(590.0,2.0,40.5))),
            _ => None,
        };
        if let Some((eye,target)) = pair {
            let offset = Vec3::Y * crate::habitats_region::floor(cfg);
            return Some((eye+offset,target+offset));
        }
    }
    let kind = match name { "castle" => 3, "mine" => 0, "pit" | "pitflood" => 2, _ => return None };
    for ring in 1i32..=24 {
        for rz in -ring..=ring { for rx in -ring..=ring {
            if rx.abs().max(rz.abs()) != ring { continue; }
            let Some(s) = site(cfg,rx,rz) else { continue; };
            if s.kind != kind { continue; }
            let (eye, target) = match name {
                "castle" => (Vec3::new(24.0,24.0,36.0),Vec3::new(0.0,6.0,0.0)),
                "mine" => (Vec3::new(12.0,-12.0,-31.5),Vec3::new(12.0,-11.5,-36.0)),
                "pitflood" => (Vec3::new(-6.5,-18.0,-12.0),Vec3::new(5.5,-19.0,-10.0)),
                _ => (Vec3::new(-5.5,-19.0,-11.5),Vec3::new(5.5,-17.5,-11.5)),
            };
            return Some((site_point(s,eye),site_point(s,target)));
        }}
    }
    None
}
