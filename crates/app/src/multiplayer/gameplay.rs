//! Complete authoritative gameplay images. Transient combat and machinery state
//! travel alongside the existing bounded subsystem checkpoint formats.
use super::*;
use crate::world::{adventure::Adventure,expedition::Expedition,inventory::SurvivalInventory};
use std::io::Write;
pub const MAX_IMAGE:usize=16*1024*1024;
pub struct Target {pub id:PlayerId,pub feet:Vec3,pub min:Vec3,pub max:Vec3,pub alive:bool}
pub struct Hit {pub id:PlayerId,pub amount:f32,pub direction:Vec3}
pub struct ContextFrame {pub actor:PlayerId,pub players:Vec<Target>,pub shared_tick:bool,pub damage:Vec<Hit>,
    pub contacts:Vec<(PlayerId,super::audio_events::ContactEvent)>,pub creature_tick:u64}
impl ContextFrame {
    pub fn owns(&self,pos:Vec3)->bool {
        self.players.iter().min_by(|a,b| b.alive.cmp(&a.alive)
            .then_with(|| a.feet.distance_squared(pos).total_cmp(&b.feet.distance_squared(pos)))
            .then_with(|| a.id.cmp(&b.id)))
            .is_none_or(|p|p.id==self.actor)
    }

    pub fn opponents(&self) -> impl Iterator<Item = &Target> {
        self.players.iter().filter(|player| player.id != self.actor && player.alive)
    }
}
#[derive(Serialize,Deserialize,Default)]
pub struct Personal {
    pub adventure:Adventure,
    pub inventory:SurvivalInventory,
    pub expedition:Expedition,
    #[serde(skip)]
    pub progression: world::progression::State,
    #[serde(skip)]
    pub ride: Option<world::travel::Ride>,
    #[serde(skip)]
    cast: Option<world::travel::fishing::Cast>,
    #[serde(skip)]
    last_colour: Option<u32>,
    pub focus:Option<IVec3>,
    pub bench:Option<IVec3>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    realm_spawns: BTreeMap<String, Vec3>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    realm_hell_spawns: BTreeMap<String, world::spawn::Binding>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    realm_recovery: BTreeMap<String, world::spawn::RealmSpawn>,
}
impl Personal {
    pub(super) fn transfer_spawn(&mut self, source: &str, target: &World, arrival_m: Vec3) {
        for (realm, point) in std::mem::take(&mut self.realm_spawns) {
            if point.is_finite() {
                self.realm_recovery.entry(realm).or_default().point.get_or_insert(point);
            }
        }
        for (realm, binding) in std::mem::take(&mut self.realm_hell_spawns) {
            self.realm_recovery.entry(realm).or_default().binding = binding;
        }
        let mut destination = self.realm_recovery.remove(target.regions.view.session_name()).unwrap_or_default();
        let previous = self.adventure.realm_spawn();
        if previous.point.is_some() || previous.binding.bed.is_some() || previous.binding.arrival.is_some() {
            self.realm_recovery.insert(source.to_owned(), previous);
        }
        destination.on_arrival(target, arrival_m, false);
        self.adventure.set_realm_spawn(destination);
    }

