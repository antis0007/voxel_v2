const number = (label, min, max, fallback) => ({ label, type: "number", min, max, fallback });
const toggle = label => ({ label, type: "boolean" });

export const materialGroups = {
  Identity: {
    display_name: { label: "Display name", type: "string" },
    "tags.selectable": toggle("Show in Creative inventory"),
    "tags.contact_damage": number("Contact damage per second", 0, 10000, 0),
  },
  Appearance: {
    "render.color.base": { label: "Base color", type: "color" },
    "render.color.base_tint": number("Base brightness", 0, 4),
    "render.surface.roughness": number("Roughness", 0, 1),
    "render.surface.specular": number("Specular reflection", 0, 1),
    "render.surface.detail_strength": number("Surface detail", 0, 2),
    "render.surface.ao_strength": number("Ambient occlusion", 0, 2),
    "render.occludes_light": toggle("Blocks light"),
  },
  Physics: {
    "physics.density": number("Density (kg/m³)", 0, 40_000_000),
    "physics.hardness": number("Mining hardness", 0, 10000),
    "physics.friction": number("Friction", 0, 2),
    "physics.restitution": number("Bounce", 0, 1),
    "physics.fluid.viscosity": number("Viscosity", 0, 255),
    "physics.fluid.compressibility": number("Compressibility", 0, 2),
    "physics.fluid.pressure_response": number("Pressure response", 0, 2.55),
  },
  Simulation: {
    "sim.cohesion": number("Cohesion", 0, 1, 1),
    "sim.thermal.conductivity": number("Thermal conductivity (W/m·K)", 0, 655.35, 1),
    "sim.thermal.capacity": number("Specific heat (J/kg·K)", 0, 65535, 800),
    "sim.thermal.fusion": number("Latent heat of fusion (J/kg)", 0, 1e9, 334000),
    "sim.electric_conductivity": number("Electrical conductivity (S/m)", 0, 1e9, 0),
  },
};
export const materialProperties = Object.assign({}, ...Object.values(materialGroups));
export const materialPath = "assets/materials.ron";
export const blockRegistryPath = "crates/content/src/lib.rs";
