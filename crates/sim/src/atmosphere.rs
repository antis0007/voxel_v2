//! Isothermal gas quantities and conservative pressure-driven transfers.
//! Quantities are pressure-volume products in Pa m³ at one fixed temperature.

pub fn pressure(gas: [f64; 2], volume: f64) -> f64 {
    (gas[0] + gas[1]) / volume.max(0.001)
}

/// Transfer a mixture without changing its composition or creating gas.
pub fn transfer(source: &mut [f64; 2], target: &mut [f64; 2], amount: f64) {
    let total = source[0] + source[1];
    if !amount.is_finite() || amount <= 0.0 || total <= 0.0 { return; }
    let fraction = (amount / total).min(1.0);
    for i in 0..2 {
        let moved = source[i] * fraction;
        source[i] -= moved;
        target[i] += moved;
    }
}

/// Conductance is an explicit gameplay approximation, independent of rendering.
pub fn exchange(a: &mut [f64; 2], av: f64, b: &mut [f64; 2], bv: f64,
    area: f64, dt: f64) {
    if area <= 0.0 || dt <= 0.0 || !area.is_finite() || !dt.is_finite() { return; }
    let difference = pressure(*a, av) - pressure(*b, bv);
    // Never overshoot equilibrium, even with a large opening or small volume.
    let equalize = difference.abs() / (av.max(0.001).recip() + bv.max(0.001).recip());
    let amount = (difference.abs() * area * dt * 20.0).min(equalize);
    if difference > 0.0 { transfer(a, b, amount); }
    else { transfer(b, a, amount); }
}
