use super::*;
const SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/city_service_expansion.ron"));

#[derive(Deserialize)]
pub(super) struct Content {
    pub version: u32,
    pub style: Tradition,
    pub foundation: B,
    pub frontage: B,
    pub open_windows: bool,
    pub apron: i32,
    pub complexity: u8,
    pub capacity: u8,
    pub furniture: [i32; 3],
    pub candidates: Vec<Candidate>,
}
#[derive(Deserialize)]
pub(super) struct Candidate {
    pub hub: [i32; 2],
    pub origin: [i32; 2],
    pub turn: bool,
    pub approach: [i32; 2],
    pub route: Vec<[i32; 2]>,
}
pub(super) fn provenance() -> u64 {
    static VALUE: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *VALUE.get_or_init(|| SOURCE.bytes().fold(0xcbf29ce484222325,
        |h, byte| (h ^ u64::from(byte)).wrapping_mul(0x100000001b3)))
}
pub(super) fn read() -> Result<&'static Content, String> {
    static VALUE: std::sync::OnceLock<Result<Content, String>> = std::sync::OnceLock::new();
    VALUE.get_or_init(parse).as_ref().map_err(Clone::clone)
}
fn parse() -> Result<Content, String> {
    let c: Content = ron::from_str(SOURCE).map_err(|e| format!("Service Content: {e}"))?;
    if c.version != 1 || c.complexity != 1 || !(1..=8).contains(&c.capacity)
        || c.style.form != Roof::Gable || c.style.frame != B::Log
        || [c.style.wall, c.style.roof, c.foundation, c.frontage].iter().any(|b| *b != B::Planks)
        || !c.open_windows || c.apron != 1 || c.furniture != [1, 1, 13]
        || c.candidates.is_empty() || c.candidates.len() > 8 {
        return Err("Service Content Invalid".into());
    }
    for spec in &c.candidates {
        if spec.route.is_empty() || spec.route.len() > 8
            || spec.route.iter().chain([&spec.hub, &spec.origin, &spec.approach]).flatten()
                .any(|n| n.unsigned_abs() > 48)
            || (spec.hub[0] - spec.approach[0]).abs() + (spec.hub[1] - spec.approach[1]).abs() != 1 {
            return Err("Service Candidate Invalid".into());
        }
        let r = route(spec.approach, &spec.route)?;
        let q = parcel(&c, spec, [0; 3]);
        let sign = if spec.turn { -1 } else { 1 };
        if !r.contains(&q.door()) || r.last() != Some(&[q.door()[0], q.door()[1] + sign]) {
            return Err("Service Entrance Disconnected".into());
        }
    }
    Ok(c)
}
