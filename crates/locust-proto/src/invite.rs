//! Invitations and the request that redeems one.
//!
//! An invitation is a single-use capability the inviter hands over through a
//! channel of their choosing. It names the goal, who will admit the joiner and
//! how to reach them. Redemption binds it to the first key that presents it:
//! retrying with the same key recovers the result, any other key is refused.
//! Holding an invitation shares nothing and enrolls nothing by itself.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::PROTOCOL_VERSION;
use crate::codec;
use crate::crypto::{self, Keypair, domain};
use crate::id::{EndpointId, GoalId, Hex, PublicKey, Signature, hex_to_vec};

/// Text prefix of an invitation ticket.
pub const TICKET_PREFIX: &str = "locust-invite-";

/// Most contact hints one invitation may carry.
pub const MAX_HINTS: usize = 8;

/// Longest single contact hint, in bytes.
pub const MAX_HINT_BYTES: usize = 256;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invitation {
    /// [`PROTOCOL_VERSION`]; the first encoded byte.
    pub version: u8,
    pub goal: GoalId,
    /// The goal's coordinator, shown to the joiner as a fingerprint and
    /// checked against the genesis record once it arrives.
    pub coordinator: PublicKey,
    /// The daemon that redeems the invitation.
    pub endpoint: EndpointId,
    /// Relay URLs or addresses that help reach `endpoint`.
    pub hints: Vec<String>,
    /// The capability. Never logged or printed outside the ticket itself.
    pub secret: [u8; 32],
    /// Issuer-chosen expiry in Unix milliseconds, judged by the issuer's clock.
    pub expires_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InviteError {
    /// The text does not start with [`TICKET_PREFIX`].
    NotATicket,
    Malformed,
    UnsupportedVersion(u8),
    /// Too many contact hints, or one too long.
    BadHints,
}

impl fmt::Display for InviteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotATicket => f.write_str("text is not a Locust invitation"),
            Self::Malformed => f.write_str("invitation is damaged or incomplete"),
            Self::UnsupportedVersion(version) => {
                write!(f, "invitation uses unsupported protocol version {version}")
            }
            Self::BadHints => f.write_str("invitation carries too many or too long contact hints"),
        }
    }
}

impl std::error::Error for InviteError {}

impl Invitation {
    fn check(&self) -> Result<(), InviteError> {
        if self.version != PROTOCOL_VERSION {
            return Err(InviteError::UnsupportedVersion(self.version));
        }
        let hints_fit = self.hints.len() <= MAX_HINTS
            && self.hints.iter().all(|hint| hint.len() <= MAX_HINT_BYTES);
        if hints_fit {
            Ok(())
        } else {
            Err(InviteError::BadHints)
        }
    }

    /// The pasteable form.
    pub fn to_ticket(&self) -> Result<String, InviteError> {
        self.check()?;
        let bytes = codec::encode(self).map_err(|_| InviteError::Malformed)?;
        Ok(format!("{TICKET_PREFIX}{}", Hex(&bytes)))
    }

    pub fn from_ticket(ticket: &str) -> Result<Self, InviteError> {
        let hex = ticket
            .trim()
            .strip_prefix(TICKET_PREFIX)
            .ok_or(InviteError::NotATicket)?;
        let bytes = hex_to_vec(hex).map_err(|_| InviteError::Malformed)?;
        match bytes.first() {
            Some(&PROTOCOL_VERSION) => {}
            Some(&version) => return Err(InviteError::UnsupportedVersion(version)),
            None => return Err(InviteError::Malformed),
        }
        let invitation: Self = codec::decode(&bytes).map_err(|_| InviteError::Malformed)?;
        invitation.check()?;
        Ok(invitation)
    }

    /// What the issuer stores in place of the secret.
    pub fn secret_digest(&self) -> [u8; 32] {
        secret_digest(&self.secret)
    }
}

impl fmt::Debug for Invitation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Invitation")
            .field("goal", &self.goal)
            .field("coordinator", &self.coordinator)
            .field("endpoint", &self.endpoint)
            .field("hints", &self.hints)
            .field("expires_ms", &self.expires_ms)
            .finish_non_exhaustive()
    }
}

pub fn secret_digest(secret: &[u8; 32]) -> [u8; 32] {
    crypto::domain_hash(domain::INVITE_SECRET, secret)
}

