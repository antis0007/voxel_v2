//! Incoming sediment abrades uncovered tributary beds.

/// Cover is an evolving layer, not a hard incoming-flux cutoff. Excess supply
/// may cross a cell before settling; it must not be silently discarded or
/// switch all incision off at an arbitrary capacity ratio (Davy et al. 2026).
pub(in crate::planetary) fn exposed_abrasion(incoming: f64, capacity: f64, area: f64, soil: f64,
    erodibility: f64, grade: f64, spacing: f64) -> f64
{
    if capacity <= 0.0 || incoming <= 0.0 || grade <= 0.0 { return 0.0; }
    let impacting_depth = capacity * -(-incoming / capacity).exp_m1() / area;
    (0.27 * impacting_depth * (-soil / 0.75).exp() * erodibility * (grade / 0.16).sqrt())
        .min(0.16 * grade * spacing)
}

use super::super::review::catchment::structural::mesh::Link;

pub(super) fn forward(routes: &[Link], sediment: f64, load: &mut [f64]) {
    let total: f64 = routes.iter().map(|edge| f64::from(edge.share)).sum();
    for edge in routes {
        load[edge.to as usize] += sediment * f64::from(edge.share) / total;
    }
}

/// Effective-cycle abrasion with sediment tools and alluvial shielding.
pub(in crate::planetary) fn abrasion(incoming: f64, capacity: f64, area: f64, soil: f64,
    erodibility: f64, grade: f64, spacing: f64) -> f64
{
    if capacity <= 0.0 || incoming <= 0.0 || grade <= 0.0 { return 0.0; }
    // Sklar and Dietrich (2004): tools need moving sediment, while supply
    // approaching capacity buries the bed. Local detachment is not an input.
    let uncovered = (1.0 - incoming / capacity).max(0.0);
    let mobile_depth = incoming.min(capacity) / area;
    (0.27 * mobile_depth * uncovered * (-soil / 0.75).exp()
        * erodibility * (grade / 0.16).sqrt())
        .min(0.16 * grade * spacing)
}

#[cfg(test)]
mod tests {
    use super::abrasion;

    #[test]
    fn tools_require_transport_and_an_exposed_bed() {
        let cut = |load, capacity, soil, grade| abrasion(load,capacity,64.0,soil,1.0,grade,8.0);
        assert_eq!(cut(0.0,100.0,0.0,0.2),0.0);
        assert_eq!(cut(40.0,0.0,0.0,0.2),0.0);
        assert_eq!(cut(40.0,100.0,0.0,0.0),0.0);
        assert_eq!(cut(100.0,100.0,0.0,0.2),0.0);
        assert_eq!(cut(200.0,100.0,0.0,0.2),0.0);
        assert!(cut(40.0,100.0,0.0,0.2)>0.0);
        assert!(cut(40.0,100.0,3.0,0.2)<cut(40.0,100.0,0.0,0.2)*0.02);
    }

    #[test]
    fn incision_is_bounded_by_downhill_relief() {
        for spacing in [8.0,16.0,32.0] {
            let cut=abrasion(1e8,2e8,spacing*spacing,0.0,2.0,0.3,spacing);
            assert!(cut.is_finite() && cut>=0.0 && cut<=0.16*0.3*spacing);
        }
    }
}
