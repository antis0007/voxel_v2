//! Sound events are sent once, only to nearby listeners, outside state images.
use super::*;
const RANGE: f32 = 64.0;
const QUEUE: usize = 32;

#[derive(Clone,Copy,serde::Serialize,serde::Deserialize)]
pub struct ContactEvent { kind:crate::audio::Contact, surface: crate::audio::Surface, position: Vec3, gain: f32, seed: u32 }

impl Session {
    pub fn play_contacts(&mut self, audio: &mut crate::audio::Audio, world: &World, player: &Player) {
        for event in self.contacts.drain(..) {
            crate::soundscape::contact_event(audio,world,event.kind,event.surface,Some(event.position),event.gain,event.seed,
                player.eye(world.voxel_scale)*world.voxel_scale.meters(),player.forward_dir());
        }
    }
}

pub fn steps(world: &World, host: &Player, actors: &BTreeMap<PlayerId,Actor>, outputs: &mut BTreeMap<PlayerId,Output>,
    headless: bool, positions: &mut BTreeMap<PlayerId,(Vec3,f32)>, local: &mut Vec<ContactEvent>) {
    let scale=world.voxel_scale.meters();
    positions.retain(|id,_|(*id==HOST&&!headless)||actors.contains_key(id));
    for (id,body) in (!headless).then_some((HOST,host)).into_iter().chain(actors.iter().map(|(&id,actor)|(id,&actor.player))) {
        let (previous,accumulated)=positions.entry(id).or_insert((body.pos,0.0));
        let distance=(body.pos-*previous).with_y(0.0).length()*scale;
        *previous=body.pos;
        let block=world.get((body.pos-Vec3::Y*(0.05/scale)).floor().as_ivec3());
        if body.flying||block.is_empty()||distance>4.0 { *accumulated=0.0; continue; }
        *accumulated+=distance;
        if *accumulated<crate::soundscape::footsteps::STEP_SPACING_M {continue;}
        *accumulated%=crate::soundscape::footsteps::STEP_SPACING_M;
        let event=ContactEvent {kind:crate::audio::Contact::Step,surface:crate::soundscape::footsteps::surface(block),position:body.pos*scale,
            gain:if body.crouch_t>0.5{0.3}else{1.0},seed:crate::audio::hash(id,body.pos.x.to_bits()^body.pos.z.to_bits())};
        if !headless&&id!=HOST&&event.position.distance_squared(host.pos*scale)<32.0*32.0&&local.len()<128 {local.push(event);}
        for (&recipient,output) in outputs.iter_mut().filter(|(_,output)|output.ready) {
            if recipient!=id&&actors.get(&recipient).is_some_and(|actor|event.position.distance_squared(actor.player.pos*scale)<32.0*32.0) {
                if output.contacts.len()>=QUEUE {output.contacts.pop_front();}
                output.contacts.push_back(event);
            }
        }
    }
}

pub fn block(world: &mut World, cell: IVec3, block: BlockId, kind:crate::audio::Contact) {
    let position=(cell.as_vec3()+Vec3::splat(0.5))*world.voxel_scale.meters();
    if let Some(context)=world.multiplayer.as_mut()&&context.contacts.len()<16 {
        context.contacts.push((context.actor,ContactEvent {kind,surface:crate::soundscape::footsteps::surface(block),position,gain:1.0,
            seed:crate::audio::hash(context.actor,cell.x as u32^(cell.z as u32).rotate_left(16))}));
    }
}

pub fn contacts(world: &World, host: &Player, actors:&BTreeMap<PlayerId,Actor>, outputs:&mut BTreeMap<PlayerId,Output>,
    headless: bool, events:Vec<(PlayerId,ContactEvent)>, local:&mut Vec<ContactEvent>) {
    let scale=world.voxel_scale.meters();
    for (source,event) in events {
        if !headless&&(source!=HOST||event.kind!=crate::audio::Contact::Step)&&event.position.distance_squared(host.pos*scale)<32.0*32.0&&local.len()<128 {local.push(event);}
        for (&id,output) in outputs.iter_mut().filter(|(_,output)|output.ready) {
            if (id!=source||event.kind!=crate::audio::Contact::Step)&&actors.get(&id).is_some_and(|actor|event.position.distance_squared(actor.player.pos*scale)<32.0*32.0) {
                if output.contacts.len()>=QUEUE {output.contacts.pop_front();}
                output.contacts.push_back(event);
            }
        }
    }
}

pub fn receive_contacts(local: &mut Vec<ContactEvent>, bytes: &[u8]) -> Result<()> {
    let events:Vec<ContactEvent>=decode(bytes)?;
    ensure!(events.len()<=4,"contact audio limit");
    for event in events {
        ensure!(event.position.is_finite()&&event.position.abs().max_element()<=1_000_000.0
            &&event.gain.is_finite()&&(0.0..=1.0).contains(&event.gain),"invalid contact audio");
        if local.len()<128 {local.push(event);}
    }
    Ok(())
}

pub fn collect(world: &mut World, feet: Vec3, events: &mut Vec<crate::audio::Event>) {
    for mut event in std::mem::take(&mut world.adventure.sounds) {
        if events.len() >= 256 { break; }
        if event.pos.is_none() { event.pos = Some(feet*world.voxel_scale.meters()); }
        events.push(event);
    }
}

pub fn distribute(world: &mut World, player: &Player, actors: &BTreeMap<PlayerId, Actor>,
    outputs: &mut BTreeMap<PlayerId, Output>, headless: bool, events: Vec<crate::audio::Event>) {
    let scale = world.voxel_scale.meters();
    for event in events {
        let nearby = |position: Vec3| event.pos.is_none_or(|source| source.distance_squared(position*scale)<=RANGE*RANGE);
        if !headless && nearby(player.pos) && world.adventure.sounds.len()<128 { world.adventure.sounds.push(event); }
        for (&id, output) in outputs.iter_mut().filter(|(_,output)|output.ready) {
            if actors.get(&id).is_some_and(|actor|nearby(actor.player.pos)) {
                if output.sounds.len()>=QUEUE { output.sounds.pop_front(); }
                output.sounds.push_back(event);
            }
        }
    }
}

pub fn flush(socket: &Host, id: PlayerId, output: &mut Output) -> Result<()> {
    let contacts:Vec<_>=output.contacts.iter().take(4).copied().collect();
    if !contacts.is_empty()&&socket.send_data(id,encode(26,&contacts)?).map_err(|error|anyhow::anyhow!(error.to_string()))? {
        output.contacts.drain(..contacts.len());
    }
    let batch: Vec<_> = output.sounds.iter().take(4).copied().collect();
    if !batch.is_empty() && socket.send_data(id, encode(25,&batch)?).map_err(|error|anyhow::anyhow!(error.to_string()))? {
        output.sounds.drain(..batch.len());
    }
    Ok(())
}

pub fn receive(world: &mut World, bytes: &[u8]) -> Result<()> {
    let events: Vec<crate::audio::Event> = decode(bytes)?;
    ensure!(events.len()<=4, "audio event limit");
    for event in events {
        ensure!(event.pos.is_none_or(|position|position.is_finite()&&position.abs().max_element()<=1_000_000.0)
            && event.gain.is_finite() && (0.0..=4.0).contains(&event.gain)
            && event.pitch.is_finite() && (0.1..=8.0).contains(&event.pitch), "invalid audio event");
        if world.adventure.sounds.len()<128 { world.adventure.sounds.push(event); }
    }
    Ok(())
}
