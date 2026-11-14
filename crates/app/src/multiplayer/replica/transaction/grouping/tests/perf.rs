use super::*;

#[test]
#[ignore = "wall-clock grouping comparison; run without concurrent workloads"]
fn grouping_bucket_cost() {
    for (label,frame) in [("dense4k",dense(4096)),("dense64k",dense(65536)),("dense256k",dense(MAX_EDITS)),
        ("sparse512",sparse()),("full_mixed",full(false)),("full_alternating",full(true))] {
        frame.validate().unwrap();let missing=missing(&frame);let mut old=Vec::new();let mut new=Vec::new();
        for pass in 0..11 {
            let (a,b)=(copy(&frame),copy(&frame));
            let (actual,expected);
            if pass%2==0 {
                let now=Instant::now();expected=reference::group(a,&missing).unwrap();old.push(now.elapsed().as_secs_f64()*1e3);
                let now=Instant::now();actual=prepare(b,&missing).unwrap();new.push(now.elapsed().as_secs_f64()*1e3);
            } else {
                let now=Instant::now();actual=prepare(a,&missing).unwrap();new.push(now.elapsed().as_secs_f64()*1e3);
                let now=Instant::now();expected=reference::group(b,&missing).unwrap();old.push(now.elapsed().as_secs_f64()*1e3);
            }
            assert_groups(&actual.1,&expected.1);
        }
        old.remove(0);new.remove(0);old.sort_by(f64::total_cmp);new.sort_by(f64::total_cmp);
        eprintln!("[grouping-cost] case={label} reference_p50_ms={:.3} bucket_p50_ms={:.3} reference_max_ms={:.3} bucket_max_ms={:.3} ratio={:.3}",old[5],new[5],old[9],new[9],new[5]/old[5]);
    }
}
