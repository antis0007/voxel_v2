use super::*;
use crate::fluid::VoxelScalePreset;
use crate::world::WorldType;

fn fixture() -> (World, Enemy) {
    let world = World::new(WorldType::Superflat, VoxelScalePreset::OneMeter);
    let mut y = 80.0;
    while y > -40.0 && !factory::solid_point(&world, Vec3::new(4.5, y - 0.1, 4.5)) {
        y -= 0.1;
    }
    let mut e = Enemy::new(IVec3::ZERO, 5, KIND, Vec3::new(4.5, y, 4.5), 0);
    initialize(&mut e);
    (world, e)
}

fn step(e: &mut Enemy, world: &mut World, feet: Vec3, shots: &mut Vec<Projectile>) {
    let mut budget = 64usize;
    e.cooldown = (e.cooldown - 0.01).max(0.0);
    tick(e, world, feet, feet + Vec3::Y * 1.6, 0.01, shots, &mut Vec::new(), &mut budget);
}

#[test]
fn the_lattice_is_bounded_and_mending_never_grows_it() {
    for site in 0..40 {
        let mut e = Enemy::new(IVec3::new(site * 7 - 90, 0, site * 13), 5, KIND, Vec3::ZERO, 0);
        initialize(&mut e);
        assert!((4..=MAX_NODES as u8).contains(&e.glass.count));
    }
    let (mut world, mut e) = fixture();
    let count = e.glass.count;
    let feet = e.pos + Vec3::new(9.0, 0.0, 0.0);
    e.visible = true;
    let mut shots = Vec::new();
    for _ in 0..3000 {
        for node in &mut e.glass.nodes[..2] {
            node.health = 0.0;
        }
        step(&mut e, &mut world, feet, &mut shots);
        shots.clear();
        assert_eq!(e.glass.count, count);
        assert!(e.glass.mends <= MAX_MENDS);
    }
}

#[test]
fn a_broken_node_reroutes_the_next_bolt() {
    let (_, e) = fixture();
    let s = &e.glass;
    let target = s.nodes[2].pos + Vec3::new(0.0, 0.0, 0.01);
    let entry = s.nodes[0].pos;
    let (path, len) = route(s, entry, target).expect("route");
    assert_eq!(path[len as usize - 1], 2);
    let mut cut = s.clone();
    cut.nodes[2].health = 0.0;
    let (path, len) = route(&cut, entry, target).expect("reroute");
    let exit = path[len as usize - 1] as usize;
    assert_ne!(exit, 2);
    for i in 1..len as usize {
        assert!(cut.strand(path[i - 1] as usize, path[i] as usize));
    }
    // Isolating the entry leaves it the only exit: the bolt fires from the hands' node.
    let mut alone = s.clone();
    for i in 1..alone.count as usize {
        alone.nodes[i].health = 0.0;
    }
    assert_eq!(route(&alone, entry, target), Some(([0; MAX_NODES], 1)));
}

#[test]
fn a_strand_cut_under_a_bolt_stops_it() {
    let (mut world, mut e) = fixture();
    let (path, len) = route(&e.glass, e.glass.nodes[0].pos, e.glass.nodes[2].pos).expect("route");
    assert!(len >= 2);
    e.glass.bolt = Some(Bolt { path, len, at: 0, t: 0.1, payload: Payload::Shard, aim: e.pos });
    e.glass.nodes[path[1] as usize].health = 0.0;
    let mut shots = Vec::new();
    carry(&mut e, 0.01, &mut shots, &mut Vec::new());
    assert!(e.glass.bolt.is_none() && shots.is_empty());
    let _ = &mut world;
}

#[test]
fn player_shots_break_nodes() {
    let (mut world, mut e) = fixture();
    let c = e.glass.nodes[1].pos;
    let mut shots = vec![Projectile {
        farm: None,
        pos: c,
        velocity: Vec3::X * 10.0,
        hostile: false,
        payload: None,
        payload_mods: Default::default(),
        spell: 0,
        life: 2.0,
        mods: wands::ShotMods { damage_mul: 4.0, ..Default::default() },
        bore_budget: 0,
        trail_timer: 0.0,
        splashed: false,
        follow: None,
    }];
    strike_nodes(&mut e, &mut world, &mut shots, 0.01, &mut Vec::new());
    assert!(!e.glass.intact(1) && shots[0].life <= 0.0 && e.glass.breaks == 1);
}

#[test]
fn a_committed_weave_keeps_its_aim() {
    let (mut world, mut e) = fixture();
    e.visible = true;
    e.cooldown = 0.0;
    let mut feet = e.pos + Vec3::new(0.0, 0.0, -8.0);
    let mut shots = Vec::new();
    step(&mut e, &mut world, feet, &mut shots);
    assert_eq!(e.glass.run.as_ref().map(|r| r.kind), Some(Action::Weave));
    let (aim, lunge, yaw) = (e.aim, e.lunge, e.yaw);
    feet += Vec3::new(6.0, 0.0, 0.0);
    for _ in 0..110 {
        step(&mut e, &mut world, feet, &mut shots);
    }
    assert_eq!((e.aim, e.lunge, e.yaw), (aim, lunge, yaw));
    match e.glass.bolt {
        Some(bolt) => assert_eq!(bolt.aim, aim),
        None => assert!(e.glass.fired > 0),
    }
}

#[test]
fn half_health_moves_one_anchor_once() {
    let (mut world, mut e) = fixture();
    let before: Vec<Vec3> = e.glass.nodes.iter().map(|n| n.pos).collect();
    e.health = e.max_health() * 0.4;
    escalate(&mut e, &mut world, &mut Vec::new());
    let moved = e.glass.nodes.iter().zip(&before).filter(|(n, b)| n.pos != **b).count();
    assert_eq!(moved, 1);
    escalate(&mut e, &mut world, &mut Vec::new());
    assert_eq!(e.glass.nodes.iter().zip(&before).filter(|(n, b)| n.pos != **b).count(), 1);
}
