//! Repeated actual foot traffic commissions paid paving through the normal builder.
use super::*;
use super::super::{B, Order};

const LIMIT: usize = 256;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct State {
    traffic: BTreeMap<[i32; 3], u8>,
    pub built: u32,
}

pub(super) fn surface(block: B) -> bool {
    matches!(block, B::Grass | B::Dirt | B::Gravel | B::Sand)
}

fn field(w: &World, floor: IVec3) -> bool {
    let at = floor + IVec3::Y;
    w.farming.plots.contains_key(&(at.x,at.y,at.z))
        || w.farming.jobs.production.as_ref().is_some_and(|p| p.beds.iter().any(|b|b.at==at.to_array()))
}

impl State {
    pub fn valid(&self) -> bool {
        self.traffic.len() <= LIMIT && self.traffic.keys()
            .all(|p| p.iter().all(|v| v.unsigned_abs() < 1 << 28))
    }

    pub fn observe(&mut self, w: &World, pile: IVec3, before: Vec3, after: Vec3) {
        let distance=before.distance(after);
        let steps=(distance/w.voxel_scale.meters()).ceil().clamp(1.0,128.0) as usize;
        let mut previous=before;
        // A certified walk may finish many cells in one wake. Its actual segment
        // supplies the same foot traffic without per-frame resident updates.
        for i in 1..=steps {
            let point=before.lerp(after,i as f32/steps as f32);
            self.visit(w,pile,previous,point);
            previous=point;
        }
    }

    fn visit(&mut self, w:&World, pile:IVec3, before:Vec3, after:Vec3) {
        let v = w.voxel_scale.meters();
        let a = ((before - Vec3::Y * 0.05) / v).floor().as_ivec3();
        let b = ((after - Vec3::Y * 0.05) / v).floor().as_ivec3();
        if a == b || (b - pile).as_vec3().length_squared() * v * v > super::super::work_range(&w.farming.jobs).powi(2)
            || !w.resident_block(b).is_some_and(surface)
            || field(w,b) || crate::world::farming::base::overlaps(w,b,b) { return; }
        if self.traffic.len() == LIMIT && !self.traffic.contains_key(&b.to_array()) {
            let victim = self.traffic.iter().min_by_key(|(p,n)| (**n,**p)).map(|(&p,_)|p);
            if let Some(victim) = victim { self.traffic.remove(&victim); }
        }
        let n = self.traffic.entry(b.to_array()).or_default();
        *n = n.saturating_add(1);
    }

    pub fn plan(&mut self, w: &mut World) {
        let mut queued = 0;
        for (&at, &count) in &self.traffic {
            if queued >= 4 || w.farming.jobs.build.len() >= 128 { break; }
            if count < 4 || w.farming.jobs.build.iter().any(|o| o.at == at)
                || w.farming.jobs.mine.iter().any(|(p,_)| *p == at) { continue; }
            let cell = IVec3::from(at);
            let Some(block) = w.resident_block(cell).filter(|&b| surface(b)) else { continue; };
            if field(w,cell) || crate::world::farming::base::overlaps(w,cell,cell) { continue; }
            w.farming.jobs.build.push(Order::pave(cell,block));
            queued += 1;
        }
    }
}
