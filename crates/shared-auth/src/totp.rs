use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Digest, Sha256};

const BASE32_ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

pub fn generate_secret(seed: u64) -> String {
    let digest = Sha256::digest(seed.to_be_bytes());
    encode_base32(&digest[..20])
}

pub fn current_code(secret: &str) -> String {
    let unix_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_secs();
    current_code_at(secret, unix_time).unwrap_or_else(|| "000000".to_string())
}

pub fn verify_code(secret: &str, code: &str) -> bool {
    let unix_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_secs();
    [
        unix_time.saturating_sub(30),
        unix_time,
        unix_time.saturating_add(30),
    ]
    .into_iter()
    .filter_map(|candidate| current_code_at(secret, candidate))
    .any(|candidate| candidate == code)
}

fn current_code_at(secret: &str, unix_time: u64) -> Option<String> {
    let secret = decode_base32(secret)?;
    let counter = unix_time / 30;
    let mut counter_bytes = [0u8; 8];
    counter_bytes.copy_from_slice(&counter.to_be_bytes());

    let mut mac = Hmac::<Sha1>::new_from_slice(&secret).ok()?;
    mac.update(&counter_bytes);
    let digest = mac.finalize().into_bytes();

    let offset = (digest[19] & 0x0f) as usize;
    let value = u32::from_be_bytes([
        digest[offset] & 0x7f,
        digest[offset + 1],
        digest[offset + 2],
        digest[offset + 3],
    ]) % 1_000_000;

    Some(format!("{value:06}"))
}

fn encode_base32(bytes: &[u8]) -> String {
    let mut output = String::new();
    let mut buffer = 0u16;
    let mut bits = 0u8;

    for byte in bytes {
        buffer = (buffer << 8) | (*byte as u16);
        bits += 8;

        while bits >= 5 {
            let index = ((buffer >> (bits - 5)) & 0x1f) as usize;
            output.push(BASE32_ALPHABET[index] as char);
            bits -= 5;
        }
    }

    if bits > 0 {
        let index = ((buffer << (5 - bits)) & 0x1f) as usize;
        output.push(BASE32_ALPHABET[index] as char);
    }

    output
}

fn decode_base32(secret: &str) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u8;

    for char in secret.chars().filter(|char| !char.is_whitespace()) {
        let value = match char.to_ascii_uppercase() {
            'A'..='Z' => char.to_ascii_uppercase() as u8 - b'A',
            '2'..='7' => (char as u8 - b'2') + 26,
            _ => return None,
        };

        buffer = (buffer << 5) | value as u32;
        bits += 5;

        while bits >= 8 {
            output.push(((buffer >> (bits - 8)) & 0xff) as u8);
            bits -= 8;
        }
    }

    Some(output)
}

#[cfg(test)]
mod tests {
    use super::{current_code_at, generate_secret};

    #[test]
    fn generated_secret_is_base32_and_authenticator_friendly() {
        let secret = generate_secret(1);

        assert!(secret.len() >= 32);
        assert!(secret
            .chars()
            .all(|char| matches!(char, 'A'..='Z' | '2'..='7')));
    }

    #[test]
    fn current_code_matches_rfc6238_sha1_reference_window() {
        let secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

        assert_eq!(current_code_at(secret, 59).as_deref(), Some("287082"));
        assert_eq!(
            current_code_at(secret, 1_111_111_109).as_deref(),
            Some("081804")
        );
    }
}
