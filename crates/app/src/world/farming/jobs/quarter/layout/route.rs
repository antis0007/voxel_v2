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
    if !inside(start)||!inside(end) {return Err(format!("Street endpoint outside survey: {start:?} -> {end:?}"));}
    if !clear(start)||(start!=end&&!clear(end)) {return Err(format!("Street endpoint blocked: {start:?} -> {end:?}"));}
    if start==end {return Ok((vec![start],0));}
    let mut survey=Survey::new(height,clear);
    survey.clearance[index(start,0)/9]=Some(true);
    survey.clearance[index(end,0)/9]=Some(true);
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
            if !inside(next)||!survey.clear(next) {continue;}
            if nd>=4 && (!survey.clear([at[0]+dx,at[1]]) || !survey.clear([at[0],at[1]+dz])) {continue;}
            let rise=(survey.height(next).unwrap_or(100)-survey.height(at).unwrap_or(-100)).unsigned_abs();
            if rise>1 {continue;}
            let step=if nd<4 {10} else {14+p.diagonal_cost};
            let candidate=g+step+rise*p.terrain_cost+if d<8&&d!=nd {p.bend_cost} else {0};
            let ni=index(next,nd);
            if candidate<cost[ni] {cost[ni]=candidate;previous[ni]=i;open.push(Reverse((candidate+estimate(next,end),candidate,ni)));}
        }
    }
    Err(format!("No connected street: {start:?} -> {end:?}"))
}

// Arrival directions share terrain; a later route gets a fresh width/exclusion survey.
struct Survey<'a, H, C> {
    height: &'a H,
    clear: &'a C,
    heights: Vec<Option<Option<i32>>>,
    clearance: Vec<Option<bool>>,
}

impl<'a, H: Fn(Point) -> Option<i32>, C: Fn(Point) -> bool> Survey<'a, H, C> {
    fn new(height: &'a H, clear: &'a C) -> Self {
        Self { height, clear, heights: vec![None; N / 9], clearance: vec![None; N / 9] }
    }

    fn height(&mut self, at: Point) -> Option<i32> {
        let i = index(at, 0) / 9;
        *self.heights[i].get_or_insert_with(|| (self.height)(at))
    }

    fn clear(&mut self, at: Point) -> bool {
        let i = index(at, 0) / 9;
        *self.clearance[i].get_or_insert_with(|| (self.clear)(at))
    }
}

#[cfg(test)]
#[path = "route_tests.rs"]
mod tests;