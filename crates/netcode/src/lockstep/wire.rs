//! Byte form of the lockstep messages. The payload codec is the caller's; counts are bounded by
//! the session `Config`, so a hostile message cannot make the reader allocate past it.
use super::{ClientMsg, Config, Frame, HostMsg, Input, PeerId};
use crate::wire::{Reader, WireError, put_u64};

pub fn write_host_msg<T>(msg: &HostMsg<T>, out: &mut Vec<u8>, mut payload: impl FnMut(&T, &mut Vec<u8>)) {
    put_u64(out, msg.sealed_until);
    put_u64(out, u64::from(msg.input_ack));
    put_u64(out, u64::from(msg.resync));
    put_u64(out, msg.frames.len() as u64);
    let Some(first) = msg.frames.first() else { return };
    put_u64(out, first.tick);
    let mut order = 0u64;
    for frame in &msg.frames {
        put_u64(out, frame.inputs.len() as u64);
        for input in &frame.inputs {
            put_u64(out, u64::from(input.peer));
            put_u64(out, u64::from(input.sequence));
            // Orders only grow, so the step from the previous one is small.
            put_u64(out, input.order.wrapping_sub(order));
            order = input.order;
            payload(&input.payload, out);
        }
    }
}

pub fn read_host_msg<T>(
    bytes: &[u8],
    config: &Config,
    mut payload: impl FnMut(&mut Reader) -> Result<T, WireError>,
) -> Result<HostMsg<T>, WireError> {
    let mut r = Reader::new(bytes);
    let sealed_until = r.u64()?;
    let input_ack = r.u32()?;
    let resync = match r.u64()? {
        0 => false,
        1 => true,
        _ => return Err(WireError("resync flag")),
    };
    let count = r.u64()?;
    if count > config.frames_per_message as u64 {
        return Err(WireError("frame count"));
    }
    let mut frames = Vec::with_capacity(count as usize);
    if count > 0 {
        let first = r.u64()?;
        if first.checked_add(count).is_none_or(|end| end > sealed_until) {
            return Err(WireError("frame tick"));
        }
        let mut order = 0u64;
        for tick in first..first + count {
            let n = r.u64()?;
            if n > config.inputs_per_frame as u64 {
                return Err(WireError("input count"));
            }
            let mut inputs = Vec::with_capacity(n as usize);
            for _ in 0..n {
                let peer = PeerId::try_from(r.u64()?).map_err(|_| WireError("peer id"))?;
                let sequence = r.u32()?;
                order = order.wrapping_add(r.u64()?);
                inputs.push(Input { peer, sequence, order, payload: payload(&mut r)? });
            }
            frames.push(Frame { tick, inputs });
        }
    }
    if !r.done() {
        return Err(WireError("trailing bytes"));
    }
    Ok(HostMsg { sealed_until, input_ack, resync, frames })
}

pub fn write_client_msg<T>(msg: &ClientMsg<T>, out: &mut Vec<u8>, mut payload: impl FnMut(&T, &mut Vec<u8>)) {
    put_u64(out, msg.frames_ack);
    put_u64(out, u64::from(msg.first_sequence));
    put_u64(out, msg.inputs.len() as u64);
    msg.inputs.iter().for_each(|p| payload(p, out));
}

pub fn read_client_msg<T>(
    bytes: &[u8],
    config: &Config,
    mut payload: impl FnMut(&mut Reader) -> Result<T, WireError>,
) -> Result<ClientMsg<T>, WireError> {
    let mut r = Reader::new(bytes);
    let frames_ack = r.u64()?;
    let first_sequence = r.u32()?;
    let count = r.u64()?;
    if count > config.inputs_per_message as u64 {
        return Err(WireError("input count"));
    }
    let inputs = (0..count).map(|_| payload(&mut r)).collect::<Result<Vec<T>, _>>()?;
    if !r.done() {
        return Err(WireError("trailing bytes"));
    }
    Ok(ClientMsg { frames_ack, first_sequence, inputs })
}
