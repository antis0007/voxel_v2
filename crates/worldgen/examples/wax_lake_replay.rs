//! Reduced fixed-bed replay of the Wax Lake Gadwall / Main junction.
//! Hiatt & Passalacqua (2015), Table 1, https://doi.org/10.1002/2014WR016149.
//! Surveyed branch geometry is supplied to the model. Branch discharge is held out.
use engine_worldgen::planetary::review::relative_conveyance;

struct Section {
    width_m: f64,
    area_m2: f64,
    observed_m3s: f64,
}

fn replay(event: &str, parent_m3s: f64, branches: [Section; 2]) {
    let observed_total: f64 = branches.iter().map(|branch| branch.observed_m3s).sum();
    println!("{event}: parent={parent_m3s:.2} m³/s, branch sum={observed_total:.2} m³/s");
    let widths = branches.each_ref().map(|branch| branch.width_m);
    let conveyances = branches
        .each_ref()
        .map(|branch| relative_conveyance(branch.area_m2, branch.width_m));
    for (method, weights) in [("width", widths), ("conveyance", conveyances)] {
        let predicted = weights.map(|weight| parent_m3s * weight / weights.iter().sum::<f64>());
        let observed = branches.each_ref().map(|branch| branch.observed_m3s);
        let predicted_main = 100.0 * predicted[0] / parent_m3s;
        let observed_main = 100.0 * observed[0] / observed_total;
        let mae = ((predicted[0] - observed[0]).abs() + (predicted[1] - observed[1]).abs()) / 2.0;
        println!(
            "  {method:>10}: Main={:.2}, Gadwall={:.2} m³/s; branch MAE={mae:.2} m³/s; Main share={predicted_main:.2}% vs {observed_main:.2}% measured (absolute error {:.2} pp)",
            predicted[0],
            predicted[1],
            (predicted_main - observed_main).abs(),
        );
    }
}

fn main() {
    // Interpolate the two D transects (10:22 and 12:03) to the midpoint of
    // the 11:20 Gadwall and 11:39 Main transects on June 18.
    let falling_inflow = 970.0 + (1126.0 - 970.0) * 67.5 / 101.0;
    replay(
        "2014-06-18 falling tide",
        falling_inflow,
        [
            Section {
                width_m: 369.0,
                area_m2: 734.0,
                observed_m3s: 277.0,
            },
            Section {
                width_m: 431.0,
                area_m2: 1468.0,
                observed_m3s: 779.0,
            },
        ],
    );
    // June 19 has one upstream transect, 40 and 85 minutes before the
    // Main and Gadwall measurements respectively.
    replay(
        "2014-06-19 rising tide",
        972.0,
        [
            Section {
                width_m: 375.0,
                area_m2: 747.0,
                observed_m3s: 251.0,
            },
            Section {
                width_m: 429.0,
                area_m2: 1488.0,
                observed_m3s: 698.0,
            },
        ],
    );
}
