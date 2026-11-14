use super::{Decoder, Player, push};
use crate::planet::influence::{Anchor, system};
use anyhow::{Result, ensure};
use engine_space::BodyId;
use glam::{DQuat, DVec3, Quat, Vec3};

/// Seven motion words follow the common 88-byte header, then the anchor tag.
pub(super) const ANCHOR_TAG_OFFSET: usize = 88 + 7 * size_of::<f32>();
pub(super) const HEADER_BYTES: usize = 8 * size_of::<u32>();
pub(super) const ANCHOR_BYTES: usize = 10 * size_of::<f64>();

fn valid_anchor(anchor: Anchor) -> bool {
    system().is_some_and(|system| {
        usize::from(anchor.body.0) < system.len()
            && system.id("home") != Some(anchor.body)
            && system.parent(anchor.body).is_some()
            && system
                .soi(anchor.body)
                .is_some_and(|soi| anchor.pos.length() <= soi * 4.0)
            && anchor.pos.length() >= system.radius(anchor.body) * 0.5
    }) && anchor.vel.is_finite()
        && anchor.upright.is_finite()
        && (anchor.upright.length_squared() - 1.0).abs() < 0.001
}

pub(super) fn encode(player: &Player, bytes: &mut Vec<u8>) -> Result<()> {
    ensure!(
        player.vel.is_finite() && (player.gravity_frame.length_squared() - 1.0).abs() < 0.001,
        "Invalid planetary player motion"
    );
    for value in player
        .vel
        .to_array()
        .into_iter()
        .chain(player.gravity_frame.to_array())
    {
        push(bytes, value.to_bits());
    }
    push(bytes, player.anchor.map_or(0, |a| u32::from(a.body.0) + 1));
    if let Some(anchor) = player.anchor {
        ensure!(valid_anchor(anchor), "Invalid planetary body anchor");
        for value in anchor
            .pos
            .to_array()
            .into_iter()
            .chain(anchor.vel.to_array())
            .chain(anchor.upright.to_array())
        {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    Ok(())
}

pub(super) fn decode(input: &mut Decoder<'_>, player: &mut Player) -> Result<()> {
    player.vel = Vec3::new(input.float()?, input.float()?, input.float()?);
    player.gravity_frame = Quat::from_xyzw(
        input.float()?,
        input.float()?,
        input.float()?,
        input.float()?,
    );
    ensure!(
        (player.gravity_frame.length_squared() - 1.0).abs() < 0.001,
        "Invalid planetary upright frame"
    );
    let tag = input.u32()?;
    if tag != 0 {
        let mut f = || -> Result<f64> { Ok(f64::from_le_bytes(input.take(8)?.try_into()?)) };
        let anchor = Anchor {
            body: BodyId(u16::try_from(tag - 1)?),
            pos: DVec3::new(f()?, f()?, f()?),
            vel: DVec3::new(f()?, f()?, f()?),
            upright: DQuat::from_xyzw(f()?, f()?, f()?, f()?),
        };
        ensure!(valid_anchor(anchor), "Invalid planetary body anchor");
        player.anchor = Some(anchor);
    }
    Ok(())
}
