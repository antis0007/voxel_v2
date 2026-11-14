//! Exact, bounded collision reads. No gameplay mutation occurs while Pending.
use super::{player_aabb, try_move_with};
use crate::world::{BlockId, ReadOnlyPicker, Readiness, World};
use glam::{IVec3, Vec3};
use std::collections::HashSet;

type Unavailable = Readiness<()>;
#[derive(Clone, Copy)]
struct Bounds { a: IVec3, b: IVec3 }

// Check before any saturating float cast, integer addition, or bounded traversal.
fn bounds(min: Vec3, max: Vec3, origin: IVec3) -> Result<Bounds, String> {
    if !min.is_finite() || !max.is_finite() || !min.cmplt(max).all() {
        return Err("Invalid collision AABB".into());
    }
    let a = min.floor();
    let b = (max - Vec3::splat(0.001)).floor();
    for v in [a, b] {
        for i in 0..3 {
            if (v[i] as f64) < i32::MIN as f64 || (v[i] as f64) > i32::MAX as f64 {
                return Err("Collision coordinates exceed the supported range".into());
            }
        }
    }
    let (a, b) = (a.as_ivec3(), b.as_ivec3());
    let mut n = [0_i64; 3];
    for i in 0..3 {
        n[i] = i64::from(b[i]) - i64::from(a[i]) + 1;
        if n[i] < 0 || n[i] > ReadOnlyPicker::MAX_GROUP_POINTS as i64 {
            return Err("Collision AABB exceeds the read budget".into());
        }
        let halo = i64::from(i != 1);
        if i64::from(origin[i]) + i64::from(a[i]) - halo < i32::MIN as i64
            || i64::from(origin[i]) + i64::from(b[i]) + halo > i32::MAX as i64 {
            return Err("Collision origin and fence links exceed the supported range".into());
        }
    }
    // Cardinal links only: no diagonal/corner or vertical halo is queried.
    let points = if n.contains(&0) { 0 } else { (n[0]*n[2] + 2*n[0] + 2*n[2])*n[1] };
    if points > ReadOnlyPicker::MAX_GROUP_POINTS as i64 {
        return Err("Collision AABB and fence links exceed the read budget".into());
    }
    Ok(Bounds { a, b })
}

fn points(bounds: Bounds, origin: IVec3) -> Vec<IVec3> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    // Preserve the original cells first, then stable cardinal-neighbor order.
    for z in bounds.a.z..=bounds.b.z { for y in bounds.a.y..=bounds.b.y { for x in bounds.a.x..=bounds.b.x {
        let p = origin + IVec3::new(x,y,z);
        if seen.insert(p) { result.push(p); }
    }}}
    let cells = result.len();
    for i in 0..cells {
        for d in engine_content::shaped::FACINGS {
            let p = result[i] + d;
            if seen.insert(p) { result.push(p); }
        }
    }
    result
}

// The same traversal as the synchronous API, with a fallible source adapter.
fn collides_exact(
    world: &World, min: Vec3, max: Vec3, origin: IVec3,
    get: impl FnMut(IVec3) -> Result<BlockId, Unavailable>,
) -> Result<bool, Unavailable> {
    super::collides_try_with(world.sample_config(),min,max,origin,get,Some(world))
}

fn lookup(source: &mut ReadOnlyPicker, world: &World, p: IVec3) -> Result<BlockId, Unavailable> {
    match source.lookup_ready(world,p) {
        Readiness::Ready(block) => Ok(block),
        Readiness::Pending => Err(Readiness::Pending),
        Readiness::Failed(error) => Err(Readiness::Failed(error)),
    }
}

pub(crate) fn collides_ready_local(
    world: &World, min: Vec3, max: Vec3, origin: IVec3, source: &mut ReadOnlyPicker,
) -> Readiness<bool> {
    if !ReadOnlyPicker::supports(world) {
        return Readiness::Failed("Read-only collision does not support this world".into());
    }
    let bounds = match bounds(min,max,origin) { Ok(v)=>v, Err(e)=>return Readiness::Failed(e) };
    let first = collides_exact(world,min,max,origin,|p|lookup(source,world,p));
    let result = if matches!(first,Err(Readiness::Pending)) {
        // Preparation may include unused cells. Never expose their source errors.
        if let Err(error) = source.prepare_group(world,&points(bounds,origin)) {
            return Readiness::Failed(error);
        }
        collides_exact(world,min,max,origin,|p|lookup(source,world,p))
    } else { first };
    match result {
        Ok(hit)=>Readiness::Ready(hit), Err(Readiness::Failed(e))=>Readiness::Failed(e),
        Err(_)=>Readiness::Pending,
    }
}

