//! Invitations and the request that redeems one.
//!
//! An invitation is a single-use capability the inviter hands over through a
//! channel of their choosing. It names the goal, who will admit the joiner and
//! how to reach them. Redemption binds it to the first key that presents it:
//! retrying with the same key recovers the result, any other key is refused.
//! Holding an invitation shares nothing and enrolls nothing by itself.
//!
//! An invitation is signed by the goal's governance key, which only the
//! host's daemon holds, so the daemon that redeems one is the daemon that
//! signs the admission.

use std::fmt;
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crate::PROTOCOL_VERSION;
use crate::api::{InvitationPreview, InvitationPublication, InvitationSharing};
use crate::codec;
use crate::crypto::{self, Keypair, domain};
use crate::id::{EndpointId, GoalId, Hex, PublicKey, Signature, hex_to_vec};
use crate::limits::MAX_INVITATION_BYTES;

/// Text prefix of an invitation ticket.
pub const TICKET_PREFIX: &str = "locust-invite-";

/// Longest ticket text accepted: the prefix plus the hex of the largest
/// encoded invitation. Checked before anything is decoded.
pub const MAX_TICKET_BYTES: usize = TICKET_PREFIX.len() + 2 * MAX_INVITATION_BYTES;

/// Most contact hints one invitation may carry.
pub const MAX_HINTS: usize = 8;

/// Longest single contact hint, in bytes.
pub const MAX_HINT_BYTES: usize = 256;

/// The capability inside an invitation: whoever presents it first, with a key
/// of their choosing, is admitted. It travels only inside a ticket and in the
/// binary join request, and is never rendered.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct InviteSecret(pub [u8; 32]);

impl InviteSecret {
    /// What the issuer stores and looks up in place of the secret.
    pub fn digest(&self) -> [u8; 32] {
        crypto::domain_hash(domain::INVITE_SECRET, &self.0)
    }
}

impl fmt::Debug for InviteSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InviteSecret(..)")
    }
}

impl Serialize for InviteSecret {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str("<redacted>")
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for InviteSecret {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            Err(serde::de::Error::custom(
                "invitation secrets are never read from text",
            ))
        } else {
            <[u8; 32]>::deserialize(deserializer).map(Self)
        }
    }
}

/// An invitation in its pasteable text form: [`TICKET_PREFIX`] followed by
/// the hex of the encoded [`Invitation`].
///
/// The text is the capability, so `Debug` never shows it. Serialization is
/// the bare string in every format, because handing the text over is what a
/// ticket is for: it is the one secret-bearing value a JSON rendering shows.
/// Print it only where the inviter asked for it, with [`Ticket::as_str`].
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Ticket(pub String);

impl Ticket {
    /// The ticket text, for the one place that hands it to a person.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Ticket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Ticket(..)")
    }
}

/// What an invitation says. `version` is the first encoded byte, so a ticket
/// from another protocol version is reported as such before anything else is
/// read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invitation {
    /// [`PROTOCOL_VERSION`]; the first encoded byte.
    pub version: u8,
    /// The goal the holder is invited to.
    pub goal: GoalId,
    /// Presentation supplied and signed by the governance key. A title does
    /// not authenticate a human identity or establish the genesis record.
    pub goal_title: Option<String>,
    /// The goal's governance key, which signs this ticket and the admission. Shown
    /// to the joiner as a fingerprint and checked against the genesis record
    /// once it arrives.
    pub governance: PublicKey,
    pub host_name: String,
    /// The daemon that redeems the invitation.
    pub endpoint: EndpointId,
    /// Ways to reach `endpoint`; see [`Hint`]. At most [`MAX_HINTS`], each
    /// non-empty, at most [`MAX_HINT_BYTES`] and free of control characters.
    pub hints: Vec<String>,
    /// The capability. Never logged or printed outside the ticket itself.
    pub secret: InviteSecret,
    /// Issuer-chosen expiry in Unix milliseconds, judged by the issuer's clock.
    pub expires_ms: Option<u64>,
    /// The actual read boundary offered by this protocol.
    pub sharing: InvitationSharing,
    /// Publication policy advertised at issuance and reconciled against shared history.
    pub publication: Option<InvitationPublication>,
    /// Host signature over every preceding field, including the
    /// capability digest. Verified before any preview or join intent.
    pub role: Option<String>,
    pub signature: Signature,
}

