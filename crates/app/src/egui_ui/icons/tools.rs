//! Original modular silhouettes. Inventory and held meshes sample these same texels.
use super::Icon;
use engine_tools::{MATERIALS, PARTS, parts::PartRole};
type Pixel = Option<(u8,[u8;3])>;
const SIZE: usize = 24;

fn material(index:u8)->[u8;3] {
    static COLORS:std::sync::LazyLock<Vec<[u8;3]>>=std::sync::LazyLock::new(|| MATERIALS.iter().map(|m| {
        if let Some(b)=m.block.as_ref().and_then(|b|ron::from_str::<engine_content::BlockId>(b).ok()) {
            let c=super::super::material_color32(b); return [c.r(),c.g(),c.b()];
        }
        match m.id.0.as_str() {
            "wood"=>[142,98,56], "flint"=>[100,112,125], "bone"=>[221,209,167],
            "paper"=>[232,219,183], "slime"=>[104,181,95], "copper"=>[196,112,66],
            "iron"=>[175,188,199], "bronze"=>[183,140,70], "steel"=>[120,144,161],
            _ if m.props.crystalline=>[135,192,211],
            _ if m.props.living=>[116,153,80],
            _=>[155,148,137],
        }
    }).collect());
    COLORS.get(index as usize).copied().unwrap_or([142,98,56])
}
struct Sprite { cells:Vec<Option<(u8,[u8;3])>> }
impl Sprite {
    fn new()->Self {Self{cells:vec![None;SIZE*SIZE]}}
    fn put(&mut self,x:i32,y:i32,role:u8,c:[u8;3]) {
        if (0..SIZE as i32).contains(&x)&&(0..SIZE as i32).contains(&y) {self.cells[y as usize*SIZE+x as usize]=Some((role,c));}
    }
    fn rect(&mut self,x:i32,y:i32,w:i32,h:i32,role:u8,c:[u8;3]) {
        for j in y..y+h {for i in x..x+w {self.put(i,j,role,c);}}
    }
    fn line(&mut self,a:(i32,i32),b:(i32,i32),width:i32,role:u8,c:[u8;3]) {
        let steps=(b.0-a.0).abs().max((b.1-a.1).abs()).max(1);
        for t in 0..=steps {let x=a.0+(b.0-a.0)*t/steps;let y=a.1+(b.1-a.1)*t/steps;self.rect(x,y,width,width,role,c);}
    }
    fn head(&mut self,shape:&str,c:[u8;3]) {
        match shape {
            "hammer"=> {self.rect(7,3,14,6,1,c);self.rect(9,2,10,8,1,c);}
            "axe"=>{self.rect(12,3,5,9,1,c);self.rect(16,4,4,7,1,c);self.rect(19,6,2,4,1,c);}
            "shovel"|"excavator"=>{self.rect(13,3,6,7,1,c);self.rect(14,2,4,9,1,c);self.rect(15,10,2,2,1,c);if shape=="excavator"{self.rect(11,4,10,5,1,c);}}
            "scythe"=>{self.line((5,3),(16,3),2,1,c);self.line((16,3),(21,7),2,1,c);self.line((2,6),(5,3),1,1,c);}
            "sword"|"dagger"|"broadsword"|"cleaver"=>{
                let start=if shape=="dagger"{8}else{2};
                let width=if shape=="cleaver"{5}else if shape=="broadsword"{4}else{3};
                self.line((11,12),(20-start/2,start),width,1,c);
                self.line((9,11),(14,16),2,2,[166,132,67]);
            }
            "shortbow"|"longbow"=>{self.cells.fill(None);let reach=if shape=="longbow"{1}else{4};self.line((9,reach),(17,8),2,1,c);self.line((17,8),(17,14),2,1,c);self.line((17,14),(9,23-reach),2,1,c);self.line((9,reach+1),(9,22-reach),1,3,[219,210,178]);}
            _=>{self.line((6,3),(15,4),3,1,c);self.line((15,4),(20,9),3,1,c);self.rect(5,4,3,4,1,c);self.rect(20,10,2,3,1,c);}
        }
    }
    fn finish(self,flags:u8)->Vec<Pixel> {
        let mut out=vec![None;SIZE*SIZE];
        for y in 0..SIZE {for x in 0..SIZE {
            if let Some((role,c))=self.cells[y*SIZE+x] {
                let same=|dx:i32,dy:i32|->bool {let(a,b)=(x as i32+dx,y as i32+dy);a>=0&&b>=0&&a<SIZE as i32&&b<SIZE as i32&&self.cells[b as usize*SIZE+a as usize].is_some_and(|(r,_)|r==role)};
                let shade=if !same(-1,0)||!same(0,-1){1.23}else if !same(1,0)||!same(0,1){0.65}else if (x+y)%5==0{0.91}else{1.0};
                let color=c.map(|v|(v as f32*shade).min(255.0) as u8);
                out[y*SIZE+x]=Some((role,if flags&128!=0&&role==1&&(x+y)%7<2{[48,43,46]}else{color}));
            } else {
                let near=[(-1,0),(1,0),(0,-1),(0,1)].iter().any(|&(dx,dy)|{let(a,b)=(x as i32+dx,y as i32+dy);a>=0&&b>=0&&a<SIZE as i32&&b<SIZE as i32&&self.cells[b as usize*SIZE+a as usize].is_some()});
                if near {out[y*SIZE+x]=Some((0,[35,30,34]));}
            }
        }}
        out
    }
}
/// Sheet-drawn sprite from `assets/tools/sprites16.txt` when the sheet covers the tool.
fn sheet(icon: Icon, head_only: bool) -> Option<(usize, usize, Vec<Pixel>)> {
    use engine_tools::sprite::{SHEET, SIZE as S};
    let (schema,mats):(&str,Vec<&str>)=match icon {
        Icon::BuiltTool(schema,mats,_)=>(PARTS.schemas.get(schema as usize)?.id.as_str(),mats.iter().map(|&m|MATERIALS.get(m as usize).map_or("stone",|m|m.id.as_str())).collect()),
        Icon::Sword(tier)=>("sword",vec![match tier {1=>"copper",2=>"steel",3=>"cinder",4=>"crystal",5=>"cobalt",_=>"iron"},"iron","wood"]),
        Icon::Pickaxe(tier @ 6..=8)=>("pickaxe",vec![["wood","stone","iron"][tier as usize-6],"wood","wood"]),
        Icon::Axe(tier @ 12..=14) => ("axe", vec![["wood", "stone", "iron"][tier as usize - 12], "wood", "wood"]),
        Icon::Shovel(tier @ 15..=17) => ("shovel", vec![["wood", "stone", "iron"][tier as usize - 15], "wood", "wood"]),
        _=>return None,
    };
    let pixels = if head_only { SHEET.render_head(schema, &mats)? } else { SHEET.render(schema, &mats)? };
    Some((S, S, pixels))
}