    /// The shared creature/encounter ledger and dropped items never become a
    /// player's private copy. Calling twice restores the original mounted actor.
    pub fn swap(&mut self,w:&mut World) {
        self.adventure.swap_multiplayer_shared(&mut w.adventure);
        std::mem::swap(&mut self.adventure,&mut w.adventure);
        std::mem::swap(&mut self.inventory.dropped,&mut w.inventory.dropped);
        std::mem::swap(&mut self.inventory,&mut w.inventory);
        if w.inventory.balance != w.rules.stacks { w.inventory.migrate_balance(w.rules.stacks); }
        std::mem::swap(&mut self.expedition,&mut w.expedition);
        std::mem::swap(&mut self.progression, &mut w.progression);
        std::mem::swap(&mut self.ride, &mut w.travel.riding);
        std::mem::swap(&mut self.cast, &mut w.travel.cast);
        std::mem::swap(&mut self.last_colour, &mut w.travel.last_colour);
        world::progression::study::restore_spells(w);
        std::mem::swap(&mut self.focus,&mut w.factory.focus);
        std::mem::swap(&mut self.bench,&mut w.factory.bench);
        // Kit counters belong to the mounted inventory, not the shared factory.
        for kind in world::factory::Kind::ALL {
            w.factory.kits[kind as usize] = w.inventory.count_item(world::inventory::Item::Block(kind.block()));
        }
        crate::world::containers::refresh(w);
    }
}
pub fn pack<T:Serialize>(value:&T)->Result<Vec<u8>> {
    let text=ron::to_string(value)?;
    ensure!(text.len()<=MAX_IMAGE,"gameplay image exceeds budget");
    let mut encoder=flate2::write::ZlibEncoder::new(Vec::new(),flate2::Compression::fast());
    encoder.write_all(text.as_bytes())?;let bytes=encoder.finish()?;
    ensure!(bytes.len()<=engine_net::transfer::MAX_TRANSFER,"compressed image exceeds budget");Ok(bytes)
}
pub fn unpack<T:serde::de::DeserializeOwned>(bytes:&[u8])->Result<T> {unpack_limit(bytes,MAX_IMAGE)}
pub fn unpack_limit<T:serde::de::DeserializeOwned>(bytes:&[u8],limit:usize)->Result<T> {
    ensure!(bytes.len()<=MAX_IMAGE,"oversized gameplay image");
    let mut decoded=Vec::new();
    flate2::read::ZlibDecoder::new(bytes).take(limit as u64+1).read_to_end(&mut decoded)?;
    ensure!(decoded.len()<=limit,"expanded gameplay image exceeds budget");
    Ok(ron::from_str(std::str::from_utf8(&decoded)?)?)
}
fn words<T>(bytes:&[u8],f:impl FnOnce(&mut dyn FnMut()->Result<u32>)->Result<T>)->Result<T> {
    let mut offset=0;
    let mut next=|| {let slice=bytes.get(offset..offset+4).context("truncated gameplay component")?;offset+=4;Ok(u32::from_le_bytes(slice.try_into()?))};
    let value=f(&mut next)?;ensure!(offset==bytes.len(),"trailing gameplay component data");Ok(value)
}
#[derive(Serialize)]
struct ImageRef<'a> {
    caches: Vec<u8>, block_data: Vec<u8>, rules: Vec<u8>,
    version:u32, command_ack:u64, body:&'a Player, day:DayNightCycle, adventure:&'a Adventure, inventory:&'a SurvivalInventory, expedition:&'a Expedition,
    factory:&'a world::factory::Factory, parts:Vec<Vec<u8>>, water:Vec<(IVec3,u8,glam::Vec2)>,
    others:Vec<&'a Adventure>,
    farming:Vec<u8>, travel:Vec<u8>,
    progression: Vec<u8>,
    industry:Vec<u8>,
    manyfold_weight: crate::manyfold::Weight,
}
#[derive(Deserialize)]
pub struct Image {
    #[serde(default)]
    caches: Vec<u8>,
    #[serde(default)]
    block_data: Vec<u8>,
    #[serde(default)]
    rules: Vec<u8>,
    version:u32, pub command_ack:u64, body:Player, day:DayNightCycle, adventure:Adventure, inventory:SurvivalInventory, expedition:Expedition,
    factory:world::factory::Factory, parts:Vec<Vec<u8>>, water:Vec<(IVec3,u8,glam::Vec2)>,
    others:Vec<Adventure>,
    #[serde(default)]
    industry:Vec<u8>,
    #[serde(default)]
    farming:Vec<u8>,
    #[serde(default)]
    travel:Vec<u8>,
    #[serde(default)]
    progression: Vec<u8>,
    #[serde(default)]
    manyfold_weight: crate::manyfold::Weight,
}
pub fn capture<'a>(w:&'a World, body:&'a Player, day:&DayNightCycle, command_ack:u64, others:Vec<&'a Adventure>)->Result<Vec<u8>> {
    let mut parts=vec![Vec::new();7];
    w.factory.chips.encode(&mut parts[0]);w.undercrown.encode(&mut parts[1]);
    w.sandbox.encode(&mut parts[2]);w.orbital.encode(&mut parts[3],world::storage::SAVE_VERSION);
    w.habitats.encode_versioned(&mut parts[4],world::storage::SAVE_VERSION);
    w.environment.encode(&mut parts[5]);w.activity.snapshot().encode(&mut parts[6]);
    let snapshot=w.water_levels.snapshot_all();
    let mut water:Vec<_>=snapshot.iter().map(|(&p,&v)|(p,v,w.water_levels.velocity(p))).collect();
    water.sort_by_key(|(p,_,_)|p.to_array());
    let mut industry=Vec::new();w.industry.encode(&mut industry);
    let mut caches = Vec::new();
    world::containers::encode(&w.caches, &mut caches);
    pack(&ImageRef{caches,block_data:world::block_data::sidecar::encode(&w.block_data),rules:w.rules.encode(),version:1,command_ack,body,day:*day,adventure:&w.adventure,inventory:&w.inventory,expedition:&w.expedition,
        factory:&w.factory,parts,water,others,industry,
        farming:world::farming::sidecar::encode(w),travel:world::travel::sidecar::encode(&w.travel),
        progression: world::progression::sidecar::encode(&w.progression, &w.inventory),
        manyfold_weight:w.manyfold_weight})
}
impl Image {
    pub fn apply(self,w:&mut World,p:&mut Player,day:&mut DayNightCycle,blocks:&mut HashSet<IVec3>,chunks:&mut HashSet<IVec3>)->Result<()> {
        ensure!(self.version==1 && self.parts.len()==7 && self.others.len()<=16,"invalid gameplay image schema");
        ensure!(self.adventure.enemies.len()<=512 && self.adventure.projectiles.len()<=4096
            && self.factory.nodes.len()<=8192 && self.inventory.dropped.len()<=1024 && self.water.len()<=262144,"gameplay entity budget exceeded");
        // Decode every component before publishing any of them.
        let industry=if self.industry.is_empty(){world::industry::State::default()}else{words(&self.industry,|r|world::industry::State::decode(&mut ||r()))?};
        let chips=words(&self.parts[0],|r|world::factory::chips::Bay::decode(&mut ||r()))?;
        let undercrown=words(&self.parts[1],|r|world::undercrown::State::decode(&mut ||r()))?;
        let sandbox=words(&self.parts[2],|r|crate::simulation::sandbox::Sandbox::decode(&mut ||r()))?;
        let orbital=words(&self.parts[3],|r|world::orbital::State::decode(&mut ||r(),world::storage::SAVE_VERSION))?;
        let habitats=words(&self.parts[4],|r|world::habitats::State::decode(&mut ||r(),world::storage::SAVE_VERSION))?;
        let environment=words(&self.parts[5],|r|world::environment::State::decode(&mut ||r(),w.voxel_scale))?;
        let activity=words(&self.parts[6],|r|world::activity::Activity::decode(&mut ||r(),true))?;
        let farming=if self.farming.is_empty(){None}else{Some(world::farming::sidecar::decode(&self.farming)?.0)};
        let travel=if self.travel.is_empty(){None}else{Some(world::travel::sidecar::decode(&self.travel)?)};
        let progression = if self.progression.is_empty() {
            world::progression::State::default()
        } else {
            world::progression::sidecar::decode(&self.progression)?
        };
        let caches=if self.caches.is_empty(){None}else{Some(words(&self.caches, |r| world::containers::decode(&mut ||r(), &self.inventory))?)};
        let block_data=if self.block_data.is_empty(){None}else{Some(world::block_data::sidecar::decode(&self.block_data)?)};
        let rules=if self.rules.is_empty(){None}else{Some(world::rules::Rules::decode(&self.rules)?)};
        ensure!(self.manyfold_weight.valid(), "Invalid Manyfold weight");
        let old=w.water_levels.snapshot_all();
        let mut levels=world::water::WaterLevels::default();
        for (p,v,velocity) in self.water {
            super::validate_cell(p.to_array(),BlockId::Water.as_u32())?;
            ensure!(v>0 && v<=8 && velocity.is_finite(),"invalid water replica");
            levels.insert(p,v);levels.set_velocity(p,velocity);
        }
        // Validate the complete image first. Quiet water must not rebuild its
        // surrounding chunks every time another player's state arrives.
        let current = levels.snapshot_all();
        let mut changed = HashSet::new();
        for (&p, &v) in current.iter() {
            if old.get(&p).copied().unwrap_or(8) != v { changed.insert(p); }
        }
        for (&p, &v) in old.iter() {
            if current.get(&p).copied().unwrap_or(8) != v { changed.insert(p); }
        }
        // A thin film can move without crossing a coarse eighth-cell boundary.
        for (key, value) in environment.water_deficit.iter() {
            if w.environment.water_deficit.get(key) != Some(value) {
                changed.insert(IVec3::new(key.0, key.1, key.2));
            }
        }
        for (key, value) in w.environment.water_deficit.iter() {
            if environment.water_deficit.get(key) != Some(value) {
                changed.insert(IVec3::new(key.0, key.1, key.2));
            }
        }
        for p in changed { blocks.insert(p);world::mark_chunk_and_neighbors_dirty(chunks,p); }
        w.industry=industry;
        w.manyfold_weight=self.manyfold_weight;
        let location = std::mem::take(&mut w.adventure.location);
        w.adventure=self.adventure;w.inventory=self.inventory;w.expedition=self.expedition;
        w.adventure.location = location;
        w.progression = progression;
        world::progression::study::restore_spells(w);
        w.factory=self.factory;w.factory.chips=chips;w.undercrown=undercrown;w.sandbox=sandbox;
        w.orbital=orbital;w.habitats=habitats;w.environment=environment;w.activity=activity;
        w.water_levels=levels;
        if let Some(farming)=farming {w.farming=farming;}
        if let Some(travel)=travel {w.travel=travel;}
        if let Some(caches)=caches {w.caches=caches;}
        if let Some(block_data)=block_data {w.block_data=block_data;}
        if let Some(rules)=rules {w.rules=rules;}
        // Movement state stays with the joiner, which steps its own body.
        p.scale=self.body.scale;p.viewmodel=self.body.viewmodel;
        p.traversal.fuel_seconds=self.body.traversal.fuel_seconds;
        p.traversal.charge_fe=self.body.traversal.charge_fe;
        *day=self.day;
        for other in self.others {w.adventure.merge_multiplayer_visuals(other);}
        w.sync_from_inventory();
        Ok(())
    }
}