/// Why text was not accepted as an invitation, or an invitation could not be
/// issued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InviteError {
    /// The text does not start with [`TICKET_PREFIX`].
    NotATicket,
    /// The text is longer than [`MAX_TICKET_BYTES`]. Nothing was decoded.
    TooLong,
    /// The text after the prefix is not the hex of one encoded invitation.
    Malformed,
    /// The invitation opens with this protocol version, which is not
    /// [`PROTOCOL_VERSION`].
    UnsupportedVersion(u8),
    /// Too many contact hints, or one that is empty, too long or carries a
    /// control character.
    BadHints,
    BadName,
    /// The governance key did not sign these exact invitation fields.
    InvalidSignature,
}

impl fmt::Display for InviteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotATicket => f.write_str("text is not a Locust invitation"),
            Self::TooLong => f.write_str("text is too long to be a Locust invitation"),
            Self::Malformed => f.write_str("invitation is damaged or incomplete"),
            Self::UnsupportedVersion(version) => {
                write!(f, "invitation uses unsupported protocol version {version}")
            }
            Self::BadName => f.write_str("invitation carries an unusable member or role name"),
            Self::BadHints => f.write_str("invitation carries unusable contact hints"),
            Self::InvalidSignature => f.write_str(
                "invitation signature is invalid; request a fresh invitation from the host",
            ),
        }
    }
}

impl std::error::Error for InviteError {}

/// What a consumer makes of one contact hint. A hint that is neither form is
/// skipped, never an error, so a later version can add forms without
/// breaking tickets for this one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hint<'a> {
    /// A relay URL: `https://` followed by anything. The whole hint.
    Relay(&'a str),
    /// A socket address: `ip:port`, with an IPv6 address in brackets.
    Address(SocketAddr),
}

impl<'a> Hint<'a> {
    /// Reads one hint; `None` for a form this version does not recognize.
    pub fn parse(hint: &'a str) -> Option<Self> {
        match hint.strip_prefix("https://") {
            Some("") => None,
            Some(_) => Some(Self::Relay(hint)),
            None => hint.parse().ok().map(Self::Address),
        }
    }
}

/// True if `hint` may be carried: non-empty, within [`MAX_HINT_BYTES`] and
/// with no control character, so a hint can be shown to the joiner as is.
fn is_hint_text(hint: &str) -> bool {
    !hint.is_empty()
        && hint.len() <= MAX_HINT_BYTES
        && !hint.bytes().any(|byte| byte < 0x20 || byte == 0x7f)
}

impl Invitation {
    /// Signs all reviewable facts and the capability together.
    #[allow(clippy::too_many_arguments)]
    pub fn signed(
        goal: GoalId,
        goal_title: Option<String>,
        endpoint: EndpointId,
        hints: Vec<String>,
        secret: InviteSecret,
        expires_ms: Option<u64>,
        host_name: String,
        role: Option<String>,
        governance: &Keypair,
    ) -> Result<Self, InviteError> {
        let mut invitation = Self {
            version: PROTOCOL_VERSION,
            goal,
            goal_title,
            governance: governance.public(),
            host_name,
            role,
            endpoint,
            hints,
            secret,
            expires_ms,
            sharing: InvitationSharing::WholeGoal,
            publication: None,
            signature: Signature([0; 64]),
        };
        invitation.sign(governance)?;
        Ok(invitation)
    }

    /// Re-signs the fields the governance key owns. A different key cannot
    /// attest to the named governance key.
    pub fn sign(&mut self, governance: &Keypair) -> Result<(), InviteError> {
        if governance.public() != self.governance {
            return Err(InviteError::InvalidSignature);
        }
        self.signature = governance.sign(INVITATION_SIGNATURE, &self.signing_digest()?);
        Ok(())
    }

