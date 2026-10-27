//! Focused geometry, pose and warm-path allocation checks.
use super::{Model, wand::{self, WandView}, wand_pose};
use glam::{Vec2, Vec3};
use std::{alloc::{GlobalAlloc, Layout, System}, cell::Cell};

thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}
struct Counted;
#[global_allocator]
static ALLOCATOR: Counted = Counted;
unsafe impl GlobalAlloc for Counted {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        TRACK.with(|enabled| if enabled.get() { ALLOCATIONS.with(|n| n.set(n.get()+1)); });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) { unsafe { System.dealloc(ptr,layout) } }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        TRACK.with(|enabled| if enabled.get() { ALLOCATIONS.with(|n| n.set(n.get()+1)); });
        unsafe { System.realloc(ptr,layout,size) }
    }
}

fn view() -> WandView {
    let mut motion=crate::player::viewmodel::State::default();
    motion.tier=1;
    motion.equip=1.0;
    WandView { eye:Vec3::ZERO,forward:Vec3::NEG_Z,right:Vec3::X,up:Vec3::Y,
        time:2.0,cast:0.0,spell:[0.3,0.7,0.6],boost:0.0,
        motion }
}

#[test]
fn viewmodel_poses_preserve_finite_attached_geometry() {
    let mut m=Model::new(1.0);
    let mut v=view();
    let base=crate::wand_style::WandStyle::default();
    for pose in 0..7 {
        v.motion.aim=if pose==1 {1.0}else{0.0};
        v.cast=match pose {2=>0.82,3=>0.60,_=>0.0};
        v.motion.inspect=if pose==4 {1.0}else{0.0};
        v.motion.interact=if pose==5 {1.0}else{0.0};
        v.motion.equip=if pose==6 {0.15}else{1.0};
        v.motion.sway.value=Vec2::new(0.035,-0.035);
        let rig=wand_pose::pose(&v);
        assert!(rig.grip.is_finite() && rig.rotation.is_finite());
        assert!(wand_pose::elbow(&v).is_finite());
        for shaft in 0..6 {
            m.vertices.clear();
            wand::draw(&mut m,crate::wand_style::WandStyle {shaft,head:shaft,..base},&v);
            assert!(!m.vertices.is_empty());
            assert_eq!(m.vertices.len()%3,0);
            for vertex in &m.vertices {
                let p=Vec3::from_array(vertex.pos);
                let n=Vec3::from_array(vertex.normal);
                assert!(p.is_finite() && n.is_finite());
                assert!(p.length()<1.5, "pose {pose} outside attachment bounds");
                assert!((n.length()-1.0).abs()<0.001,"pose={pose} shaft={shaft} normal={n:?} surface={:?}",vertex.surface);
                assert!(vertex.color.iter().all(|c| c.is_finite() && *c>=0.0));
            }
        }
    }
}

#[test]
fn viewmodel_warm_draw_allocations_and_time() {
    let mut m=Model::new(1.0);
    let v=view();
    let style=crate::wand_style::WandStyle::default();
    wand::draw(&mut m,style,&v);
    let capacity=m.vertices.capacity();
    let vertices=m.vertices.len();
    ALLOCATIONS.with(|n| n.set(0));
    TRACK.with(|enabled| enabled.set(true));
    let start=std::time::Instant::now();
    for _ in 0..1000 {
        m.vertices.clear();
        wand::draw(&mut m,style,std::hint::black_box(&v));
        std::hint::black_box(&m.vertices);
    }
    let elapsed=start.elapsed();
    TRACK.with(|enabled| enabled.set(false));
    let allocations=ALLOCATIONS.with(Cell::get);
    assert_eq!(allocations,0,"warm viewmodel path allocated");
    assert_eq!(m.vertices.capacity(),capacity);
    eprintln!("[viewmodel-profile] iterations=1000 vertices={vertices} draw_us={:.3} warm_allocations={allocations}",elapsed.as_secs_f64()*1000.0);
}
