//! Default-deny export selection: credentials and private material that are
//! left out of every export, wherever they sit and whether or not Git tracks
//! them. Matching ignores ASCII case, because the same names on a
//! case-insensitive filesystem are the same files, so the lists below are
//! written in lowercase.
//!
//! These lists are a convention to extend, not a secrecy proof; the export
//! report names every path they leave out so the participant can review it.

/// Directories left out with everything beneath them.
const DIRECTORIES: &[&str] = &[".aws", ".azure", ".gnupg", ".kube", ".ssh"];

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
const PREFIXES: &[&str] = &[
    ".env.",
    "id_dsa",
    "id_ecdsa",
    "id_ed25519",
    "id_rsa",
    ".locust-apply-",
];

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
        || PREFIXES.iter().any(|prefix| name.starts_with(prefix))
        || SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

/// Checks repository-relative directory ancestry before paths are rebased
/// to the export root. Git paths may contain non-UTF-8 directory names.
pub(crate) fn has_denied_directory(path: &[u8]) -> bool {
    path.split(|&byte| byte == b'/').any(|directory| {
        directory
            .get(..14)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b".locust-apply-"))
            || DIRECTORIES
                .iter()
                .any(|denied| directory.eq_ignore_ascii_case(denied.as_bytes()))
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
}
