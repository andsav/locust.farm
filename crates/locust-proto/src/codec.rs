//! Binary encoding shared by signed headers, peer frames and the local API.
//!
//! The encoding is postcard 1.x: positional fields, variable-length integers,
//! no field names. Enum variants are identified by their declaration index, so
//! variants are only ever appended and never reordered.

use std::fmt;
use std::io::{self, Read, Write};

use serde::Serialize;
use serde::de::DeserializeOwned;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecError {
    Encode,
    Decode,
    /// The value decoded but bytes were left over.
    TrailingBytes,
    /// The bytes decode, but are not the one encoding of the value.
    NotCanonical,
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Encode => "value could not be encoded",
            Self::Decode => "bytes are not a valid encoding",
            Self::TrailingBytes => "bytes continue past the encoded value",
            Self::NotCanonical => "bytes are not the canonical encoding of the value",
        })
    }
}

impl std::error::Error for CodecError {}

pub fn encode<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, CodecError> {
    postcard::to_allocvec(value).map_err(|_| CodecError::Encode)
}

/// Decodes a value that must occupy the whole input.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, CodecError> {
    let (value, rest) = postcard::take_from_bytes(bytes).map_err(|_| CodecError::Decode)?;
    if rest.is_empty() {
        Ok(value)
    } else {
        Err(CodecError::TrailingBytes)
    }
}

/// Decodes a value whose identity is the hash of its bytes. Accepts only the
/// one encoding of the value, so what was hashed and what was decoded can
/// never name two different things.
pub fn decode_canonical<T: Serialize + DeserializeOwned>(bytes: &[u8]) -> Result<T, CodecError> {
    let value: T = decode(bytes)?;
    if encode(&value)? == bytes {
        Ok(value)
    } else {
        Err(CodecError::NotCanonical)
    }
}

/// A frame is a little-endian `u32` length followed by that many bytes.
pub const FRAME_PREFIX_BYTES: usize = 4;

/// A frame prefix announced more bytes than the reader admits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameTooLarge {
    /// The length the prefix announced.
    pub length: usize,
    /// The limit it was checked against.
    pub max: usize,
}

impl fmt::Display for FrameTooLarge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "frame of {} bytes exceeds the admitted {} bytes",
            self.length, self.max
        )
    }
}

impl std::error::Error for FrameTooLarge {}

impl From<FrameTooLarge> for io::Error {
    fn from(error: FrameTooLarge) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, error)
    }
}

/// The payload length a frame prefix announces, refused if above `max`.
/// Every reader, blocking or not, calls this before it allocates.
pub fn frame_len(prefix: [u8; FRAME_PREFIX_BYTES], max: usize) -> Result<usize, FrameTooLarge> {
    let length = u32::from_le_bytes(prefix) as usize;
    if length > max {
        Err(FrameTooLarge { length, max })
    } else {
        Ok(length)
    }
}

/// Appends one whole frame carrying `value` to `out`: one buffer, one write.
pub fn encode_frame<T: Serialize + ?Sized>(value: &T, out: &mut Vec<u8>) -> Result<(), CodecError> {
    let start = out.len();
    out.extend_from_slice(&[0; FRAME_PREFIX_BYTES]);
    let mut framed =
        postcard::to_extend(value, std::mem::take(out)).map_err(|_| CodecError::Encode)?;
    let length =
        u32::try_from(framed.len() - start - FRAME_PREFIX_BYTES).map_err(|_| CodecError::Encode)?;
    framed[start..start + FRAME_PREFIX_BYTES].copy_from_slice(&length.to_le_bytes());
    *out = framed;
    Ok(())
}

/// Writes one frame around an already encoded payload.
pub fn write_frame<W: Write>(writer: &mut W, payload: &[u8]) -> io::Result<()> {
    let length = u32::try_from(payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "frame exceeds u32 length"))?;
    let mut frame = Vec::with_capacity(FRAME_PREFIX_BYTES + payload.len());
    frame.extend_from_slice(&length.to_le_bytes());
    frame.extend_from_slice(payload);
    writer.write_all(&frame)
}

