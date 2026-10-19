//! PKCE (RFC 7636) with the S256 method, and the random `state` that ties a redirect to the
//! sign-in that asked for it.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};

/// A code verifier and its S256 challenge.
#[derive(Clone, PartialEq, Eq)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

// The verifier is a secret until the code is exchanged: never in logs.
impl std::fmt::Debug for Pkce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pkce").field("challenge", &self.challenge).finish_non_exhaustive()
    }
}

impl Pkce {
    /// The verifier from 32 random bytes: 43 characters of base64url, as RFC 7636 §4.1 suggests.
    pub fn from_random(bytes: &[u8; 32]) -> Pkce {
        Pkce::from_verifier(URL_SAFE_NO_PAD.encode(bytes))
    }

    pub fn from_verifier(verifier: String) -> Pkce {
        let challenge = challenge_s256(&verifier);
        Pkce { verifier, challenge }
    }

    /// A new verifier from the system's random numbers.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn generate() -> Result<Pkce, String> {
        Ok(Pkce::from_random(&random_bytes()?))
    }
}

/// `BASE64URL(SHA256(ASCII(verifier)))`, without padding (RFC 7636 §4.2).
pub fn challenge_s256(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Whether a verifier has the length and characters RFC 7636 §4.1 allows.
pub fn valid_verifier(v: &str) -> bool {
    (43..=128).contains(&v.len()) && v.bytes().all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
}

/// A random `state` (or nonce): 32 bytes of base64url.
#[cfg(not(target_arch = "wasm32"))]
pub fn random_state() -> Result<String, String> {
    Ok(URL_SAFE_NO_PAD.encode(random_bytes()?))
}

#[cfg(not(target_arch = "wasm32"))]
fn random_bytes() -> Result<[u8; 32], String> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(|e| format!("no random numbers from the system: {e}"))?;
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 7636 Appendix B: the verifier from its 32 octets, and its S256 challenge.
    #[test]
    fn rfc7636_appendix_b() {
        let octets: [u8; 32] = [
            116, 24, 223, 180, 151, 153, 224, 37, 79, 250, 96, 125, 216, 173, 187, 186, 22, 212, 37, 77, 105, 214, 191, 240, 91, 88, 5, 88,
            83, 132, 141, 121,
        ];
        let p = Pkce::from_random(&octets);
        assert_eq!(p.verifier, "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk");
        assert_eq!(p.challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        assert!(valid_verifier(&p.verifier));
    }

    #[test]
    fn generated_verifiers_are_valid_and_differ() {
        let a = Pkce::generate().unwrap();
        let b = Pkce::generate().unwrap();
        assert!(valid_verifier(&a.verifier) && valid_verifier(&b.verifier));
        assert_ne!(a.verifier, b.verifier);
        assert_eq!(a.challenge, challenge_s256(&a.verifier));
        assert_ne!(random_state().unwrap(), random_state().unwrap());
        // Debug output leaves the verifier out.
        assert!(!format!("{a:?}").contains(&a.verifier));
    }

    #[test]
    fn verifier_rules() {
        assert!(!valid_verifier("short"));
        assert!(!valid_verifier(&"a".repeat(129)));
        assert!(!valid_verifier(&format!("{}+", "a".repeat(43))));
        assert!(valid_verifier(&"a-._~".repeat(10)));
    }
}
