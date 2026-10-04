use super::*;

#[test]
fn missing_selected_profile_has_recovery_without_creating_any_state() {
    let root = crate::testdir::short_dir();
    let profile = root.path().join("profile");
    fs::create_dir(&profile).unwrap();
    let home = root.path().join("missing-daemon");
    let matches = super::super::args::command()
        .try_get_matches_from([
            "locust",
            "doctor",
            "--client",
            "codex",
            "--profile-home",
            profile.to_str().unwrap(),
        ])
        .unwrap();
    let (result, human, status, ok) = run(&matches, &home);
    assert_eq!(status, 1);
    assert!(!ok);
    assert!(!home.exists());
    assert_eq!(fs::read_dir(profile).unwrap().count(), 0);
    let checks = result["checks"].as_array().unwrap();
    assert!(
        checks
            .iter()
            .any(|check| check["name"] == "onboarding_journal" && check["ok"] == false)
    );
    assert!(
        checks
            .iter()
            .filter(|check| check["ok"] == false)
            .all(|check| check["recovery"]
                .as_str()
                .is_some_and(|text| !text.is_empty()))
    );
    assert!(human.contains("Next:"));
    assert_eq!(result["model_ready"], false);
}

#[test]
fn standalone_profile_options_need_a_selected_client() {
    for option in ["--profile-home", "--workspace"] {
        assert!(
            command()
                .try_get_matches_from(["doctor", option, "/tmp"])
                .is_err()
        );
    }
}

#[test]
fn nonexistent_prefix_and_disabled_service_are_read_only() {
    let root = crate::testdir::short_dir();
    let prefix = root.path().join("software");
    let home = root.path().join("daemon");
    let matches = command()
        .try_get_matches_from([
            "doctor",
            "--prefix",
            prefix.to_str().unwrap(),
            "--service",
            "none",
            "--service-profile-home",
            root.path().to_str().unwrap(),
        ])
        .unwrap();
    let mut checks = Vec::new();
    profile::inspect(&matches, &home, &mut checks);
    assert!(
        checks
            .iter()
            .any(|check| check["name"] == "installation" && check["ok"] == false)
    );
    assert!(!prefix.exists());
    assert!(!home.exists());
}
