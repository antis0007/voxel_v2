//! Player keys: who a player is, independent of the name they choose.
//!
//! Each client keeps one Ed25519 key. On join the host sends a random
//! challenge; the client signs it with the world identity, so a signature
//! cannot be replayed on another connection or another world. Operator rights
//! follow the key's fingerprint, never the display name.
use crate::protocol::WorldIdentity;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

/// Separates introduction signatures from any other use of the same key.
const INTRODUCE_DOMAIN: &[u8] = b"VXMP introduce v1";
/// Fingerprint length in bytes; shown and stored as twice as many hex digits.
const FINGERPRINT_BYTES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentityError(pub &'static str);

impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for IdentityError {}

/// 32 bytes from the operating system's secure generator.
pub fn random_bytes() -> Result<[u8; 32], IdentityError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| IdentityError("no secure randomness"))?;
    Ok(bytes)
}

/// A player's private key. Store `to_bytes` privately; never send it.
pub struct PlayerKey(SigningKey);

impl PlayerKey {
    pub fn generate() -> Result<Self, IdentityError> {
        Ok(Self::from_bytes(random_bytes()?))
    }

    pub fn from_bytes(secret: [u8; 32]) -> Self {
        Self(SigningKey::from_bytes(&secret))
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    pub fn public(&self) -> [u8; 32] {
        self.0.verifying_key().to_bytes()
    }

    pub fn fingerprint(&self) -> String {
        fingerprint(&self.public())
    }

    /// Signature for `Packet::Introduce` answering the host's `challenge`.
    pub fn sign_introduce(&self, challenge: &[u8; 32], world: &WorldIdentity) -> [u8; 64] {
        self.0.sign(&transcript(challenge, world)).to_bytes()
    }
}

/// Whether `signature` proves the holder of `public` answered `challenge`.
pub fn verify_introduce(
    public: &[u8; 32],
    challenge: &[u8; 32],
    world: &WorldIdentity,
    signature: &[u8; 64],
) -> bool {
    VerifyingKey::from_bytes(public).is_ok_and(|key| {
        key.verify(&transcript(challenge, world), &Signature::from_bytes(signature))
            .is_ok()
    })
}

/// Lower-case hex of the first 16 bytes of a public key, as access files store it.
pub fn fingerprint(public: &[u8; 32]) -> String {
    public[..FINGERPRINT_BYTES].iter().map(|b| format!("{b:02x}")).collect()
}

/// Whether `text` has the form `fingerprint` produces.
pub fn is_fingerprint(text: &str) -> bool {
    text.len() == FINGERPRINT_BYTES * 2 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

fn transcript(challenge: &[u8; 32], world: &WorldIdentity) -> Vec<u8> {
    let mut out = Vec::with_capacity(INTRODUCE_DOMAIN.len() + 32 + 46);
    out.extend_from_slice(INTRODUCE_DOMAIN);
    out.extend_from_slice(challenge);
    out.extend_from_slice(&world.seed.to_le_bytes());
    out.extend_from_slice(&world.generator.to_le_bytes());
    out.extend_from_slice(&world.cell_mm.to_le_bytes());
    out.extend_from_slice(&world.content);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(seed: u64) -> WorldIdentity {
        WorldIdentity { seed, generator: 1, cell_mm: 1000, content: [7; 32] }
    }

    #[test]
    fn a_signature_answers_only_its_own_challenge_and_world() {
        let key = PlayerKey::from_bytes([3; 32]);
        let challenge = [9; 32];
        let signature = key.sign_introduce(&challenge, &world(1));
        assert!(verify_introduce(&key.public(), &challenge, &world(1), &signature));
        assert!(!verify_introduce(&key.public(), &[8; 32], &world(1), &signature));
        assert!(!verify_introduce(&key.public(), &challenge, &world(2), &signature));
        let other = PlayerKey::from_bytes([4; 32]);
        assert!(!verify_introduce(&other.public(), &challenge, &world(1), &signature));
    }

    #[test]
    fn keys_survive_storage_and_fingerprints_are_stable() {
        let key = PlayerKey::generate().expect("randomness");
        let restored = PlayerKey::from_bytes(key.to_bytes());
        assert_eq!(restored.public(), key.public());
        assert!(is_fingerprint(&key.fingerprint()));
        assert_eq!(key.fingerprint(), restored.fingerprint());
        assert!(!is_fingerprint("not hex"));
    }
}