pub(crate) fn held_head(icon: Icon) -> Option<Vec<Pixel>> {
    sheet(icon, true).map(|(_, _, pixels)| pixels)
}
pub fn texels(icon:Icon)->Option<(usize,usize,Vec<Pixel>)> {
    if let Some(sprite)=sheet(icon, false) {return Some(sprite);}
    let mut s=Sprite::new();
    let mut flags=0;
    match icon {
        Icon::BuiltTool(schema,mats,f)=>{
            flags=f;
            let def=PARTS.schemas.get(schema as usize)?;
            let mut head=material(mats[0]);let mut handle=material(0);let mut binding=[186,148,92];
            for (p,&m) in def.parts.iter().zip(&mats) {if let Some(k)=PARTS.part_kind(&p.kind.0) {match k.role {PartRole::Head=>head=material(m),PartRole::Handle=>handle=material(m),PartRole::Binding|PartRole::Reinforcement=>binding=material(m),_=>{}}}}
            if def.id == "shield" {
                s.rect(6, 4, 12, 13, 1, head);
                s.rect(7, 17, 10, 2, 1, head);
                s.rect(9, 19, 6, 2, 1, head);
                s.rect(11, 21, 2, 1, 1, head);
                s.rect(5, 3, 14, 3, 2, binding);
                s.rect(10, 9, 4, 4, 3, handle);
                return Some((SIZE, SIZE, s.finish(flags)));
            }
            s.line((4,19),(15,8),2,3,handle);
            s.line((3,20),(5,18),3,3,handle);
            s.head(&def.id,head);
            if !matches!(def.id.as_str(),"shortbow"|"longbow") {
                s.line((11,11),(13,9),3,2,binding);
                for n in 0..(f&15).min(3) {s.rect(6+n as i32*2,18-n as i32*2,2,1,4,[116,191,196]);}
            }
        }
        Icon::ToolPart(kind,mat)=>{
            let k=PARTS.part_kinds.get(kind as usize)?;let c=material(mat);
            match k.id.0.as_str() {
                "head"=>s.head("pickaxe",c),"blade"=>s.head("sword",c),
                "bow_limb"=>s.head("shortbow",c),"large_plate"=>{s.rect(6,5,12,13,1,c);s.rect(8,3,8,17,1,c);},
                "binding"|"tough_binding"=>{s.rect(7,6,10,3,2,c);s.rect(7,14,10,3,2,c);s.rect(6,8,3,7,2,c);s.rect(15,8,3,7,2,c);},
                "guard"=>s.line((5,15),(17,5),3,2,c),
                "bowstring"=>{s.line((8,4),(16,9),1,3,c);s.line((16,9),(8,20),1,3,c);s.line((8,20),(8,4),1,3,c);},
                _=>s.line((5,19),(17,5),3,3,c),
            }
        }
        Icon::ToolCast(kind)=>{
            // Raised clay rim around a readable negative of the part.
            let inset=texels(Icon::ToolPart(kind,0));
            s.rect(3,2,18,20,5,[174,110,72]);
            s.rect(4,3,16,18,5,[197,136,89]);
            if let Some((_,_,cells))=inset {for(y,row)in cells.chunks(SIZE).enumerate(){for(x,p)in row.iter().enumerate(){if p.is_some_and(|(r,_)|r!=0){s.put(x as i32,y as i32,6,[80,51,43]);}}}}
            else {s.rect(7,8,10,7,6,[80,51,43]);}
        }
        _=>return None,
    }
    Some((SIZE,SIZE,s.finish(flags)))
}

