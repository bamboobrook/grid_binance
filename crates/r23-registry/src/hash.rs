//! SHA-256 helpers.
//!
//! §4.1 forbids short hashes (e.g. a 63-char hash). All hashes recorded by the
//! registry are full 64-char lowercase hex SHA-256 digests.

use sha2::{Digest, Sha256};

/// A full 64-character lowercase-hex SHA-256 digest.
pub const FULL_HEX_LEN: usize = 64;

/// Hash an arbitrary byte slice and return its full 64-char hex digest.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex_lower(&h.finalize())
}

/// Hash a string and return its full 64-char hex digest.
pub fn sha256_str(s: &str) -> String {
    sha256_hex(s.as_bytes())
}

/// True iff `s` is exactly 64 lowercase-hex chars.
pub fn is_full_sha256_hex(s: &str) -> bool {
    s.len() == FULL_HEX_LEN && s.as_bytes().iter().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Hex-encode a byte slice in lowercase (no external dep).
fn hex_lower(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(TABLE[(b >> 4) as usize] as char);
        out.push(TABLE[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_of_empty_is_known_constant() {
        // e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        assert_eq!(
            sha256_str(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn full_hex_predicate_rejects_short_and_uppercase() {
        assert!(is_full_sha256_hex(&"a".repeat(64)));
        assert!(!is_full_sha256_hex(&"a".repeat(63))); // §4.1 canary #3
        assert!(!is_full_sha256_hex(&"A".repeat(64))); // must be lowercase
        assert!(!is_full_sha256_hex("not hex at all but 64 chars xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"));
    }
}
