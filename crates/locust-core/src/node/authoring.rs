//! Building, sealing and signing the events this daemon authors.
//!
//! Every text a request carries is sealed under the goal's key for the
//! event's epoch and travels as the event's payload, in the same commit as
//! the event. Nothing is signed for a halted goal, and a principal that is
//! not a current member signs nothing.

use locust_proto::PROTOCOL_VERSION;
use locust_proto::api::{ApiError, ErrorCode};
use locust_proto::crypto::{ContentKey, Keypair};
use locust_proto::engine::Entropy;
use locust_proto::event::{Body, Event, Header, PayloadRef};
use locust_proto::id::{EventId, GoalId, PublicKey};
use locust_proto::limits::MAX_PAYLOAD_BYTES;
use locust_proto::seal;
use locust_proto::store::{Blob, Store};

use super::Node;
use super::commit::Tx;
use super::entry::Entry;

/// Where an event sits: its place in its author's log and the decision it
/// anchors to.
#[derive(Clone, Copy, Debug)]
pub(super) struct Place {
    pub seq: u64,
    pub prev: Option<EventId>,
    pub anchor: Option<EventId>,
    pub epoch: u32,
}

/// Seals `text` for `goal` under `key` at `epoch`: the stored object and the
/// reference an event names it by.
pub(super) fn seal_text(
    goal: &GoalId,
    epoch: u32,
    key: &ContentKey,
    text: &[u8],
) -> Result<(PayloadRef, Blob), ApiError> {
    let too_long = || ApiError::new(ErrorCode::LimitExceeded, "the text is too long");
    if seal::sealed_len(text.len() as u64).is_none_or(|len| len > MAX_PAYLOAD_BYTES as u64) {
        return Err(too_long());
    }
    let blob = Blob::new(seal::seal(goal, epoch, key, text).map_err(|_| too_long())?);
    let payload = PayloadRef {
        hash: blob.hash(),
        len: blob.bytes().len() as u32,
        key_epoch: epoch,
    };
    Ok((payload, blob))
}

/// Signs one event at `place` and adds it, with its sealed text, to `tx`.
pub(super) fn sign_at(
    goal: GoalId,
    key: &Keypair,
    place: Place,
    body: Body,
    text: Option<(&ContentKey, &str)>,
    now_ms: u64,
    tx: &mut Tx,
) -> Result<Event, ApiError> {
    let payload = match text {
        Some((content_key, text)) => {
            let (payload, blob) = seal_text(&goal, place.epoch, content_key, text.as_bytes())?;
            tx.commit.blobs.push(blob);
            Some(payload)
        }
        None => None,
    };
    let header = Header {
        version: PROTOCOL_VERSION,
        goal,
        author: key.public(),
        seq: place.seq,
        prev: place.prev,
        anchor: place.anchor,
        parents: Vec::new(),
        at_ms: now_ms,
        payload,
        body,
    };
    let event = Event::sign(header, key)?;
    tx.commit.events.push(event.clone());
    tx.authored = true;
    tx.touch(goal);
    Ok(event)
}

impl<S: Store, E: Entropy> Node<S, E> {
    /// Signs `author`'s next event in a goal, sealing `text` under the key
    /// of the event's epoch, and adds both to `tx`.
    pub(super) fn author(
        &self,
        entry: &Entry,
        author: &PublicKey,
        body: Body,
        text: Option<&str>,
        now_ms: u64,
        tx: &mut Tx,
    ) -> Result<EventId, ApiError> {
        let place = self.next_place(entry, author)?;
        let key = match text {
            Some(_) => Some(entry.keys.get(&place.epoch).ok_or_else(|| {
                ApiError::new(
                    ErrorCode::Unavailable,
                    "the goal's content key for the current epoch has not arrived",
                )
            })?),
            None => None,
        };
        let signer = self.signer(author)?;
        let event = sign_at(entry.id(), signer, place, body, key.zip(text), now_ms, tx)?;
        Ok(event.id())
    }

    /// The signing key of a principal this daemon holds.
    pub(super) fn signer(&self, author: &PublicKey) -> Result<&Keypair, ApiError> {
        self.principals
            .get(author)
            .map(|principal| &principal.key)
            .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "no enrolled principal has that key"))
    }

    /// Where `author`'s next event in the goal goes, after the checks every
    /// signature needs: the goal is not halted and the author is a member.
    pub(super) fn next_place(&self, entry: &Entry, author: &PublicKey) -> Result<Place, ApiError> {
        if entry.goal.halt().is_some() {
            return Err(ApiError::new(
                ErrorCode::Halted,
                "the goal's authority history conflicts; nothing more is signed for it",
            ));
        }
        if !entry.is_member(author) {
            return Err(ApiError::new(
                ErrorCode::Denied,
                "the principal is not a current member of the goal",
            ));
        }
        let next = entry.goal.next(author).ok_or_else(|| {
            ApiError::new(
                ErrorCode::Unavailable,
                "the goal's history has not arrived yet",
            )
        })?;
        Ok(Place {
            seq: next.seq,
            prev: next.prev,
            anchor: Some(next.anchor),
            epoch: next.epoch,
        })
    }
}
