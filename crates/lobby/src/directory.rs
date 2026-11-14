//! Session directory with filtered, top-k browse. Browse costs one pass over the sessions plus a
//! partial selection of the page, so a large list does not need a full sort per query.

use crate::random;
use engine_net::lobby::wire::{Entry, HostInfo, Query, Sort};
use std::{cmp::Ordering, collections::HashMap, io, net::IpAddr, time::Instant};

const CODE_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const CODE_CHARACTERS: usize = 8;
const CODE_ATTEMPTS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_sessions: usize,
    pub max_per_address: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterError {
    Full,
    TooManyFromAddress,
    CodeBusy,
    /// The operating system could not supply randomness, so no code is issued.
    Entropy,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub id: u32,
    pub code: String,
    pub info: HostInfo,
    pub players: u8,
    created: Instant,
    seq: u64,
    lower_name: String,
    lower_world: String,
    owner: IpAddr,
}

pub struct Directory {
    sessions: HashMap<u32, Session>,
    by_code: HashMap<String, u32>,
    per_address: HashMap<IpAddr, usize>,
    limits: Limits,
    next_id: u32,
    sequence: u64,
}

impl Directory {
    pub fn new(limits: Limits) -> Self {
        Self {
            sessions: HashMap::new(),
            by_code: HashMap::new(),
            per_address: HashMap::new(),
            limits,
            next_id: 1,
            sequence: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Draws codes from the operating system's CSPRNG. Each code is 40 bits; an unlisted session is
    /// hidden from browse and guessing is rate-limited per address by the server.
    fn fresh_code(&mut self) -> io::Result<Option<String>> {
        for _ in 0..CODE_ATTEMPTS {
            let mut entropy = [0u8; 8];
            random::fill(&mut entropy)?;
            let bits = u64::from_le_bytes(entropy) & ((1u64 << (5 * CODE_CHARACTERS)) - 1);
            let mut code = String::with_capacity(CODE_CHARACTERS + 1);
            for index in 0..CODE_CHARACTERS {
                if index == CODE_CHARACTERS / 2 {
                    code.push('-');
                }
                let shift = 5 * (CODE_CHARACTERS - 1 - index);
                code.push(char::from(CODE_ALPHABET[((bits >> shift) & 31) as usize]));
            }
            if !self.by_code.contains_key(&code) {
                return Ok(Some(code));
            }
        }
        Ok(None)
    }

    pub fn register(&mut self, owner: IpAddr, info: &HostInfo, now: Instant) -> Result<(u32, String), RegisterError> {
        if self.sessions.len() >= self.limits.max_sessions {
            return Err(RegisterError::Full);
        }
        if self.per_address.get(&owner).copied().unwrap_or(0) >= self.limits.max_per_address {
            return Err(RegisterError::TooManyFromAddress);
        }
        let code = match self.fresh_code() {
            Ok(Some(code)) => code,
            Ok(None) => return Err(RegisterError::CodeBusy),
            Err(_) => return Err(RegisterError::Entropy),
        };
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.sequence += 1;
        let session = Session {
            id,
            code: code.clone(),
            info: info.clone(),
            players: 1,
            created: now,
            seq: self.sequence,
            lower_name: info.name.to_lowercase(),
            lower_world: info.world.to_lowercase(),
            owner,
        };
        self.sessions.insert(id, session);
        self.by_code.insert(code.clone(), id);
        *self.per_address.entry(owner).or_insert(0) += 1;
        Ok((id, code))
    }

    pub fn remove(&mut self, id: u32) {
        if let Some(session) = self.sessions.remove(&id) {
            self.by_code.remove(&session.code);
            if let Some(count) = self.per_address.get_mut(&session.owner) {
                *count -= 1;
                if *count == 0 {
                    self.per_address.remove(&session.owner);
                }
            }
        }
    }

    pub fn set_players(&mut self, id: u32, players: u8) {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.players = players;
        }
    }

    pub fn session(&self, id: u32) -> Option<&Session> {
        self.sessions.get(&id)
    }

    /// Finds a session by invite code. Accepts lowercase, spaces, missing dashes, and O/I/L for 0/1.
    pub fn find_code(&self, code: &str) -> Option<&Session> {
        let canonical = canonical_code(code)?;
        self.by_code.get(&canonical).and_then(|id| self.sessions.get(id))
    }

    /// Returns the total number of matches and the requested page, best first.
    pub fn browse(&self, query: &Query, now: Instant) -> (u32, Vec<Entry>) {
        let terms: Vec<String> = query.text.to_lowercase().split_whitespace().map(str::to_owned).collect();
        let mut found: Vec<&Session> = self.sessions.values().filter(|s| matches(s, query, &terms)).collect();
        let total = u32::try_from(found.len()).unwrap_or(u32::MAX);
        let offset = usize::from(query.offset);
        let want = offset + usize::from(query.limit);
        if want == 0 || offset >= found.len() {
            return (total, Vec::new());
        }
        let order = |a: &&Session, b: &&Session| order(query.sort, a, b);
        if found.len() > want {
            found.select_nth_unstable_by(want - 1, order);
            found.truncate(want);
        }
        found.sort_unstable_by(order);
        let page = found.into_iter().skip(offset).map(|s| entry(s, now)).collect();
        (total, page)
    }
}

/// Canonical `XXXX-XXXX` form of a typed code, or `None` when the characters cannot be a code.
pub fn canonical_code(code: &str) -> Option<String> {
    let mut digits = String::with_capacity(CODE_CHARACTERS + 1);
    for c in code.chars().filter(|c| !matches!(c, '-' | ' ')) {
        let c = match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            upper => upper,
        };
        // `as u8` truncates, so a non-ASCII char could pass as a letter and later
        // split a slice mid-character.
        if !c.is_ascii() || !CODE_ALPHABET.contains(&(c as u8)) {
            return None;
        }
        digits.push(c);
    }
    if digits.len() != CODE_CHARACTERS {
        return None;
    }
    Some(format!("{}-{}", &digits[..CODE_CHARACTERS / 2], &digits[CODE_CHARACTERS / 2..]))
}

