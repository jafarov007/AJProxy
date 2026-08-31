//! Modular Codec Subsystem for AJProxy

pub mod base64;
pub mod url;
pub mod hex;
pub mod html;
pub mod jwt;
pub mod hash;

use crate::models::EncodingType;

/// Main Entry Point for Encoding Operations
pub fn encode(enc: &EncodingType, input: &str) -> Result<String, String> {
    let clean_input = input.trim_end_matches(['\r', '\n']);
    if clean_input.is_empty() {
        return Ok(String::new());
    }

    match enc {
        EncodingType::Base64 => Ok(base64::encode(clean_input.as_bytes())),
        EncodingType::URL => Ok(url::encode(clean_input)),
        EncodingType::HTML => Ok(html::encode(clean_input)),
        EncodingType::Hex => Ok(hex::encode(clean_input.as_bytes())),
        EncodingType::JWT => Ok(jwt::encode(clean_input)),
        EncodingType::MD5 => Ok(hash::md5(clean_input)),
        EncodingType::SHA1 => Ok(hash::sha1(clean_input)),
        EncodingType::SHA256 => Ok(hash::sha256(clean_input)),
        EncodingType::SHA512 => Ok(hash::sha512(clean_input)),
    }
}

/// Main Entry Point for Decoding Operations
pub fn decode(enc: &EncodingType, input: &str) -> Result<String, String> {
    let clean_input = input.trim_end_matches(['\r', '\n']);
    if clean_input.is_empty() {
        return Ok(String::new());
    }

    match enc {
        EncodingType::Base64 => {
            let bytes = base64::decode(clean_input)?;
            String::from_utf8(bytes).map_err(|_| "Base64 decoded bytes are not valid UTF-8 text".to_string())
        }
        EncodingType::URL => url::decode(clean_input),
        EncodingType::HTML => html::decode(clean_input),
        EncodingType::Hex => {
            let bytes = hex::decode(clean_input)?;
            String::from_utf8(bytes).map_err(|_| "Hex decoded bytes are not valid UTF-8 text".to_string())
        }
        EncodingType::JWT => jwt::decode(clean_input),
        EncodingType::MD5 | EncodingType::SHA1 | EncodingType::SHA256 | EncodingType::SHA512 => {
            Err("One-way cryptographic hash functions cannot be decoded.".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64() {
        let raw = "AJProxy";
        let encoded = encode(&EncodingType::Base64, raw).unwrap();
        assert_eq!(encoded, "QUpQcm94eQ==");
        let decoded = decode(&EncodingType::Base64, &encoded).unwrap();
        assert_eq!(decoded, raw);
    }

    #[test]
    fn test_url() {
        let raw = "AJProxy?a=1&b=2";
        let encoded = encode(&EncodingType::URL, raw).unwrap();
        assert_eq!(encoded, "AJProxy%3Fa%3D1%26b%3D2");
        let decoded = decode(&EncodingType::URL, &encoded).unwrap();
        assert_eq!(decoded, raw);
    }

    #[test]
    fn test_hex() {
        let raw = "AJProxy";
        let encoded = encode(&EncodingType::Hex, raw).unwrap();
        assert_eq!(encoded, "414a50726f7879");
        let decoded = decode(&EncodingType::Hex, &encoded).unwrap();
        assert_eq!(decoded, raw);
    }

    #[test]
    fn test_html() {
        let raw = "<script>alert('AJProxy')</script>";
        let encoded = encode(&EncodingType::HTML, raw).unwrap();
        let decoded = decode(&EncodingType::HTML, &encoded).unwrap();
        assert_eq!(decoded, raw);
    }

    #[test]
    fn test_md5() {
        let raw = "AJProxy";
        let hash_val = encode(&EncodingType::MD5, raw).unwrap();
        assert_eq!(hash_val, "85e116c92d51a1a2c5947d4eeaa7afb8");
    }

    #[test]
    fn test_sha1() {
        let raw = "AJProxy";
        let hash_val = encode(&EncodingType::SHA1, raw).unwrap();
        assert_eq!(hash_val, "722be805eeeaca41cb0f42eba0f04bc7a4e927e9");
    }

    #[test]
    fn test_sha256() {
        let raw = "AJProxy";
        let hash_val = encode(&EncodingType::SHA256, raw).unwrap();
        assert_eq!(hash_val, "67343bf679639d416fa24f6dd52e202c60df1a3e179884031284bcd2f83f3d74");
    }

    #[test]
    fn test_jwt() {
        let jwt_sample = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkFKUHJveHkiLCJpYXQiOjE1MTYyMzkwMjJ9.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let decoded = decode(&EncodingType::JWT, jwt_sample).unwrap();
        assert!(decoded.contains("AJProxy"));
        assert!(decoded.contains("=== JWT HEADER ==="));
        assert!(decoded.contains("=== JWT PAYLOAD ==="));
    }
}