    fn signing_digest(&self) -> Result<[u8; 32], InviteError> {
        let bytes = codec::encode(&(
            self.version,
            self.goal,
            &self.goal_title,
            self.governance,
            &self.host_name,
            self.endpoint,
            &self.hints,
            self.secret.digest(),
            self.expires_ms,
            self.sharing,
            &self.publication,
            &self.role,
        ))
        .map_err(|_| InviteError::Malformed)?;
        Ok(crypto::domain_hash(INVITATION_SIGNATURE, &bytes))
    }

    pub fn verify(&self) -> Result<(), InviteError> {
        self.check()?;
        if crypto::verify(
            &self.governance,
            INVITATION_SIGNATURE,
            &self.signing_digest()?,
            &self.signature,
        ) {
            Ok(())
        } else {
            Err(InviteError::InvalidSignature)
        }
    }

    /// Verified, capability-free review. The digest covers the complete
    /// canonical signed invitation, so whitespace wrapping does not change
    /// it and substituting a different valid ticket does.
    pub fn preview(&self, now_ms: u64) -> Result<InvitationPreview, InviteError> {
        self.verify()?;
        let bytes = codec::encode(self).map_err(|_| InviteError::Malformed)?;
        Ok(InvitationPreview {
            goal: self.goal,
            goal_title: self.goal_title.clone(),
            governance: self.governance,
            host_name: self.host_name.clone(),
            role: self.role.clone(),
            endpoint: self.endpoint,
            hints: self.hints.clone(),
            expires_ms: self.expires_ms,
            expired: self.expires_ms.is_some_and(|expiry| now_ms >= expiry),
            sharing: self.sharing,
            publication: self.publication.clone(),
            review: format!("{}", Hex(&crypto::domain_hash(INVITATION_REVIEW, &bytes))),
            signature_verified: true,
            title_provenance: "host_signed_presentation".into(),
            identity_provenance: "signing_key_only; human_identity_not_verified".into(),
            admission_status: "unconfirmed; checked_with_issuer_on_join".into(),
            sharing_facts: InvitationPreview::sharing_facts(),
        })
    }
    /// Checks the version and the contact hints. Runs when a ticket is
    /// written and again when one is read.
    fn check(&self) -> Result<(), InviteError> {
        if !crate::event::is_member_name(&self.host_name)
            || self
                .role
                .as_ref()
                .is_some_and(|role| !crate::organization::is_role_name(role))
        {
            return Err(InviteError::BadName);
        }
        if self.version != PROTOCOL_VERSION {
            return Err(InviteError::UnsupportedVersion(self.version));
        }
        if let Some(advertised) = &self.publication {
            let publication = &advertised.publication;
            if publication.farm_id != crate::farm::FarmId::from_key(publication.upload_key)
                || publication.policy.validate().is_err()
            {
                return Err(InviteError::Malformed);
            }
        }
        if self.hints.len() <= MAX_HINTS && self.hints.iter().all(|hint| is_hint_text(hint)) {
            Ok(())
        } else {
            Err(InviteError::BadHints)
        }
    }

    /// The pasteable form, bounded by [`MAX_INVITATION_BYTES`] including its
    /// title. Readers authenticate the signature before showing a preview.
    pub fn to_ticket(&self) -> Result<Ticket, InviteError> {
        self.check()?;
        let bytes = codec::encode(self).map_err(|_| InviteError::Malformed)?;
        if bytes.len() > MAX_INVITATION_BYTES {
            return Err(InviteError::TooLong);
        }
        Ok(Ticket(format!("{TICKET_PREFIX}{}", Hex(&bytes))))
    }

