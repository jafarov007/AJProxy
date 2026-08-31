//! Cryptographic Hashing Algorithms (MD5, SHA1, SHA256, SHA512)

use openssl::hash::{hash, MessageDigest};
use super::hex;

pub fn md5(input: &str) -> String {
    match hash(MessageDigest::md5(), input.as_bytes()) {
        Ok(digest) => hex::encode(&digest),
        Err(e) => format!("MD5 Error: {}", e),
    }
}

pub fn sha1(input: &str) -> String {
    match hash(MessageDigest::sha1(), input.as_bytes()) {
        Ok(digest) => hex::encode(&digest),
        Err(e) => format!("SHA1 Error: {}", e),
    }
}

pub fn sha256(input: &str) -> String {
    match hash(MessageDigest::sha256(), input.as_bytes()) {
        Ok(digest) => hex::encode(&digest),
        Err(e) => format!("SHA256 Error: {}", e),
    }
}

pub fn sha512(input: &str) -> String {
    match hash(MessageDigest::sha512(), input.as_bytes()) {
        Ok(digest) => hex::encode(&digest),
        Err(e) => format!("SHA512 Error: {}", e),
    }
}
