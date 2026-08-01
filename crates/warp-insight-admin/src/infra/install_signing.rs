use std::fs;
use std::path::Path;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use ring::signature::{Ed25519KeyPair, KeyPair};

const PEM_BEGIN_PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----";
const PEM_END_PRIVATE_KEY: &str = "-----END PRIVATE KEY-----";
const ED25519_PUBLIC_KEY_SPKI_PREFIX: [u8; 12] = [
    0x30, 0x2A, 0x30, 0x05, 0x06, 0x03, 0x2B, 0x65, 0x70, 0x03, 0x21, 0x00,
];

pub fn load_install_script_public_key_pem(path: &Path) -> Result<String, String> {
    let key_pair = load_install_script_signing_key_pair(path)?;
    Ok(ed25519_public_key_pem(key_pair.public_key().as_ref()))
}

pub fn sign_install_script(path: &Path, script: &[u8]) -> Result<Vec<u8>, String> {
    let key_pair = load_install_script_signing_key_pair(path)?;
    Ok(key_pair.sign(script).as_ref().to_vec())
}

fn load_install_script_signing_key_pair(path: &Path) -> Result<Ed25519KeyPair, String> {
    let pem = fs::read_to_string(path).map_err(|err| {
        format!(
            "failed to read install script signing key {}: {err}",
            path.display()
        )
    })?;
    let der = decode_private_key_pem(&pem)?;
    Ed25519KeyPair::from_pkcs8_maybe_unchecked(&der).map_err(|err| {
        format!(
            "invalid install script signing key {}: {err}",
            path.display()
        )
    })
}

fn decode_private_key_pem(pem: &str) -> Result<Vec<u8>, String> {
    let begin = pem
        .find(PEM_BEGIN_PRIVATE_KEY)
        .ok_or_else(|| "install script signing key must be a PKCS#8 PEM private key".to_string())?;
    let pem = &pem[begin + PEM_BEGIN_PRIVATE_KEY.len()..];
    let end = pem.find(PEM_END_PRIVATE_KEY).ok_or_else(|| {
        "install script signing key must end with a PKCS#8 PEM footer".to_string()
    })?;
    let body = &pem[..end];
    let body: String = body.chars().filter(|ch| !ch.is_whitespace()).collect();
    if body.is_empty() {
        return Err("install script signing key PEM body is empty".to_string());
    }
    STANDARD
        .decode(body)
        .map_err(|err| format!("failed to decode install script signing key PEM: {err}"))
}

fn ed25519_public_key_pem(public_key: &[u8]) -> String {
    let mut der = Vec::with_capacity(ED25519_PUBLIC_KEY_SPKI_PREFIX.len() + public_key.len());
    der.extend_from_slice(&ED25519_PUBLIC_KEY_SPKI_PREFIX);
    der.extend_from_slice(public_key);
    encode_pem("PUBLIC KEY", &der)
}

fn encode_pem(label: &str, der: &[u8]) -> String {
    let encoded = STANDARD.encode(der);
    let wrapped = wrap_base64_lines(&encoded, 64);
    format!("-----BEGIN {label}-----\n{wrapped}-----END {label}-----\n")
}

fn wrap_base64_lines(value: &str, width: usize) -> String {
    let mut output = String::new();
    for chunk in value.as_bytes().chunks(width) {
        output.push_str(std::str::from_utf8(chunk).unwrap_or_default());
        output.push('\n');
    }
    output
}
