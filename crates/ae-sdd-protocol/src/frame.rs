use thiserror::Error;

/// Default maximum payload size for one protocol frame (16 MiB).
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// Pure framing validation error, independent of socket I/O.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FrameError {
    /// A decoder received fewer than the four length-prefix bytes.
    #[error("frame header requires 4 bytes, received {actual}")]
    HeaderTooShort {
        /// Number of bytes available to the decoder.
        actual: usize,
    },
    /// The unsigned length prefix was zero, which is never a valid JSON frame.
    #[error("zero-length frames are forbidden")]
    EmptyPayload,
    /// The declared or encoded payload exceeds [`MAX_FRAME_BYTES`].
    #[error("frame payload of {actual} bytes exceeds the {maximum}-byte limit")]
    PayloadTooLarge {
        /// Payload size declared or requested.
        actual: usize,
        /// Active protocol maximum.
        maximum: usize,
    },
    /// The prefix and actual payload length differ.
    #[error("frame declares {declared} payload bytes but contains {actual}")]
    LengthMismatch {
        /// Payload bytes stated by the prefix.
        declared: usize,
        /// Payload bytes actually present after the prefix.
        actual: usize,
    },
}

/// Encodes one non-empty payload with a four-byte big-endian length prefix.
///
/// This function performs no I/O and rejects payloads larger than 16 MiB.
pub fn encode_frame(payload: &[u8]) -> Result<Vec<u8>, FrameError> {
    let payload_length = payload.len();
    if payload_length == 0 {
        return Err(FrameError::EmptyPayload);
    }
    if payload_length > MAX_FRAME_BYTES {
        return Err(FrameError::PayloadTooLarge {
            actual: payload_length,
            maximum: MAX_FRAME_BYTES,
        });
    }

    let length = u32::try_from(payload_length).map_err(|_| FrameError::PayloadTooLarge {
        actual: payload_length,
        maximum: MAX_FRAME_BYTES,
    })?;
    let mut frame = Vec::with_capacity(4 + payload_length);
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

/// Validates and decodes one complete four-byte big-endian framed payload.
///
/// The returned slice borrows the input. Transport implementations remain
/// responsible for reading exactly one complete frame before calling this
/// pure decoder.
pub fn decode_frame(frame: &[u8]) -> Result<&[u8], FrameError> {
    let header: [u8; 4] = frame
        .get(..4)
        .ok_or(FrameError::HeaderTooShort {
            actual: frame.len(),
        })?
        .try_into()
        .expect("a four-byte slice always converts to a four-byte array");
    let declared = u32::from_be_bytes(header) as usize;
    if declared == 0 {
        return Err(FrameError::EmptyPayload);
    }
    if declared > MAX_FRAME_BYTES {
        return Err(FrameError::PayloadTooLarge {
            actual: declared,
            maximum: MAX_FRAME_BYTES,
        });
    }

    let payload = &frame[4..];
    if payload.len() != declared {
        return Err(FrameError::LengthMismatch {
            declared,
            actual: payload.len(),
        });
    }
    Ok(payload)
}
