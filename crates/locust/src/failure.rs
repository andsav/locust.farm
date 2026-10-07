//! Why a command did not succeed, and the exit status that says so.
//!
//! Every failure carries one of the API's stable error codes, including the
//! ones that happen before any frame exists: a daemon that is not answering
//! is `unavailable`, a refused version is `unsupported_version`. The exit
//! status follows from the code, except that a mistake in the command line
//! itself exits with [`USAGE`].

use std::fmt;

use locust_proto::api::{ApiError, ErrorCode, Refused, Voice, render};
use locust_proto::local::LocalError;

/// Exit status of a usage error: bad arguments, or a missing option.
pub const USAGE: u8 = 2;

/// The exit status that reports an error code.
pub fn exit_status(code: ErrorCode) -> u8 {
    match code {
        ErrorCode::Internal => 1,
        ErrorCode::Denied => 3,
        ErrorCode::LevelRequired => 4,
        ErrorCode::NotEligible => 13,
        ErrorCode::NotFound => 5,
        ErrorCode::Invalid => 6,
        ErrorCode::Conflict
        | ErrorCode::ClaimHeld
        | ErrorCode::Superseded
        | ErrorCode::IdempotencyMismatch => 7,
        ErrorCode::Unavailable => 8,
        ErrorCode::Halted | ErrorCode::ReadOnly => 9,
        ErrorCode::UnsupportedVersion => 10,
        ErrorCode::Corrupted => 11,
        ErrorCode::LimitExceeded => 12,
    }
}

/// A command's failure: what JSON output reports and what the exit status
/// is derived from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// The stable code.
    pub code: ErrorCode,
    /// For people.
    pub message: String,
    pub details_json: Option<String>,
    /// True for a mistake in the command line. It is reported with the code
    /// `invalid` and exits with [`USAGE`].
    pub usage: bool,
}

impl Failure {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details_json: None,
            usage: false,
        }
    }

    /// A mistake in the command line or in the options it needs.
    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Invalid,
            message: message.into(),
            details_json: None,
            usage: true,
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Invalid, message)
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unavailable, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }

    pub fn exit_status(&self) -> u8 {
        if self.usage {
            USAGE
        } else {
            exit_status(self.code)
        }
    }

    /// A refusal reworded for the person who owns the agent: the daemon's
    /// message names the agent by its local name and quotes nothing another
    /// member wrote; the person reads the names and titles from `details`.
    /// Words a command put around the daemon's sentence stay. Any other
    /// failure is unchanged.
    pub fn for_person(mut self) -> Self {
        if let Some(refused) = self
            .details_json
            .as_deref()
            .and_then(|text| serde_json::from_str::<Refused>(text).ok())
        {
            let agent = render(&refused, Voice::Agent);
            let person = render(&refused, Voice::Person);
            self.message = if self.message.contains(&agent) {
                self.message.replacen(&agent, &person, 1)
            } else {
                person
            };
        }
        self
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for Failure {}

impl From<ApiError> for Failure {
    fn from(error: ApiError) -> Self {
        let mut failure = Self::new(error.code, error.message);
        failure.details_json = error.details_json;
        failure
    }
}

/// A local convention that cannot be applied is a mistake in the options or
/// the environment the command was given.
impl From<LocalError> for Failure {
    fn from(error: LocalError) -> Self {
        Self::usage(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_error_code_has_the_published_exit_status() {
        let table = [
            (ErrorCode::Internal, 1),
            (ErrorCode::Denied, 3),
            (ErrorCode::LevelRequired, 4),
            (ErrorCode::NotEligible, 13),
            (ErrorCode::NotFound, 5),
            (ErrorCode::Invalid, 6),
            (ErrorCode::Conflict, 7),
            (ErrorCode::ClaimHeld, 7),
            (ErrorCode::Superseded, 7),
            (ErrorCode::IdempotencyMismatch, 7),
            (ErrorCode::Unavailable, 8),
            (ErrorCode::Halted, 9),
            (ErrorCode::ReadOnly, 9),
            (ErrorCode::UnsupportedVersion, 10),
            (ErrorCode::Corrupted, 11),
            (ErrorCode::LimitExceeded, 12),
        ];
        for (code, status) in table {
            assert_eq!(exit_status(code), status, "{}", code.as_str());
            assert_eq!(Failure::new(code, "x").exit_status(), status);
        }
        assert_eq!(Failure::usage("x").exit_status(), USAGE);
        assert_eq!(Failure::usage("x").code, ErrorCode::Invalid);
    }

    #[test]
    fn a_refusal_is_reworded_for_the_person_only_with_owner() {
        use locust_proto::api::{Act, Level, Why};
        use locust_proto::id::{GoalId, PublicKey};
        let refused = Refused {
            agent: PublicKey([2; 32]),
            agent_name: "codex-maple-1a2b3c4d".into(),
            member_name: Some("Maple".into()),
            goal: Some(GoalId([1; 32])),
            goal_title: Some("Static site search".into()),
            act: Act::Post,
            task: None,
            task_title: None,
            why: Why::YourSetting {
                level: Level::Read,
                needs: Level::Ask,
            },
        };
        let from_daemon = Failure::from(ApiError {
            code: ErrorCode::LevelRequired,
            message: render(&refused, Voice::Agent),
            details_json: Some(serde_json::to_string(&refused).unwrap()),
        });
        // Under --json the answer is printed as it came: `for_person` is not
        // applied, so the message stays the daemon's.
        assert!(
            from_daemon.message.contains("this goal"),
            "{}",
            from_daemon.message
        );
        let for_person = from_daemon.clone().for_person();
        assert_eq!(for_person.message, render(&refused, Voice::Person));
        assert!(for_person.message.contains("\"Static site search\""));
        assert_eq!(for_person.code, from_daemon.code);
        assert_eq!(for_person.details_json, from_daemon.details_json);
        // A command's own words around the daemon's sentence stay.
        let prefixed = Failure {
            message: format!(
                "publication uncertain; recover exact operation 7: {}",
                from_daemon.message
            ),
            ..from_daemon.clone()
        }
        .for_person();
        assert_eq!(
            prefixed.message,
            format!(
                "publication uncertain; recover exact operation 7: {}",
                render(&refused, Voice::Person)
            )
        );
        // Details that are not a refusal leave the message alone.
        let other = Failure {
            details_json: Some(r#"{"role":"lead"}"#.into()),
            ..Failure::new(ErrorCode::Conflict, "this role clashes")
        };
        assert_eq!(other.clone().for_person(), other);
        assert_eq!(Failure::usage("x").for_person(), Failure::usage("x"));
    }
}
