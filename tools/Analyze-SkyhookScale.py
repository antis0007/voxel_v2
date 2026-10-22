"""Reproducible Earth-reference angular sizes and surface-view kinematics.

This is a research calculator, not a claim of a validated engineering design or
an in-game orbital simulation. No dependencies, downloads, or personal saves.
"""
import argparse
import csv
import json
import math
from pathlib import Path

R = 6_371_000.0
MU = 3.986004418e14
EARTH_RATE = 2 * math.pi / 86164.0905


def add(a, b):
    return tuple(x + y for x, y in zip(a, b))


def mul(a, s):
    return tuple(x * s for x in a)


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def norm(a):
    return math.sqrt(dot(a, a))


def observation(point, seconds, latitude):
    rotation = EARTH_RATE * seconds
    up = (math.cos(latitude) * math.sin(rotation),
          math.cos(latitude) * math.cos(rotation), math.sin(latitude))
    east = (math.cos(rotation), -math.sin(rotation), 0.0)
    north = (-math.sin(latitude) * math.sin(rotation),
             -math.sin(latitude) * math.cos(rotation), math.cos(latitude))
    relative = add(point, mul(up, -R))
    e, n, u = dot(relative, east), dot(relative, north), dot(relative, up)
    return dict(slant_m=norm(relative), altitude_m=norm(point) - R,
                azimuth_deg=math.degrees(math.atan2(e, n)) % 360,
                elevation_deg=math.degrees(math.atan2(u, math.hypot(e, n))))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    focal_pixels = 900 / (2 * math.tan(math.radians(70) / 2))
    examples = [
        ("Atmospheric deck (illustrative slant)", 80, 700),
        ("Earlier archive ring (overhead approximation)", 420, 18000),
        ("Current archive crown (overhead approximation)", 2400, 18000),
        ("Loom orbital foundry (overhead approximation)", 64000, 420000),
        ("Continuum cylinder length (overhead approximation)", 36000, 700000),
        ("100 m orbital station", 100, 400000),
        ("420 m orbital habitat", 420, 500000),
        ("1 km orbital habitat", 1000, 500000),
        ("10 km megahabitat", 10000, 500000),
        ("1.2 m tether width", 1.2, 150000),
    ]
    sizes = []
    for name, span, distance in examples:
        angle = 2 * math.atan(span / (2 * distance))
        sizes.append(dict(name=name, span_m=span, slant_m=distance,
                          angle_deg=math.degrees(angle),
                          centre_pixels=2 * focal_pixels * math.tan(angle / 2)))
    # Illustrative circular orbit + a rigid spinning line, not the full HASTOL
    # design. No tension, elasticity, taper, pickup impulse, drag or reboost model.
    radius = R + 700000
    lower_arm, counter_arm = 550000.0, 100000.0
    orbit_rate = math.sqrt(MU / radius**3)
    spin_rate = 3500 / lower_arm  # inertial, not relative to rotating local vertical
    latitude = 100000 / R        # observer 100 km north of the orbital plane
    samples = []
    for t in range(-180, 181, 5):
        centre = mul((math.sin(orbit_rate*t), math.cos(orbit_rate*t), 0), radius)
        axis = (math.sin(spin_rate*t), math.cos(spin_rate*t), 0)
        for name, point in [("hub", centre), ("catcher", add(centre, mul(axis, -lower_arm))),
                            ("counterweight", add(centre, mul(axis, counter_arm)))]:
            samples.append(dict(seconds=t, component=name, **observation(point, t, latitude)))
    summary = dict(
        assumptions="Earth-reference, 70 degree vertical FOV, 1280x900, centre-screen transverse spans",
        earth_radius_m=R, mu_m3_s2=MU, focal_pixels=focal_pixels,
        model="Illustrative circular centre orbit plus rigid inertially spinning tether; not engineering validation",
        hub_altitude_m=700000, lower_arm_m=lower_arm, counter_arm_m=counter_arm,
        lowest_catcher_altitude_m=150000,
        centre_orbit_period_s=2*math.pi/orbit_rate,
        inertial_spin_period_s=2*math.pi/spin_rate,
        relative_to_local_vertical_spin_period_s=2*math.pi/(spin_rate-orbit_rate),
        centre_speed_m_s=radius*orbit_rate,
        approximate_equatorial_catcher_ground_speed_m_s=radius*orbit_rate-3500-R*EARTH_RATE,
        rotational_tip_acceleration_m_s2=3500**2/lower_arm,
        orbital_reference_g_at_700km=MU/radius**2,
        apparent_sizes=sizes,
    )
    (args.output / "reference.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    for filename, rows in [("angular-sizes.csv", sizes), ("surface-pass.csv", samples)]:
        with (args.output / filename).open("w", newline="", encoding="utf-8") as file:
            writer = csv.DictWriter(file, fieldnames=list(rows[0]))
            writer.writeheader()
            writer.writerows(rows)
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
