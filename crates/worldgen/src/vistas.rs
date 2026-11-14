//! Composed geological destinations: a foreground frame, a crossing and a skyline.
//! Shapes are metre-space fields shared by columns and physical voxel sampling.
use crate::{BlockId as B, WorldSampleConfig, WorldType, hash2, smoothstep};
use glam::{IVec3, Vec3};
use std::{cell::RefCell, collections::HashMap};

mod form;

pub const VERSION:u32=95;
const CELL:f32=640.0;
#[derive(Clone,Copy)]
struct Site {x:f32,z:f32,base:f32,sin:f32,cos:f32,kind:u8,reworked:bool,flared:bool,natural:bool,formed:bool,eroded:bool,key:u32}
/// Arch legs flare into the ground and the opening closes onto it.
pub const FLARE_VERSION:u32=98;
/// Rarer sites, per-peak heights, flank gullies, uneven beds with recessed soft layers.
pub const FORM_VERSION:u32=134;
thread_local! {static SITES:RefCell<(Option<WorldSampleConfig>,HashMap<(i32,i32),Option<Site>>)>=RefCell::new((None,HashMap::new()));}
fn active(cfg:&WorldSampleConfig)->bool {cfg.generator_version>=VERSION&&matches!(cfg.world_type,WorldType::Hills|WorldType::Ridged)}
fn site(cfg:&WorldSampleConfig,ix:i32,iz:i32)->Option<Site> {
    if !active(cfg){return None;}
    SITES.with(|cache|{
        let mut c=cache.borrow_mut();
        if c.0!=Some(*cfg){*c=(Some(*cfg),HashMap::new());}
        if let Some(s)=c.1.get(&(ix,iz)){return *s;}
        let h=|salt|hash2(ix,iz,cfg.seed.wrapping_add(salt));
        let result=(||{
            // Near spawn nearly every suitable cell held a landmark; keep about half as many.
            let skip=if cfg.generator_version>=FORM_VERSION {0.62}else{0.32};
            if (ix!=0||iz!=0)&&h(95101)<skip{return None;}
            let x=(ix as f32+0.5)*CELL+(h(95103)-0.5)*56.0;
            let z=(iz as f32+0.5)*CELL+(h(95107)-0.5)*56.0;
            let p=crate::geo::planet_for(cfg);
            let floor=|x:f32,z:f32|p.grid.bilinear(&p.height,x as f64,z as f64) as f32;
            let base=floor(x,z);
            if !(25.0..300.0).contains(&base){return None;}
            // Avoid lifting coastlines and replacing a steep existing mountain.
            for(dx,dz)in[(-115.0,0.0),(115.0,0.0),(0.0,-90.0),(0.0,90.0)]{
                let y=floor(x+dx,z+dz);if y<20.0||(y-base).abs()>38.0{return None;}
            }
            let(sin,cos)=(h(95111)*std::f32::consts::TAU).sin_cos();
            Some(Site{x,z,base:base.floor(),sin,cos,kind:(h(95113)*3.0)as u8%3,reworked:cfg.generator_version>=96,flared:cfg.generator_version>=FLARE_VERSION,natural:cfg.generator_version>=99,
                formed:cfg.generator_version>=FORM_VERSION,eroded:cfg.generator_version>=crate::geo::RAVINE_GENERATOR,
                key:(h(95119)*16_777_216.0)as u32^cfg.seed})
        })();
        if c.1.len()>=512 {c.1.clear();}
        c.1.insert((ix,iz),result);result
    })
}
impl Site {
    fn grain(self,a:f32,b:f32,scale:f32)->f32 {
        crate::value_noise2((self.x+a)*scale,(self.z+b)*scale,99013)*2.0-1.0
    }
    /// Bed-space height offset: beds dip gently across the site and wander with the grain.
    fn tilt(self,a:f32,b:f32)->f32{a*0.055+b*0.02+1.4*self.grain(a,b,0.02)}
    fn local(self,x:f32,z:f32)->(f32,f32){let(dx,dz)=(x-self.x,z-self.z);(dx*self.cos+dz*self.sin,-dx*self.sin+dz*self.cos)}
    fn point(self,a:f32,y:f32,b:f32)->Vec3{Vec3::new(self.x+a*self.cos-b*self.sin,self.base+y,self.z+a*self.sin+b*self.cos)}
    fn mask(self,a:f32,b:f32)->f32{(1.0-smoothstep(112.0,145.0,a.abs()))*(1.0-smoothstep(72.0,100.0,b.abs()))}
    // The span follows a crooked resistant bed, rather than an extruded ellipse.
    // Crown and soffit share the same field so there is no flat cut-off tunnel wall.
    fn arch(self,a:f32,b:f32)->(f32,f32,f32) {
        let centre=-29.0+0.18*b+1.8*(b*0.11).sin();
        let foot=self.foot(b);
        let erosion=if self.natural {self.grain(-29.0,b,0.065)}else{0.0};
        let width=(4.0+2.0*smoothstep(10.0,39.0,b.abs())+1.2*erosion)*(1.0+0.9*foot);
        let across=(a-centre).abs()/width;
        let crown=43.0-0.011*(b+5.0).powi(2)+0.07*b
            +0.8*(b*0.21).sin()+0.4*(b*0.49).sin()+1.7*erosion;
        let thickness=5.5+2.5*smoothstep(12.0,38.0,b.abs())+1.3*erosion;
        let soffit=crown-thickness+1.4*across.min(1.0).powi(2);
        // Intrados curves down onto the valley floor so the span springs from
        // the ground as haunches instead of ending in a cut face.
        let soffit=soffit-soffit.max(0.0)*foot.powf(1.5);
        (across,crown-1.2*across.min(1.0).powi(2),soffit)
    }
    /// Weight of the leg zone, zero for older generators.
    fn foot(self,b:f32)->f32{if self.flared{smoothstep(26.0,43.0,b.abs())}else{0.0}}
    fn hero_r(self,a:f32,b:f32)->f32{
        ((a-82.0)/1.25).hypot(b)+2.0*(b*0.15).sin()+1.2*(a*0.17).cos()
            +if self.natural {4.0*self.grain(a,b,0.045)}else{0.0}
    }
    /// Gully depth on a flank band; zero before `FORM_VERSION`.
    fn cut(self,along:f32,count:f32,band:f32,depth:f32)->f32{
        if self.formed {form::gully(self.key,along,count)*band*depth}else{0.0}
    }
    fn relief(self,a:f32,b:f32)->f32{
        let hero_r=self.hero_r(a,b);
        let cap=1.0-smoothstep(19.0,46.0,hero_r);
        let hero=42.0*cap+if self.natural {5.0*(1.0-cap)*(1.0-smoothstep(36.0,65.0,hero_r))}else{0.0};
        let banks=match self.kind {
            0 if !self.reworked=>{
                let flank=1.0-smoothstep(14.0,36.0,(b.abs()-49.0-3.0*(a*0.07).sin()).abs());
                let cliffs=flank*(1.0-smoothstep(70.0,120.0,a.abs()))*(48.0+7.0*(a*0.045).sin()+2.8*(a*0.17).sin());
                let bridge=(44.0+2.5*(b*0.18).sin()+1.2*(b*0.34).cos())*(1.0-smoothstep(3.5,8.5,(a+29.0).abs()))*(1.0-smoothstep(35.0,47.0,b.abs()));
                cliffs.max(bridge)
            }
            0=>{let side=if b<0.0 {-55.0}else{58.0};
                let flank=1.0-smoothstep(8.0,29.0,(b-side-3.0*(a*0.065).sin()).abs());
                let height=if b<0.0 {51.0}else{38.0};
                let cliffs=flank*(1.0-smoothstep(42.0,112.0,(a+18.0).abs()))
                    *(height+5.0*(a*0.06).sin()+2.0*(a*0.15).sin()
                        +if self.natural {7.0*self.grain(a,b,0.035)}else{0.0});
                let face=smoothstep(4.0,14.0,(b-side).abs())*(1.0-smoothstep(22.0,30.0,(b-side).abs()));
                let cliffs=cliffs-self.cut(a*0.05,6.0,face*flank,6.0);
                let (across,crown,_)=self.arch(a,b);
                // Legs spread into a sloped skirt where the span meets the ground.
                let foot=self.foot(b);
                let bridge=crown.max(0.0)*(1.0-smoothstep(0.70-0.35*foot,1.0+1.1*foot,across))
                    *(1.0-smoothstep(40.0,48.0,b.abs()));
                cliffs.max(bridge)}
            // The watershed owns natural relief. Preserve the lookout and
            // its graded approach, without adding three un-eroded towers.
            1 if self.eroded=>0.0,
            1=>{
                let mut peak=0.0_f32;
                for(i,(x,z,h))in[(-35.0,-40.0,52.0),(12.0,46.0,65.0),(-85.0,24.0,34.0)].into_iter().enumerate(){
                    let h=if self.formed {h*form::jitter(self.key,i as i32)}else{h};
                    let warp=if self.natural {self.grain(a,b,0.05)}else{0.0};
                    let (dx,dz)=((a-x+3.0*warp)/(26.0+if self.natural {x*0.035}else{0.0}),(b-z+2.0*warp)/20.0);
                    let angle=dz.atan2(dx);
                    let r=dx.hypot(dz)+0.10*(angle*3.0+1.2).sin()+0.07*(angle*5.0).cos()+0.025*(a*0.25+b*0.18).sin()+0.10*warp;
                    if self.formed {
                        // Beyond any bed's reach and the talus apron this peak adds nothing.
                        if r>form::OUTER_R {continue;}
                        // Joints facet the rim, gullies cut it back, and the wall steps down bed by bed.
                        let band=smoothstep(0.3,0.6,r)*(1.0-smoothstep(1.3,1.7,r));
                        let r=form::faceted(self.key,i as i32,dx,dz,r)
                            +form::gully(self.key,angle+i as f32*2.1,7.0+i as f32*2.0)*band*0.2;
                        let tilt=self.tilt(x,z);
                        let drop=form::stepped(self.key,i as i32,h+tilt,h+8.0,(r-form::CAP_R)*form::UNIT_M,angle);
                        let wall=h+tilt-drop-self.tilt(a,b);
                        let talus=12.0*(1.0-smoothstep(0.9,2.1,r));
                        peak=peak.max(wall.max(talus).max(0.0));
                        continue;
                    }
                    let cap=1.0-smoothstep(0.72,1.08,r);
                    let talus=if self.natural {12.0*(1.0-smoothstep(0.75,1.95,r))}else{8.0*(1.0-smoothstep(0.90,1.65,r))};
                    // Gullies notch the cap edge and fan into the talus below it.
                    let band=smoothstep(0.45,0.8,r)*(1.0-smoothstep(1.05,1.45,r));
                    let gully=self.cut(angle+i as f32*2.1,7.0+i as f32*2.0,band,0.22*h);
                    peak=peak.max((cap*(h+3.0*warp)+talus-gully).max(0.0));
                }
                peak}
            _=>{
                let erosion=if self.natural {self.grain(a,b,0.04)}else{0.0};
                let ridge=(1.0-smoothstep(12.0,42.0,(b-40.0-5.0*erosion).abs()))*(1.0-smoothstep(75.0,130.0,a.abs()));
                let face=smoothstep(8.0,20.0,(b-40.0).abs())*(1.0-smoothstep(32.0,42.0,(b-40.0).abs()));
                (ridge*(53.0+8.0*(a*0.035).cos()+7.0*erosion)-self.cut(a*0.06,5.0,face,7.0)).max(0.0)}
        };
        let mut rise=hero.max(banks);
        // A broad sloping approach reaches the destination without a climbing ability.
        if b.abs()<5.0&&(-76.0..88.0).contains(&a)&&!(self.kind==0&&(a+29.0).abs()<12.0){
            let road=((a+26.0)*0.40).clamp(0.0,42.0);
            let blend=1.0-smoothstep(2.0,5.0,b.abs());
            rise=rise+(road-rise)*blend;
        }
        rise
    }
}
fn at(cfg:&WorldSampleConfig,x:f32,z:f32)->Option<(Site,f32,f32)>{
    let s=site(cfg,(x/CELL).floor()as i32,(z/CELL).floor()as i32)?;
    let(a,b)=s.local(x,z);(s.mask(a,b)>0.0).then_some((s,a,b))
}
pub fn height(cfg:&WorldSampleConfig,x:f32,z:f32,raw:f64)->f64{
    if !active(cfg){return raw;}
    let Some((s,a,b))=at(cfg,x,z)else{return raw;};
    let relief=s.relief(a,b);
    let target=s.base+relief;
    if s.eroded&&s.kind==1 {
        // Blend only the remaining authored footing into its natural slope;
        // a zero-height former stack must not level the valley to site.base.
        return raw+(f64::from(target)-raw).max(0.0)
            *f64::from(s.mask(a,b)*smoothstep(0.0,5.0,relief));
    }
    let target=f64::from(target);
    raw+(target-raw)*f64::from(s.mask(a,b))
}
pub fn detail_weight(cfg:&WorldSampleConfig,x:f32,z:f32)->f64 {
    if !active(cfg){return 1.0;}
    at(cfg,x,z).map_or(1.0,|(s,a,b)|{
        if !s.formed {return f64::from(1.0-s.mask(a,b));}
        // Platform, span and road stay graded; flanks keep the eroded gullies.
        let platform=1.0-smoothstep(46.0,62.0,s.hero_r(a,b));
        let span=if s.kind==0&&b.abs()<48.0 {1.0-smoothstep(1.5,3.0,s.arch(a,b).0)}else{0.0};
        let road=if (-80.0..92.0).contains(&a) {1.0-smoothstep(5.0,9.0,b.abs())}else{0.0};
        let protected=platform.max(span).max(road);
        let protected=if s.eroded&&s.kind==1 {protected}else{protected.max(0.3)};
        f64::from(1.0-s.mask(a,b)*protected)
    })
}
/// Landmarks own their rock and openings before underground rooms are sampled.
/// Otherwise cavern lining can refill carved air with rectangular walls.
/// Metre height of the span's underside above column (x, z), for ambient sky light.
/// The span is a bridge over open ground, not a cave roof, so callers ignore it.
pub fn span_soffit(cfg:&WorldSampleConfig,x:f32,z:f32)->Option<f32>{
    if cfg.generator_version<FLARE_VERSION||!active(cfg){return None;}
    let (s,a,b)=at(cfg,x,z)?;
    if s.kind!=0||b.abs()>=43.0{return None;}
    let (across,_,soffit)=s.arch(a,b);
    (across<2.0).then_some(s.base+soffit)
}
pub fn priority_block(cfg:&WorldSampleConfig,p:IVec3,column:crate::ColumnSample)->Option<B>{
    if cfg.generator_version<96||!active(cfg){return None;}
    let m=(p.as_vec3()+Vec3::splat(0.5))*cfg.voxel_meters();
    let (s,_,_)=at(cfg,m.x,m.z)?;
    if s.kind!=0&&!s.natural{return None;}
    block(cfg,p,column)
}
pub fn block(cfg:&WorldSampleConfig,p:IVec3,column:crate::ColumnSample)->Option<B>{
    if !active(cfg){return None;}
    let v=cfg.voxel_meters();let m=(p.as_vec3()+Vec3::splat(0.5))*v;
    let(s,a,b)=at(cfg,m.x,m.z)?;
    let y=m.y-s.base;
    // Open the entire cross-section beneath the bent span, including its taper.
    // A broad footprint avoids leaving vertical end caps beside a finite bore.
    if s.kind==0&&s.reworked&&b.abs()<43.0&&y> -5.0 {
        let (across,_,soffit)=s.arch(a,b);
        if across<2.0&&y<soffit {
            if cfg.generator_version>=97 {
                // Carve above the valley floor, not five metres into it. Keep
                // the same ascending road profile on both sides of the span.
                let floor=((a+26.0)*0.40).clamp(0.0,42.0)
                    *(1.0-smoothstep(2.0,5.0,b.abs()));
                return Some(if y>=floor {B::Air}
                    else if y>=floor-v {if b.abs()<1.8 {B::Gravel}else{column.surface}}
                    else {column.filler});
            }
            return Some(B::Air);
        }
    }
    if s.kind==0&&!s.reworked&&(a+29.0).abs()<9.0&&b.abs()<39.0&&y> -5.0&&y<46.0 {
        let bore=(b/29.0).powi(2)+((y-10.0-1.2*(b*0.23).sin())/28.0).powi(2);
        if bore<1.0{return Some(B::Air);}
    }
    // Cliff shelf: the resistant upper bed projects over a sheltered lower ledge.
    if s.kind==2 {
        if s.natural {
            // An eroded lens pinches out at both ends; no rectangular alcove walls.
            let along=((a+10.0)/48.0).abs();
            let centre=25.0+2.0*s.grain(a,b,0.045);
            let radius=1.0-smoothstep(0.5,1.0,along);
            let lens=((b-centre)/6.0).powi(2)+((y-22.0)/10.0).powi(2);
            if lens<radius{return Some(B::Air);}
        } else if (-55.0..35.0).contains(&a)&&(19.0..30.0).contains(&b)&&(12.0..31.0).contains(&y){return Some(B::Air);}
    }
    // Ruined lookout: founded on the same raised platform used by the terrain field.
    let (rx,rz)=(a-82.0,b);
    if rx.abs()<12.0&&rz.abs()<12.0&&(41.0..63.0).contains(&y){
        let r=rx.abs().max(rz.abs());
        if y<42.0{return Some(B::DressedStone);}
        let tower=r<6.0;
        if tower {
            let wall=r>4.5;
            let door=rx< -4.0&&rz.abs()<1.8&&y<46.0;
            let window=(47.0..54.0).contains(&y)&&rx.abs().min(rz.abs())<1.4;
            let broken_top=57.0+((rx+rz)*0.35).sin()*2.0;
            if door||window||y>broken_top{return Some(B::Air);}
            if wall{return Some(if (y-42.0).rem_euclid(5.0)<0.7 {B::DressedStone}else{B::WeatheredBricks});}
            if y<43.0{return Some(B::StoneBricks);}
            // Attached stair treads wind around a central pier to a broken lookout deck.
            if r<0.85&&y<54.0{return Some(B::Limestone);}
            let radius=rx.hypot(rz);
            if (2.3..4.4).contains(&radius)&&y<55.0 {
                let phase=(rz.atan2(rx)+std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)/std::f32::consts::TAU;
                let tread=(phase*16.0).floor()*0.5;
                if (y-43.0-tread).rem_euclid(8.0)>7.25{return Some(B::StoneBricks);}
            }
            if (54.0..55.0).contains(&y)&&rx> -1.0{return Some(B::Planks);}
            return Some(B::Air);
        }
        // An incomplete colonnade frames the tower without sealing the approach.
        let pier=(rx.abs()-9.0).abs()<0.9&&(rz.abs()-8.0).abs()<0.9;
        if pier&&y<49.0{return Some(B::Limestone);}
        if (48.0..49.5).contains(&y)&&rz.abs()>7.1&&rz.abs()<8.9&&rx.abs()<10.0{return Some(B::DressedStone);}
    }
    let top=cfg.world_y(column.height)+v;
    if m.y>top||y< -5.0||s.mask(a,b)<0.90{return None;}
    // Former stack footprints keep their actual geology and surface cover.
    if s.eroded&&s.kind==1&&s.relief(a,b)<=0.0 {return None;}
    if s.formed {
        return Some(form::bedded(cfg,s,a,b,y,m.y,top,column));
    }
    if (m.y-top).abs()<v*1.1&&s.relief(a,b)<43.0 {
        return Some(if b.abs()<1.8&&(-76.0..77.0).contains(&a){B::Gravel}else{column.surface});
    }
    let band=if s.natural {(y+a*0.055+b*0.02+1.4*s.grain(a,b,0.02)).rem_euclid(11.5)}
        else {(y+a*0.045+0.7*(a*0.03).sin()).rem_euclid(7.0)};
    Some(if s.kind==1 {if band<0.28{B::Clay}else{B::Sandstone}}
        else if band<0.22{B::Sandstone}else{B::Limestone})
}

