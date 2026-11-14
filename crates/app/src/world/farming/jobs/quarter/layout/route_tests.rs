//! Exact routing regression checks against the pre-cache implementation.
use super::*;
use std::cell::{Cell, RefCell};

mod reference {
//! Direction-state A* with deterministic tie breaks and a hard expansion cap.
use super::{Point, Profile, X, Z, inside};
use std::{cmp::Reverse, collections::BinaryHeap};
const DIR: [Point;8]=[[0,-1],[1,0],[0,1],[-1,0],[1,-1],[1,1],[-1,1],[-1,-1]];
const W:usize=(X*2+1) as usize;
const N:usize=W*(Z*2+1) as usize*9;

fn index([x,z]:Point,d:usize)->usize { (((z+Z) as usize*W)+(x+X) as usize)*9+d }
fn decode(i:usize)->(Point,usize) { let cell=i/9; ([(cell%W) as i32-X,(cell/W) as i32-Z],i%9) }
fn estimate(a:Point,b:Point)->u32 { let x=(a[0]-b[0]).unsigned_abs(); let z=(a[1]-b[1]).unsigned_abs(); 10*x.max(z)+4*x.min(z) }

pub(super) fn find(start:Point,end:Point,p:&Profile,height:&impl Fn(Point)->Option<i32>,
    clear:&impl Fn(Point)->bool)->Result<(Vec<Point>,usize),String> {
    if !clear(start)||!clear(end) {return Err(format!("Street endpoint blocked: {start:?} -> {end:?}"));}
    let first=index(start,8);
    let mut cost=vec![u32::MAX;N]; let mut previous=vec![usize::MAX;N];
    let mut open=BinaryHeap::from([Reverse((estimate(start,end),0,first))]); cost[first]=0;
    let mut expanded=0;
    while let Some(Reverse((_,g,i)))=open.pop() {
        if cost[i]!=g {continue;}
        let (at,d)=decode(i);
        if at==end {
            let mut path=vec![at]; let mut cursor=i;
            while cursor!=first {cursor=previous[cursor]; path.push(decode(cursor).0);}
            path.reverse(); return Ok((path,expanded));
        }
        expanded+=1;
        if expanded>20_000 {return Err("Street survey expansion limit".into());}
        for (nd,[dx,dz]) in DIR.into_iter().enumerate() {
            let next=[at[0]+dx,at[1]+dz];
            if !inside(next)||!clear(next) {continue;}
            if nd>=4 && (!clear([at[0]+dx,at[1]]) || !clear([at[0],at[1]+dz])) {continue;}
            let rise=(height(next).unwrap_or(100)-height(at).unwrap_or(-100)).unsigned_abs();
            if rise>1 {continue;}
            let step=if nd<4 {10} else {14+p.diagonal_cost};
            let candidate=g+step+rise*p.terrain_cost+if d<8&&d!=nd {p.bend_cost} else {0};
            let ni=index(next,nd);
            if candidate<cost[ni] {cost[ni]=candidate;previous[ni]=i;open.push(Reverse((candidate+estimate(next,end),candidate,ni)));}
        }
    }
    Err(format!("No connected street: {start:?} -> {end:?}"))
}

}

fn profile(index: usize) -> Profile {
    let (diagonal_cost, bend_cost, terrain_cost) = [(22, 24, 30), (5, 8, 40), (1, 4, 55)][index];
    Profile { name: String::new(), diagonal_cost, bend_cost, terrain_cost,
        frontage_jitter: 0, setback_jitter: 0 }
}

fn terrain([x, z]: Point, seed: u32, kind: usize) -> Option<i32> {
    let bridge = (seed % 29) as i32 - 14;
    match kind {
        1 if x.abs() <= 2 && (z - bridge).abs() > 5 => None,
        2 => Some((x + X) / 12 + i32::from(x.abs() < 6 && z.abs() < 7) * 4),
        3 if x == 0 => None,
        _ => Some(0),
    }
}

fn width_clear(at: Point, radius: i32, height: &impl Fn(Point) -> Option<i32>) -> bool {
    height(at).is_some_and(|h| (-radius..=radius).all(|dx| (-radius..=radius).all(|dz| {
        let q = [at[0] + dx, at[1] + dz];
        inside(q) && height(q).is_some_and(|n| (n-h).abs() <= 1)
    })))
}

#[test]
fn route_survey_preserves_paths_errors_and_expansion_counts() {
    let mut passed = 0;
    let mut rejected = 0;
    for style in 0..3 { for seed in [0, 1, 7, 71, 424242, 8675309] {
        for radius in [0, 1, 2, 4] { for kind in 0..4 {
            let height = |at| terrain(at, seed, kind);
            let clear = |at| width_clear(at, radius, &height);
            let start = [-39, -23];
            let end = [39, 23];
            let expected = reference::find(start, end, &profile(style), &height, &clear);
            let actual = find(start, end, &profile(style), &height, &clear);
            assert_eq!(actual, expected, "style={style} seed={seed} radius={radius} kind={kind}");
            if actual.is_ok() { passed += 1; } else { rejected += 1; }
        }}
    }}
    assert!(passed > 0 && rejected > 0);
    println!("route-survey differential: {passed} successful routes, {rejected} identical rejections");
}

