//! Typed framing layer on top of the raw byte forwarding the bridge
//! does after the handshake.
//!
//! Each frame is `[1 byte type][4 byte BE length][payload bytes]`. We
//! reserve the high half of the byte for future use — the three live
//! types are:
//!
//!   * `0x01` = DATA — opaque application payload, forwarded to the
//!     consumer's read API
//!   * `0x02` = PING — reachability probe; the receiving actor
//!     auto-responds with PONG echoing the same payload
//!   * `0x03` = PONG — response to a PING; matched against pending
//!     `ping()` waiters by payload byte-equality
//!
//! This layer is **only** active under the `icmp` cargo feature. With
//! the flag off, [`PairStream`] is byte-identical to the
//! pre-icmp shape: every consumer write goes out raw, every recv frame
//! is forwarded raw to the consumer. That keeps the phone (which today
//! speaks raw bytes) interoperable until both sides flip the flag on.
//!
//! [`PairStream`]: crate::stream::PairStream

#![cfg(feature = "icmp")]

use thiserror::Error;

/// Frame-type byte — opaque application payload.
pub const FRAME_TYPE_DATA: u8 = 0x01;
/// Frame-type byte — reachability probe.
pub const FRAME_TYPE_PING: u8 = 0x02;
/// Frame-type byte — response to a PING.
pub const FRAME_TYPE_PONG: u8 = 0x03;

/// Encode a single typed frame. Caller is responsible for sending the
/// returned Vec as a single binary frame to the bridge.
pub fn encode_frame(ty: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + payload.len());
    out.push(ty);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    out
}

/// Decode a single typed frame. Returns the frame type byte + the
/// payload slice. Rejects partial / over-long buffers — callers MUST
/// pass one bridge binary-frame at a time. (The bridge preserves WS
/// frame boundaries, so this is the natural unit.)
pub fn decode_frame(buf: &[u8]) -> Result<(u8, &[u8]), FrameError> {
    if buf.len() < 5 {
        return Err(FrameError::TooShort);
    }
    let ty = buf[0];
    let len = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize;
    if buf.len() != 5 + len {
        return Err(FrameError::LengthMismatch {
            declared: len,
            actual: buf.len().saturating_sub(5),
        });
    }
    Ok((ty, &buf[5..]))
}

#[derive(Debug, Error)]
pub enum FrameError {
    #[error("frame too short — needs ≥ 5 byte header")]
    TooShort,
    #[error("length mismatch: declared {declared}, actual payload {actual}")]
    LengthMismatch { declared: usize, actual: usize },
}

#[derive(Debug, Error)]
pub enum PingError {
    #[error("ping timed out after {0:?}")]
    Timeout(std::time::Duration),
    #[error("stream closed before pong arrived")]
    StreamClosed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_frame_round_trip() {
        let payload = b"hello";
        let bytes = encode_frame(FRAME_TYPE_DATA, payload);
        assert_eq!(bytes[0], FRAME_TYPE_DATA);
        assert_eq!(&bytes[1..5], &(payload.len() as u32).to_be_bytes());
        assert_eq!(&bytes[5..], payload);
        let (ty, body) = decode_frame(&bytes).unwrap();
        assert_eq!(ty, FRAME_TYPE_DATA);
        assert_eq!(body, payload);
    }

    #[test]
    fn empty_payload_round_trip() {
        let bytes = encode_frame(FRAME_TYPE_PING, &[]);
        assert_eq!(bytes.len(), 5);
        let (ty, body) = decode_frame(&bytes).unwrap();
        assert_eq!(ty, FRAME_TYPE_PING);
        assert!(body.is_empty());
    }

    #[test]
    fn ping_pong_distinct_types() {
        let p = encode_frame(FRAME_TYPE_PING, b"nonce");
        let q = encode_frame(FRAME_TYPE_PONG, b"nonce");
        assert_ne!(p[0], q[0]);
        assert_eq!(&p[1..], &q[1..]);
    }

    #[test]
    fn too_short_rejected() {
        for n in 0..5 {
            let buf = vec![0u8; n];
            assert!(matches!(decode_frame(&buf), Err(FrameError::TooShort)));
        }
    }

    #[test]
    fn length_mismatch_rejected() {
        // Declare 100 bytes but only ship 3.
        let mut buf = vec![FRAME_TYPE_DATA, 0, 0, 0, 100];
        buf.extend_from_slice(b"abc");
        assert!(matches!(
            decode_frame(&buf),
            Err(FrameError::LengthMismatch { declared: 100, actual: 3 })
        ));
    }

    #[test]
    fn trailing_bytes_rejected() {
        // Declare 3 but ship 6 — strict equality required.
        let mut buf = vec![FRAME_TYPE_DATA, 0, 0, 0, 3];
        buf.extend_from_slice(b"abcDEF");
        assert!(matches!(
            decode_frame(&buf),
            Err(FrameError::LengthMismatch { declared: 3, actual: 6 })
        ));
    }
}
