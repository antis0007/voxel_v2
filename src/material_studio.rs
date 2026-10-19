// Renderer-side, scale-independent surface families. No mesh rebuild on edits.
use std::sync::{LazyLock, Mutex};

#[derive(Clone, Copy, PartialEq)]
pub struct Palette {
    pub tints: [[f32; 4]; 256],
    pub properties: [[f32; 4]; 256],
}
impl Default for Palette {
    fn default() -> Self {
        Self {
            tints: [[1.0, 1.0, 1.0, -1.0]; 256],
            properties: [[-1.0, 1.0, 1.0, 1.0]; 256],
        }
    }
}
#[derive(Default)]
struct Studio {
    palette: Palette,
    undo: Vec<Palette>,
    redo: Vec<Palette>,
    pending: Option<Palette>,
    selected: usize,
    status: String,
    individual: bool,
    block: usize,
    saved: Palette,
    wetness: f32,
    weather_tick: Option<std::time::Instant>,
}
static STUDIO: LazyLock<Mutex<Studio>> = LazyLock::new(|| {
    Mutex::new(Studio {
        selected: 12,
        ..Default::default()
    })
});
pub fn snapshot() -> Palette {
    STUDIO.lock().unwrap().palette
}
const FAMILIES: &[(usize, &str)] = &[
    (1, "Meadow"),
    (2, "Earth / clay"),
    (3, "Natural stone"),
    (4, "Sandstone"),
    (5, "Bark"),
    (6, "Canopy"),
    (7, "Snow"),
    (8, "Water"),
    (9, "Ice"),
    (11, "Timber"),
    (12, "Masonry"),
    (13, "Cobblestone"),
    (16, "Fungal soil"),
    (17, "Mushroom stem"),
    (18, "Glow cap"),
    (19, "Red cap"),
    (22, "Crystal"),
];
static WORLD_PATH: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);
fn path() -> std::path::PathBuf {
    WORLD_PATH
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| crate::world::storage::save_directory().join("material-presets"))
        .join("palette-preset.vmat")
}
/// The session identity follows the region folder through rename and reopen.
pub fn bind_world(id: &str) {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return;
    }
    let root = crate::world::storage::save_directory()
        .join("regions")
        .join(id);
    let mut s = STUDIO.lock().unwrap();
    let mut active = WORLD_PATH.lock().unwrap();
    if active.as_ref() == Some(&root) {
        return;
    }
    if s.palette!=s.saved {if let Some(old)=active.as_ref(){
        if let Err(e)=write_palette(&old.join("materials.vmat"),s.palette){eprintln!("[material-world] save failed: {e}");}
    }}
    *active = Some(root.clone());
    *s = Studio {
        selected: 12,
        ..Default::default()
    };
    let file = root.join("materials.vmat");
    if file.exists() {
        match read_palette(&file) {
            Ok(p) => {
                s.palette = p;
                s.saved = p;
            }
            Err(e) => s.status = format!("Palette load failed: {e}"),
        }
    }
    println!("[material-world] bound {id}");
}
pub fn reference_palette(id: &str) {
    bind_world(id);
    let mut s = STUDIO.lock().unwrap();
    s.palette = Palette::default();
    s.palette.tints[1] = [0.82, 1.0, 0.85, -1.0];
    s.palette.tints[6] = [0.80, 1.0, 0.86, -1.0];
    s.palette.tints[11] = [1.08, 0.92, 0.80, -1.0];
    s.palette.tints[12] = [1.08, 1.02, 0.90, -1.0];
    let pine = 64 + crate::material::BlockId::PineLeaves as usize;
    s.palette.tints[pine] = [0.78, 0.94, 0.91, -1.0];
    if let Some(root) = WORLD_PATH.lock().unwrap().as_ref() {
        match write_palette(&root.join("materials.vmat"), s.palette) {
            Ok(()) => s.saved = s.palette,
            Err(e) => s.status = e,
        }
    }
}
pub fn weather_surface(storm: f32) -> [f32; 4] {
    let mut s = STUDIO.lock().unwrap();
    let now = std::time::Instant::now();
    let dt = s
        .weather_tick
        .map_or(0.0, |t| now.duration_since(t).as_secs_f32().min(0.25));
    s.weather_tick = Some(now);
    let rain = ((storm - 0.10) / 0.65).clamp(0.0, 1.0);
    let rate = if rain > s.wetness { 0.24 } else { 0.025 };
    s.wetness += (rain - s.wetness) * (1.0 - (-dt * rate).exp());
    [s.wetness, rain, 0.0, 0.0]
}
fn write_palette(path: &std::path::Path, p: Palette) -> Result<(), String> {
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let mut data = String::from("voxel-materials 2\n");
    for i in 0..256 {
        let t = p.tints[i];
        let v = p.properties[i];
        data += &format!(
            "{i} {} {} {} {} {} {} {} {}\n",
            t[0], t[1], t[2], t[3], v[0], v[1], v[2], v[3]
        );
    }
    let pending = path.with_extension("pending");
    std::fs::write(&pending, data).map_err(|e| e.to_string())?;
    if path.exists() {
        std::fs::copy(path, path.with_extension("backup")).map_err(|e| e.to_string())?;
    }
    std::fs::rename(pending, path).map_err(|e| e.to_string())
}
fn save(p: Palette) -> Result<(), String> {
    write_palette(&path(), p)
}
fn load() -> Result<Palette, String> {
    read_palette(&path())
}
fn read_palette(path: &std::path::Path) -> Result<Palette, String> {
    let data = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut lines = data.lines();
    let version = match lines.next() {
        Some("voxel-materials 1") => 1,
        Some("voxel-materials 2") => 2,
        _ => return Err("Unsupported palette version".into()),
    };
    let count = if version == 1 { 64 } else { 256 };
    let mut p = Palette::default();
    let mut seen = [false; 256];
    for line in lines {
        let f: Vec<_> = line.split_whitespace().collect();
        if f.len() != if version == 1 { 6 } else { 9 } {
            return Err("Incomplete material record".into());
        }
        let i = f[0].parse::<usize>().map_err(|_| "Invalid material")?;
        if i >= count || seen[i] {
            return Err("Duplicate or unknown material".into());
        }
        seen[i] = true;
        let mut v = [1.0; 8];
        for k in 0..f.len() - 1 {
            v[k] = f[k + 1].parse::<f32>().map_err(|_| "Invalid number")?;
            if !v[k].is_finite() {
                return Err("Non-finite value".into());
            }
        }
        if v[..3].iter().any(|x| !(0.0..=2.0).contains(x))
            || !(v[3] == -1.0 || (0.08..=1.0).contains(&v[3]))
            || !(v[4] == -1.0 || (0.0..=1.0).contains(&v[4]))
            || !(0.25..=4.0).contains(&v[5])
            || !(0.0..=2.0).contains(&v[6])
            || !(0.0..=2.0).contains(&v[7])
        {
            return Err("Material value out of range".into());
        }
        p.tints[i].copy_from_slice(&v[..4]);
        p.properties[i].copy_from_slice(&v[4..]);
    }
    if seen[..count].iter().any(|v| !*v) {
        return Err("Incomplete palette".into());
    }
    Ok(p)
}
pub fn draw(
    ctx: &egui::Context,
    open: &mut bool,
    time: &mut crate::egui_ui::TimeUiState,
    action: &mut crate::egui_ui::UiAction,
) {
    egui::Window::new(egui::RichText::new("Material studio").size(18.0))
        .open(open).default_pos(egui::pos2(12.0,24.0)).default_width(280.0)
        .resizable(false).auto_sized().show(ctx,|ui| {
        for (style,size) in [(egui::TextStyle::Body,14.0),(egui::TextStyle::Button,14.0),
            (egui::TextStyle::Small,11.0),(egui::TextStyle::Heading,18.0),(egui::TextStyle::Monospace,13.0)] {
            ui.style_mut().text_styles.insert(style,egui::FontId::proportional(size));
        }
        ui.set_width(280.0);
        ui.spacing_mut().item_spacing=egui::vec2(6.0,4.0);
        ui.spacing_mut().button_padding=egui::vec2(6.0,3.0);
        let mut s=STUDIO.lock().unwrap(); let before=s.palette;
        // Only the controls scroll. Presets, history and persistence stay visible.
        egui::ScrollArea::vertical().max_height((ctx.content_rect().height()-210.0).max(80.0)).show(ui,|ui| {
            ui.label(egui::RichText::new("MOSS & EMBER").color(egui::Color32::from_rgb(202,169,112)).strong());
            ui.small("Saved automatically with this world");
            if ui.checkbox(&mut s.individual,"Individual material override").changed() && s.individual {
                s.block=crate::material::ALL_BLOCK_IDS.iter().position(|b|b.surface().texture_kind as usize==s.selected).unwrap_or(0);
            }
            if s.individual { egui::ComboBox::from_label("Block").selected_text(crate::material::ALL_BLOCK_IDS[s.block].display_name()).show_ui(ui,|ui|{for (id,block) in crate::material::ALL_BLOCK_IDS.iter().enumerate(){ui.selectable_value(&mut s.block,id,block.display_name());}}); }
            if !s.individual {
            egui::ComboBox::from_label("Family")
                .selected_text(FAMILIES.iter().find(|f|f.0==s.selected).unwrap().1)
                .show_ui(ui,|ui|{for &(id,name) in FAMILIES {ui.selectable_value(&mut s.selected,id,name);}});
            }
            ui.horizontal(|ui|{for (label,hour,rain) in [("Noon",0.5,false),("Sunset",0.73,false),("Night",0.0,false),("Rain",0.5,true)] {
                if ui.small_button(label).clicked(){time.time_of_day=hour;time.freeze_time=true;
                    *action=crate::egui_ui::UiAction::SetWeatherPreset(if rain{crate::world::WeatherPreset::Stormy}else{crate::world::WeatherPreset::Clear});}
            }});
            ui.separator();let i=if s.individual {64+s.block} else {s.selected};
            let mut rgb=[s.palette.tints[i][0],s.palette.tints[i][1],s.palette.tints[i][2]];
            ui.horizontal(|ui|{ui.label("Tint");ui.color_edit_button_rgb(&mut rgb);});
            s.palette.tints[i][..3].copy_from_slice(&rgb);
            ui.collapsing("RGB channels",|ui|{for (k,name) in [(0,"Red"),(1,"Green"),(2,"Blue")] {
                ui.add(egui::Slider::new(&mut s.palette.tints[i][k],0.0..=2.0).text(name));
            }});
            let mut finish=s.palette.tints[i][3]>=0.0;
            if ui.checkbox(&mut finish,"Override finish").changed(){
                s.palette.tints[i][3]=if finish{0.75}else{-1.0};
                s.palette.properties[i][0]=if finish{0.25}else{-1.0};
            }
            if finish {
                ui.add(egui::Slider::new(&mut s.palette.tints[i][3],0.08..=1.0).text("Roughness"));
                ui.add(egui::Slider::new(&mut s.palette.properties[i][0],0.0..=1.0).text("Reflectance"));
            }
            ui.collapsing("Surface response",|ui|{
                ui.add(egui::Slider::new(&mut s.palette.properties[i][1],0.25..=4.0).text("Texture scale"));
                ui.add(egui::Slider::new(&mut s.palette.properties[i][2],0.0..=2.0).text("Detail"));
                ui.add(egui::Slider::new(&mut s.palette.properties[i][3],0.0..=2.0).text("Rain wetness"));
            });
            if ui.button("Reset selected").clicked(){
                s.palette.tints[i]=Palette::default().tints[i];s.palette.properties[i]=Palette::default().properties[i];
            }
            ui.small("Individual tints multiply the family. Finish overrides inherit when disabled.");
        });
        ui.separator();
        ui.horizontal(|ui|{
            if ui.button("Moss & Ember").clicked(){s.palette=Palette::default();
                s.palette.tints[6]=[0.78,1.0,0.85,-1.0];s.palette.tints[12]=[1.12,1.02,0.86,-1.0];s.palette.tints[11]=[1.12,0.87,0.71,-1.0];}
            if review_button(ui,"Moonstone").clicked(){s.palette=Palette::default();
                s.palette.tints[12]=[0.68,0.85,1.35,0.38];s.palette.properties[12][0]=0.6;s.palette.tints[6]=[0.65,0.8,1.2,-1.0];}
        });
        if s.palette!=before {s.pending.get_or_insert(before);s.redo.clear();}
        if !ctx.input(|i|i.pointer.any_down()) {if let Some(p)=s.pending.take(){s.undo.push(p);if s.undo.len()>128{s.undo.remove(0);}}}
        ui.horizontal(|ui|{
            if review_button(ui,"Undo").clicked(){if let Some(p)=s.undo.pop(){let old=s.palette;s.redo.push(old);s.palette=p;}}
            if review_button(ui,"Redo").clicked(){if let Some(p)=s.redo.pop(){let old=s.palette;s.undo.push(old);s.palette=p;}}
            if review_button(ui,"Reset all").clicked(){let old=s.palette;s.undo.push(old);s.redo.clear();s.palette=Palette::default();}
        });
        ui.horizontal(|ui|{
            if review_button(ui,"Save palette").on_hover_text(path().display().to_string()).clicked(){
                s.status=match save(s.palette){Ok(())=>"Palette saved".into(),Err(e)=>e};}
            if review_button(ui,"Load palette").clicked(){match load(){
                Ok(p)=>{let old=s.palette;s.undo.push(old);s.redo.clear();s.palette=p;s.status="Palette loaded".into();},Err(e)=>s.status=e};}
        });
        if !ctx.input(|i|i.pointer.any_down()) && s.palette!=s.saved {
            if let Some(root)=WORLD_PATH.lock().unwrap().as_ref() {
                match write_palette(&root.join("materials.vmat"),s.palette) {
                    Ok(())=>s.saved=s.palette,Err(e)=>s.status=format!("World palette not saved: {e}"),
                }
            }
        }
        if !s.status.is_empty(){ui.small(&s.status);}
    });
}

