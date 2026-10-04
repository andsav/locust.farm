//! Authenticated, private authoring catalog. Prepare and commit remain within
//! one serialized node turn; source and presentation have independent CAS.
use super::{Plan, Planned, answer};
use crate::node::{Node, callers::Actor, commit::Tx};
use crate::organization::{catalog, inspect};
use locust_proto::api::{ApiError, ErrorCode, Request, Response};
use locust_proto::engine::Entropy;
use locust_proto::store::Store;

fn error(error: catalog::Error) -> ApiError {
    match error {
        catalog::Error::Store(error) => error.into(),
        catalog::Error::NotFound => {
            ApiError::new(ErrorCode::NotFound, "no such private catalog entry")
        }
        catalog::Error::Invalid(message) => ApiError::new(ErrorCode::Invalid, message),
        catalog::Error::SourceConflict { current } => {
            ApiError::new(ErrorCode::Conflict, "the draft revision changed")
                .with_details(serde_json::json!({"current_draft":current}))
        }
        catalog::Error::PresentationConflict { current } => {
            ApiError::new(ErrorCode::Conflict, "the presentation revision changed")
                .with_details(serde_json::json!({"current_presentation":current}))
        }
        catalog::Error::PublicationConflict { current } => ApiError::new(
            ErrorCode::Conflict,
            "the publication identifier already exists",
        )
        .with_details(serde_json::json!({"current_publication":current})),
        catalog::Error::InvalidDocument { diagnostics } => {
            ApiError::new(ErrorCode::Invalid, "the blueprint is invalid")
                .with_details(serde_json::json!({"diagnostics": diagnostics}))
        }
    }
}
fn prepared<T>(
    result: Result<catalog::Prepared<T>, catalog::Error>,
    response: impl FnOnce(T) -> Response,
) -> Plan {
    let prepared = result.map_err(error)?;
    Ok(Planned {
        response: response(prepared.value),
        tx: Tx {
            commit: prepared.commit,
            ..Tx::none()
        },
    })
}
impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn catalog(&self, actor: &Actor, request: Request) -> Plan {
        let principal = actor.principal()?;
        match request {
            Request::BlueprintDraftCreate {
                id,
                expected_revision,
                source,
            } => prepared(
                catalog::prepare_create(&self.store, principal, &id, expected_revision, source),
                Response::BlueprintDraft,
            ),
            Request::BlueprintDraftUpdate {
                id,
                expected_revision,
                source,
            } => prepared(
                catalog::prepare_update(&self.store, principal, &id, expected_revision, source),
                Response::BlueprintDraft,
            ),
            Request::BlueprintDraft { id } => answer(Response::BlueprintDraft(
                catalog::draft(&self.store, principal, &id).map_err(error)?,
            )),
            Request::BlueprintDrafts => answer(Response::BlueprintDrafts(
                catalog::drafts(&self.store, principal).map_err(error)?,
            )),
            Request::BlueprintPublish {
                draft,
                id,
                expected_revision,
                expected_source_hash,
            } => prepared(
                catalog::prepare_publish(
                    &self.store,
                    principal,
                    &draft,
                    &id,
                    expected_revision,
                    &expected_source_hash,
                ),
                Response::BlueprintPublication,
            ),
            Request::BlueprintPublication { id } => answer(Response::BlueprintPublication(
                catalog::publication(&self.store, principal, &id).map_err(error)?,
            )),
            Request::BlueprintPublications => answer(Response::BlueprintPublications(
                catalog::publications(&self.store, principal).map_err(error)?,
            )),
            Request::BlueprintPresentation { id } => answer(Response::BlueprintPresentation(
                catalog::presentation(&self.store, principal, &id).map_err(error)?,
            )),
            Request::BlueprintPresentationUpdate {
                id,
                expected_revision,
                data_json,
            } => prepared(
                catalog::prepare_presentation(
                    &self.store,
                    principal,
                    &id,
                    expected_revision,
                    data_json,
                ),
                Response::BlueprintPresentation,
            ),
            Request::BlueprintValidate { source } | Request::BlueprintExplain { source } => {
                answer(Response::BlueprintInspection {
                    json: serde_json::to_string(&inspect(&source)).expect("inspection encodes"),
                })
            }
            _ => unreachable!("only catalog operations route here"),
        }
    }
}
