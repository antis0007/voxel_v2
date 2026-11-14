//! One connection contract for network compilation, visible conduit arms and UI.
//! Capabilities come from the authored machine catalogue and process emissions.
//! This is derived data; saved inventories, charge and pipe traffic keep their owners.
use super::Kind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Domain { Items, Power, Fluid, Gas }

impl Domain {
    pub const ALL: [Self; 4] = [Self::Items, Self::Power, Self::Fluid, Self::Gas];
    pub fn name(self) -> &'static str {
        match self { Self::Items => "Items", Self::Power => "Power", Self::Fluid => "Fluid", Self::Gas => "Gas" }
    }
}

pub fn conduit(kind: Kind) -> Option<Domain> {
    match kind {
        Kind::Pipe => Some(Domain::Items),
        Kind::Cable => Some(Domain::Power),
        Kind::FluidPipe => Some(Domain::Fluid),
        Kind::ExhaustDuct => Some(Domain::Gas),
        _ => None,
    }
}

pub fn supports(kind: Kind, domain: Domain) -> bool {
    static MASKS: std::sync::OnceLock<[u8; Kind::COUNT]> = std::sync::OnceLock::new();
    let masks = MASKS.get_or_init(|| std::array::from_fn(|index| {
        let kind = Kind::ALL[index];
        if let Some(domain) = conduit(kind) { return 1 << domain as u8; }
        if kind.structural() { return 0; }
        let mut mask = 1 << Domain::Items as u8;
        if kind.powered() || kind.stores_power() || kind.generator() { mask |= 1 << Domain::Power as u8; }
        if kind.water_port() { mask |= 1 << Domain::Fluid as u8; }
        if super::industry::port(kind) { mask |= 1 << Domain::Gas as u8; }
        mask
    }));
    masks[kind as usize] & (1 << domain as u8) != 0
}

pub fn accepts(connector: Kind, endpoint: Kind) -> bool {
    conduit(connector).is_some_and(|domain| supports(endpoint, domain))
}

pub fn names(kind: Kind) -> String {
    Domain::ALL.into_iter().filter(|d| supports(kind, *d)).map(Domain::name).collect::<Vec<_>>().join(" · ")
}