    /// Reads pasted text. Surrounding whitespace is ignored, and so is ASCII
    /// whitespace inside the hex, because chat and mail clients wrap long
    /// lines. Text longer than [`MAX_TICKET_BYTES`] is refused before any of
    /// it is decoded.
    pub fn from_ticket(ticket: &str) -> Result<Self, InviteError> {
        let text = ticket.trim();
        if text.len() > MAX_TICKET_BYTES {
            return Err(InviteError::TooLong);
        }
        let body = text
            .strip_prefix(TICKET_PREFIX)
            .ok_or(InviteError::NotATicket)?;
        let bytes = if body.bytes().any(|byte| byte.is_ascii_whitespace()) {
            let joined: String = body
                .chars()
                .filter(|character| !character.is_ascii_whitespace())
                .collect();
            hex_to_vec(&joined)
        } else {
            hex_to_vec(body)
        }
        .map_err(|_| InviteError::Malformed)?;
        match bytes.first() {
            Some(&PROTOCOL_VERSION) => {}
            Some(&version) => return Err(InviteError::UnsupportedVersion(version)),
            None => return Err(InviteError::Malformed),
        }
        let invitation: Self = codec::decode(&bytes).map_err(|_| InviteError::Malformed)?;
        invitation.verify()?;
        Ok(invitation)
    }

    /// What the issuer stores in place of the secret.
    pub fn secret_digest(&self) -> [u8; 32] {
        self.secret.digest()
    }
}

const INVITATION_SIGNATURE: &str = "locust invitation signature";
const INVITATION_REVIEW: &str = "locust invitation review";

/// Sent to the inviting daemon before the joiner is a member. The signature
/// proves the joiner holds the key that the invitation will be bound to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinRequest {
    pub name: String,
    /// The goal to join.
    pub goal: GoalId,
    /// The key to admit.
    pub member: PublicKey,
    /// The endpoint that will speak for `member`.
    pub endpoint: EndpointId,
    /// The invitation's capability.
    pub secret: InviteSecret,
    /// By `member`, over the goal, member, endpoint and the secret's digest.
    pub signature: Signature,
}

impl JoinRequest {
    /// Builds the request `key` sends to redeem an invitation for itself,
    /// with `endpoint` as the daemon that will speak for it.
    pub fn sign(
        goal: GoalId,
        endpoint: EndpointId,
        name: String,
        secret: InviteSecret,
        key: &Keypair,
    ) -> Self {
        let member = key.public();
        let digest = join_digest(&goal, &member, &endpoint, &name, &secret);
        Self {
            goal,
            member,
            endpoint,
            name,
            secret,
            signature: key.sign(domain::JOIN_SIGNATURE, &digest),
        }
    }

    /// True if `member` signed this goal, endpoint and secret together.
    ///
    /// This proves key possession only. The caller must also require
    /// `endpoint` to equal the authenticated remote endpoint of the link the
    /// request arrived on; otherwise a relayed copy of the request would bind
    /// the member to an endpoint that never asked. Whether the secret names a
    /// live invitation is the issuer's lookup by [`InviteSecret::digest`].
    pub fn verify(&self) -> bool {
        let digest = join_digest(
            &self.goal,
            &self.member,
            &self.endpoint,
            &self.name,
            &self.secret,
        );
        crate::event::is_member_name(&self.name)
            && crypto::verify(
                &self.member,
                domain::JOIN_SIGNATURE,
                &digest,
                &self.signature,
            )
    }
}

fn join_digest(
    goal: &GoalId,
    member: &PublicKey,
    endpoint: &EndpointId,
    name: &str,
    secret: &InviteSecret,
) -> [u8; 32] {
    let mut hasher = crypto::domain_hasher(domain::JOIN_SIGNATURE);
    hasher.update(&goal.0);
    hasher.update(&member.0);
    hasher.update(&endpoint.0);
    hasher.update(&(name.len() as u64).to_le_bytes());
    hasher.update(name.as_bytes());
    hasher.update(&secret.digest());
    *hasher.finalize().as_bytes()
}

impl schemars::JsonSchema for Ticket {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Ticket".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"type":"string","pattern":"^locust-invite-[0-9a-f]+$"})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit;

