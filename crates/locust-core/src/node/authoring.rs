//! Building, sealing and signing the events this daemon authors.
//!
//! Every text a request carries is sealed under the goal's key for the
//! event's epoch and travels as the event's payload, in the same commit as
//! the event. Nothing is signed for a halted goal. An agent signs only while
//! it is a current member; the goal's governance key, held by the daemon that
//! hosts the goal, signs governance and the host's steps and is no member.

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
use super::access::Attempted;
use super::callers::Actor;
use super::commit::Tx;
use super::entry::Entry;
use super::local;

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
    /// Build the actual signed candidate in memory, ask replay and the local
    /// level in that order, then move only an accepted candidate into `tx`.
    #[allow(clippy::too_many_arguments)] // Carries the signed event's typed fields and the acting principal.
    pub(super) fn sign_for(
        &self,
        actor: &Actor,
        entry: &Entry,
        author: &PublicKey,
        body: Body,
        text: Option<&str>,
        now_ms: u64,
        tx: &mut Tx,
    ) -> Result<EventId, ApiError> {
        let mut candidate = Tx::none();
        let id = self.author(entry, author, body, text, now_ms, &mut candidate)?;
        let event = candidate
            .commit
            .events
            .last()
            .expect("author signed one candidate");
        let trial = self.allowed_keeping(
            actor,
            entry,
            *author,
            Attempted::Sign {
                event,
                preceding: &tx.commit.events,
            },
        )?;
        tx.trial = trial;
        tx.commit.events.append(&mut candidate.commit.events);
        tx.commit.blobs.append(&mut candidate.commit.blobs);
        tx.commit.local.append(&mut candidate.commit.local);
        tx.authored = true;
        tx.touch(entry.id());
        if actor.owner_act {
            tx.local(local::by_owner_write(&entry.id(), &id));
        }
        Ok(id)
    }

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
        let signer = self.key_for(entry, author)?;
        let event = sign_at(entry.id(), signer, place, body, key.zip(text), now_ms, tx)?;
        Ok(event.id())
    }

    /// Signs a record with nobody present: no text and no clock reading, so
    /// the same request from the same records is the same record. The host's
    /// computer signs admissions and stage steps this way and nothing else.
    pub(super) fn author_alone(
        &self,
        entry: &Entry,
        author: &PublicKey,
        body: Body,
        tx: &mut Tx,
    ) -> Result<EventId, ApiError> {
        self.author(entry, author, body, None, 0, tx)
    }

    /// The key that signs for `author` here: the goal's governance key when
    /// `author` is its public key and this daemon holds it, else the agent's.
    pub(super) fn key_for<'a>(
        &'a self,
        entry: &'a Entry,
        author: &PublicKey,
    ) -> Result<&'a Keypair, ApiError> {
        match &entry.local.governance {
            Some(governance) if governance.public() == *author => Ok(governance),
            _ => self.signer(author),
        }
    }

    /// The signing key of a principal this daemon holds.
    pub(super) fn signer(&self, author: &PublicKey) -> Result<&Keypair, ApiError> {
        self.principals
            .active(author)
            .map(|principal| &principal.key)
            .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "no enrolled principal has that key"))
    }

    /// Where `author`'s next event in the goal goes, after the checks every
    /// signature needs: the goal is not halted and the author is a member.
    /// The governance key is no member and has not left; it skips both tests.
    pub(super) fn next_place(&self, entry: &Entry, author: &PublicKey) -> Result<Place, ApiError> {
        let governance = entry.state().governance.as_ref() == Some(author);
        if !governance && (!entry.is_member(author) || entry.local.part.get(author) == Some(&true))
        {
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
