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

#[test]
fn doctor_names_the_marks_directory() {
    use std::os::unix::fs::DirBuilderExt;
    let root = crate::testdir::short_dir();
    let home = root.path().join("daemon");
    fs::DirBuilder::new().mode(0o700).create(&home).unwrap();
    let marks = local::marks_dir(&home).unwrap();
    let found = |home: &Path| {
        let matches = super::super::args::command()
            .try_get_matches_from(["locust", "doctor"])
            .unwrap();
        let (result, human, _, _) = run(&matches, home);
        let check = result["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["name"] == "marks")
            .unwrap()
            .clone();
        (check, human)
    };
    // Not made yet: the daemon creates it at its start, and doctor does not.
    let (check, human) = found(&home);
    assert_eq!(check["ok"], true, "{check}");
    assert_eq!(check["detail"], marks.display().to_string());
    assert!(
        human.contains(&format!("ok marks: {}", marks.display())),
        "{human}"
    );
    assert!(!marks.exists());
    // Private: still the path.
    fs::DirBuilder::new().mode(0o700).create(&marks).unwrap();
    let (check, _) = found(&home);
    assert_eq!(check["ok"], true, "{check}");
    assert_eq!(check["detail"], marks.display().to_string());
    // Readable by others: it fails, with what to do.
    fs::set_permissions(&marks, fs::Permissions::from_mode(0o755)).unwrap();
    let (check, human) = found(&home);
    assert_eq!(check["ok"], false, "{check}");
    assert!(
        check["detail"]
            .as_str()
            .unwrap()
            .contains(&marks.display().to_string())
    );
    assert!(
        check["recovery"].as_str().unwrap().contains("chmod 700"),
        "{check}"
    );
    assert!(human.contains("failed marks:"), "{human}");
    // A link to a private directory, which the daemon opens through the
    // link, as doctor's home check does (G2 review 4); a link to nothing.
    fs::remove_dir(&marks).unwrap();
    let real = root.path().join("real");
    fs::DirBuilder::new().mode(0o700).create(&real).unwrap();
    std::os::unix::fs::symlink(&real, &marks).unwrap();
    let (check, _) = found(&home);
    assert_eq!(check["ok"], true, "{check}");
    assert_eq!(check["detail"], marks.display().to_string());
    fs::remove_dir(&real).unwrap();
    let (check, _) = found(&home);
    assert_eq!(check["ok"], false, "{check}");
    assert!(
        check["detail"]
            .as_str()
            .unwrap()
            .ends_with("is a link to something that does not exist"),
        "{check}"
    );
    // A file in its place.
    fs::remove_file(&marks).unwrap();
    fs::write(&marks, b"").unwrap();
    let (check, _) = found(&home);
    assert_eq!(check["ok"], false, "{check}");
    assert!(
        check["detail"]
            .as_str()
            .unwrap()
            .ends_with("is not a directory")
    );
}