    fn invitation() -> Invitation {
        Invitation::signed(
            GoalId([1; 32]),
            Some("Shared compiler work".into()),
            EndpointId([2; 32]),
            vec!["https://relay.example".to_string()],
            InviteSecret([0x5a; 32]),
            Some(1_790_000_000_000),
            "host".into(),
            None,
            &testkit::keypair(1),
        )
        .unwrap()
    }

    #[test]
    fn publication_policy_is_signed_and_reviewable() {
        use crate::farm::{DisclosurePolicy, FARM_VERSION, FarmId, FarmVisibility, PublicationSet};
        let mut ticket = invitation();
        let key = testkit::keypair(9).public();
        ticket.publication = Some(InvitationPublication {
            event: crate::id::EventId([7; 32]),
            publication: PublicationSet {
                farm_id: FarmId::from_key(key),
                upload_key: key,
                visibility: Some(FarmVisibility::Listed),
                policy: DisclosurePolicy {
                    version: FARM_VERSION,
                    title: None,
                    formation: "Shared work".into(),
                    stage_labels: Default::default(),
                    role_labels: Default::default(),
                    recent_changes: 20,
                },
            },
        });
        assert!(ticket.verify().is_err());
        ticket.sign(&testkit::keypair(1)).unwrap();
        let decoded = Invitation::from_ticket(ticket.to_ticket().unwrap().as_str()).unwrap();
        assert_eq!(decoded.preview(0).unwrap().publication, ticket.publication);
        ticket
            .publication
            .as_mut()
            .unwrap()
            .publication
            .policy
            .formation = "Changed".into();
        assert_eq!(ticket.verify(), Err(InviteError::InvalidSignature));
    }

    fn with_hints(hints: &[&str]) -> Invitation {
        let mut invitation = Invitation {
            hints: hints.iter().map(|hint| hint.to_string()).collect(),
            ..invitation()
        };
        invitation.sign(&testkit::keypair(1)).unwrap();
        invitation
    }

    #[test]
    fn a_ticket_round_trips() {
        let ticket = invitation().to_ticket().unwrap();
        assert!(ticket.as_str().starts_with(TICKET_PREFIX));
        assert_eq!(
            Invitation::from_ticket(&format!("  {}\n", ticket.as_str())),
            Ok(invitation())
        );
    }

    #[test]
    fn every_reviewed_fact_and_capability_is_signed() {
        let original = invitation();
        let changes: [fn(&mut Invitation); 10] = [
            |i| i.goal = GoalId([9; 32]),
            |i| i.goal_title = Some("Different goal".into()),
            |i| i.host_name = "another host".into(),
            |i| i.role = Some("reviewer".into()),
            |i| i.governance = testkit::keypair(9).public(),
            |i| i.endpoint = EndpointId([9; 32]),
            |i| i.hints = vec!["https://other.example".into()],
            |i| i.secret = InviteSecret([9; 32]),
            |i| i.expires_ms = None,
            |i| i.signature.0[0] ^= 1,
        ];
        for change in changes {
            let mut altered = original.clone();
            change(&mut altered);
            assert_eq!(
                Invitation::from_ticket(altered.to_ticket().unwrap().as_str()),
                Err(InviteError::InvalidSignature)
            );
            assert_eq!(altered.preview(0), Err(InviteError::InvalidSignature));
        }
    }

    #[test]
    fn preview_identifies_exact_ticket_without_rendering_its_capability() {
        let invitation = invitation();
        let preview = invitation.preview(0).unwrap();
        let serialized = serde_json::to_string(&preview).unwrap();
        assert!(!serialized.contains(&"5a".repeat(32)));
        assert!(!serialized.contains(TICKET_PREFIX));
        assert!(preview.signature_verified);
        assert!(!preview.expired);
        assert_eq!(preview.sharing, InvitationSharing::WholeGoal);
        assert!(invitation.preview(u64::MAX).unwrap().expired);
        let mut changed = invitation.clone();
        changed.expires_ms = None;
        changed.sign(&testkit::keypair(1)).unwrap();
        assert_ne!(preview.review, changed.preview(0).unwrap().review);
        let wrapped = format!("  {}\n", invitation.to_ticket().unwrap().as_str());
        assert_eq!(
            preview,
            Invitation::from_ticket(&wrapped)
                .unwrap()
                .preview(0)
                .unwrap()
        );
    }

