//! URL Percent-Encoding & Decoding using standard official `urlencoding` crate

pub fn encode(input: &str) -> String {
    urlencoding::encode(input).into_owned()
}

pub fn decode(input: &str) -> Result<String, String> {
    urlencoding::decode(input)
        .map(|s| s.into_owned())
        .map_err(|e| format!("URL Decode Error: {}", e))
}
