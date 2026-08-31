//! JWT Token Parser & Decoder

use super::base64;

pub fn decode(input: &str) -> Result<String, String> {
    let clean = input.trim();
    let parts: Vec<&str> = clean.split('.').collect();

    if parts.len() < 2 {
        return Err("Invalid JWT structure: Token must contain at least 2 dot-separated parts (Header.Payload.[Signature])".to_string());
    }

    let raw_header = parts[0];
    let raw_payload = parts[1];
    let raw_signature = if parts.len() > 2 { parts[2] } else { "" };

    let header_bytes = base64::decode(raw_header)
        .map_err(|e| format!("JWT Header Base64 Decode Error: {}", e))?;
    let payload_bytes = base64::decode(raw_payload)
        .map_err(|e| format!("JWT Payload Base64 Decode Error: {}", e))?;

    let header_str = String::from_utf8(header_bytes)
        .map_err(|_| "JWT Header is not valid UTF-8".to_string())?;
    let payload_str = String::from_utf8(payload_bytes)
        .map_err(|_| "JWT Payload is not valid UTF-8".to_string())?;

    let formatted_header = serde_json::from_str::<serde_json::Value>(&header_str)
        .map(|v| serde_json::to_string_pretty(&v).unwrap_or(header_str.clone()))
        .unwrap_or(header_str);

    let formatted_payload = serde_json::from_str::<serde_json::Value>(&payload_str)
        .map(|v| serde_json::to_string_pretty(&v).unwrap_or(payload_str.clone()))
        .unwrap_or(payload_str);

    let mut out = String::new();
    out.push_str("=== JWT HEADER ===\n");
    out.push_str(&formatted_header);
    out.push_str("\n\n=== JWT PAYLOAD ===\n");
    out.push_str(&formatted_payload);
    out.push_str("\n\n=== JWT SIGNATURE ===\n");
    if raw_signature.is_empty() {
        out.push_str("[Unsigned / None Signature]");
    } else {
        out.push_str(&format!("Raw Signature (Base64Url): {}", raw_signature));
    }

    Ok(out)
}

pub fn encode(input: &str) -> String {
    format!("[JWT Encoding Note]\nTo create a valid JWT, construct JSON header and payload objects, Base64Url encode them, and append a HMAC/RSA signature.\n\nInput Payload:\n{}", input.trim_end())
}
