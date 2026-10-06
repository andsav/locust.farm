//! Who this daemon is and the principals it holds keys for.
//!
//! `Space::Identity` holds the endpoint secret, the digest of the owner
//! credential and the transport's last reported endpoint and hints.
//! `Space::Agent` holds one record per principal, keyed by its public key,
//! The index
//! from a principal's credential digest to the principal is rebuilt from the
//! principal records when they are loaded, so it cannot disagree with them.

use std::collections::{BTreeMap, HashMap};

use locust_proto::api::{AgentView, Caller};
use locust_proto::crypto::Keypair;
use locust_proto::id::{EndpointId, PublicKey};
use locust_proto::store::{LocalWrite, Space, StoreError};
use serde::{Deserialize, Serialize};

use super::records;

const ENDPOINT_SECRET: &[u8] = b"endpoint-secret";
const OWNER_CREDENTIAL: &[u8] = b"owner-credential";
const ENDPOINT: &[u8] = b"endpoint";

const PRINCIPAL: u8 = b'p';

/// The transport's identity and how to reach it, as last reported by the
/// shell. Stored so an invitation can be issued before the transport reports
/// again after a restart.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct EndpointRecord {
    pub endpoint: EndpointId,
    pub hints: Vec<String>,
}

/// The daemon's own identity.
#[derive(Debug)]
pub(super) struct Identity {
    /// What the transport's endpoint identity is derived from.
    pub endpoint_secret: [u8; 32],
    /// Digest of the owner credential, as the shell passed it at start.
    pub owner: [u8; 32],
    /// Absent until the transport has reported once.
    pub endpoint: Option<EndpointRecord>,
}

impl Identity {
    pub fn secret_write(secret: &[u8; 32]) -> LocalWrite {
        records::put(Space::Identity, ENDPOINT_SECRET.to_vec(), secret)
    }

    pub fn owner_write(digest: &[u8; 32]) -> LocalWrite {
        records::put(Space::Identity, OWNER_CREDENTIAL.to_vec(), digest)
    }

    pub fn endpoint_write(record: &EndpointRecord) -> LocalWrite {
        records::put(Space::Identity, ENDPOINT.to_vec(), record)
    }

    /// Applies one committed write of `Space::Identity`.
    pub fn absorb(&mut self, key: &[u8], value: Option<&[u8]>) -> Result<(), StoreError> {
        let Some(value) = value else { return Ok(()) };
        match key {
            ENDPOINT_SECRET => self.endpoint_secret = records::read(value)?,
            OWNER_CREDENTIAL => self.owner = records::read(value)?,
            ENDPOINT => self.endpoint = Some(records::read(value)?),
            _ => return Err(records::bad_key()),
        }
        Ok(())
    }
}

/// What the store keeps about one enrolled principal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct PrincipalRecord {
    pub name: String,
    /// The signing seed. It never leaves the daemon.
    pub seed: [u8; 32],
    /// Digest of the principal's credential.
    pub credential: [u8; 32],
    pub revoked: bool,
    pub author_only: bool,
}

/// One enrolled principal with its signing key ready.
#[derive(Debug)]
pub(super) struct Principal {
    pub record: PrincipalRecord,
    pub key: Keypair,
}

impl Principal {
    pub fn view(&self) -> AgentView {
        AgentView {
            agent: self.key.public(),
            name: self.record.name.clone(),
            author_only: self.record.author_only,
            revoked: self.record.revoked,
        }
    }
}

/// Every enrolled principal, and what each credential digest resolves to.
#[derive(Debug, Default)]
pub(super) struct Principals {
    by_key: BTreeMap<PublicKey, Principal>,
    by_name: BTreeMap<String, PublicKey>,
    /// Agent credentials. A revoked principal's entries stay, so
    /// the digest stays taken; resolution checks the revoked flag.
    by_credential: HashMap<[u8; 32], Caller>,
}

impl Principals {
    pub fn principal_write(key: &PublicKey, record: &PrincipalRecord) -> LocalWrite {
        records::put(Space::Agent, records::key(PRINCIPAL, &[&key.0]), record)
    }

    /// Applies one committed write of `Space::Agent`.
    pub fn absorb(&mut self, key: &[u8], value: Option<&[u8]>) -> Result<(), StoreError> {
        let Some(value) = value else { return Ok(()) };
        match key.first() {
            Some(&PRINCIPAL) => {
                let record: PrincipalRecord = records::read(value)?;
                let keypair = Keypair::from_seed(record.seed);
                let public = keypair.public();
                self.by_name.insert(record.name.clone(), public);
                self.by_credential.insert(
                    record.credential,
                    if record.author_only {
                        Caller::Author(public)
                    } else {
                        Caller::Agent(public)
                    },
                );
                self.by_key.insert(
                    public,
                    Principal {
                        record,
                        key: keypair,
                    },
                );
            }
            _ => return Err(records::bad_key()),
        }
        Ok(())
    }

    /// The principal with this key, revoked or not.
    pub fn get(&self, key: &PublicKey) -> Option<&Principal> {
        self.by_key.get(key)
    }

    /// The principal with this key if its credential still works.
    pub fn active(&self, key: &PublicKey) -> Option<&Principal> {
        self.by_key.get(key).filter(|found| !found.record.revoked)
    }

    pub fn inactive_message(&self, key: &PublicKey) -> String {
        if self
            .get(key)
            .is_some_and(|principal| principal.record.revoked)
        {
            format!(
                "the agent is disconnected. Connect it again: locust --owner agent reconnect --agent {key}"
            )
        } else {
            "no enrolled principal has that key".into()
        }
    }

    pub fn by_name(&self, name: &str) -> Option<&Principal> {
        self.by_name.get(name).and_then(|key| self.by_key.get(key))
    }

    /// What a credential digest names, whether or not it still works.
    pub fn credential(&self, digest: &[u8; 32]) -> Option<Caller> {
        self.by_credential.get(digest).copied()
    }

    /// Every principal, ascending by key.
    pub fn iter(&self) -> impl Iterator<Item = &Principal> {
        self.by_key.values()
    }

    /// True if this daemon holds the signing key of `key`.
    pub fn holds(&self, key: &PublicKey) -> bool {
        self.by_key.contains_key(key)
    }
}
