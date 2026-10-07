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
    CannotMaterialize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Attempting {
    pub member: PublicKey,
    pub status: Option<AttemptStatus>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Verdict {
    pub opinion: bool,
    pub member: PublicKey,
    pub approve: bool,
    pub event: EventId,
}

/// Whose words a refusal is put in: the person who owns the agent, or the
/// agent itself, which is told nothing another member wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Voice {
    Person,
    Agent,
}

/// A refusal as one sentence: `WHO can't ACT: REASON (SIDE). FIX`. The
/// person's voice names the agent by its name in the goal and quotes titles
/// and role names; the agent's voice names it by its local name and says
/// "this task", "this goal", "the host" and "a role".
pub fn render(refused: &Refused, voice: Voice) -> String {
    let who = match voice {
        Voice::Person => safe(
            refused
                .member_name
                .as_deref()
                .unwrap_or(&refused.agent_name),
        ),
        Voice::Agent => refused.agent_name.clone(),
    };
    let goal = refused.goal.map(|_| match voice {
        Voice::Person => refused
            .goal_title
            .as_deref()
            .map(quoted)
            .unwrap_or_else(|| "this goal".into()),
        Voice::Agent => "this goal".into(),
    });
    let task = match voice {
        Voice::Person => refused
            .task_title
            .as_deref()
            .map(quoted)
            .unwrap_or_else(|| "this task".into()),
        Voice::Agent => "this task".into(),
    };
    let act = act_phrase(refused.act, goal.as_deref(), &task);
    let (reason, side, fix) = match &refused.why {
        Why::YourSetting { level, needs } => {
            let level = level_word(*level);
            let needs = level_word(*needs);
            let allow = refused.task.is_some() && needs == "auto" && level == "ask";
            match voice {
                Voice::Person => (
                    format!("{who}'s level here is {level}"),
                    "your setting".to_owned(),
                    match (refused.goal, allow, refused.task) {
                        (Some(goal), true, Some(task)) => format!(
                            "Set {who} to auto, or allow this task: {}",
                            allow_command(
                                &goal.to_string()[..8],
                                &short(&task.to_string(), &[]),
                                &refused.agent_name,
                                false
                            )
                        ),
                        (Some(goal), _, _) => format!(
                            "Set {who} to {needs}: {}",
                            level_command(&goal.to_string()[..8], &refused.agent_name, needs)
                        ),
                        (None, _, _) => format!("Set {who} to {needs}."),
                    },
                ),
                Voice::Agent => (
                    format!("{who}'s level here is {level}"),
                    format!("set by {who}'s owner"),
                    if allow {
                        format!("{who}'s owner can allow this task or set {who} to auto.")
                    } else {
                        format!("{who}'s owner can set {who} to {needs}.")
                    },
                ),
            }
        }
        Why::Rules {
            rule,
            qualifies,
            except_author,
            host_name,
            ..
        } => (
            rule_sentence(*rule, qualifies, *except_author, &who, voice),
            "the goal's rules".to_owned(),
            if !names_a_role(qualifies) {
                "Nothing to change; pick other work.".to_owned()
            } else {
                match (voice, host_name) {
                    (Voice::Person, Some(host)) => {
                        format!("The host, {}'s owner, gives roles.", safe(host))
                    }
                    _ => "The host gives roles.".to_owned(),
                }
            },
        ),
        Why::State { reason } => (
            reason.clone(),
            "the goal's state".to_owned(),
            "Nothing to change; pick other work.".to_owned(),
        ),
        Why::OnlyYou { operation, host } => {
            let line = help_line(operation);
            (
                "no agent can".to_owned(),
                if *host {
                    "only the host".to_owned()
                } else if voice == Voice::Person {
                    "only you".to_owned()
                } else {
                    format!("only {who}'s owner")
                },
                if *host {
                    format!("The host can run: {line}")
                } else if voice == Voice::Person {
                    format!("Run it yourself: {line}")
                } else {
                    format!("{who}'s owner can run: {line}")
                },
            )
        }
    };
    format!("{who} can't {act}: {reason} ({side}). {fix}")
}

