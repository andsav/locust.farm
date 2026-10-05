//! Default-deny export selection: credentials and private material that are
//! left out of every export, wherever they sit and whether or not Git tracks
//! them. Matching ignores ASCII case, because the same names on a
//! case-insensitive filesystem are the same files, so the lists below are
//! written in lowercase.
//!
//! These lists are a convention to extend, not a secrecy proof; the export
//! report names every path they leave out so the participant can review it.
//!
//! Reserved workspace metadata (`.locust`, `.locust-apply-*`,
//! `.locust-workspace-*`, `.locust-recovery-*`) is denied wherever it appears,
//! matching the reserved-path policy enforced by `tree::tree_path`. The
//! same policy is applied to repository ancestors before paths are rebased
//! to the export root, and to each export path before its bytes reach the
//! blob sink.

/// Directories left out with everything beneath them.
const DIRECTORIES: &[&str] = &[".aws", ".azure", ".gnupg", ".kube", ".ssh"];

/// Reserved workspace metadata directory names, denied wherever they appear.
const RESERVED_DIRECTORIES: &[&str] = &[".locust"];

/// Reserved workspace metadata name prefixes, denied wherever they appear.
const RESERVED_PREFIXES: &[&str] = &[".locust-apply-", ".locust-workspace-", ".locust-recovery-"];

/// File names left out wherever they appear.
const NAMES: &[&str] = &[
    ".dockercfg",
    ".env",
    ".envrc",
    ".git-credentials",
    ".htpasswd",
    ".netrc",
    "_netrc",
    ".npmrc",
    ".pgpass",
    ".pypirc",
];

/// File name prefixes: environment variants such as `.env.local`, and SSH
/// keys such as `id_rsa` or `id_ed25519_work`.
const PREFIXES: &[&str] = &[".env.", "id_dsa", "id_ecdsa", "id_ed25519", "id_rsa"];

/// File name suffixes: private keys, keystores, password databases and
/// infrastructure state.
const SUFFIXES: &[&str] = &[
    ".jks",
    ".kdbx",
    ".key",
    ".keystore",
    ".p12",
    ".p8",
    ".pem",
    ".pfx",
    ".ppk",
    ".tfstate",
    ".tfstate.backup",
];

/// True if `path`, relative to the export root with `/` separators, is left
/// out of an export.
pub(crate) fn is_denied(path: &str) -> bool {
    let path = path.to_ascii_lowercase();
    let (directories, name) = path.rsplit_once('/').unwrap_or(("", &path));
    has_denied_directory(directories.as_bytes())
        || NAMES.contains(&name)
        || RESERVED_DIRECTORIES.contains(&name)
        || PREFIXES.iter().any(|prefix| name.starts_with(prefix))
        || RESERVED_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix))
        || SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

/// Checks repository-relative directory ancestry before paths are rebased
/// to the export root. Git paths may contain non-UTF-8 directory names.
pub(crate) fn has_denied_directory(path: &[u8]) -> bool {
    path.split(|&byte| byte == b'/').any(|directory| {
        DIRECTORIES
            .iter()
            .any(|denied| directory.eq_ignore_ascii_case(denied.as_bytes()))
            || RESERVED_DIRECTORIES
                .iter()
                .any(|denied| directory.eq_ignore_ascii_case(denied.as_bytes()))
            || RESERVED_PREFIXES.iter().any(|prefix| {
                directory
                    .get(..prefix.len())
                    .is_some_and(|d| d.eq_ignore_ascii_case(prefix.as_bytes()))
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_and_private_material_are_denied() {
        for path in [
            ".env",
            ".ENV",
            "app/.env.production",
            ".envrc",
            "deploy/server.pem",
            "certs/TLS.KEY",
            "store.p12",
            "id_rsa",
            "home/id_ed25519_work",
            ".npmrc",
            "web/.netrc",
            ".aws/credentials",
            ".locust-apply-123/original-0",
            ".LOCUST-APPLY-123/original-0",
            "user/.ssh/config",
            ".gnupg/private-keys-v1.d/x.key",
            "infra/terraform.tfstate.backup",
            ".locust",
            ".locust/journal",
            "app/.locust/plan.json",
            ".LOCUST/plan.json",
            ".locust-workspace-123/plan.json",
            ".LOCUST-WORKSPACE-123/plan.json",
            ".locust-recovery-123/original-0",
            ".LOCUST-RECOVERY-123/original-0",
            "sub/.locust-apply-456/replacement-0",
            ".locust-apply-789",
            ".locust-recovery-789",
            ".locust-workspace-789",
        ] {
            assert!(is_denied(path), "{path} should be denied");
        }
    }

    #[test]
    fn ordinary_files_with_similar_names_are_kept() {
        for path in [
            "src/env.rs",
            "docs/environment.md",
            "src/keys.rs",
            "keyboard.rs",
            "monkey",
            "pem",
            "notes/ssh.md",
            "aws/README.md",
            "my.env",
            "identity.rs",
        ] {
            assert!(!is_denied(path), "{path} should be kept");
        }
    }

    #[test]
    fn reserved_metadata_in_repository_ancestors_is_denied_before_rebasing() {
        for prefix in [
            ".locust",
            ".LOCUST",
            ".locust-apply-123",
            ".locust-workspace-123",
            ".locust-recovery-123",
            "deep/.locust",
            "deep/.locust-apply-123",
            "deep/.locust-workspace-123",
            "deep/.locust-recovery-123",
        ] {
            assert!(
                has_denied_directory(prefix.as_bytes()),
                "{prefix} should be denied as an ancestor"
            );
        }
        assert!(!has_denied_directory(b"ordinary/src"));
        assert!(!has_denied_directory(b"locust/src"));
        assert!(!has_denied_directory(b""));
    }
}
