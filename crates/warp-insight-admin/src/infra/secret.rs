use ring::{digest, rand as ring_rand};

pub fn new_secret_token(prefix: &str) -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    let rng = ring_rand::SystemRandom::new();
    ring_rand::SecureRandom::fill(&rng, &mut bytes)
        .map_err(|_| "failed to read system random source".to_string())?;
    Ok(format!("{prefix}_{}", hex_lower(&bytes)))
}

pub fn sha256_hex(value: &str) -> String {
    bytes_sha256_hex(value.as_bytes())
}

pub fn bytes_sha256_hex(bytes: &[u8]) -> String {
    hex_lower(digest::digest(&digest::SHA256, bytes).as_ref())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}