fn review_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let response = ui.button(label);
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new(("studio-review", label)), response.rect));
    response
}
// Opt-in replay through the same egui pointer event path as user input.
pub fn review_input(ctx: &egui::Context, input: &mut egui::RawInput) {
    if std::env::var_os("VOXEL_REVIEW_EDITOR").is_none() {
        return;
    }
    input.focused = true;
    static REPLAY: Mutex<(Option<std::time::Instant>, usize)> = Mutex::new((None, usize::MAX));
    let mut replay = REPLAY.lock().unwrap();
    let labels = [
        "Moonstone",
        "Save palette",
        "Reset all",
        "Load palette",
        "Undo",
        "Redo",
    ];
    if ctx
        .data_mut(|d| d.get_temp::<egui::Rect>(egui::Id::new(("studio-review", labels[0]))))
        .is_none()
    {
        return;
    }
    // Advance only one phase per rendered frame; slow startup cannot skip a press.
    let now = std::time::Instant::now();
    if replay.0.is_none() {
        replay.0 = Some(now);
        replay.1 = 0;
        return;
    }
    if now.duration_since(replay.0.unwrap()).as_secs_f32() < 0.3 {
        return;
    }
    replay.0 = Some(now);
    replay.1 += 1;
    let step = replay.1;
    if (4..34).contains(&step) {
        let index = (step - 4) / 5;
        let phase = (step - 4) % 5;
        if phase < 3 {
            if let Some(rect) = ctx.data_mut(|d| {
                d.get_temp::<egui::Rect>(egui::Id::new(("studio-review", labels[index])))
            }) {
                input.focused = true;
                input.events.push(egui::Event::PointerMoved(rect.center()));
                if phase > 0 {
                    input.events.push(egui::Event::PointerButton {
                        pos: rect.center(),
                        button: egui::PointerButton::Primary,
                        pressed: phase == 1,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
            }
        }
        if phase == 3 {
            let palette = snapshot();
            let expected = if index == 2 || index == 4 { 1.0 } else { 0.68 };
            let ok =
                (palette.tints[12][0] - expected).abs() < 0.001 && (index != 1 || path().exists());
            println!(
                "[studio-review] {} {} masonry={:?}",
                labels[index],
                if ok { "PASS" } else { "FAIL" },
                palette.tints[12]
            );
        }
    }
}
