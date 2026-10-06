//! Fixed-size identifiers.
//!
//! Binary encodings carry the raw bytes with no length prefix. Human-readable
//! encodings (JSON output) use lowercase hex, so identifiers can be copied
//! between a terminal and a command.

use std::fmt;

/// A string was not the lowercase or uppercase hex form of an identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseIdError;

impl fmt::Display for ParseIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("expected a hex identifier of the right length")
    }
}

impl std::error::Error for ParseIdError {}

pub(crate) fn write_hex(bytes: &[u8], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        let pair = [
            DIGITS[usize::from(byte >> 4)],
            DIGITS[usize::from(byte & 0x0f)],
        ];
        // Both bytes are ASCII hex digits.
        f.write_str(std::str::from_utf8(&pair).map_err(|_| fmt::Error)?)?;
    }
    Ok(())
}

pub(crate) fn parse_hex(text: &str, out: &mut [u8]) -> Result<(), ParseIdError> {
    let text = text.as_bytes();
    if text.len() != out.len() * 2 {
        return Err(ParseIdError);
    }
    for (slot, pair) in out.iter_mut().zip(text.chunks_exact(2)) {
        *slot = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Ok(())
}

/// Displays bytes as lowercase hex.
pub(crate) struct Hex<'a>(pub &'a [u8]);

impl fmt::Display for Hex<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(self.0, f)
    }
}

pub(crate) fn hex_to_vec(text: &str) -> Result<Vec<u8>, ParseIdError> {
    if !text.len().is_multiple_of(2) {
        return Err(ParseIdError);
    }
    let mut bytes = vec![0u8; text.len() / 2];
    parse_hex(text, &mut bytes)?;
    Ok(bytes)
}

fn nibble(digit: u8) -> Result<u8, ParseIdError> {
    match digit {
        b'0'..=b'9' => Ok(digit - b'0'),
        b'a'..=b'f' => Ok(digit - b'a' + 10),
        b'A'..=b'F' => Ok(digit - b'A' + 10),
        _ => Err(ParseIdError),
    }
}

macro_rules! byte_id {
    ($(#[$meta:meta])* $name:ident, $len:expr) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub [u8; $len]);

        impl $name {
            pub const LEN: usize = $len;

            pub const fn as_bytes(&self) -> &[u8; $len] {
                &self.0
            }
        }

        impl schemars::JsonSchema for $name {
            fn schema_name() -> std::borrow::Cow<'static, str> { stringify!($name).into() }
            fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
                schemars::json_schema!({
                    "type": "string",
                    "pattern": format!("^[0-9a-fA-F]{{{}}}$", $len * 2)
                })
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write_hex(&self.0, f)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($name), "("))?;
                write_hex(&self.0[..4], f)?;
                f.write_str("..)")
            }
        }

        impl std::str::FromStr for $name {
            type Err = ParseIdError;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                let mut bytes = [0u8; $len];
                parse_hex(text, &mut bytes)?;
                Ok(Self(bytes))
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                if serializer.is_human_readable() {
                    serializer.collect_str(self)
                } else {
                    use serde::ser::SerializeTuple;
                    let mut tuple = serializer.serialize_tuple($len)?;
                    for byte in &self.0 {
                        tuple.serialize_element(byte)?;
                    }
                    tuple.end()
                }
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct Visitor;

                impl<'de> serde::de::Visitor<'de> for Visitor {
                    type Value = $name;

                    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        write!(f, "{} bytes, or their hex form", $len)
                    }

                    fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<$name, E> {
                        text.parse().map_err(E::custom)
                    }

                    fn visit_seq<A: serde::de::SeqAccess<'de>>(
                        self,
                        mut seq: A,
                    ) -> Result<$name, A::Error> {
                        let mut bytes = [0u8; $len];
                        for (index, slot) in bytes.iter_mut().enumerate() {
                            *slot = seq
                                .next_element()?
                                .ok_or_else(|| serde::de::Error::invalid_length(index, &self))?;
                        }
                        Ok($name(bytes))
                    }
                }

                if deserializer.is_human_readable() {
                    deserializer.deserialize_str(Visitor)
                } else {
                    deserializer.deserialize_tuple($len, Visitor)
                }
            }
        }
    };
}

byte_id!(
    /// Identifies a signed event: the domain-separated BLAKE3 digest of its
    /// exact header bytes. Tasks, assignments, results and revisions are
    /// identified by the event that created them.
    EventId,
    32
);

byte_id!(
    /// Identifies a goal: the digest of its first record's governance key,
    /// host's agent, initial definition and salt.
    GoalId,
    32
);

byte_id!(
    /// Semantic identity of a normalized organization definition.
    DefinitionHash,
    32
);

byte_id!(
    /// Stable logical identity of a configured transition effect.
    EffectId,
    32
);

byte_id!(
    /// Plain BLAKE3 digest of stored content: an event payload, a manifest or
    /// a file.
    BlobHash,
    32
);

byte_id!(
    /// Ed25519 verifying key of an enrolled principal. The daemon holds the
    /// matching signing key and signs as that principal's delegate.
    PublicKey,
    32
);

byte_id!(
    /// Transport identity of a daemon. A membership decision binds each
    /// principal to the endpoint that may speak for it.
    EndpointId,
    32
);

byte_id!(
    /// Ed25519 signature.
    Signature,
    64
);

byte_id!(
    /// Handle of one execution session of a coding client. A claim is bound to
    /// the session that took it, so a second session of the same principal
    /// cannot silently take over.
    InstanceId,
    16
);

byte_id!(
    /// Client-chosen key that makes a retried request return its first result.
    IdempotencyKey,
    16
);

byte_id!(
    /// One daemon-local ordinary-directory checkout, scoped to a goal and principal.
    CheckoutId,
    16
);

byte_id!(
    /// Durable local workspace operation, including capture and recovery state.
    WorkspaceOperationId,
    16
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec;

    #[test]
    fn hex_round_trips_and_rejects_bad_input() {
        let id = EventId([0xab; 32]);
        let text = id.to_string();
        assert_eq!(text.len(), 64);
        assert_eq!(text.parse::<EventId>(), Ok(id));
        assert_eq!(text.to_uppercase().parse::<EventId>(), Ok(id));
        assert_eq!("abcd".parse::<EventId>(), Err(ParseIdError));
        assert_eq!("zz".repeat(32).parse::<EventId>(), Err(ParseIdError));
    }

    #[test]
    fn binary_form_is_the_raw_bytes() {
        let signature = Signature([7; 64]);
        let bytes = codec::encode(&signature).unwrap();
        assert_eq!(bytes, vec![7; 64]);
        assert_eq!(codec::decode::<Signature>(&bytes), Ok(signature));
    }

    #[test]
    fn human_readable_form_is_hex() {
        let key = PublicKey([0x1f; 32]);
        let json = serde_json::to_string(&key).unwrap();
        assert_eq!(json, format!("\"{}\"", "1f".repeat(32)));
        assert_eq!(serde_json::from_str::<PublicKey>(&json).unwrap(), key);
    }

    #[test]
    fn debug_is_abbreviated() {
        assert_eq!(format!("{:?}", GoalId([0x12; 32])), "GoalId(12121212..)");
    }
}
