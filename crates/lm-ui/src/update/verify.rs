//! Checking an update before anything is installed (docs/lm/update-feed.md › Signature): the
//! feed's Ed25519 signature over the new version and its disk image's SHA-256, made with the
//! owner's private key (`cargo xtask update-sign`, which keeps the key in the macOS keychain), is
//! checked against the public key compiled in here; then the downloaded file's SHA-256 must be the
//! one the signature covers. A compromised download host, mirror or DNS can serve a different
//! file or feed, but can't make either check pass (the Sparkle model).

use std::io::Read;
use std::path::Path;

use base64::Engine as _;
use ring::signature::{ED25519, UnparsedPublicKey};

/// The public half of the update signing key, base64 (32 bytes). Made with
/// `cargo xtask update-keygen`, which keeps the private half only in the owner's keychain
/// (service `nimblemap-update`, account `ed25519`). Replacing it means every build already out
/// there rejects updates signed with the new key: don't, unless the old key is lost or leaked.
pub const PUBLIC_KEY: &str = "bxaUHigI18sLRPoijV2wQFTWDofahlGWtGPSyNJaZag=";

/// The bytes the signature covers: the version and the disk image's SHA-256 (lowercase hex), in
/// a fixed text form. `cargo xtask update-sign` signs exactly this.
pub fn signed_message(version: &str, sha256: &str) -> String {
    format!("NimbleMap update\nformat 1\nversion {version}\nsha256 {sha256}\n")
}

/// Why an update was refused before installing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No signature, a malformed one, or one that doesn't match the version and checksum.
    Signature,
    /// The downloaded file isn't the one the feed names.
    Checksum,
}

/// A SHA-256 written as 64 lowercase hex digits.
pub fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Whether `signature` (base64) is the owner's signature over `version` and `sha256`, checked
/// against `public_key` (base64).
pub fn verify_with(public_key: &str, version: &str, sha256: &str, signature: &str) -> Result<(), Refusal> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let key = b64.decode(public_key.trim()).map_err(|_| Refusal::Signature)?;
    let sig = b64.decode(signature.trim()).map_err(|_| Refusal::Signature)?;
    if key.len() != 32 || sig.len() != 64 || !is_sha256(sha256) || version.trim() != version || version.is_empty() {
        return Err(Refusal::Signature);
    }
    UnparsedPublicKey::new(&ED25519, &key).verify(signed_message(version, sha256).as_bytes(), &sig).map_err(|_| Refusal::Signature)
}

/// [`verify_with`] against the compiled-in [`PUBLIC_KEY`].
pub fn verify(version: &str, sha256: &str, signature: &str) -> Result<(), Refusal> {
    verify_with(PUBLIC_KEY, version, sha256, signature)
}

/// Whether this build has a public key to check updates with (without one, Update Now is not
/// offered and the notice links the download instead).
pub fn has_key() -> bool {
    base64::engine::general_purpose::STANDARD.decode(PUBLIC_KEY).is_ok_and(|k| k.len() == 32)
}

/// A file's SHA-256, lowercase hex.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        ctx.update(&buf[..n]);
    }
    Ok(ctx.finish().as_ref().iter().map(|b| format!("{b:02x}")).collect())
}

/// Checks a downloaded file against the SHA-256 the signature covered.
pub fn check_file(path: &Path, sha256: &str) -> Result<(), Refusal> {
    match sha256_file(path) {
        Ok(h) if h == sha256 => Ok(()),
        _ => Err(Refusal::Checksum),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use ring::signature::{Ed25519KeyPair, KeyPair};

    use super::*;

    /// A throwaway key pair for tests: (public key base64, key pair).
    pub(crate) fn test_key() -> (String, Ed25519KeyPair) {
        let rng = ring::rand::SystemRandom::new();
        let doc = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let pair = Ed25519KeyPair::from_pkcs8(doc.as_ref()).unwrap();
        (base64::engine::general_purpose::STANDARD.encode(pair.public_key().as_ref()), pair)
    }

    pub(crate) fn sign(pair: &Ed25519KeyPair, version: &str, sha: &str) -> String {
        base64::engine::general_purpose::STANDARD.encode(pair.sign(signed_message(version, sha).as_bytes()).as_ref())
    }

    const SHA: &str = "053ac3d7965cd2cff0a1b85044cd926e40b552ae0e437526d223849ecf6a3c05";

    #[test]
    fn a_good_signature_verifies() {
        let (pk, pair) = test_key();
        let sig = sign(&pair, "0.7.2", SHA);
        assert_eq!(verify_with(&pk, "0.7.2", SHA, &sig), Ok(()));
    }

    #[test]
    fn a_bad_or_tampered_signature_is_refused() {
        let (pk, pair) = test_key();
        let sig = sign(&pair, "0.7.2", SHA);
        // Another version, another file: the same signature no longer matches.
        assert_eq!(verify_with(&pk, "0.7.3", SHA, &sig), Err(Refusal::Signature));
        let other = SHA.replace('0', "1");
        assert_eq!(verify_with(&pk, "0.7.2", &other, &sig), Err(Refusal::Signature));
        // A flipped bit in the signature.
        let mut raw = base64::engine::general_purpose::STANDARD.decode(&sig).unwrap();
        raw[10] ^= 1;
        let flipped = base64::engine::general_purpose::STANDARD.encode(raw);
        assert_eq!(verify_with(&pk, "0.7.2", SHA, &flipped), Err(Refusal::Signature));
        // Signed by someone else's key.
        let (other_pk, _) = test_key();
        assert_eq!(verify_with(&other_pk, "0.7.2", SHA, &sig), Err(Refusal::Signature));
        // Garbage, empty, or a checksum that isn't one.
        assert_eq!(verify_with(&pk, "0.7.2", SHA, "not base64!"), Err(Refusal::Signature));
        assert_eq!(verify_with(&pk, "0.7.2", SHA, ""), Err(Refusal::Signature));
        assert_eq!(verify_with(&pk, "0.7.2", &SHA.to_uppercase(), &sig), Err(Refusal::Signature));
        assert_eq!(verify_with(&pk, " 0.7.2", SHA, &sig), Err(Refusal::Signature));
        assert_eq!(verify_with("", "0.7.2", SHA, &sig), Err(Refusal::Signature));
    }

    #[test]
    fn the_compiled_in_key_is_a_real_key() {
        assert!(has_key(), "PUBLIC_KEY is not a 32-byte base64 key: run cargo xtask update-keygen");
    }

    #[test]
    fn a_file_must_match_its_checksum() {
        let dir = std::env::temp_dir().join(format!("lm-update-sha-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("x.dmg");
        std::fs::write(&f, b"abc").unwrap();
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(sha256_file(&f).unwrap(), abc);
        assert_eq!(check_file(&f, abc), Ok(()));
        assert_eq!(check_file(&f, SHA), Err(Refusal::Checksum));
        assert_eq!(check_file(&dir.join("missing.dmg"), abc), Err(Refusal::Checksum));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
