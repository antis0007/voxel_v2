#[path = "../src/planetary/erosion_clock.rs"]
mod clock;

#[test]
fn bare_bed_gets_the_full_erosion_interval() {
    assert_eq!(clock::exposed_time(0.0,0.0),1.0);
    assert_eq!(clock::exposed_time(0.0,0.3),1.0);
}

#[test]
fn clearing_alluvium_consumes_time_before_bedrock_abrasion() {
    assert_eq!(clock::exposed_time(0.15,0.3),0.5);
    assert_eq!(clock::exposed_time(0.3,0.3),0.0);
    assert_eq!(clock::exposed_time(3.0,0.3),0.0);
    assert_eq!(clock::exposed_time(0.15,0.0),0.0);
}

#[test]
fn subdivision_preserves_total_exposed_time_for_constant_forcing() {
    let mut cover = 0.35_f64;
    let mut time = 0.0;
    for _ in 0..10 {
        time += 0.1*clock::exposed_time(cover,0.1);
        cover = (cover-0.1).max(0.0);
    }
    assert!((time-clock::exposed_time(0.35,1.0)).abs()<1e-12);
}