    #[test]
    fn signed_title_must_fit_the_published_ticket_envelope() {
        let mut invitation = invitation();
        invitation.goal_title = Some("x".repeat(MAX_INVITATION_BYTES));
        invitation.sign(&testkit::keypair(1)).unwrap();
        assert_eq!(invitation.to_ticket(), Err(InviteError::TooLong));
    }

    #[test]
    fn damaged_or_foreign_text_is_refused() {
        let ticket = invitation().to_ticket().unwrap();
        let ticket = ticket.as_str();
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
        assert_eq!(
            Invitation::from_ticket(&ticket[..ticket.len() - 1]),
            Err(InviteError::Malformed)
        );
        assert_eq!(
            Invitation::from_ticket(&format!("{ticket}zz")),
            Err(InviteError::Malformed)
        );
        assert_eq!(
            Invitation::from_ticket(TICKET_PREFIX),
            Err(InviteError::Malformed)
        );

        let other_version = Invitation {
            version: PROTOCOL_VERSION + 1,
            ..invitation()
        };
        assert_eq!(
            other_version.to_ticket(),
            Err(InviteError::UnsupportedVersion(PROTOCOL_VERSION + 1))
        );
        let bytes = codec::encode(&other_version).unwrap();
        assert_eq!(
            Invitation::from_ticket(&format!("{TICKET_PREFIX}{}", Hex(&bytes))),
            Err(InviteError::UnsupportedVersion(PROTOCOL_VERSION + 1))
        );
    }

    #[test]
    fn whitespace_from_line_wrapping_is_dropped_from_the_hex() {
        let ticket = invitation().to_ticket().unwrap();
        let (prefix, hex) = ticket.as_str().split_at(TICKET_PREFIX.len());
        let wrapped = hex
            .as_bytes()
            .chunks(40)
            .map(|line| std::str::from_utf8(line).unwrap())
            .collect::<Vec<_>>()
            .join("\r\n\t ");
        assert_eq!(
            Invitation::from_ticket(&format!("{prefix}{wrapped}")),
            Ok(invitation())
        );
        // Whitespace is dropped from the hex only: a broken prefix is not a ticket.
        assert_eq!(
            Invitation::from_ticket(&format!("locust-in vite-{hex}")),
            Err(InviteError::NotATicket)
        );
    }

    #[test]
    fn overlong_text_is_refused_before_it_is_decoded() {
        let exact = format!(
            "{TICKET_PREFIX}{PROTOCOL_VERSION:02x}{}",
            "0".repeat(MAX_TICKET_BYTES - TICKET_PREFIX.len() - 2)
        );
        assert_eq!(exact.len(), MAX_TICKET_BYTES);
        // Within the limit the text is decoded and found damaged.
        assert_eq!(Invitation::from_ticket(&exact), Err(InviteError::Malformed));
        assert_eq!(
            Invitation::from_ticket(&format!("{exact}0")),
            Err(InviteError::TooLong)
        );
        // The limit is on the text, whatever it is, and ignores outer whitespace.
        assert_eq!(
            Invitation::from_ticket(&"x".repeat(MAX_TICKET_BYTES + 1)),
            Err(InviteError::TooLong)
        );
        assert_eq!(
            Invitation::from_ticket(&format!("\n{exact}\n\n")),
            Err(InviteError::Malformed)
        );
    }

