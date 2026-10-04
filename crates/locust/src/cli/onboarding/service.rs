//! Reviewed service preparation without restarting an already running daemon.
use crate::{
    failure::Failure,
    installation::{
        self,
        service::{ServiceAction, ServiceSpec, ServiceState},
        service_install::{self, ServicePlan},
    },
};
use locust_proto::api::ErrorCode;
use serde_json::Value;
use std::path::Path;

pub(super) fn prepare(
    prefix: &Path,
    spec: &ServiceSpec,
    review: &mut impl FnMut(&Value) -> Result<bool, Failure>,
) -> Result<ServiceState, Failure> {
    let installed = installation::status(prefix)?;
    if installed["installed"] != true || installed["withdrawn"] != false {
        return Err(Failure::new(
            ErrorCode::Denied,
            "onboarding requires an installed, verified, non-withdrawn release",
        ));
    }
    let plan = service_install::plan(prefix, spec, false)?;
    prepare_plan(
        &plan,
        review,
        |digest| service_install::apply(prefix, spec, digest).map(|_| ()),
        |action| service_install::control(prefix, spec, action),
    )
}

fn prepare_plan(
    plan: &ServicePlan,
    review: &mut impl FnMut(&Value) -> Result<bool, Failure>,
    mut apply: impl FnMut(&str) -> Result<(), Failure>,
    mut control: impl FnMut(ServiceAction) -> Result<ServiceState, Failure>,
) -> Result<ServiceState, Failure> {
    match plan.action {
        "disabled" => return Ok(ServiceState::Disabled),
        "unchanged" => {}
        "create" | "recover" => {
            if !review(&plan.json()?)? {
                return Err(Failure::new(
                    ErrorCode::Denied,
                    "service setup was declined; rerun onboarding to review and resume",
                ));
            }
            apply(&plan.digest()?)?;
        }
        _ => {
            return Err(Failure::new(
                ErrorCode::Conflict,
                "service unit is unavailable for this installation; resolve its ownership or modified unit before retrying onboarding",
            ));
        }
    }
    match control(ServiceAction::Status)? {
        ServiceState::Running => Ok(ServiceState::Running),
        ServiceState::Disabled => Ok(ServiceState::Disabled),
        ServiceState::Stopped | ServiceState::Loaded => {
            match control(ServiceAction::Start) {
                Err(error)
                    if error.code == ErrorCode::Unavailable
                        && error.message == "service has not reached running state" =>
                {
                    // The manager accepted every start command, but its immediate
                    // observation was not running. Let onboarding observe readiness
                    // without issuing another start; other manager errors propagate.
                    Ok(ServiceState::Loaded)
                }
                result => result,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn plan(action: &'static str) -> ServicePlan {
        ServicePlan {
            format: "locust-service-plan-v1",
            operation: "install",
            prefix: PathBuf::from("/selected installation"),
            label: "selected-service".into(),
            unit_path: PathBuf::from("/selected profile/service.plist"),
            action,
            rendered_sha256: "unit-digest".into(),
            record_sha256: None,
            record_phase: None,
            observed_unit_state: "absent",
            observed_unit_sha256: None,
        }
    }

    #[test]
    fn already_running_service_is_only_observed() {
        let mut actions = Vec::new();
        let state = prepare_plan(
            &plan("unchanged"),
            &mut |_| panic!("unchanged unit needs no review"),
            |_| panic!("unchanged unit must not be written"),
            |action| {
                actions.push(action);
                Ok(ServiceState::Running)
            },
        )
        .unwrap();
        assert_eq!(state, ServiceState::Running);
        assert_eq!(actions, [ServiceAction::Status]);
    }

    #[test]
    fn declining_creation_or_recovery_has_no_side_effects() {
        for action in ["create", "recover"] {
            let proposed = plan(action);
            let error = prepare_plan(
                &proposed,
                &mut |review| {
                    assert_eq!(*review, proposed.json().unwrap());
                    Ok(false)
                },
                |_| panic!("declined unit must not be written"),
                |_| panic!("declined unit must not be controlled"),
            )
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::Denied);
            assert!(error.message.contains("rerun"));
        }
    }

    #[test]
    fn collisions_and_modified_ownership_are_refused() {
        for action in ["collision", "modified", "remove_recovery", "unexpected"] {
            let error = prepare_plan(
                &plan(action),
                &mut |_| panic!("conflicting unit must not be approved"),
                |_| panic!("conflicting unit must not be written"),
                |_| panic!("conflicting unit must not be controlled"),
            )
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::Conflict);
        }
    }

    #[test]
    fn disabled_service_needs_no_mutation_or_manager() {
        let state = prepare_plan(
            &plan("disabled"),
            &mut |_| panic!("disabled service needs no review"),
            |_| panic!("disabled service must not be written"),
            |_| panic!("disabled service must not be controlled"),
        )
        .unwrap();
        assert_eq!(state, ServiceState::Disabled);
    }

    #[test]
    fn reviewed_digest_is_applied_before_observing_and_starting() {
        for action in ["create", "recover"] {
            for initial in [ServiceState::Stopped, ServiceState::Loaded] {
                let proposed = plan(action);
                let events = std::cell::RefCell::new(Vec::new());
                let state = prepare_plan(
                    &proposed,
                    &mut |_| {
                        events.borrow_mut().push("review");
                        Ok(true)
                    },
                    |digest| {
                        assert_eq!(digest, proposed.digest().unwrap());
                        events.borrow_mut().push("apply");
                        Ok(())
                    },
                    |action| match action {
                        ServiceAction::Status => {
                            events.borrow_mut().push("status");
                            Ok(initial)
                        }
                        ServiceAction::Start => {
                            events.borrow_mut().push("start");
                            Ok(ServiceState::Running)
                        }
                        ServiceAction::Stop => panic!("onboarding never stops a service"),
                    },
                )
                .unwrap();
                assert_eq!(state, ServiceState::Running);
                assert_eq!(*events.borrow(), ["review", "apply", "status", "start"]);
            }
        }
    }

    #[test]
    fn accepted_delayed_start_is_loaded_and_other_errors_are_preserved() {
        let unavailable = Failure::unavailable("service has not reached running state");
        let conflict = Failure::new(ErrorCode::Conflict, "manager has another unit");
        for original in [unavailable, conflict.clone()] {
            let result = prepare_plan(
                &plan("unchanged"),
                &mut |_| panic!("unchanged"),
                |_| panic!("unchanged"),
                |action| match action {
                    ServiceAction::Status => Ok(ServiceState::Loaded),
                    ServiceAction::Start => Err(original.clone()),
                    ServiceAction::Stop => panic!("never stop"),
                },
            );
            if original == conflict {
                assert_eq!(result.unwrap_err(), original);
            } else {
                assert_eq!(result.unwrap(), ServiceState::Loaded);
            }
        }
    }
}