/// Reads one frame. Returns `None` when the stream ends cleanly before a
/// frame starts. The length is checked against `max` before any allocation.
pub fn read_frame<R: Read>(reader: &mut R, max: usize) -> io::Result<Option<Vec<u8>>> {
    let mut prefix = [0u8; FRAME_PREFIX_BYTES];
    let mut filled = 0;
    while filled < prefix.len() {
        match reader.read(&mut prefix[filled..]) {
            Ok(0) if filled == 0 => return Ok(None),
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    let mut payload = vec![0u8; frame_len(prefix, max)?];
    reader.read_exact(&mut payload)?;
    Ok(Some(payload))
}

/// Serde adapter that writes a byte vector as one length-prefixed run instead
/// of element by element. Use it for every `Vec<u8>` field.
pub mod bytes {
    use std::fmt;

    use serde::de::{SeqAccess, Visitor};
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(bytes)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        struct Bytes;

        impl<'de> Visitor<'de> for Bytes {
            type Value = Vec<u8>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a byte string")
            }

            fn visit_bytes<E>(self, bytes: &[u8]) -> Result<Vec<u8>, E> {
                Ok(bytes.to_vec())
            }

            fn visit_byte_buf<E>(self, bytes: Vec<u8>) -> Result<Vec<u8>, E> {
                Ok(bytes)
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<u8>, A::Error> {
                let mut bytes = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(4096));
                while let Some(byte) = seq.next_element()? {
                    bytes.push(byte);
                }
                Ok(bytes)
            }
        }

        deserializer.deserialize_byte_buf(Bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_rejects_trailing_bytes() {
        let mut bytes = encode(&7u32).unwrap();
        assert_eq!(decode::<u32>(&bytes), Ok(7));
        bytes.push(0);
        assert_eq!(decode::<u32>(&bytes), Err(CodecError::TrailingBytes));
    }

    #[test]
    fn frames_round_trip_and_enforce_the_limit() {
        let mut wire = Vec::new();
        write_frame(&mut wire, b"hello").unwrap();
        write_frame(&mut wire, b"").unwrap();

        let mut reader = wire.as_slice();
        assert_eq!(
            read_frame(&mut reader, 16).unwrap(),
            Some(b"hello".to_vec())
        );
        assert_eq!(read_frame(&mut reader, 16).unwrap(), Some(Vec::new()));
        assert_eq!(read_frame(&mut reader, 16).unwrap(), None);

        let mut reader = wire.as_slice();
        let error = read_frame(&mut reader, 4).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn encode_frame_matches_write_frame_and_appends() {
        let mut expected = Vec::new();
        write_frame(&mut expected, &encode(&("hello", 7u32)).unwrap()).unwrap();

        let mut out = vec![0xaa];
        encode_frame(&("hello", 7u32), &mut out).unwrap();
        assert_eq!(out[0], 0xaa);
        assert_eq!(&out[1..], expected.as_slice());
        assert_eq!(frame_len([5, 0, 0, 0], 16), Ok(5));
        assert_eq!(
            frame_len([17, 0, 0, 0], 16),
            Err(FrameTooLarge {
                length: 17,
                max: 16
            })
        );
    }

    #[test]
    fn canonical_decoding_refuses_an_overlong_integer() {
        assert_eq!(decode_canonical::<u32>(&[0x01]), Ok(1));
        assert_eq!(decode::<u32>(&[0x81, 0x00]), Ok(1));
        assert_eq!(
            decode_canonical::<u32>(&[0x81, 0x00]),
            Err(CodecError::NotCanonical)
        );
    }

    #[test]
    fn a_truncated_frame_is_an_error_not_a_clean_end() {
        let mut wire = Vec::new();
        write_frame(&mut wire, b"hello").unwrap();
        wire.truncate(6);
        let mut reader = wire.as_slice();
        let error = read_frame(&mut reader, 16).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }
}