    #[test]
    fn the_largest_valid_invitation_fits_the_published_size() {
        let mut largest = Invitation {
            host_name: "h".repeat(crate::limits::MAX_MEMBER_NAME_BYTES),
            hints: vec!["h".repeat(MAX_HINT_BYTES); MAX_HINTS],
            expires_ms: Some(u64::MAX),
            ..invitation()
        };
        largest.sign(&testkit::keypair(1)).unwrap();
        let ticket = largest.to_ticket().unwrap();
        assert!(codec::encode(&largest).unwrap().len() <= MAX_INVITATION_BYTES);
        assert!(ticket.as_str().len() <= MAX_TICKET_BYTES);
        assert_eq!(Invitation::from_ticket(ticket.as_str()), Ok(largest));
    }

    #[test]
    fn a_role_is_bounded_by_the_whole_invitation_size() {
        let mut invitation = invitation();
        invitation.role = Some("r".repeat(MAX_INVITATION_BYTES));
        invitation.sign(&testkit::keypair(1)).unwrap();
        assert_eq!(invitation.to_ticket(), Err(InviteError::TooLong));
    }

    #[test]
    fn unusable_hints_are_refused_when_writing_and_when_reading() {
        let relay = "https://relay.example";
        let cases: [Vec<&str>; 6] = [
            vec![relay; MAX_HINTS + 1],
            vec![""],
            vec![relay, ""],
            vec!["https://relay.example/\n"],
            vec!["relay\u{1b}[31m"],
            vec!["relay\u{7f}"],
        ];
        for hints in cases {
            let invitation = with_hints(&hints);
            assert_eq!(
                invitation.to_ticket(),
                Err(InviteError::BadHints),
                "{hints:?}"
            );
            let bytes = codec::encode(&invitation).unwrap();
            assert_eq!(
                Invitation::from_ticket(&format!("{TICKET_PREFIX}{}", Hex(&bytes))),
                Err(InviteError::BadHints),
                "{hints:?}"
            );
        }
        let too_long = "h".repeat(MAX_HINT_BYTES + 1);
        assert_eq!(
            with_hints(&[&too_long]).to_ticket(),
            Err(InviteError::BadHints)
        );

        assert!(with_hints(&[]).to_ticket().is_ok());
        assert!(with_hints(&[relay; MAX_HINTS]).to_ticket().is_ok());
        // Printable text of an unknown form is carried; consumers skip it.
        assert!(with_hints(&["café relay ü"]).to_ticket().is_ok());
    }

    #[test]
    fn a_hint_is_a_relay_url_or_a_socket_address_and_anything_else_is_skipped() {
        assert_eq!(
            Hint::parse("https://relay.example/path"),
            Some(Hint::Relay("https://relay.example/path"))
        );
        assert_eq!(
            Hint::parse("203.0.113.7:4433"),
            Some(Hint::Address("203.0.113.7:4433".parse().unwrap()))
        );
        assert_eq!(
            Hint::parse("[2001:db8::1]:4433"),
            Some(Hint::Address("[2001:db8::1]:4433".parse().unwrap()))
        );
        for unknown in [
            "",
            "https://",
            "http://relay.example",
            "relay.example:4433",
            "203.0.113.7",
            "quic://203.0.113.7:4433",
        ] {
            assert_eq!(Hint::parse(unknown), None, "{unknown:?}");
        }
        // An unrecognized hint is still a valid part of an invitation.
        let mixed = with_hints(&["quic://later", "203.0.113.7:4433"]);
        let read = Invitation::from_ticket(mixed.to_ticket().unwrap().as_str()).unwrap();
        let known: Vec<Hint> = read.hints.iter().filter_map(|h| Hint::parse(h)).collect();
        assert_eq!(known, [Hint::Address("203.0.113.7:4433".parse().unwrap())]);
    }

