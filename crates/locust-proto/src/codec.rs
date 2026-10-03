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
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Encode => "value could not be encoded",
            Self::Decode => "bytes are not a valid encoding",
            Self::TrailingBytes => "bytes continue past the encoded value",
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

/// Writes one frame: a little-endian `u32` length, then the payload.
pub fn write_frame<W: Write>(writer: &mut W, payload: &[u8]) -> io::Result<()> {
    let length = u32::try_from(payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "frame exceeds u32 length"))?;
    writer.write_all(&length.to_le_bytes())?;
    writer.write_all(payload)
}

/// Reads one frame. Returns `None` when the stream ends cleanly before a
/// frame starts. The length is checked against `max` before any allocation.
pub fn read_frame<R: Read>(reader: &mut R, max: usize) -> io::Result<Option<Vec<u8>>> {
    let mut length = [0u8; 4];
    let mut filled = 0;
    while filled < length.len() {
        match reader.read(&mut length[filled..]) {
            Ok(0) if filled == 0 => return Ok(None),
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    let length = u32::from_le_bytes(length) as usize;
    if length > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame exceeds the admitted size",
        ));
    }
    let mut payload = vec![0u8; length];
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
    fn a_truncated_frame_is_an_error_not_a_clean_end() {
        let mut wire = Vec::new();
        write_frame(&mut wire, b"hello").unwrap();
        wire.truncate(6);
        let mut reader = wire.as_slice();
        let error = read_frame(&mut reader, 16).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }
}