fn level_word(level: Level) -> &'static str {
    match level {
        Level::Read => "read",
        Level::Ask => "ask",
        Level::Auto => "auto",
    }
}

/// A title as the person's voice prints it: quoted, through [`safe`] and cut
/// at 60 characters with a visible mark.
fn quoted(title: &str) -> String {
    let mut text: String = title.chars().take(60).collect();
    if title.chars().count() > 60 {
        text.push('…');
    }
    format!("\"{}\"", safe(&text))
}

/// The verb phrase of one act; `goal` is `None` when the refusal names no goal.
fn act_phrase(act: Act, goal: Option<&str>, task: &str) -> String {
    let in_goal = goal.map(|goal| format!(" in {goal}")).unwrap_or_default();
    let to_goal = goal.map(|goal| format!(" to {goal}")).unwrap_or_default();
    match act {
        Act::Post => format!("post{to_goal}"),
        Act::OpenTask => format!("open a task{in_goal}"),
        Act::TakeTask => format!("take {task}{in_goal}"),
        Act::Resume => format!("resume {task}{in_goal}"),
        Act::Approve => format!("approve this result{in_goal}"),
        Act::Attest => format!("report a check{in_goal}"),
        Act::DeclareDone => format!("declare {task} done{in_goal}"),
        Act::Pick => format!("pick a result{in_goal}"),
        Act::Close => format!("close {task}{in_goal}"),
        Act::Reopen => format!("reopen {task}{in_goal}"),
        Act::HandOut => format!("hand out {task}{in_goal}"),
        Act::Cancel => format!("cancel an attempt{in_goal}"),
        Act::MergeFiles => format!("merge files{in_goal}"),
        Act::ProposeFiles => format!("propose files{in_goal}"),
        Act::ConnectFolder => match goal {
            Some(goal) => format!("connect a folder to {goal}"),
            None => "connect a folder".into(),
        },
        Act::Start => "start a goal".into(),
        Act::Join => match goal {
            Some(goal) => format!("join {goal}"),
            None => "join a goal".into(),
        },
        Act::Leave => match goal {
            Some(goal) => format!("leave {goal}"),
            None => "leave a goal".into(),
        },
        Act::Invite => match goal {
            Some(goal) => format!("invite to {goal}"),
            None => "invite to a goal".into(),
        },
        Act::RemoveMember => match goal {
            Some(goal) => format!("remove a member from {goal}"),
            None => "remove a member".into(),
        },
        Act::ChangeRules => match goal {
            Some(goal) => format!("change the rules of {goal}"),
            None => "change a goal's rules".into(),
        },
        Act::GiveRole => format!("give or take a role{in_goal}"),
        Act::Revise => format!("revise {task}{in_goal}"),
        Act::Publish => match goal {
            Some(goal) => format!("publish {goal}"),
            None => "publish a goal".into(),
        },
        Act::Withdraw => format!("withdraw content{in_goal}"),
        Act::PersonCommand => "run this command".into(),
    }
}

fn rule_gerund(rule: Rule) -> &'static str {
    match rule {
        Rule::Propose => "opening a task",
        Rule::Publish => "posting",
        Rule::Start => "taking a task",
        Rule::Offer => "handing out a task",
        Rule::Declare => "declaring a task done",
        Rule::Review => "approving",
        Rule::Attest => "reporting a check",
        Rule::Select => "picking",
        Rule::Finish => "closing",
        Rule::Integrate => "merging files",
        Rule::Cancel => "cancelling",
    }
}

fn names_a_role(selector: &Selector) -> bool {
    match selector {
        Selector::Role { .. } => true,
        Selector::Any { selectors } => selectors.iter().any(names_a_role),
        _ => false,
    }
}