fn matches(session: &Session, query: &Query, terms: &[String]) -> bool {
    !session.info.unlisted
        && query.mode.is_none_or(|mode| session.info.mode == mode)
        && query.version.is_none_or(|version| session.info.version == version)
        && (query.include_full || session.players < session.info.max_players)
        && terms.iter().all(|term| session.lower_name.contains(term.as_str()) || session.lower_world.contains(term.as_str()))
}

fn order(sort: Sort, a: &Session, b: &Session) -> Ordering {
    let by_id = a.id.cmp(&b.id);
    match sort {
        Sort::Players => b.players.cmp(&a.players).then(b.seq.cmp(&a.seq)).then(by_id),
        Sort::Newest => b.seq.cmp(&a.seq).then(by_id),
        Sort::Name => a.lower_name.cmp(&b.lower_name).then(b.seq.cmp(&a.seq)).then(by_id),
    }
}

fn entry(session: &Session, now: Instant) -> Entry {
    Entry {
        code: session.code.clone(),
        name: session.info.name.clone(),
        world: session.info.world.clone(),
        mode: session.info.mode,
        version: session.info.version,
        players: session.players,
        max_players: session.info.max_players,
        age_secs: u32::try_from(now.saturating_duration_since(session.created).as_secs()).unwrap_or(u32::MAX),
        motd: session.info.motd.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_net::lobby::wire::{Mode, WorldParams};
    use std::net::Ipv4Addr;

    fn limits() -> Limits {
        Limits { max_sessions: 64, max_per_address: 4 }
    }

    fn host(name: &str, world: &str, mode: Mode, version: u32, max: u8) -> HostInfo {
        HostInfo { name: name.into(), world: world.into(), mode, version, digest: [1; 32], max_players: max, unlisted: false, world_params: WorldParams { seed: 1, kind: 0, scale: 0 }, motd: "notice".into() }
    }

    fn query(text: &str) -> Query {
        Query { text: text.into(), mode: None, version: None, include_full: true, sort: Sort::Players, offset: 0, limit: 32 }
    }

    fn addr(last: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, last))
    }

    #[test]
    fn codes_are_eight_characters_and_canonicalize() {
        let mut dir = Directory::new(limits());
        let (_, code) = dir.register(addr(1), &host("a", "b", Mode::Survival, 1, 4), Instant::now()).unwrap();
        assert_eq!(code.len(), 9);
        assert_eq!(&code[4..5], "-");
        assert!(dir.find_code(&code.to_lowercase()).is_some());
        assert!(dir.find_code(&code.replace('-', " ")).is_some());
        assert_eq!(canonical_code("abcdefgh").as_deref(), Some("ABCD-EFGH"));
        assert_eq!(canonical_code("0O1IL2"), None, "six characters is not a code");
        assert_eq!(canonical_code("ABCD-EFG@"), None);
        // U+0141 truncates to 'A' under `as u8`; it must be refused, not sliced.
        assert_eq!(canonical_code("AAA\u{141}AAA"), None);
    }

    #[test]
    fn text_search_matches_every_term_across_name_and_world() {
        let mut dir = Directory::new(limits());
        let now = Instant::now();
        dir.register(addr(1), &host("Riverside", "Hills 0.5 m", Mode::Survival, 1, 8), now).unwrap();
        dir.register(addr(2), &host("Mountain camp", "Ridged", Mode::Survival, 1, 8), now).unwrap();
        let (total, entries) = dir.browse(&query("RIVER hills"), now);
        assert_eq!(total, 1);
        assert_eq!(entries[0].name, "Riverside");
        assert_eq!(dir.browse(&query("river mountain"), now).0, 0, "terms must all match");
        assert_eq!(dir.browse(&query(""), now).0, 2);
    }

    #[test]
    fn filters_hide_full_unlisted_and_other_modes_and_versions() {
        let mut dir = Directory::new(limits());
        let now = Instant::now();
        let (full, _) = dir.register(addr(1), &host("full", "w", Mode::Survival, 1, 2), now).unwrap();
        dir.set_players(full, 2);
        dir.register(addr(2), &host("creative", "w", Mode::Creative, 2, 8), now).unwrap();
        let mut hidden = host("hidden", "w", Mode::Survival, 1, 8);
        hidden.unlisted = true;
        let (secret, secret_code) = dir.register(addr(3), &hidden, now).unwrap();
        let mut q = query("");
        q.include_full = false;
        let names: Vec<_> = dir.browse(&q, now).1.into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["creative"]);
        q.include_full = true;
        assert_eq!(dir.browse(&q, now).0, 2, "full sessions shown on request, unlisted never");
        q.mode = Some(Mode::Creative);
        assert_eq!(dir.browse(&q, now).0, 1);
        q.mode = None;
        q.version = Some(1);
        assert_eq!(dir.browse(&q, now).0, 1);
        assert_eq!(dir.session(secret).map(|s| s.info.name.as_str()), Some("hidden"));
        assert!(dir.find_code(&secret_code).is_some(), "unlisted sessions still join by code");
    }

    #[test]
    fn sort_orders_and_pages_are_stable() {
        let mut dir = Directory::new(limits());
        let now = Instant::now();
        let (a, _) = dir.register(addr(1), &host("alpha", "w", Mode::Survival, 1, 16), now).unwrap();
        let (b, _) = dir.register(addr(2), &host("bravo", "w", Mode::Survival, 1, 16), now).unwrap();
        let (c, _) = dir.register(addr(3), &host("charlie", "w", Mode::Survival, 1, 16), now).unwrap();
        dir.set_players(b, 5);
        dir.set_players(c, 5);
        dir.set_players(a, 1);
        let mut q = query("");
        q.sort = Sort::Players;
        let names: Vec<_> = dir.browse(&q, now).1.into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["charlie", "bravo", "alpha"], "players desc, newer first on ties");
        q.sort = Sort::Newest;
        assert_eq!(dir.browse(&q, now).1[0].name, "charlie");
        q.sort = Sort::Name;
        assert_eq!(dir.browse(&q, now).1[0].name, "alpha");
        q.sort = Sort::Players;
        q.offset = 1;
        q.limit = 1;
        let (total, page) = dir.browse(&q, now);
        assert_eq!((total, page.len(), page[0].name.as_str()), (3, 1, "bravo"));
        q.offset = 9;
        assert!(dir.browse(&q, now).1.is_empty());
    }

    #[test]
    fn top_k_matches_a_full_sort_on_a_large_directory() {
        let mut dir = Directory::new(Limits { max_sessions: 1000, max_per_address: 1000 });
        let now = Instant::now();
        for n in 0..600u32 {
            let (id, _) = dir.register(addr(0), &host(&format!("s{n:03}"), "w", Mode::Survival, 1, 16), now).unwrap();
            dir.set_players(id, ((n * 7) % 16 + 1) as u8);
        }
        let mut q = query("");
        q.limit = 32;
        q.offset = 40;
        let paged: Vec<_> = dir.browse(&q, now).1.into_iter().map(|e| e.code).collect();
        // Brute-force reference: sort every session, no top-k cut.
        let mut all: Vec<&Session> = dir.sessions.values().collect();
        all.sort_by(|a, b| order(Sort::Players, a, b));
        let expected: Vec<_> = all[40..72].iter().map(|s| s.code.clone()).collect();
        assert_eq!(paged, expected);
    }

    #[test]
    fn limits_and_removal_free_capacity() {
        let mut dir = Directory::new(Limits { max_sessions: 2, max_per_address: 1 });
        let now = Instant::now();
        let info = host("x", "w", Mode::Survival, 1, 4);
        let (first, _) = dir.register(addr(1), &info, now).unwrap();
        assert_eq!(dir.register(addr(1), &info, now), Err(RegisterError::TooManyFromAddress));
        dir.register(addr(2), &info, now).unwrap();
        assert_eq!(dir.register(addr(3), &info, now), Err(RegisterError::Full));
        dir.remove(first);
        assert!(dir.register(addr(1), &info, now).is_ok());
        assert_eq!(dir.len(), 2);
    }
}