/// Sent to the inviting daemon before the joiner is a member. The signature
/// proves the joiner holds the key that the invitation will be bound to.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinRequest {
    pub goal: GoalId,
    pub member: PublicKey,
    /// The endpoint that will speak for `member`.
    pub endpoint: EndpointId,
    pub secret: [u8; 32],
    pub signature: Signature,
}

impl JoinRequest {
    pub fn sign(goal: GoalId, endpoint: EndpointId, secret: [u8; 32], key: &Keypair) -> Self {
        let member = key.public();
        let digest = join_digest(&goal, &member, &endpoint, &secret);
        Self {
            goal,
            member,
            endpoint,
            secret,
            signature: key.sign(domain::JOIN_SIGNATURE, &digest),
        }
    }

    pub fn verify(&self) -> bool {
        let digest = join_digest(&self.goal, &self.member, &self.endpoint, &self.secret);
        crypto::verify(
            &self.member,
            domain::JOIN_SIGNATURE,
            &digest,
            &self.signature,
        )
    }
}

impl fmt::Debug for JoinRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JoinRequest")
            .field("goal", &self.goal)
            .field("member", &self.member)
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

fn join_digest(
    goal: &GoalId,
    member: &PublicKey,
    endpoint: &EndpointId,
    secret: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = crypto::domain_hasher(domain::JOIN_SIGNATURE);
    hasher.update(&goal.0);
    hasher.update(&member.0);
    hasher.update(&endpoint.0);
    hasher.update(&secret_digest(secret));
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit;

    fn invitation() -> Invitation {
        Invitation {
            version: PROTOCOL_VERSION,
            goal: GoalId([1; 32]),
            coordinator: testkit::keypair(1).public(),
            endpoint: EndpointId([2; 32]),
            hints: vec!["https://relay.example".to_string()],
            secret: [0x5a; 32],
            expires_ms: Some(1_790_000_000_000),
        }
    }

    #[test]
    fn a_ticket_round_trips() {
        let ticket = invitation().to_ticket().unwrap();
        assert!(ticket.starts_with(TICKET_PREFIX));
        assert_eq!(
            Invitation::from_ticket(&format!("  {ticket}\n")),
            Ok(invitation())
        );
    }

    #[test]
    fn damaged_or_foreign_text_is_refused() {
        let ticket = invitation().to_ticket().unwrap();
        assert_eq!(
            Invitation::from_ticket("hello"),
            Err(InviteError::NotATicket)
        );
        assert_eq!(
            Invitation::from_ticket(&ticket[..ticket.len() - 2]),
            Err(InviteError::Malformed)
        );
        assert_eq!(
            Invitation::from_ticket(&format!("{ticket}00")),
            Err(InviteError::Malformed)
        );

        let other_version = Invitation {
            version: PROTOCOL_VERSION + 1,
            ..invitation()
        };
        let bytes = codec::encode(&other_version).unwrap();
        assert_eq!(
            Invitation::from_ticket(&format!("{TICKET_PREFIX}{}", Hex(&bytes))),
            Err(InviteError::UnsupportedVersion(PROTOCOL_VERSION + 1))
        );

        let crowded = Invitation {
            hints: vec![String::new(); MAX_HINTS + 1],
            ..invitation()
        };
        assert_eq!(crowded.to_ticket(), Err(InviteError::BadHints));
    }

    #[test]
    fn debug_output_never_shows_the_secret() {
        let printed = format!("{:?}", invitation());
        assert!(!printed.contains("5a5a"));
        assert!(!printed.contains("90, 90"));
    }

    #[test]
    fn a_join_request_is_bound_to_its_key_goal_and_endpoint() {
        let key = testkit::keypair(3);
        let request = JoinRequest::sign(GoalId([1; 32]), EndpointId([2; 32]), [9; 32], &key);
        assert!(request.verify());

        let mut moved = request.clone();
        moved.endpoint = EndpointId([7; 32]);
        assert!(!moved.verify());

        let mut stolen = request.clone();
        stolen.member = testkit::keypair(4).public();
        assert!(!stolen.verify());

        let mut other_goal = request;
        other_goal.goal = GoalId([8; 32]);
        assert!(!other_goal.verify());
    }
}