#[test]
fn route_survey_caches_missing_heights_and_blocked_cells() {
    let height_calls = Cell::new(0);
    let clear_calls = Cell::new(0);
    let height = |_| { height_calls.set(height_calls.get() + 1); None };
    let clear = |_| { clear_calls.set(clear_calls.get() + 1); false };
    let mut survey = Survey::new(&height, &clear);
    for at in [[-X+1, -Z+1], [X-1, Z-1], [0, 0]] {
        for _ in 0..12 {
            assert_eq!(survey.height(at), None);
            assert!(!survey.clear(at));
        }
    }
    assert_eq!(height_calls.get(), 3);
    assert_eq!(clear_calls.get(), 3);
}

#[test]
fn route_survey_is_fresh_for_changed_exclusions_heights_and_widths() {
    let blocked = Cell::new(false);
    let raised = Cell::new(false);
    let height = |[x, _]: Point| Some(if raised.get() && x >= 0 { 4 } else { 0 });
    let clear = |[x, z]: Point| !blocked.get() || x != 0 || z == 0;
    let p = profile(1);
    assert!(find([-8, 0], [8, 0], &p, &height, &clear).is_ok());
    blocked.set(true);
    assert!(find([-8, 0], [8, 0], &p, &height, &|q| width_clear(q, 0, &|a|
        if clear(a) {height(a)} else {None})).is_ok());
    assert!(find([-8, 0], [8, 0], &p, &height, &|q| width_clear(q, 1, &|a|
        if clear(a) {height(a)} else {None})).is_err());
    blocked.set(false);
    raised.set(true);
    assert!(find([-8, 0], [8, 0], &p, &height, &clear).is_err());
    raised.set(false);
    assert!(find([-8, 0], [8, 0], &p, &height, &clear).is_ok());
}

#[test]
fn route_survey_rejects_out_of_bounds_endpoints_before_callbacks() {
    let height = |_| panic!("Invalid endpoint reached height callback");
    let clear = |_| panic!("Invalid endpoint reached clearance callback");
    for bad in [[i32::MIN, 0], [i32::MAX, 0], [0, i32::MIN], [0, i32::MAX],
        [-X, 0], [X, 0], [0, -Z], [0, Z]] {
        for (a, b) in [(bad, [0, 0]), ([0, 0], bad)] {
            assert!(find(a, b, &profile(0), &height, &clear).is_err());
        }
    }
}

#[test]
fn route_survey_retains_corner_and_expansion_guards() {
    let height = |_| Some(0);
    let clear = |at| at != [1, 0] && at != [0, 1];
    let p = profile(2);
    let result = find([0, 0], [1, 1], &p, &height, &clear);
    assert_eq!(result, reference::find([0, 0], [1, 1], &p, &height, &clear));
    let path = result.unwrap().0;
    assert!(path.len() > 2);
    for pair in path.windows(2) {
        if pair[0][0] != pair[1][0] && pair[0][1] != pair[1][1] {
            assert!(clear([pair[0][0], pair[1][1]]) && clear([pair[1][0], pair[0][1]]));
        }
    }
    let clear = |[x, _]: Point| x != 0;
    let result = find([-39, -23], [39, 23], &p, &height, &clear);
    assert_eq!(result, reference::find([-39, -23], [39, 23], &p, &height, &clear));
    assert_eq!(result.unwrap_err(), "Street survey expansion limit");
    assert_eq!(find([0, 0], [0, 0], &p, &height, &|_|true).unwrap(), (vec![[0, 0]], 0));
    assert_eq!(find([0, 0], [0, 0], &p, &height, &|_|false),
        reference::find([0, 0], [0, 0], &p, &height, &|_|false));
}

#[test]
fn route_survey_bounds_work_per_cell_and_reduces_wide_street_sampling() {
    let p = profile(0);
    let run = |cached: bool| {
        let clearance_calls = Cell::new(0usize);
        let height_calls = Cell::new(0usize);
        let terrain_calls = Cell::new(0usize);
        let seen = RefCell::new(std::collections::BTreeSet::new());
        let ground = |at| { terrain_calls.set(terrain_calls.get()+1); terrain(at, 7, 1) };
        let height = |at| { height_calls.set(height_calls.get()+1); ground(at) };
        let clear = |at| {
            clearance_calls.set(clearance_calls.get()+1);
            if cached { assert!(seen.borrow_mut().insert(at), "Repeated clearance survey at {at:?}"); }
            width_clear(at, 4, &ground)
        };
        let result = if cached {find([-39,-23],[39,23],&p,&height,&clear)}
            else {reference::find([-39,-23],[39,23],&p,&height,&clear)};
        (result, clearance_calls.get(), height_calls.get(), terrain_calls.get())
    };
    let before = run(false);
    let after = run(true);
    assert!(after.0.is_ok());
    assert_eq!(after.0, before.0);
    assert!(after.1 <= N/9 && after.2 <= N/9);
    assert!(after.3 * 2 < before.3, "Wide-road source sampling did not halve");
    println!("route-survey 9m road: clearance {} -> {}; route heights {} -> {}; source heights {} -> {}; expansions {}",
        before.1,after.1,before.2,after.2,before.3,after.3,after.0.as_ref().unwrap().1);
}
#[test]
fn route_survey_short_circuits_immediate_outcomes() {
    let calls = Cell::new(0);
    let height = |_| panic!("Immediate route sampled terrain");
    let open = |_| { calls.set(calls.get()+1); true };
    assert_eq!(find([0,0],[0,0],&profile(0),&height,&open).unwrap(),(vec![[0,0]],0));
    assert_eq!(calls.get(),1);
    calls.set(0);
    let blocked = |_| { calls.set(calls.get()+1); false };
    assert!(find([0,0],[5,0],&profile(0),&height,&blocked).is_err());
    assert_eq!(calls.get(),1);
}