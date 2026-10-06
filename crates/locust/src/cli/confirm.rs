//! A person's review of a concrete command, bound to the state it showed.

use super::{Output, print};
use crate::failure::Failure;
use clap::{Arg, ArgAction, ArgMatches, Command};
use locust_proto::api::ErrorCode;
use serde_json::{Value, json};
use std::io::{self, IsTerminal};

pub(super) struct Plan {
    pub command: &'static str,
    pub review: Value,
    pub human: String,
    pub warning: Option<String>,
    pub again: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PlanId(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Decision {
    Show,
    Proceed,
}

impl Plan {
    fn display(&self) -> String {
        let mut human = self.human.clone();
        if let Some(warning) = &self.warning {
            human.push('\n');
            human.push_str(warning);
        }
        human
    }

    pub fn id(&self) -> PlanId {
        let bytes = serde_json::to_vec(&json!({
            "command": self.command,
            "review": self.review,
        }))
        .expect("plan review is JSON");
        PlanId(format!("plan-{}", &crate::package::sha256(&bytes)[..16]))
    }

    pub fn json(&self) -> Value {
        let mut result = json!({
            "action": "review_required",
            "plan": self.review,
            "plan_id": self.id().0,
            "changed": false,
        });
        if let Some(warning) = &self.warning {
            result["warning"] = json!(warning);
        }
        result
    }

    pub fn shown(&self) -> Output {
        let id = self.id();
        let mut human = self.display();
        human.push_str(&format!(
            "\nPlan id: {}\nRun again with {}--confirm {}.",
            id.0, self.again, id.0
        ));
        Output::success(self.json(), human)
    }
}

pub(super) fn flags(command: Command) -> Command {
    command
        .arg(
            Arg::new("plan")
                .long("plan")
                .action(ArgAction::SetTrue)
                .conflicts_with("confirm")
                .help("Show the exact action and its plan identifier without acting"),
        )
        .arg(
            Arg::new("confirm")
                .long("confirm")
                .value_name("PLAN_ID")
                .help("Proceed only if this plan identifier still matches"),
        )
}

pub(super) fn decide(
    matches: &ArgMatches,
    args: &ArgMatches,
    plan: &Plan,
) -> Result<Decision, Failure> {
    if args.get_flag("plan") {
        return Ok(Decision::Show);
    }
    if let Some(value) = args.get_one::<String>("confirm") {
        bound(&PlanId(value.clone()), plan)?;
        return Ok(Decision::Proceed);
    }
    if !matches.get_flag("json") && io::stdin().is_terminal() && io::stderr().is_terminal() {
        let id = plan.id();
        print::stderr(format_args!(
            "{}\nPlan id: {}\nProceed? [y/N] ",
            plan.display(),
            id.0
        ))
        .map_err(|error| Failure::invalid(format!("cannot show plan: {error}")))?;
        let mut response = String::new();
        io::stdin()
            .read_line(&mut response)
            .map_err(|error| Failure::invalid(format!("cannot read confirmation: {error}")))?;
        if matches!(response.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            return Ok(Decision::Proceed);
        }
        return Err(Failure::new(ErrorCode::Denied, "declined; nothing changed"));
    }
    Ok(Decision::Show)
}

pub(super) fn bound(expected: &PlanId, recomputed: &Plan) -> Result<(), Failure> {
    if *expected == recomputed.id() {
        Ok(())
    } else {
        Err(Failure::new(
            ErrorCode::Conflict,
            "the plan changed; run --plan again",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(review: Value) -> Plan {
        Plan {
            command: "goal invite",
            review,
            human: "Invitation review".into(),
            warning: None,
            again: String::new(),
        }
    }

    #[test]
    fn id_binds_command_and_review_but_not_display_words() {
        let first = plan(json!({"goal": "one"}));
        let mut wording = plan(json!({"goal": "one"}));
        wording.human = "Other words".into();
        wording.warning = Some("Caution".into());
        wording.again = "--name agent ".into();
        assert_eq!(first.id(), wording.id());
        wording.command = "goal add";
        assert_ne!(first.id(), wording.id());
        assert_ne!(first.id(), plan(json!({"goal": "two"})).id());
        assert!(first.id().0.starts_with("plan-"));
        assert_eq!(first.id().0.len(), 21);
    }

    #[test]
    fn shown_json_carries_warning_and_bound_refuses_a_changed_review() {
        let mut first = plan(json!({"pending": 0}));
        first.warning = Some("Send privately".into());
        let shown = first.json();
        assert_eq!(shown["action"], "review_required");
        assert_eq!(shown["changed"], false);
        assert_eq!(shown["plan"], first.review);
        assert_eq!(shown["warning"], "Send privately");
        assert_eq!(shown["plan_id"], first.id().0);
        assert_eq!(
            bound(&first.id(), &plan(json!({"pending": 1})))
                .unwrap_err()
                .code,
            ErrorCode::Conflict
        );
    }

    #[test]
    fn nonterminal_run_shows_the_plan_without_proceeding() {
        let root = Command::new("locust")
            .arg(Arg::new("json").long("json").action(ArgAction::SetTrue))
            .subcommand(flags(Command::new("invite")));
        let parsed = root
            .try_get_matches_from(["locust", "--json", "invite"])
            .unwrap();
        let (_, args) = parsed.subcommand().unwrap();
        assert_eq!(
            decide(&parsed, args, &plan(json!({}))).unwrap(),
            Decision::Show
        );
    }
}
