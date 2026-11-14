//! Saturated slope failures coupled to drainage and the sediment budget.
//! Reduced CPU model informed by Jain et al. (2024), not their GPU solver.
use super::{Mesh, transport, smooth};
use super::super::review::catchment::structural::mesh::Routing;

pub(super) fn advance(mesh: &Mesh, flow: &Routing, fixed: &[bool], rain: &[f64],
    resistance: &[f64], bed: &mut [f64], soil: &mut [f64], mass: &mut [f64])
{
    mass.fill(0.0);
    for &node in flow.order.iter().rev() {
        let i = node as usize;
        let links = flow.links[i].as_slice();
        if fixed[i] {
            transport::forward(links, mass[i], mass);
            continue;
        }
        let saturation = smooth(0.42, 0.90, rain[i]);
        let cohesion = 0.48 + 0.26 * resistance[i].clamp(0.3, 2.0);
        let excess = (flow.grade[i] - cohesion * (1.0 - 0.30 * saturation)).max(0.0);
        let failed = (excess * mesh.grid.cell_m * 0.13 * saturation).min(1.5);
        let loose = failed.min(soil[i]);
        // Resistant strata remain as ledges above the evacuated chute.
        let fresh = (failed - loose) / resistance[i].max(0.35);
        soil[i] -= loose;
        bed[i] -= fresh;
        let load = mass[i] + (loose + fresh) * mesh.area[i];
        let mobility = smooth(0.06, 0.36, flow.grade[i]) * (0.45 + 0.45 * saturation);
        let retained = if links.is_empty() { load } else { load * (1.0 - mobility) };
        soil[i] += retained / mesh.area[i];
        transport::forward(links, load - retained, mass);
    }
}
