use bytes::{Buf, BufMut, Bytes, BytesMut};
use crc32fast::Hasher;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Protocol magic header: "PFWD" in little-endian
pub const FRAME_HEAD: [u8; 4] = [0x50, 0x46, 0x57, 0x44];

/// Protocol version
pub const PROTOCOL_VERSION: u8 = 1;

/// Maximum frame payload size (1MB)
pub const MAX_FRAME_SIZE: usize = 1024 * 1024;

/// Frame header size: HEAD(4) + VERSION(1) + TYPE(1) + CHANNEL(4) + LENGTH(4) = 14
pub const FRAME_HEADER_SIZE: usize = 14;

/// Frame trailer size: CRC32(4)
pub const FRAME_TRAILER_SIZE: usize = 4;

#[derive(Debug, Error)]
pub enum FrameError {
    #[error("Invalid frame header")]
    InvalidHeader,
    #[error("Invalid frame version")]
    InvalidVersion,
    #[error("Frame too large: {0} bytes")]
    FrameTooLarge(usize),
    #[error("Invalid frame length")]
    InvalidLength,
    #[error("CRC checksum mismatch")]
    CrcMismatch,
    #[error("Incomplete frame")]
    IncompleteFrame,
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Frame type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum FrameType {
    /// Data frame for channel communication
    Data = 0x00,
    /// Open channel request
    OpenChannel = 0x01,
    /// Close channel notification
    CloseChannel = 0x02,
    /// Heartbeat/ping frame
    Heartbeat = 0x03,
    /// Heartbeat response/pong
    HeartbeatAck = 0x04,
    /// Authentication challenge
    AuthChallenge = 0x10,
    /// Authentication response
    AuthResponse = 0x11,
    /// Authentication success
    AuthSuccess = 0x12,
    /// Authentication failure
    AuthFailure = 0x13,
    /// Error notification
    Error = 0xFF,
}

impl TryFrom<u8> for FrameType {
    type Error = FrameError;

    fn try_from(value: u8) -> Result<Self, FrameError> {
        match value {
            0x00 => Ok(FrameType::Data),
            0x01 => Ok(FrameType::OpenChannel),
            0x02 => Ok(FrameType::CloseChannel),
            0x03 => Ok(FrameType::Heartbeat),
            0x04 => Ok(FrameType::HeartbeatAck),
            0x10 => Ok(FrameType::AuthChallenge),
            0x11 => Ok(FrameType::AuthResponse),
            0x12 => Ok(FrameType::AuthSuccess),
            0x13 => Ok(FrameType::AuthFailure),
            0xFF => Ok(FrameType::Error),
            _ => Err(FrameError::InvalidHeader),
        }
    }
}

/// Protocol frame
#[derive(Debug, Clone)]
pub struct Frame {
    /// Frame type
    pub frame_type: FrameType,
    /// Channel ID (0 for control frames)
    pub channel_id: u32,
    /// Payload data
    pub payload: Bytes,
}

impl Frame {
    /// Create a new frame
    pub fn new(frame_type: FrameType, channel_id: u32, payload: Bytes) -> Self {
        Self {
            frame_type,
            channel_id,
            payload,
        }
    }

    /// Create a data frame for a channel
    pub fn data(channel_id: u32, payload: Bytes) -> Self {
        Self::new(FrameType::Data, channel_id, payload)
    }

    /// Create an open channel request
    pub fn open_channel(channel_id: u32, payload: Bytes) -> Self {
        Self::new(FrameType::OpenChannel, channel_id, payload)
    }

    /// Create a close channel notification
    pub fn close_channel(channel_id: u32) -> Self {
        Self::new(FrameType::CloseChannel, channel_id, Bytes::new())
    }

    /// Create a heartbeat frame
    pub fn heartbeat() -> Self {
        Self::new(FrameType::Heartbeat, 0, Bytes::new())
    }

    /// Create a heartbeat acknowledgment
    pub fn heartbeat_ack() -> Self {
        Self::new(FrameType::HeartbeatAck, 0, Bytes::new())
    }

    /// Create an auth challenge frame
    pub fn auth_challenge(nonce: Bytes) -> Self {
        Self::new(FrameType::AuthChallenge, 0, nonce)
    }

    /// Create an auth response frame
    pub fn auth_response(response: Bytes) -> Self {
        Self::new(FrameType::AuthResponse, 0, response)
    }

    /// Create an error frame
    pub fn error(message: &str) -> Self {
        Self::new(FrameType::Error, 0, Bytes::copy_from_slice(message.as_bytes()))
    }