pub struct SavedPlayer {
    pub player: Player,
    pub personal: Personal,
}

pub fn load_player(bytes: &[u8]) -> Result<SavedPlayer> {
    #[derive(Deserialize)]
    struct Record {
        player: Player,
        personal: Personal,
        #[serde(default)]
        progression: Vec<u8>,
    }
    let Record { player, mut personal, progression } = unpack(bytes)?;
    if !progression.is_empty() {
        personal.progression = world::progression::sidecar::decode(&progression)?;
    }
    Ok(SavedPlayer { player, personal })
}

pub fn save_player(player:&Player,personal:&Personal)->Result<Vec<u8>> {
    #[derive(Serialize)]
    struct Ref<'a> {
        player: &'a Player,
        personal: &'a Personal,
        progression: Vec<u8>,
    }
    pack(&Ref {
        player,
        personal,
        progression: world::progression::sidecar::encode(&personal.progression, &personal.inventory),
    })
}

pub fn profile()->Result<String> {
    if let Some(token)=std::env::args().find_map(|a|a.strip_prefix("--multiplayer-profile=").map(str::to_owned)) {
        ensure!(valid_profile(&token),"profile must contain 32 hexadecimal characters");return Ok(token);
    }
    let path=world::storage::save_directory().join("multiplayer-profile");
    if path.exists() {let token=std::fs::read_to_string(&path)?;let token=token.trim().to_owned();ensure!(valid_profile(&token),"invalid multiplayer profile file");return Ok(token);}
    let mut random=[0u8;16];getrandom::fill(&mut random).map_err(|e|anyhow::anyhow!("profile random source: {e}"))?;
    let token:String=random.iter().map(|b|format!("{b:02x}")).collect();
    if let Some(parent)=path.parent(){std::fs::create_dir_all(parent)?;}
    let mut file=private_file(&path)?;
    file.write_all(token.as_bytes())?;file.sync_all()?;Ok(token)
}
/// Creates a secret file readable only by its owner. Windows profile folders already are.
fn private_file(path:&std::path::Path)->Result<std::fs::File> {
    let mut options=std::fs::OpenOptions::new();options.write(true).create_new(true);
    #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
    Ok(options.open(path)?)
}
pub fn valid_profile(token:&str)->bool {token.len()==32 && token.bytes().all(|b|b.is_ascii_hexdigit())}

