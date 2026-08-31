//! Base64 Encoding & Decoding using standard official `base64` crate

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};

pub fn encode(data: &[u8]) -> String {
    STANDARD.encode(data)
}

pub fn decode(input: &str) -> Result<Vec<u8>, String> {
    let clean: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.is_empty() {
        return Ok(Vec::new());
    }

    STANDARD.decode(&clean)
        .or_else(|_| URL_SAFE_NO_PAD.decode(&clean))
        .map_err(|e| format!("Base64 Decode Error: {}", e))
}