    /// Encode frame to bytes
    /// Format: [HEAD(4)][VERSION(1)][TYPE(1)][CHANNEL(4)][LENGTH(4)][PAYLOAD(N)][CRC32(4)]
    pub fn encode(&self) -> Bytes {
        let total_len = FRAME_HEADER_SIZE + self.payload.len() + FRAME_TRAILER_SIZE;
        let mut buf = BytesMut::with_capacity(total_len);

        // Header
        buf.put_slice(&FRAME_HEAD);
        buf.put_u8(PROTOCOL_VERSION);
        buf.put_u8(self.frame_type as u8);
        buf.put_u32_le(self.channel_id);
        buf.put_u32_le(self.payload.len() as u32);

        // Payload
        buf.put_slice(&self.payload);

        // CRC32
        let crc = calculate_crc32(&buf);
        buf.put_u32_le(crc);

        buf.freeze()
    }

    /// Decode frame from bytes
    pub fn decode(data: &[u8]) -> Result<Self, FrameError> {
        if data.len() < FRAME_HEADER_SIZE + FRAME_TRAILER_SIZE {
            return Err(FrameError::IncompleteFrame);
        }

        // Verify header
        if &data[..4] != FRAME_HEAD {
            return Err(FrameError::InvalidHeader);
        }

        let version = data[4];
        if version != PROTOCOL_VERSION {
            return Err(FrameError::InvalidVersion);
        }

        let frame_type = FrameType::try_from(data[5])?;
        let channel_id = u32::from_le_bytes([data[6], data[7], data[8], data[9]]);
        let payload_len = u32::from_le_bytes([data[10], data[11], data[12], data[13]]) as usize;

        if payload_len > MAX_FRAME_SIZE {
            return Err(FrameError::FrameTooLarge(payload_len));
        }

        let total_len = FRAME_HEADER_SIZE + payload_len + FRAME_TRAILER_SIZE;
        if data.len() < total_len {
            return Err(FrameError::IncompleteFrame);
        }

        // Verify CRC32
        let expected_crc = u32::from_le_bytes([
            data[total_len - 4],
            data[total_len - 3],
            data[total_len - 2],
            data[total_len - 1],
        ]);
        let calculated_crc = calculate_crc32(&data[..total_len - 4]);
        if expected_crc != calculated_crc {
            return Err(FrameError::CrcMismatch);
        }

        let payload = Bytes::copy_from_slice(&data[FRAME_HEADER_SIZE..FRAME_HEADER_SIZE + payload_len]);

        Ok(Self {
            frame_type,
            channel_id,
            payload,
        })
    }

    /// Get the total encoded size of this frame
    pub fn encoded_size(&self) -> usize {
        FRAME_HEADER_SIZE + self.payload.len() + FRAME_TRAILER_SIZE
    }
}

/// Calculate CRC32 checksum
fn calculate_crc32(data: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

/// Open channel payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenChannelPayload {
    pub target_ip: String,
    pub target_port: u16,
}

impl OpenChannelPayload {
    pub fn new(target_ip: String, target_port: u16) -> Self {
        Self { target_ip, target_port }
    }

    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    pub fn decode(data: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(data)
    }
}

/// Close channel payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseChannelPayload {
    pub reason: String,
}

impl CloseChannelPayload {
    pub fn new(reason: &str) -> Self {
        Self {
            reason: reason.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_encode_decode() {
        let frame = Frame::data(123, Bytes::from(&b"hello world"[..]));
        let encoded = frame.encode();
        let decoded = Frame::decode(&encoded).unwrap();

        assert_eq!(decoded.frame_type, FrameType::Data);
        assert_eq!(decoded.channel_id, 123);
        assert_eq!(&decoded.payload[..], b"hello world");
    }

    #[test]
    fn test_heartbeat_frame() {
        let frame = Frame::heartbeat();
        let encoded = frame.encode();
        let decoded = Frame::decode(&encoded).unwrap();

        assert_eq!(decoded.frame_type, FrameType::Heartbeat);
        assert_eq!(decoded.channel_id, 0);
        assert!(decoded.payload.is_empty());
    }

    #[test]
    fn test_crc_mismatch() {
        let frame = Frame::data(1, Bytes::from(&b"test"[..]));
        let mut encoded = frame.encode().to_vec();

        // Corrupt the payload
        encoded[20] ^= 0xFF;

        let result = Frame::decode(&encoded);
        assert!(matches!(result, Err(FrameError::CrcMismatch)));
    }
}
