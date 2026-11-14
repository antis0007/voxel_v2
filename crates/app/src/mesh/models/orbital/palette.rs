//! Linear-light orbital material ramps derived from assets/pixel_palette.txt.
//! Shared by atmospheric and celestial hardware; illumination stays continuous.
const PAPER:[f32;3]=[0.83077,0.75294,0.55834];
const OUTLINE:[f32;3]=[0.01229,0.01370,0.01600];
const EMBER:[f32;3]=[0.53328,0.13287,0.03071];
const GOLD:[f32;3]=[0.70110,0.41789,0.11697];
const fn mix(a:[f32;3],b:[f32;3],t:f32)->[f32;3] {
    [a[0]+(b[0]-a[0])*t,a[1]+(b[1]-a[1])*t,a[2]+(b[2]-a[2])*t]
}
pub const CERAMIC:[f32;3]=mix(PAPER,[0.68,0.73,0.70],0.2);
pub const STEEL:[f32;3]=mix(CERAMIC,OUTLINE,0.64);
pub const GRAPHITE:[f32;3]=mix(STEEL,OUTLINE,0.81);
pub const COPPER:[f32;3]=mix(EMBER,GOLD,0.30);
pub const BRAID:[f32;3]=mix(STEEL,PAPER,0.22);
pub const CARGO:[f32;3]=mix(EMBER,GOLD,0.45);
