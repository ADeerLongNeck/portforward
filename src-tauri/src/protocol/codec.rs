use bytes::BytesMut;
use tokio_util::codec::{Decoder, Encoder};

use super::{Frame, FrameError, FRAME_HEADER_SIZE, FRAME_TRAILER_SIZE};

/// Frame codec for tokio streams
pub struct FrameCodec;

impl FrameCodec {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FrameCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder for FrameCodec {
    type Item = Frame;
    type Error = FrameError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Frame>, Self::Error> {
        // Check if we have enough data for the header
        if src.len() < FRAME_HEADER_SIZE {
            return Ok(None);
        }

        // Parse header to get payload length
        let payload_len =
            u32::from_le_bytes([src[10], src[11], src[12], src[13]]) as usize;

        let total_len = FRAME_HEADER_SIZE + payload_len + FRAME_TRAILER_SIZE;

        // Check if we have the complete frame
        if src.len() < total_len {
            // Reserve more space if needed
            src.reserve(total_len - src.len());
            return Ok(None);
        }

        // Split off the frame data
        let frame_data = src.split_to(total_len);

        // Decode the frame
        Frame::decode(&frame_data).map(Some)
    }
}

impl Encoder<Frame> for FrameCodec {
    type Error = FrameError;

    fn encode(&mut self, item: Frame, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let encoded = item.encode();
        dst.extend_from_slice(&encoded);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[test]
    fn test_codec_roundtrip() {
        let mut codec = FrameCodec::new();
        let frame = Frame::data(42, Bytes::from(&b"test data"[..]));

        let mut buf = BytesMut::new();
        codec.encode(frame.clone(), &mut buf).unwrap();

        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(decoded.frame_type, frame.frame_type);
        assert_eq!(decoded.channel_id, frame.channel_id);
        assert_eq!(decoded.payload, frame.payload);
    }
}
