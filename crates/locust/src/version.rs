//! What this binary is: its version, the commit it was built from and the
//! contract versions it speaks.

use locust_proto::{API_VERSION, PROTOCOL_VERSION};

/// The package version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The commit the binary was built from, as `build.rs` embedded it.
pub const COMMIT: &str = env!("LOCUST_BUILD_COMMIT");

/// What `locust --version` prints after the program name:
/// `<semver> (<commit>) api <n> protocol <n>`.
pub fn line() -> String {
    format!("{VERSION} ({COMMIT}) api {API_VERSION} protocol {PROTOCOL_VERSION}")
}

/// The daemon's own version as it reports it in the hello and in `status`.
pub fn daemon() -> String {
    format!("{VERSION} ({COMMIT})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_line_names_the_version_the_commit_and_both_contract_versions() {
        let line = line();
        let (version, rest) = line.split_once(" (").unwrap();
        let (commit, rest) = rest.split_once(") ").unwrap();
        assert_eq!(version, VERSION);
        assert_eq!(commit, COMMIT);
        assert_eq!(
            rest,
            format!("api {API_VERSION} protocol {PROTOCOL_VERSION}")
        );
        // A build from an archive may name the commit any way it likes.
        assert!(!commit.is_empty() && !commit.contains(char::is_whitespace));
    }
}
