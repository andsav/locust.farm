//! Local levels and structured reasons for a refused action.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Claim, Membership};
use crate::event::{AttemptStatus, TaskId};
use crate::id::{EffectId, EventId, GoalId, PublicKey};
use crate::organization::Selector;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Read,
    Ask,
    #[default]
    Auto,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Rule {
    Propose,
    Publish,
    Start,
    Offer,
    Declare,
    Review,
    Attest,
    Select,
    Finish,
    Integrate,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Act {
    Post,
    OpenTask,
    TakeTask,
    Resume,
    Approve,
    Attest,
    DeclareDone,
    Pick,
    Close,
    Reopen,
    HandOut,
    Cancel,
    MergeFiles,
    ProposeFiles,
    ConnectFolder,
    Start,
    Join,
    Leave,
    Invite,
    RemoveMember,
    ChangeRules,
    GiveRole,
    Revise,
    Publish,
    Withdraw,
    PersonCommand,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "side", rename_all = "snake_case")]
pub enum Why {
    YourSetting {
        level: Level,
        needs: Level,
    },
    Rules {
        rule: Rule,
        #[serde(with = "selector_wire")]
        #[schemars(with = "Selector")]
        qualifies: Selector,
        except_author: bool,
        host: PublicKey,
        host_name: Option<String>,
    },
    State {
        reason: String,
    },
    OnlyYou {
        operation: String,
        host: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Refused {
    pub agent: PublicKey,
    pub agent_name: String,
    pub member_name: Option<String>,
    pub goal: Option<GoalId>,
    pub goal_title: Option<String>,
    pub act: Act,
    pub task: Option<TaskId>,
    pub task_title: Option<String>,
    pub why: Why,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Ability {
    pub rule: Rule,
    #[serde(with = "selector_wire")]
    #[schemars(with = "Selector")]
    pub qualifies: Selector,
    pub except_author: bool,
    pub eligible: bool,
    pub needs: Level,
    pub allowed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WantedTask {
    pub task: TaskId,
    pub title: Option<String>,
    pub since_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Abilities {
    pub goal: GoalId,
    pub agent: PublicKey,
    pub name: String,
    pub membership: Option<Membership>,
    pub level: Level,
    pub host: Option<PublicKey>,
    pub hosted_here: bool,
    pub roles: Vec<String>,
    pub rules: Vec<Ability>,
    pub allowed_tasks: Vec<TaskId>,
    pub wanted_tasks: Vec<WantedTask>,
    pub claims: Vec<Claim>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Stalled {
    pub effect: EffectId,
    pub runner: PublicKey,
    pub reason: Stall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Stall {
    RunnerRevoked,
    RunnerLeft,
    RunnerNotMember,
    Halted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Attempting {
    pub member: PublicKey,
    pub status: Option<AttemptStatus>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Verdict {
    pub member: PublicKey,
    pub approve: bool,
    pub event: EventId,
}

/// Selectors use an internally tagged JSON representation in the organization
/// format. Postcard cannot decode that representation, so API binary frames
/// carry the same JSON as one string. Human-readable API JSON stays structured.
mod selector_wire {
    use super::Selector;
    use serde::{Deserialize, Serialize};

    pub fn serialize<S: serde::Serializer>(
        value: &Selector,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            value.serialize(serializer)
        } else {
            serde_json::to_string(value)
                .map_err(serde::ser::Error::custom)?
                .serialize(serializer)
        }
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Selector, D::Error> {
        if deserializer.is_human_readable() {
            Selector::deserialize(deserializer)
        } else {
            let json = String::deserialize(deserializer)?;
            serde_json::from_str(&json).map_err(serde::de::Error::custom)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refusals_keep_their_wire_names() {
        assert_eq!(serde_json::to_value(Level::Ask).unwrap(), "ask");
        assert_eq!(serde_json::to_value(Rule::Attest).unwrap(), "attest");
        assert_eq!(serde_json::to_value(Act::TakeTask).unwrap(), "take_task");
        assert_eq!(
            serde_json::to_value(Why::YourSetting {
                level: Level::Read,
                needs: Level::Ask
            })
            .unwrap()["side"],
            "your_setting"
        );
    }
    #[test]
    fn ability_selector_roundtrips_in_binary_and_json() {
        let ability = Ability {
            rule: Rule::Start,
            qualifies: Selector::Any {
                selectors: vec![
                    Selector::Members,
                    Selector::Role {
                        name: "worker".into(),
                    },
                ],
            },
            except_author: false,
            eligible: true,
            needs: Level::Auto,
            allowed: true,
        };
        let bytes = crate::codec::encode(&ability).unwrap();
        assert_eq!(crate::codec::decode::<Ability>(&bytes).unwrap(), ability);
        let json = serde_json::to_value(&ability).unwrap();
        assert!(json["qualifies"].is_object());
        assert_eq!(serde_json::from_value::<Ability>(json).unwrap(), ability);
    }
}
