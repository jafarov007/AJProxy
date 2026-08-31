//! Hexadecimal Encoding & Decoding using standard official `hex` crate

pub fn encode(input: &[u8]) -> String {
    hex::encode(input)
}

pub fn decode(input: &str) -> Result<Vec<u8>, String> {
    let clean: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.is_empty() {
        return Ok(Vec::new());
    }

    hex::decode(&clean).map_err(|e| format!("Hex Decode Error: {}", e))
}