    #[test]
    fn no_rendering_shows_the_secret() {
        let invitation = invitation();
        let ticket = invitation.to_ticket().unwrap();
        let request = JoinRequest::sign(
            invitation.goal,
            EndpointId([3; 32]),
            "member".into(),
            invitation.secret,
            &testkit::keypair(3),
        );
        let secret_hex = "5a".repeat(32);
        assert!(ticket.as_str().contains(&secret_hex));

        let renderings = [
            format!("{invitation:?}"),
            format!("{:?}", invitation.secret),
            format!("{ticket:?}"),
            format!("{request:?}"),
            serde_json::to_string(&invitation).unwrap(),
            serde_json::to_string(&invitation.secret).unwrap(),
            serde_json::to_string(&request).unwrap(),
        ];
        for rendering in &renderings {
            assert!(!rendering.contains("5a5a"), "{rendering}");
            assert!(!rendering.contains("90, 90"), "{rendering}");
            assert!(!rendering.contains("90,90"), "{rendering}");
        }
        assert_eq!(format!("{ticket:?}"), "Ticket(..)");
        assert_eq!(format!("{:?}", invitation.secret), "InviteSecret(..)");
        assert!(renderings[4].contains("\"secret\":\"<redacted>\""));

        // A secret is never read back from text, so a rendering cannot be
        // mistaken for the real thing.
        assert!(serde_json::from_str::<Invitation>(&renderings[4]).is_err());
        assert!(serde_json::from_str::<JoinRequest>(&renderings[6]).is_err());
        assert!(serde_json::from_str::<InviteSecret>(&format!("\"{secret_hex}\"")).is_err());
    }

    #[test]
    fn a_ticket_serializes_as_its_bare_text() {
        let ticket = invitation().to_ticket().unwrap();
        let json = serde_json::to_string(&ticket).unwrap();
        assert_eq!(json, format!("\"{}\"", ticket.as_str()));
        assert_eq!(serde_json::from_str::<Ticket>(&json).unwrap(), ticket);
        assert_eq!(
            codec::encode(&ticket).unwrap(),
            codec::encode(ticket.as_str()).unwrap()
        );
    }

    #[test]
    fn the_secret_newtype_keeps_the_binary_encoding_of_raw_bytes() {
        let secret = InviteSecret([0x5a; 32]);
        let bytes = codec::encode(&secret).unwrap();
        assert_eq!(bytes, vec![0x5a; 32]);
        assert_eq!(codec::decode::<InviteSecret>(&bytes), Ok(secret));
        assert_eq!(bytes, codec::encode(&[0x5au8; 32]).unwrap());

        let request = JoinRequest::sign(
            GoalId([1; 32]),
            EndpointId([2; 32]),
            "member".into(),
            secret,
            &testkit::keypair(3),
        );
        let frame = codec::encode(&request).unwrap();
        assert_eq!(frame.len(), 32 + 32 + 32 + 32 + 64 + 7);
        assert_eq!(codec::decode::<JoinRequest>(&frame), Ok(request));
    }

    #[test]
    fn the_issuer_keeps_a_digest_not_the_secret() {
        let invitation = invitation();
        assert_eq!(invitation.secret_digest(), invitation.secret.digest());
        assert_ne!(invitation.secret_digest(), invitation.secret.0);
        assert_ne!(
            invitation.secret_digest(),
            InviteSecret([0x5b; 32]).digest()
        );
    }

    #[test]
    fn a_join_request_is_bound_to_its_key_goal_endpoint_name_and_secret() {
        let key = testkit::keypair(3);
        let request = JoinRequest::sign(
            GoalId([1; 32]),
            EndpointId([2; 32]),
            "member".into(),
            InviteSecret([9; 32]),
            &key,
        );
        assert!(request.verify());

        let mut moved = request.clone();
        moved.endpoint = EndpointId([7; 32]);
        assert!(!moved.verify());

        let mut stolen = request.clone();
        stolen.member = testkit::keypair(4).public();
        assert!(!stolen.verify());

        let mut other_secret = request.clone();
        other_secret.secret = InviteSecret([8; 32]);
        assert!(!other_secret.verify());

        let mut other_name = request.clone();
        other_name.name = "other".into();
        assert!(!other_name.verify());
        let mut other_goal = request;
        other_goal.goal = GoalId([8; 32]);
        assert!(!other_goal.verify());
    }
}
