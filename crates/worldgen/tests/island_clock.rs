#[path = "../src/planetary/island_clock.rs"]
mod clock;

#[test]
fn biological_response_is_independent_of_subdivision() {
    let full = 1.0-clock::relaxation(0.25);
    let substep = 1.0-clock::relaxation(0.025);
    assert!((full-substep.powi(10)).abs() < 1e-14);
    assert_eq!(clock::relaxation(0.0),0.0);
    for i in 0..1000 {
        let value = clock::relaxation(i as f64/1000.0);
        assert!(value.is_finite() && (0.0..=1.0).contains(&value));
    }
}

#[test]
fn rock_to_sand_and_growth_limits_scale_with_elapsed_time() {
    assert_eq!(clock::growth_cap(0.25),3.0);
    assert_eq!(clock::weathering_cap(0.25),0.5);
    assert!((10.0*clock::growth_cap(0.025)-clock::growth_cap(0.25)).abs() < 1e-14);
    assert!((10.0*clock::weathering_cap(0.025)-clock::weathering_cap(0.25)).abs() < 1e-14);
}

#[test]
fn old_neighbour_does_not_age_a_young_volcano() {
    assert_eq!(clock::elapsed(12.0,2.0,60.0),2.0);
    assert_eq!(clock::elapsed(0.0,0.0,60.0),60.0);
    assert_eq!(clock::elapsed(12.0,60.0,2.0),60.0);
}