/// "GERUND needs WHAT and WHO is not one", from the rule that refused.
fn rule_sentence(
    rule: Rule,
    qualifies: &Selector,
    except_author: bool,
    who: &str,
    voice: Voice,
) -> String {
    let gerund = rule_gerund(rule);
    if matches!(qualifies, Selector::Nobody) {
        return format!("{gerund} is nobody's under these rules");
    }
    let other = if except_author {
        " other than the author"
    } else {
        ""
    };
    let (needs, tail) = match (qualifies, voice) {
        (Selector::Role { name }, Voice::Person) => (
            format!("{}{other}", with_article(&safe(name))),
            format!("{who} is not one"),
        ),
        (Selector::Role { .. }, Voice::Agent) => (
            format!("a role {who} does not hold"),
            if except_author {
                format!("{who} may be its author")
            } else {
                String::new()
            },
        ),
        (Selector::Members, _) => {
            if except_author {
                (
                    "a member other than the author".to_owned(),
                    format!("{who} is the author"),
                )
            } else {
                ("a member".to_owned(), format!("{who} is not one"))
            }
        }
        (Selector::OnlyMember, _) => (
            format!("the goal's only member{other}"),
            format!("{who} is not it"),
        ),
        (Selector::TaskCreator, _) => (
            format!("the member who added the task{other}"),
            format!("{who} did not add it"),
        ),
        (Selector::ContributionAuthor, _) => {
            ("the result's author".to_owned(), format!("{who} is not it"))
        }
        (Selector::Participant { .. }, _) => (
            format!("one named member{other}"),
            format!("{who} is not it"),
        ),
        (Selector::Any { selectors }, Voice::Person) => (
            format!(
                "{}{other}",
                selectors
                    .iter()
                    .map(|selector| match selector {
                        Selector::Role { name } => with_article(&safe(name)),
                        Selector::Members => "a member".into(),
                        Selector::OnlyMember => "the goal's only member".into(),
                        Selector::TaskCreator => "the member who added the task".into(),
                        Selector::ContributionAuthor => "the result's author".into(),
                        Selector::Participant { .. } => "one named member".into(),
                        Selector::Nobody => "nobody".into(),
                        Selector::Any { .. } => "one of the members the rules name".into(),
                    })
                    .collect::<Vec<_>>()
                    .join(" or ")
            ),
            format!("{who} is none of them"),
        ),
        (Selector::Any { .. }, Voice::Agent) => (
            format!("one of the members the rules name{other}"),
            format!("{who} is none of them"),
        ),
        (Selector::Nobody, _) => unreachable!("answered above"),
    };
    if tail.is_empty() {
        format!("{gerund} needs {needs}")
    } else {
        format!("{gerund} needs {needs} and {tail}")
    }
}

fn with_article(noun: &str) -> String {
    let vowel = noun
        .chars()
        .next()
        .is_some_and(|first| "aeiouAEIOU".contains(first));
    format!("{} {noun}", if vowel { "an" } else { "a" })
}

/// The help line of the command an operation name stands for.
fn help_line(operation: &str) -> String {
    let words = match operation {
        "level.set" => "level".to_owned(),
        "task.allow" | "task.disallow" => "allow".to_owned(),
        "workspace.epoch" => "workspace init".to_owned(),
        other => other.replace('.', " "),
    };
    format!("locust --owner {words} --help")
}

/// The shortest prefix of `id` that none of `others` starts with, at least
/// eight characters after any `task:` or `effect:` tag.
pub fn short(id: &str, others: &[String]) -> String {
    let (tag, hex) = match id.split_once(':') {
        Some((tag, hex)) if matches!(tag, "task" | "effect") => (format!("{tag}:"), hex),
        _ => (String::new(), id),
    };
    let rivals: Vec<&str> = others
        .iter()
        .map(String::as_str)
        .filter(|other| *other != id)
        .filter_map(|other| match other.split_once(':') {
            Some((other_tag, other_hex)) if matches!(other_tag, "task" | "effect") => {
                (format!("{other_tag}:") == tag).then_some(other_hex)
            }
            _ => tag.is_empty().then_some(other),
        })
        .collect();
    let chars: Vec<char> = hex.chars().collect();
    let mut length = 8.min(chars.len());
    while length < chars.len() {
        let candidate: String = chars[..length].iter().collect();
        if !rivals.iter().any(|rival| rival.starts_with(&candidate)) {
            break;
        }
        length += 1;
    }
    format!("{tag}{}", chars[..length].iter().collect::<String>())
}

