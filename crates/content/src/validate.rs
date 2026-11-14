//! Schema and cross-reference checks for `assets/materials.ron`.
//!
//! Parse errors already carry a position from ron. These checks run on the
//! parsed rows and cite the line of the offending field, as
//! `assets/materials.ron:LINE: Block: message`, at startup and in tests.
use super::{BlockId, MaterialDef, MatterState};

const FILE: &str = "assets/materials.ron";
/// Smallest threshold that still packs to a non-zero tenth of a kelvin (0 means "none").
const KELVIN_MIN: f32 = 0.05;
const KELVIN_MAX: f32 = 6553.5;

/// 1-based line of `field:` inside the row whose `id:` is `block`; the row's
/// own line when the field is not written out.
fn line_of(text: &str, block: BlockId, field: &str) -> usize {
    let row_start = format!("id: {block:?},");
    let Some(start) = text.lines().position(|l| l.trim() == row_start) else { return 1 };
    let key = format!("{field}:");
    for (offset, line) in text.lines().enumerate().skip(start + 1) {
        let line = line.trim();
        if line.starts_with("id: ") {
            break;
        }
        if line.starts_with(&key) {
            return offset + 1;
        }
    }
    start + 1
}

/// Every problem in the table, empty when it is valid.
pub(crate) fn validate(rows: &[MaterialDef], text: &str) -> Vec<String> {
    let mut errors = Vec::new();
    for def in rows {
        let mut fail = |field: &str, message: String| {
            errors.push(format!("{FILE}:{}: {:?}: {message}", line_of(text, def.id, field), def.id));
        };
        let (p, s, t) = (&def.physics, &def.sim, &def.sim.thermal);
        if (def.id as usize) >= crate::SIM_DIRECT {
            // No sim row of its own: the cellular sim runs it as `sim_as`.
            match def.sim_as {
                Some(proxy) if (proxy as usize) >= crate::SIM_DIRECT => {
                    fail("sim_as", format!("sim_as {proxy:?} must be a block below id {}", crate::SIM_DIRECT));
                }
                None if !s.is_generic() || p.matter_state != MatterState::Solid => {
                    fail("sim", "a non-solid or simulated block past the one-byte sim table needs sim_as".into());
                }
                _ => {}
            }
        }
        for (field, value) in [
            ("density", p.density),
            ("pressure_response", p.fluid.pressure_response),
            ("viscosity", p.fluid.viscosity),
            ("cohesion", s.cohesion),
            ("conductivity", t.conductivity),
            ("capacity", t.capacity),
            ("fusion", t.fusion),
            ("fuel", t.fuel),
            ("electric_conductivity", s.electric_conductivity),
            ("contact_damage", def.tags.contact_damage),
        ] {
            if !value.is_finite() || value < 0.0 {
                fail(field, format!("{field} must be a finite number >= 0, got {value}"));
            }
        }
        if p.density > 40_000_000.0 {
            fail("density", format!("density {} kg/m3 does not fit 32 bits in 0.01 steps", p.density));
        }
        if s.cohesion > 1.0 {
            fail("cohesion", format!("cohesion {} must be in 0..1", s.cohesion));
        }
        if t.conductivity > 655.35 {
            fail("conductivity", format!("conductivity {} W/(m K) does not fit 0.01 steps in 16 bits", t.conductivity));
        }
        if t.capacity > 65535.0 {
            fail("capacity", format!("capacity {} J/(kg K) does not fit 16 bits", t.capacity));
        }
        if t.fuel > 65535.0 {
            fail("fuel", format!("fuel {} does not fit 16 bits", t.fuel));
        }
        if t.fuel > 0.0 && t.fuel < 0.5 {
            fail("fuel", format!("fuel {} rounds to 0, which means \"does not burn\"", t.fuel));
        }
        // A full cell's heat capacity is 64 levels of this, and must fit the u32 the kernel works in.
        if crate::sim_traits::thermal_mass(def) > u32::MAX / 64 {
            fail("capacity", format!("density x capacity gives a thermal mass of {} per level; 64 levels must fit 32 bits", crate::sim_traits::thermal_mass(def)));
        }
        if p.fluid.pressure_response > 2.55 {
            fail("pressure_response", format!("pressure_response {} does not fit 0.01 steps in 8 bits", p.fluid.pressure_response));
        }
        // flow = 255 / viscosity, so above 255 a liquid would never spread.
        if p.matter_state == MatterState::Liquid && !(p.fluid.viscosity > 0.0 && p.fluid.viscosity <= 255.0) {
            fail("viscosity", "a liquid needs viscosity in (0, 255]".to_string());
        }
        // (threshold field, its value, the product it needs)
        let pairs = [
            ("melt", t.melt, s.melts_into, "melts_into"),
            ("boil", t.boil, s.boils_into, "boils_into"),
            ("cool", t.cool, s.cools_into, "cools_into"),
        ];
        for (field, kelvin, into, into_field) in pairs {
            if let Some(k) = kelvin
                && !(KELVIN_MIN..=KELVIN_MAX).contains(&k)
            {
                fail(field, format!("{field} {k} K must be in [{KELVIN_MIN}, {KELVIN_MAX}]"));
            }
            match (kelvin.is_some(), into.is_some()) {
                (true, false) => fail(field, format!("{field} is set but {into_field} is not")),
                (false, true) => fail(into_field, format!("{into_field} is set but {field} is not")),
                _ => {}
            }
        }
        if let (Some(melt), Some(boil)) = (t.melt, t.boil)
            && melt >= boil
        {
            fail("melt", format!("melt {melt} K must be below boil {boil} K"));
        }
        match (t.fuel > 0.0, t.ignite.is_some(), s.burns_into.is_some()) {
            (true, true, true) | (false, false, false) => {}
            (true, _, _) => fail("fuel", "a fuel needs both ignite and burns_into".to_string()),
            (false, _, _) => fail("ignite", "ignite and burns_into are only valid with fuel > 0".to_string()),
        }
        if let Some(k) = t.ignite
            && !(KELVIN_MIN..=KELVIN_MAX).contains(&k)
        {
            fail("ignite", format!("ignite {k} K must be in [{KELVIN_MIN}, {KELVIN_MAX}]"));
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MATERIALS_RON, MATERIAL_TABLE, SIM_TABLE};
    use engine_core::material::{MaterialId, MovementClass, SIM_MATERIAL_ROWS};

    /// The shipped file, with one block's field value replaced.
    fn broken(block: &str, field: &str, value: &str) -> (Vec<MaterialDef>, &'static str) {
        let mut lines: Vec<String> = MATERIALS_RON.replace('\r', "").lines().map(str::to_owned).collect();
        let start = lines.iter().position(|l| l.trim() == format!("id: {block},")).expect("block row");
        let at = (start..lines.len())
            .find(|&i| lines[i].trim_start().starts_with(&format!("{field}:")))
            .expect("field row");
        let indent = &lines[at][..lines[at].len() - lines[at].trim_start().len()];
        lines[at] = format!("{indent}{field}: {value},");
        // Row names are `&'static str`, so the edited text must outlive the run.
        let text: &'static str = Box::leak(lines.join("\n").into_boxed_str());
        let rows = ron::from_str(text).expect("edited file still parses");
        (rows, text)
    }

    fn line_number(text: &str, block: &str, field: &str) -> usize {
        let start = text.lines().position(|l| l.trim() == format!("id: {block},")).expect("row");
        start + 1 + text.lines().skip(start + 1).position(|l| l.trim_start().starts_with(&format!("{field}:"))).expect("field") + 1
    }

    #[test]
    fn the_shipped_table_is_valid() {
        let errors = validate(&MATERIAL_TABLE, MATERIALS_RON);
        assert!(errors.is_empty(), "{}", errors.join("\n"));
    }

    #[test]
    fn errors_cite_the_line_of_the_field() {
        let (rows, text) = broken("Water", "viscosity", "0.0");
        let errors = validate(&rows, text);
        let line = line_number(text, "Water", "viscosity");
        assert_eq!(errors, [format!("assets/materials.ron:{line}: Water: a liquid needs viscosity in (0, 255]")]);
    }

    #[test]
    fn a_product_needs_its_threshold_and_back() {
        let (rows, text) = broken("Water", "boil", "None");
        let errors = validate(&rows, text);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("Water: boils_into is set but boil is not"), "{errors:?}");
        let (rows, text) = broken("Ice", "melts_into", "None");
        assert!(validate(&rows, text)[0].contains("Ice: melt is set but melts_into is not"));
    }

    #[test]
    fn melt_must_be_below_boil() {
        let (rows, text) = broken("Water", "melt", "Some(400.0)");
        let errors = validate(&rows, text);
        assert!(errors.iter().any(|e| e.contains("Water: melt 400 K must be below boil")), "{errors:?}");
    }

    #[test]
    fn fuel_needs_ignition_and_a_residue() {
        let (rows, text) = broken("Log", "ignite", "None");
        let errors = validate(&rows, text);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("Log: a fuel needs both ignite and burns_into"), "{errors:?}");
    }

    /// Values that parse and are in range but would pack to something that means
    /// the opposite: flow 0, "no threshold", "does not burn".
    #[test]
    fn values_that_would_pack_to_nonsense_are_rejected() {
        let cases = [
            ("Water", "viscosity", "300.0", "a liquid needs viscosity in (0, 255]"),
            ("Water", "boil", "Some(0.01)", "boil 0.01 K must be in [0.05, 6553.5]"),
            ("Log", "fuel", "0.2", "fuel 0.2 rounds to 0"),
            ("Log", "fuel", "70000.0", "fuel 70000 does not fit 16 bits"),
            ("Steam", "viscosity", "-1.0", "viscosity must be a finite number >= 0"),
        ];
        for (block, field, value, expect) in cases {
            let (rows, text) = broken(block, field, value);
            let errors = validate(&rows, text);
            let line = line_number(text, block, field);
            assert!(
                errors.iter().any(|e| e.contains(expect) && e.contains(&format!("materials.ron:{line}:"))),
                "{block}.{field} = {value}: want '{expect}' at line {line}, got {errors:?}"
            );
        }
    }

    #[test]
    fn electrical_decades_land_exactly_on_authored_powers_of_ten() {
        use crate::sim_traits::electric_decades as decades;
        assert_eq!([decades(0.0), decades(9.9e-7), decades(1e-6), decades(1e-3), decades(1e6), decades(5.96e7), decades(1e8)], [0, 0, 1, 4, 13, 14, 15]);
    }

    #[test]
    fn the_table_has_256_rows_and_unused_ones_are_inert() {
        assert_eq!(SIM_TABLE.len(), SIM_MATERIAL_ROWS);
        for row in &SIM_TABLE[BlockId::COUNT.min(crate::SIM_DIRECT)..] {
            assert_eq!(*row, engine_core::material::SimMaterial::INERT);
        }
    }

    #[test]
    fn movement_comes_from_state_and_cohesion() {
        use BlockId as B;
        let class = |b: B| b.sim().class;
        assert_eq!(class(B::Air), MovementClass::Void);
        assert_eq!(class(B::Stone), MovementClass::Solid);
        for loose in [B::Sand, B::Gravel, B::Ash] {
            assert_eq!(class(loose), MovementClass::Powder, "{loose:?}");
        }
        // Powders in the table that must stay put: terrain does not collapse.
        for cohesive in [B::Dirt, B::Clay, B::Mud] {
            assert_eq!(class(cohesive), MovementClass::Solid, "{cohesive:?}");
        }
        assert_eq!(class(B::Water), MovementClass::Liquid);
        assert_eq!(class(B::Lava), MovementClass::Liquid);
        assert_eq!(class(B::Steam), MovementClass::Gas);
        assert_eq!(class(B::Fire), MovementClass::Plasma);
    }

    #[test]
    fn packed_values_match_the_authored_units() {
        use BlockId as B;
        assert_eq!(B::Water.sim().flow, 255, "water flows freely");
        assert_eq!(B::Water.sim().density, 100_000, "1000 kg/m3 in 0.01 steps");
        assert!(B::Smoke.sim().density < B::Air.sim().density, "smoke rises through air");
        assert!(B::Steam.sim().density < B::Smoke.sim().density);
        assert_eq!(B::Lava.sim().flow, 21, "lava is 12x as viscous: 255 / 12");
        // 373.15 K is 3731.5 tenths, exactly half a step; round() goes away from zero.
        assert_eq!(B::Water.sim().boil, 3732);
        assert_eq!(B::Ice.sim().melt, 2732, "273.15 K is half a step too");
        assert_eq!(B::Water.sim().boils_into, MaterialId::from(B::Steam));
        assert_eq!(B::Log.sim().burns_into, MaterialId::from(B::Ash));
        assert_eq!(B::Stone.sim().melts_into, MaterialId::NONE);
        assert_eq!(B::CopperBlock.sim().electric, 14, "copper is 14 decades above 1e-6 S/m");
        assert_eq!(B::Stone.sim().electric, 0);
    }
}
