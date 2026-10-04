//! Why a command did not succeed, and the exit status that says so.
//!
//! Every failure carries one of the API's stable error codes, including the
//! ones that happen before any frame exists: a daemon that is not answering
//! is `unavailable`, a refused version is `unsupported_version`. The exit
//! status follows from the code, except that a mistake in the command line
//! itself exits with [`USAGE`].

use std::fmt;

use locust_proto::api::{ApiError, ErrorCode};
use locust_proto::local::LocalError;

/// Exit status of a usage error: bad arguments, or a missing option.
pub const USAGE: u8 = 2;

/// The exit status that reports an error code.
pub fn exit_status(code: ErrorCode) -> u8 {
    match code {
        ErrorCode::Internal => 1,
        ErrorCode::Denied => 3,
        ErrorCode::AuthorizationRequired => 4,
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
            (ErrorCode::AuthorizationRequired, 4),
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
}
