//! Steam carries only the candidate exchange and the invitation UI.
use super::{NetResult, Runtime, identity, pump, runtime};
use crate::punch::{CODE_LIMIT, Offer};
use std::{sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc}, time::{Duration, Instant}};
use steamworks::{CallbackHandle, GameRichPresenceJoinRequested, SteamId,
    networking_messages::NetworkingMessages, networking_types::{NetworkingIdentity, SendFlags}};

pub const LAUNCH_ARG: &str = "--multiplayer-invite=";
const CHANNEL: u32 = 42;
const RETRY: Duration = Duration::from_secs(2);

fn messages(sdk: &Runtime) -> NetworkingMessages {
    sdk.0.server.as_ref().map_or_else(|| sdk.networking_messages(), |server| server.networking_messages())
}

pub(crate) struct Host {
    sdk: Runtime,
    accepting: Arc<AtomicBool>,
}

impl Host {
    pub(crate) fn start() -> NetResult<Self> {
        let sdk = runtime()?;
        let accepting = sdk.0.invite_listener.get_or_init(|| {
            let accepting = Arc::new(AtomicBool::new(false));
            let enabled = Arc::clone(&accepting);
            messages(&sdk).session_request_callback(move |request| {
                if enabled.load(Ordering::Relaxed) && request.remote().steam_id().is_some() {
                    request.accept();
                }
            });
            accepting
        }).clone();
        accepting.store(true, Ordering::Relaxed);
        Ok(Self { sdk, accepting })
    }

    pub(crate) fn id(&self) -> u64 { identity(&self.sdk) }

    pub(crate) fn receive(&self, mut accept: impl FnMut(&str)) {
        for message in messages(&self.sdk).receive_messages_on_channel(CHANNEL, 16) {
            let bytes = message.data();
            if message.identity_peer().steam_id().is_some() && bytes.len() <= CODE_LIMIT
                && let Ok(reply) = std::str::from_utf8(bytes)
            {
                accept(reply);
            }
        }
    }
}

impl Drop for Host {
    fn drop(&mut self) { self.accepting.store(false, Ordering::Relaxed); }
}

pub(crate) struct Guest {
    sdk: Runtime,
    host: u64,
    next_send: Instant,
}

impl Guest {
    pub(crate) fn start(host: u64) -> NetResult<Self> {
        Ok(Self { sdk: runtime()?, host, next_send: Instant::now() })
    }

    pub(crate) fn send(&mut self, reply: &str) -> NetResult<()> {
        let now = Instant::now();
        if now >= self.next_send && self.host != identity(&self.sdk) {
            self.next_send = now + RETRY;
            let _ = messages(&self.sdk).send_message_to_user(
                NetworkingIdentity::new_steam_id(SteamId::from_raw(self.host)),
                SendFlags::RELIABLE | SendFlags::AUTO_RESTART_BROKEN_SESSION,
                reply.as_bytes(), CHANNEL,
            );
        }
        pump(&self.sdk)
    }
}

pub fn open_overlay(code: &str) -> NetResult<()> {
    if Offer::decode(code)?.signaling_host().is_none() {
        return Err("Invalid Steam invite".into());
    }
    let sdk = runtime()?;
    if !sdk.utils().is_overlay_enabled() { return Err("Steam Overlay Unavailable".into()); }
    let connect = format!("{LAUNCH_ARG}{code}");
    if connect.len() >= 256 { return Err("Steam Invite Too Long".into()); }
    sdk.friends().activate_invite_dialog_connect_string(&connect);
    Ok(())
}

pub struct Incoming {
    _callback: CallbackHandle,
    sdk: Runtime,
    invites: mpsc::Receiver<String>,
    next_pump: Instant,
}

impl Incoming {
    pub fn start() -> NetResult<Self> {
        let sdk = runtime()?;
        let (send, invites) = mpsc::sync_channel(1);
        let callback = sdk.register_callback(move |event: GameRichPresenceJoinRequested| {
            if let Some(code) = event.connect.strip_prefix(LAUNCH_ARG)
                && let Ok(offer) = Offer::decode(code)
                && offer.signaling_host() == Some(event.friend_steam_id.raw())
            {
                let _ = send.try_send(code.to_owned());
            }
        });
        Ok(Self { _callback: callback, sdk, invites, next_pump: Instant::now() })
    }

    pub fn poll(&mut self, now: Instant) -> Option<String> {
        if now >= self.next_pump {
            self.next_pump = now + Duration::from_millis(200);
            let _ = pump(&self.sdk);
        }
        self.invites.try_recv().ok()
    }
}
