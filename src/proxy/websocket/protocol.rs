use std::io::{Read, Write};
use crate::models::WsOpcode;

#[derive(Clone, Debug)]
pub struct WsRawFrame {
    pub fin: bool,
    pub opcode_u8: u8,
    pub masked: bool,
    pub mask_key: Option<[u8; 4]>,
    pub payload: Vec<u8>,
}

impl WsRawFrame {
    pub fn to_opcode(&self) -> WsOpcode {
        match self.opcode_u8 {
            0x1 => WsOpcode::Text,
            0x2 => WsOpcode::Binary,
            0x8 => WsOpcode::Close,
            0x9 => WsOpcode::Ping,
            0xA => WsOpcode::Pong,
            0x0 => WsOpcode::Continuation,
            other => WsOpcode::Unknown(other),
        }
    }
}

/// Parses 2-byte Close status code and reason string from a Close (0x8) frame payload
pub fn parse_close_code(payload: &[u8]) -> (u16, String) {
    if payload.len() >= 2 {
        let code = u16::from_be_bytes([payload[0], payload[1]]);
        let reason = if payload.len() > 2 {
            String::from_utf8_lossy(&payload[2..]).to_string()
        } else {
            String::new()
        };

        let desc = match code {
            1000 => "Normal Closure",
            1001 => "Going Away",
            1002 => "Protocol Error",
            1003 => "Unsupported Data",
            1006 => "Abnormal Closure",
            1007 => "Invalid Payload",
            1008 => "Policy Violation",
            1009 => "Message Too Big",
            1011 => "Internal Error",
            _ => "Status Code",
        };

        let full_reason = if reason.is_empty() {
            format!("{} ({})", desc, code)
        } else {
            format!("{} ({}): {}", desc, code, reason)
        };

        (code, full_reason)
    } else {
        (1006, "Abnormal Closure (No Close Code)".to_string())
    }
}

/// Constructs a matching Pong (0xA) frame in response to a Ping (0x9) frame
pub fn build_pong_frame(ping_frame: &WsRawFrame) -> WsRawFrame {
    WsRawFrame {
        fin: true,
        opcode_u8: 0xA, // Pong
        masked: false,
        mask_key: None,
        payload: ping_frame.payload.clone(),
    }
}

fn read_exact_blocking<R: Read>(reader: &mut R, buf: &mut [u8]) -> std::io::Result<()> {
    let mut offset = 0;
    while offset < buf.len() {
        match reader.read(&mut buf[offset..]) {
            Ok(0) => return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Unexpected EOF")),
            Ok(n) => offset += n,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                std::thread::sleep(std::time::Duration::from_millis(2));
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Parses a single RFC 6455 frame from any std::io::Read stream.
/// Safe against socket timeouts: returns WouldBlock ONLY if byte 0 has not yet arrived.
pub fn read_ws_frame<R: Read>(reader: &mut R) -> std::io::Result<WsRawFrame> {
    let mut first_byte = [0u8; 1];
    let n = reader.read(&mut first_byte)?;
    if n == 0 {
        return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "EOF"));
    }

    let fin = (first_byte[0] & 0x80) != 0;
    let opcode_u8 = first_byte[0] & 0x0F;

    let mut second_byte = [0u8; 1];
    read_exact_blocking(reader, &mut second_byte)?;

    let masked = (second_byte[0] & 0x80) != 0;
    let mut payload_len = (second_byte[0] & 0x7F) as u64;

    if payload_len == 126 {
        let mut ext = [0u8; 2];
        read_exact_blocking(reader, &mut ext)?;
        payload_len = u16::from_be_bytes(ext) as u64;
    } else if payload_len == 127 {
        let mut ext = [0u8; 8];
        read_exact_blocking(reader, &mut ext)?;
        payload_len = u64::from_be_bytes(ext);
    }

    let mask_key = if masked {
        let mut key = [0u8; 4];
        read_exact_blocking(reader, &mut key)?;
        Some(key)
    } else {
        None
    };

    let mut payload = vec![0u8; payload_len as usize];
    if payload_len > 0 {
        read_exact_blocking(reader, &mut payload)?;
    }

    // Unmask payload in-place
    if let Some(key) = mask_key {
        for (i, byte) in payload.iter_mut().enumerate() {
            *byte ^= key[i % 4];
        }
    }

    Ok(WsRawFrame {
        fin,
        opcode_u8,
        masked,
        mask_key,
        payload,
    })
}

/// Serializes and writes a single RFC 6455 frame to any std::io::Write stream
pub fn write_ws_frame<W: Write>(writer: &mut W, frame: &WsRawFrame) -> std::io::Result<()> {
    let mut header = Vec::with_capacity(14);
    let mut byte0 = frame.opcode_u8 & 0x0F;
    if frame.fin {
        byte0 |= 0x80;
    }
    header.push(byte0);

    let len = frame.payload.len();
    let mask_bit = if frame.masked { 0x80 } else { 0x00 };

    if len <= 125 {
        header.push(mask_bit | (len as u8));
    } else if len <= 65535 {
        header.push(mask_bit | 126);
        header.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        header.push(mask_bit | 127);
        header.extend_from_slice(&(len as u64).to_be_bytes());
    }

    if let Some(key) = frame.mask_key {
        header.extend_from_slice(&key);
        writer.write_all(&header)?;
        let mut masked_payload = frame.payload.clone();
        for (i, byte) in masked_payload.iter_mut().enumerate() {
            *byte ^= key[i % 4];
        }
        writer.write_all(&masked_payload)?;
    } else {
        writer.write_all(&header)?;
        if !frame.payload.is_empty() {
            writer.write_all(&frame.payload)?;
        }
    }

    writer.flush()
}