pub(crate) fn try_move_ready_local(
    origin: IVec3, world: &World, player_scale: f32, crouch_t: f32,
    pos: &mut Vec3, delta: Vec3, on_ground: &mut bool, sneak_edge_check: bool,
    source: &mut ReadOnlyPicker,
) -> Readiness<()> {
    if !ReadOnlyPicker::supports(world) || !pos.is_finite() || !delta.is_finite()
        || !player_scale.is_finite() || !crouch_t.is_finite() {
        return Readiness::Failed("Unsupported or invalid collision movement".into());
    }
    let mut steps = 0_u32;
    for axis in 0..3 {
        if delta[axis].abs() < 1e-6 { continue; }
        let n = (delta[axis].abs()/0.45).ceil().max(1.0);
        if n > 256.0 { return Readiness::Failed("Movement exceeds the axis-step budget".into()); }
        steps += n as u32;
    }
    if steps > 256 { return Readiness::Failed("Movement exceeds the axis-step budget".into()); }
    let (min,max) = player_aabb(*pos,world.voxel_scale,player_scale,crouch_t);
    if let Err(error) = bounds(min,max,origin) { return Readiness::Failed(error); }
    if let Err(error) = movement_bounds(world,player_scale,crouch_t,*pos,delta,sneak_edge_check,origin) {
        return Readiness::Failed(error);
    }
    let (mut candidate,mut grounded) = (*pos,*on_ground);
    let mut queries = 0_u32;
    let result: Result<(), Unavailable> = try_move_with(world.voxel_scale,player_scale,crouch_t,&mut candidate,delta,
        &mut grounded,sneak_edge_check,|min,max| {
            queries += 1;
            if queries > 4096 { return Err(Readiness::Failed("Movement exceeds the collision-query budget".into())); }
            match collides_ready_local(world,min,max,origin,source) {
                Readiness::Ready(hit)=>Ok(hit), Readiness::Pending=>Err(Readiness::Pending),
                Readiness::Failed(e)=>Err(Readiness::Failed(e)),
            }
        });
    match result {
        Ok(())=>{*pos=candidate;*on_ground=grounded;Readiness::Ready(())},
        Err(Readiness::Failed(e))=>Readiness::Failed(e), Err(_)=>Readiness::Pending,
    }
}

// Called only after the axis-step limit is checked. Every axis advances
// monotonically; contact bisection stays between that step's endpoints. Repeat
// the actual f32 additions rather than trusting pos + delta, whose rounding can
// differ from the numerical movement path. Include both for a conservative sweep.
fn movement_bounds(
    world: &World, player_scale: f32, crouch_t: f32, pos: Vec3, delta: Vec3,
    sneak_edge_check: bool, origin: IVec3,
) -> Result<Bounds, String> {
    let mut end = pos;
    for axis in 0..3 {
        if delta[axis].abs() < 1e-6 { continue; }
        let steps = (delta[axis].abs()/0.45).ceil().max(1.0) as u32;
        let step = delta[axis] / steps as f32;
        for _ in 0..steps { end[axis] += step; }
    }
    let direct = pos + delta;
    if !end.is_finite() || !direct.is_finite() {
        return Err("Invalid collision movement sweep".into());
    }
    let mut low = pos.min(end).min(direct);
    let high = pos.max(end).max(direct);
    if sneak_edge_check {
        low.y += -0.05 / world.voxel_scale.meters().max(1e-6);
    }
    let (min,_) = player_aabb(low,world.voxel_scale,player_scale,crouch_t);
    let (_,max) = player_aabb(high,world.voxel_scale,player_scale,crouch_t);
    // This bounds the complete transaction, including speculative cardinal
    // fence neighbors, below the 1024-entry cache. A rejected trajectory never
    // starts source work or mutates the caller's movement state.
    bounds(min,max,origin).map_err(|error|format!("Movement sweep: {error}"))
}

#[cfg(test)]
mod tests;
