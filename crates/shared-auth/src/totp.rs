use sha2::{Digest, Sha256};

pub fn generate_secret(seed: u64) -> String {
    format!("totp-secret-{seed:08}")
}

pub fn current_code(secret: &str) -> String {
    let digest = Sha256::digest(secret.as_bytes());
    let value = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) % 1_000_000;
    format!("{value:06}")
}

pub fn verify_code(secret: &str, code: &str) -> bool {
    current_code(secret) == code
}