/// Exports production texels without opening a world or touching saves.
pub(crate) fn export_sheet(path:&std::path::Path)->anyhow::Result<()> {
    let scale=4usize;let cell=32usize;let cols=PARTS.schemas.len().max(PARTS.part_kinds.len());
    let mut image=image::RgbaImage::from_pixel((cols*cell*scale) as u32,(6*cell*scale) as u32,image::Rgba([24,22,28,255]));
    let find=|id:&str|MATERIALS.iter().position(|m|m.id.0==id).unwrap_or(0) as u8;
    for row in 0..6 {for col in 0..cols {
        let mut mats=[find("wood");8];
        for (i,m) in mats.iter_mut().enumerate(){*m=if i==0{find(["stone","copper","iron","steel"][row.min(3)])}else if i==1{find("bone")}else{find("wood")};}
        let icon=if row<4 {if col>=PARTS.schemas.len(){continue;}Icon::BuiltTool(col as u16,mats,if row==3{2}else{0})}
            else if row==4 {if col>=PARTS.part_kinds.len(){continue;}Icon::ToolPart(col as u8,find("copper"))}
            else {if col>PARTS.part_kinds.len(){continue;}Icon::ToolCast(if col==PARTS.part_kinds.len(){255}else{col as u8})};
        if let Some((w,_,texels))=texels(icon){for (i,p) in texels.into_iter().enumerate(){if let Some((_,c))=p {for dy in 0..scale {for dx in 0..scale {image.put_pixel(((col*cell+4+i%w)*scale+dx)as u32,((row*cell+4+i/w)*scale+dy)as u32,image::Rgba([c[0],c[1],c[2],255]));}}}}}
    }}
    if let Some(parent)=path.parent(){std::fs::create_dir_all(parent)?;}
    image.save(path)?;
    Ok(())
}
