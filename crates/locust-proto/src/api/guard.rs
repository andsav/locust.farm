//! What the restore guard holds on this daemon. A key is held in a goal while
//! this daemon cannot show that it holds everything the key signed there; it
//! then signs nothing in that goal with the key.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::id::{EndpointId, PublicKey};

/// One key of this daemon held in one goal, for its own reason. An agent held
/// only because the goal's governance key is held has no view of its own.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GuardView {
    pub key: PublicKey,
    /// True for the goal's governance key, which this daemon holds as host.
    pub by_host: bool,
    pub reason: GuardReason,
    /// Other computers in the goal heard from since this daemon started.
    pub heard: Vec<EndpointId>,
    /// Other computers in the goal not heard from yet.
    pub waiting: Vec<EndpointId>,
}

/// Why a key is held. Tagged the ordinary way, as `{"behind": {..}}`:
/// responses travel in postcard too.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GuardReason {
    /// This daemon signed `signed` records with the key in the goal and holds
    /// the first `held` of them.
    Behind { held: u64, signed: u64 },
    /// This daemon's data may be an old copy.
    Unheard,
    /// The key was just admitted here, and the host's computer has not been
    /// heard from since or this daemon cannot yet read the goal's current
    /// rules or holds no content key for its current epoch.
    Admitted,
}
