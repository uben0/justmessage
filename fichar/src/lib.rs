use rand::{TryRngCore, rngs::OsRng};
use sha2::Sha256;

pub mod command;
pub mod context;
pub mod input;
pub mod language;
pub mod output;
pub mod state;

pub fn derive_key(key: &[u8]) -> [u8; 32] {
    pbkdf2::pbkdf2_hmac_array::<Sha256, 32>(key, &[], 100_000)
}

pub fn gen_key() -> [u8; 32] {
    let mut key = [0; 32];
    OsRng.try_fill_bytes(&mut key).unwrap();
    key
}

pub fn key_to_hex(key: [u8; 32]) -> String {
    use std::fmt::Write;
    let mut buffer = String::new();
    for byte in key {
        write!(buffer, "{byte:02x}").unwrap();
    }
    buffer
}