/// Name shown to other players: `--multiplayer-name=`, then the saved name, then the OS user.
pub fn player_name()->String {
    let saved=||std::fs::read_to_string(world::storage::save_directory().join("multiplayer-name")).ok();
    let candidates=std::env::args().find_map(|a|a.strip_prefix("--multiplayer-name=").map(str::to_owned)).into_iter()
        .chain(saved()).chain(std::env::var("USERNAME").ok()).chain(std::env::var("USER").ok());
    for raw in candidates {
        let cleaned:String=raw.trim().chars().filter(|c|c.is_ascii_alphanumeric()||matches!(c,'_'|'-'|'.'|' ')).take(16).collect();
        if let Ok(name)=engine_net::social::sanitize_name(&cleaned) {return name;}
    }
    "Player".to_owned()
}

/// Remembers a chosen name for later sessions.
pub fn save_player_name(name:&str)->Result<()> {
    let name=engine_net::social::sanitize_name(name).map_err(|e|anyhow::anyhow!(e))?;
    let dir=world::storage::save_directory();std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("multiplayer-name"),name)?;Ok(())
}

/// This install's signing key. Created once; the host keys bans and operators on it.
pub fn player_key()->Result<engine_net::identity::PlayerKey> {
    use engine_net::identity::PlayerKey;
    let path=world::storage::save_directory().join("multiplayer-key");
    if path.exists() {
        let text=std::fs::read_to_string(&path)?;let text=text.trim();
        ensure!(text.len()==64 && text.bytes().all(|b|b.is_ascii_hexdigit()),"invalid multiplayer key file");
        let mut secret=[0u8;32];
        for (i,byte) in secret.iter_mut().enumerate() {*byte=u8::from_str_radix(&text[i*2..i*2+2],16)?;}
        return Ok(PlayerKey::from_bytes(secret));
    }
    let key=PlayerKey::generate().map_err(|e|anyhow::anyhow!("player key: {e}"))?;
    let text:String=key.to_bytes().iter().map(|b|format!("{b:02x}")).collect();
    if let Some(parent)=path.parent(){std::fs::create_dir_all(parent)?;}
    let mut file=private_file(&path)?;
    file.write_all(text.as_bytes())?;file.sync_all()?;Ok(key)
}

#[cfg(test)]
#[path = "personal_inventory_tests.rs"]
mod personal_inventory_tests;
