use super::*;
use std::collections::{BTreeSet, VecDeque};

fn stand(cells: &BTreeSet<Cell>, [x,y,z]: Cell) -> bool {
    (y == 0 || cells.contains(&[x,y-1,z])) && !cells.contains(&[x,y,z]) && !cells.contains(&[x,y+1,z])
}

#[test]
fn upper_floor_and_roof_have_bidirectional_routes() {
    for width in 4..=7 { for depth in 6..=8 { for right in [false,true] {
        let cells: BTreeSet<_> = plan(width,depth,right,1).unwrap().into_iter().collect();
        let mirror = |[x,y,z]: Cell| [if right {width-1-x} else {x},y,z];
        let start=mirror([-1,1,1]);
        let mut reached=BTreeSet::from([start]); let mut todo=VecDeque::from([start]);
        while let Some([x,y,z])=todo.pop_front() {
            for [dx,dz] in [[1,0],[-1,0],[0,1],[0,-1]] { for dy in -1..=1 {
                let next=[x+dx,y+dy,z+dz];
                if (-5..=width+4).contains(&next[0]) && (-2..=depth+1).contains(&next[2])
                    && (0..=7).contains(&next[1]) && stand(&cells,next)
                    && (dy<=0 || !cells.contains(&[x,y+2,z]))
                    && (dy>=0 || !cells.contains(&[next[0],next[1]+2,next[2]]))
                    && reached.insert(next) {todo.push_back(next);}
            }}
        }
        assert!(reached.contains(&mirror([1,4,4])),"upper {width} {depth} {right}");
        assert!(reached.contains(&mirror([1,7,1])),"roof {width} {depth} {right}");
        assert!(!cells.contains(&mirror([0,4,4])));
        assert!(!cells.contains(&mirror([0,5,4])));
    }}}
}

#[test]
fn construction_has_support_in_each_layer() {
    for right in [false,true] {
        let mut pending: BTreeSet<_> = plan(5,6,right,1).unwrap().into_iter().collect();
        let mut built=BTreeSet::new();
        for layer in 0..=6 {
            loop {
                let ready:Vec<_>=pending.iter().copied().filter(|[x,y,z]| *y==layer && (*y==0 ||
                    [[*x-1,*y,*z],[*x+1,*y,*z],[*x,*y-1,*z],[*x,*y,*z-1],[*x,*y,*z+1]].iter().any(|p|built.contains(p)))).collect();
                if ready.is_empty() {break;}
                for cell in ready {pending.remove(&cell);built.insert(cell);}
            }
            assert!(!pending.iter().any(|p|p[1]==layer),"unsupported layer {layer}");
        }
        assert!(pending.is_empty());
    }
}

#[test]
fn mirrored_plans_preserve_materials_and_determinism() {
    let left=plan(5,6,false,1).unwrap(); let right=plan(5,6,true,1).unwrap();
    assert_eq!(left,plan(5,6,false,1).unwrap());
    assert_eq!(left.len(),right.len());
    let reflected:BTreeSet<_>=left.iter().map(|[x,y,z]|[4-x,*y,*z]).collect();
    assert_eq!(reflected,right.into_iter().collect());
    assert!(left.len()+1+32+24<256);
}

#[test]
fn shallow_upper_landings_are_rejected() {
    for depth in 0..6 {assert!(plan(5,depth,false,1).is_err());}
    assert!(plan(8,6,false,1).is_err());
}

#[test]
fn seeded_windows_vary_without_changing_paid_count() {
    let a=plan(5,6,false,0).unwrap(); let b=plan(5,6,false,1).unwrap();
    assert_ne!(a,b); assert_eq!(a.len(),b.len());
    assert_eq!(a,plan(5,6,false,0).unwrap());
}