/// The one line that allows, or with `revoke` takes back, one task for one
/// agent. Identifiers are already cut by [`short`]; `agent` is the local name.
pub fn allow_command(goal: &str, task: &str, agent: &str, revoke: bool) -> String {
    format!(
        "locust --owner allow --goal {goal} --task {task} --agent {}{}",
        shell_word(agent),
        if revoke { " --revoke" } else { "" }
    )
}

/// The one line that sets one agent's level in one goal.
pub fn level_command(goal: &str, agent: &str, level: &str) -> String {
    format!(
        "locust --owner level --goal {goal} --agent {} {level}",
        shell_word(agent)
    )
}

/// A local name as one shell word: bare when plain, else in single quotes,
/// so the printed line runs as printed.
pub fn shell_word(name: &str) -> String {
    if !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        name.to_owned()
    } else {
        format!("'{}'", name.replace('\'', "'\\''"))
    }
}

/// Render untrusted text on one terminal line without terminal controls, bidi
/// overrides or default-ignorable code points that would hide a difference
/// between two names. No content is silently removed or shortened.
pub fn safe(text: &str) -> String {
    text.chars()
        .flat_map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '\u{00ad}'
                        | '\u{034f}'
                        | '\u{061c}'
                        | '\u{115f}'..='\u{1160}'
                        | '\u{180b}'..='\u{180f}'
                        | '\u{200b}'..='\u{200f}'
                        | '\u{2028}'..='\u{202e}'
                        | '\u{2060}'..='\u{206f}'
                        | '\u{3164}'
                        | '\u{fe00}'..='\u{fe0f}'
                        | '\u{feff}'
                        | '\u{ffa0}'
                        | '\u{e0000}'..='\u{e0fff}'
                )
            {
                character.escape_unicode().collect::<Vec<_>>()
            } else {
                vec![character]
            }
        })
        .collect()
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

    fn id(prefix: [u8; 4]) -> [u8; 32] {
        let mut bytes = [0x11; 32];
        bytes[..4].copy_from_slice(&prefix);
        bytes
    }

    /// The members and goals of mockup P5-2.
    fn refusal(
        agent_name: &str,
        member_name: &str,
        goal: [u8; 4],
        goal_title: &str,
        act: Act,
        why: Why,
    ) -> Refused {
        Refused {
            agent: PublicKey([9; 32]),
            agent_name: agent_name.into(),
            member_name: Some(member_name.into()),
            goal: Some(GoalId(id(goal))),
            goal_title: Some(goal_title.into()),
            act,
            task: None,
            task_title: None,
            why,
        }
    }

    fn maple(act: Act, why: Why) -> Refused {
        let mut refused = refusal(
            "codex-maple-1a2b3c4d",
            "Maple",
            [0x7f, 0x3a, 0x9c, 0x1e],
            "Static site search",
            act,
            why,
        );
        refused.task = Some(TaskId::Authored(EventId(id([0x4b, 0x2d, 0x8e, 0x01]))));
        refused.task_title = Some("Fix the parser".into());
        refused
    }

    fn juniper(act: Act, why: Why) -> Refused {
        refusal(
            "claude-juniper-77aa0c52",
            "Juniper",
            [0xc0, 0x1d, 0x55, 0xaa],
            "Parser cleanup",
            act,
            why,
        )
    }

    fn reviewer_rule() -> Why {
        Why::Rules {
            rule: Rule::Review,
            qualifies: Selector::Role {
                name: "reviewer".into(),
            },
            except_author: false,
            host: PublicKey([8; 32]),
            host_name: Some("Harbor".into()),
        }
    }

    #[test]
    fn each_side_reads_as_one_sentence_in_both_voices() {
        let one = juniper(
            Act::Post,
            Why::YourSetting {
                level: Level::Read,
                needs: Level::Ask,
            },
        );
        assert_eq!(
            render(&one, Voice::Person),
            "Juniper can't post to \"Parser cleanup\": Juniper's level here is read (your setting). Set Juniper to ask: locust --owner level --goal c01d55aa --agent claude-juniper-77aa0c52 ask"
        );
        assert_eq!(
            render(&one, Voice::Agent),
            "claude-juniper-77aa0c52 can't post to this goal: claude-juniper-77aa0c52's level here is read (set by claude-juniper-77aa0c52's owner). claude-juniper-77aa0c52's owner can set claude-juniper-77aa0c52 to ask."
        );
        let two = maple(
            Act::TakeTask,
            Why::YourSetting {
                level: Level::Ask,
                needs: Level::Auto,
            },
        );
        assert_eq!(
            render(&two, Voice::Agent),
            "codex-maple-1a2b3c4d can't take this task in this goal: codex-maple-1a2b3c4d's level here is ask (set by codex-maple-1a2b3c4d's owner). codex-maple-1a2b3c4d's owner can allow this task or set codex-maple-1a2b3c4d to auto."
        );
        assert_eq!(
            render(&two, Voice::Person),
            "Maple can't take \"Fix the parser\" in \"Static site search\": Maple's level here is ask (your setting). Set Maple to auto, or allow this task: locust --owner allow --goal 7f3a9c1e --task task:4b2d8e01 --agent codex-maple-1a2b3c4d"
        );
        let three = maple(Act::Approve, reviewer_rule());
        assert_eq!(
            render(&three, Voice::Person),
            "Maple can't approve this result in \"Static site search\": approving needs a reviewer and Maple is not one (the goal's rules). The host, Harbor's owner, gives roles."
        );
        assert_eq!(
            render(&three, Voice::Agent),
            "codex-maple-1a2b3c4d can't approve this result in this goal: approving needs a role codex-maple-1a2b3c4d does not hold (the goal's rules). The host gives roles."
        );
        let four = maple(
            Act::TakeTask,
            Why::State {
                reason: "the task is finished".into(),
            },
        );
        assert_eq!(
            render(&four, Voice::Person),
            "Maple can't take \"Fix the parser\" in \"Static site search\": the task is finished (the goal's state). Nothing to change; pick other work."
        );
        assert_eq!(
            render(&four, Voice::Agent),
            "codex-maple-1a2b3c4d can't take this task in this goal: the task is finished (the goal's state). Nothing to change; pick other work."
        );
        let five = maple(
            Act::RemoveMember,
            Why::OnlyYou {
                operation: "member.remove".into(),
                host: true,
            },
        );
        assert_eq!(
            render(&five, Voice::Agent),
            "codex-maple-1a2b3c4d can't remove a member from this goal: no agent can (only the host). The host can run: locust --owner member remove --help"
        );
        let mut six = juniper(
            Act::Join,
            Why::OnlyYou {
                operation: "goal.join".into(),
                host: false,
            },
        );
        six.goal = None;
        six.goal_title = None;
        six.member_name = None;
        assert_eq!(
            render(&six, Voice::Agent),
            "claude-juniper-77aa0c52 can't join a goal: no agent can (only claude-juniper-77aa0c52's owner). claude-juniper-77aa0c52's owner can run: locust --owner goal join --help"
        );
        for refused in [one, two, three, four, five, six] {
            let agent = render(&refused, Voice::Agent);
            assert!(
                !agent
                    .split(|c: char| !c.is_alphanumeric())
                    .any(|word| word == "you"),
                "{agent}"
            );
        }
    }

    #[test]
    fn rules_and_state_never_suggest_a_level() {
        let rules = [
            Rule::Propose,
            Rule::Publish,
            Rule::Start,
            Rule::Offer,
            Rule::Declare,
            Rule::Review,
            Rule::Attest,
            Rule::Select,
            Rule::Finish,
            Rule::Integrate,
            Rule::Cancel,
        ];
        let selectors = [
            Selector::Members,
            Selector::Role {
                name: "lead".into(),
            },
            Selector::OnlyMember,
            Selector::TaskCreator,
            Selector::ContributionAuthor,
            Selector::Participant { key: "ab".into() },
            Selector::Nobody,
            Selector::Any {
                selectors: vec![
                    Selector::Role {
                        name: "lead".into(),
                    },
                    Selector::OnlyMember,
                ],
            },
        ];
        let mut whys = vec![Why::State {
            reason: "the task is closed".into(),
        }];
        for rule in rules {
            for qualifies in &selectors {
                for except_author in [false, true] {
                    whys.push(Why::Rules {
                        rule,
                        qualifies: qualifies.clone(),
                        except_author,
                        host: PublicKey([8; 32]),
                        host_name: None,
                    });
                }
            }
        }
        for why in whys {
            for voice in [Voice::Person, Voice::Agent] {
                let text = render(&maple(Act::Approve, why.clone()), voice);
                for word in ["level", "allow", "locust"] {
                    assert!(!text.contains(word), "{text}");
                }
                assert!(text.ends_with('.'), "{text}");
            }
        }
    }

    #[test]
    fn titles_are_quoted_escaped_and_cut() {
        let mut refused = maple(
            Act::TakeTask,
            Why::State {
                reason: "the task is finished".into(),
            },
        );
        refused.task_title = Some(format!("{}\u{1b}]52;x\u{7}", "long ".repeat(20)));
        refused.goal_title = Some("Say \"hi\"".into());
        refused.member_name = Some("Ma\u{202e}ple".into());
        let text = render(&refused, Voice::Person);
        assert!(!text.chars().any(char::is_control), "{text}");
        assert!(!text.contains('\u{202e}'), "{text}");
        let title = text.split('"').nth(1).unwrap();
        assert_eq!(title.chars().count(), 61, "{title}");
        assert!(title.ends_with('…'), "{title}");
        assert!(!text.contains("\\u{1b}"), "the cut comes first: {text}");
        assert!(text.contains("in \"Say \"hi\"\""), "{text}");
        assert!(text.starts_with("Ma\\u{202e}ple can't"), "{text}");
    }

    #[test]
    fn the_agents_voice_quotes_nothing_another_member_wrote() {
        let mut refused = maple(Act::TakeTask, reviewer_rule());
        refused.member_name = Some("MEMBER".into());
        refused.goal_title = Some("GOAL".into());
        refused.task_title = Some("TASK".into());
        refused.why = Why::Rules {
            rule: Rule::Start,
            qualifies: Selector::Role {
                name: "ROLE".into(),
            },
            except_author: true,
            host: PublicKey([8; 32]),
            host_name: Some("HOST".into()),
        };
        let text = render(&refused, Voice::Agent);
        for word in ["MEMBER", "GOAL", "TASK", "ROLE", "HOST", "\""] {
            assert!(!text.contains(word), "{text}");
        }
        assert!(text.contains("this task in this goal"), "{text}");
        let person = render(&refused, Voice::Person);
        for word in ["MEMBER", "\"GOAL\"", "\"TASK\"", "a ROLE", "HOST's owner"] {
            assert!(person.contains(word), "{person}");
        }
    }

    #[test]
    fn only_you_says_whose_it_is_and_names_no_dotted_operation() {
        for (operation, words) in [
            ("level.set", "level"),
            ("task.allow", "allow"),
            ("task.disallow", "allow"),
            ("workspace.epoch", "workspace init"),
            ("rules.bind", "rules bind"),
            ("daemon.stop", "daemon stop"),
        ] {
            for host in [false, true] {
                let refused = maple(
                    Act::PersonCommand,
                    Why::OnlyYou {
                        operation: operation.into(),
                        host,
                    },
                );
                for voice in [Voice::Person, Voice::Agent] {
                    let text = render(&refused, voice);
                    assert!(!text.contains(operation), "{text}");
                    assert!(
                        text.ends_with(&format!("locust --owner {words} --help")),
                        "{text}"
                    );
                    assert!(
                        text.contains("can't run this command: no agent can"),
                        "{text}"
                    );
                    let side = if host {
                        "(only the host)"
                    } else if voice == Voice::Person {
                        "(only you)"
                    } else {
                        "(only codex-maple-1a2b3c4d's owner)"
                    };
                    assert!(text.contains(side), "{text}");
                }
            }
        }
    }

    #[test]
    fn a_refusal_without_a_goal_or_names_still_reads() {
        let refused = Refused {
            agent: PublicKey([9; 32]),
            agent_name: "9f8e7d6c".into(),
            member_name: None,
            goal: None,
            goal_title: None,
            act: Act::Start,
            task: None,
            task_title: None,
            why: Why::OnlyYou {
                operation: "goal.create".into(),
                host: false,
            },
        };
        assert_eq!(
            render(&refused, Voice::Person),
            "9f8e7d6c can't start a goal: no agent can (only you). Run it yourself: locust --owner goal create --help"
        );
        let mut setting = refused;
        setting.why = Why::YourSetting {
            level: Level::Read,
            needs: Level::Ask,
        };
        setting.act = Act::Post;
        assert_eq!(
            render(&setting, Voice::Person),
            "9f8e7d6c can't post: 9f8e7d6c's level here is read (your setting). Set 9f8e7d6c to ask."
        );
        for act in [
            Act::Post,
            Act::OpenTask,
            Act::TakeTask,
            Act::Resume,
            Act::Approve,
            Act::Attest,
            Act::DeclareDone,
            Act::Pick,
            Act::Close,
            Act::Reopen,
            Act::HandOut,
            Act::Cancel,
            Act::MergeFiles,
            Act::ProposeFiles,
            Act::ConnectFolder,
            Act::Start,
            Act::Join,
            Act::Leave,
            Act::Invite,
            Act::RemoveMember,
            Act::ChangeRules,
            Act::GiveRole,
            Act::Revise,
            Act::Publish,
            Act::Withdraw,
            Act::PersonCommand,
        ] {
            for goal in [None, Some("\"G\"")] {
                let phrase = act_phrase(act, goal, "this task");
                assert!(!phrase.contains("  "), "{phrase}");
                assert!(!phrase.ends_with(' '), "{phrase}");
            }
        }
    }

    #[test]
    fn short_keeps_eight_characters_and_grows_until_unique() {
        let a = "task:4b2d8e01aaaa".to_owned();
        let b = "task:4b2d8e01aabb".to_owned();
        let c = "effect:4b2d8e01aabb".to_owned();
        let others = vec![a.clone(), b.clone(), c.clone()];
        assert_eq!(short(&a, &others), "task:4b2d8e01aaa");
        assert_eq!(short(&b, &others), "task:4b2d8e01aab");
        assert_eq!(short(&c, &others), "effect:4b2d8e01");
        assert_eq!(short("7f3a9c1e0000", &[]), "7f3a9c1e");
        assert_eq!(
            short(
                "7f3a9c1e0000",
                &["7f3a9c1e0010".into(), "7f3a9c1e0000".into()]
            ),
            "7f3a9c1e000"
        );
        assert_eq!(short("abc", &[]), "abc");
        assert_eq!(
            allow_command("7f3a9c1e", "task:4b2d8e01", "codex-maple-1a2b3c4d", true),
            "locust --owner allow --goal 7f3a9c1e --task task:4b2d8e01 --agent codex-maple-1a2b3c4d --revoke"
        );
        assert_eq!(
            level_command("c01d55aa", "claude-juniper-77aa0c52", "ask"),
            "locust --owner level --goal c01d55aa --agent claude-juniper-77aa0c52 ask"
        );
        assert_eq!(
            level_command("c01d55aa", "my agent's", "ask"),
            "locust --owner level --goal c01d55aa --agent 'my agent'\\''s' ask"
        );
    }

    #[test]
    fn terminal_controls_and_bidi_never_reach_the_terminal() {
        let text = "finding\u{1b}]52;c;secret\u{7}\r\nforged\u{202e}label";
        let rendered = safe(text);
        assert!(!rendered.chars().any(char::is_control));
        assert!(!rendered.contains('\u{202e}'));
        assert!(rendered.contains("\\u{1b}"));
        assert!(rendered.contains("secret"));
        assert_eq!(safe("ordinary café"), "ordinary café");
        for hidden in ['\u{ad}', '\u{34f}', '\u{fe0f}', '\u{3164}', '\u{e0001}'] {
            let name = format!("Juniper{hidden}");
            assert_ne!(safe(&name), name);
            assert!(safe(&name).starts_with("Juniper\\u{"), "{}", safe(&name));
        }
    }

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