/// Metre-space origin and local `a` axis of the arch site nearest the origin; diagnostics only.
pub fn review_frame(cfg:&WorldSampleConfig)->Option<(Vec3,Vec3,Vec3)>{
    let mut found=None;
    for iz in -3..=3 {for ix in -3..=3 {if let Some(s)=site(cfg,ix,iz).filter(|s|s.kind==0){
        let d=s.x*s.x+s.z*s.z;if found.is_none_or(|(best,_)|d<best){found=Some((d,s));}
    }}}
    let s=found?.1;
    Some((s.point(0.0,0.0,0.0),s.point(1.0,0.0,0.0)-s.point(0.0,0.0,0.0),s.point(0.0,0.0,1.0)-s.point(0.0,0.0,0.0)))
}

/// Camera only: the destination and the foreground frame remain ordinary generated voxels.
pub fn review_view(cfg:&WorldSampleConfig,name:&str)->Option<(Vec3,Vec3)>{
    let (name,reverse)=name.strip_suffix("-reverse").map_or((name,false),|name|(name,true));
    let kind=match name{"arch"=>0,"buttes"=>1,"shelf"=>2,_=>return None};
    let mut found=None;
    for iz in -3..=3 {for ix in -3..=3 {if let Some(s)=site(cfg,ix,iz).filter(|s|s.kind==kind){
        let d=s.x*s.x+s.z*s.z;if found.is_none_or(|(best,_)|d<best){found=Some((d,s));}
    }}}
    let s=found?.1;
    Some(if reverse {(s.point(76.0,46.0,-10.0),s.point(-42.0,25.0,0.0))}
        else {(s.point(-105.0,15.0,-10.0),s.point(82.0,53.0,0.0))})
}
