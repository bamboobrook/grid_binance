use sha2::{Digest, Sha256};

pub fn hash_password(password: &str) -> String {
    format!("sha256${}", digest_hex(password.as_bytes()))
}

pub fn verify_password(password: &str, stored_hash: &str) -> bool {
    stored_hash == hash_password(password)
}

fn digest_hex(input: &[u8]) -> String {
    let digest = Sha256::digest(input);
    let mut output = String::with_capacity(digest.len() * 2);

    for byte in digest {
        output.push(hex_char(byte >> 4));
        output.push(hex_char(byte & 0x0f));
    }

    output
}

fn hex_char(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'a' + (value - 10)) as char,
        _ => unreachable!("hex nibble out of range"),
    }
}
