//! Frames exchanged between two daemons over one authenticated link.
//!
//! The link is a reliable, ordered stream of length-prefixed frames (see
//! [`crate::codec`]), each one encoded [`SyncMessage`]. The transport has
//! already authenticated the remote endpoint; whether that endpoint may see a
//! goal is decided per request against the goal's membership, not once per
//! connection.
//!
//! Reconciliation is by per-author log position: each side states how much of
//! every author's log it holds, and the other sends what is missing. Live
//! pushes are hints; a missed one loses nothing because the next exchange of
//! frontiers finds the gap.

use serde::{Deserialize, Serialize};

use crate::codec;
use crate::event::WireEvent;
use crate::id::{BlobHash, GoalId, PublicKey};
use crate::invite::JoinRequest;

/// How much of one author's log a peer holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorFrontier {
    pub author: PublicKey,
    /// Number of consecutive events held, counted from sequence zero.
    pub next_seq: u64,
}

/// What a peer holds of a goal, ascending by author.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frontier {
    pub authors: Vec<AuthorFrontier>,
}

impl Frontier {
    /// The first sequence number of `author` this frontier lacks.
    pub fn next_seq(&self, author: &PublicKey) -> u64 {
        self.authors
            .binary_search_by(|entry| entry.author.cmp(author))
            .map_or(0, |index| self.authors[index].next_seq)
    }
}

/// Why a peer will not serve a request. Refusals are always sent, never
/// expressed by silence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    UnsupportedVersion,
    UnknownGoal,
    /// The remote endpoint does not speak for a current member.
    NotAMember,
    /// The invitation is unknown, expired or bound to another key.
    InvitationRefused,
    /// A frame, batch or object exceeded a published or configured limit.
    LimitExceeded,
    /// A frame did not decode or broke the exchange order.
    ProtocolError,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncMessage {
    /// Opens an exchange about one goal.
    Hello {
        version: u8,
        goal: GoalId,
    },
    /// Redeems an invitation. The only request a non-member may make.
    Join(JoinRequest),
    Refused(Refusal),
    Frontier(Frontier),
    /// Events in an order where each author's log is ascending.
    Events(Vec<WireEvent>),
    BlobRequest(Vec<BlobHash>),
    Blob {
        hash: BlobHash,
        #[serde(with = "codec::bytes")]
        bytes: Vec<u8>,
    },
    /// The peer does not hold, or has withdrawn, this object.
    BlobUnavailable(BlobHash),
    /// Nothing further to send for this exchange.
    Done,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_seq_defaults_to_zero_for_unknown_authors() {
        let frontier = Frontier {
            authors: vec![
                AuthorFrontier {
                    author: PublicKey([1; 32]),
                    next_seq: 4,
                },
                AuthorFrontier {
                    author: PublicKey([3; 32]),
                    next_seq: 9,
                },
            ],
        };
        assert_eq!(frontier.next_seq(&PublicKey([1; 32])), 4);
        assert_eq!(frontier.next_seq(&PublicKey([2; 32])), 0);
        assert_eq!(frontier.next_seq(&PublicKey([3; 32])), 9);
    }

    #[test]
    fn a_blob_frame_carries_its_bytes_as_one_run() {
        let message = SyncMessage::Blob {
            hash: BlobHash([0; 32]),
            bytes: vec![7; 300],
        };
        let frame = codec::encode(&message).unwrap();
        // Variant index, 32-byte hash, two-byte length, then the bytes.
        assert_eq!(frame.len(), 1 + 32 + 2 + 300);
        assert_eq!(codec::decode::<SyncMessage>(&frame), Ok(message));
    }
}
