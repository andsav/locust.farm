//! Connections and who a request acts as.
//!
//! The hello's credential fixes the [`Caller`] of a connection. Each request
//! is then resolved to an [`Actor`]: the principal it acts as, and whether the
//! owner's direct act stands in for that principal's grants. The rules are
//! those of `locust_proto::api` ("Who is asking").

use locust_proto::api::{ApiError, Audience, Caller, ErrorCode, Request, RequestFrame};
use locust_proto::id::{GoalId, InstanceId, PublicKey};

use super::identity::Principals;

/// A `wait` the connection is parked on.
#[derive(Clone, Copy, Debug)]
pub(super) struct ParkedWait {
    pub request_id: u64,
    pub goal: GoalId,
    /// The revision the caller had seen.
    pub seen: u64,
    pub actor: Actor,
}

/// One welcomed connection.
#[derive(Debug)]
pub(super) struct Conn {
    pub caller: Caller,
    /// The execution session this connection is, if it presented a secret.
    pub session: Option<InstanceId>,
    pub parked: Option<ParkedWait>,
}

/// Who one request acts as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Actor {
    pub caller: Caller,
    /// The principal the request acts as; absent for the owner asking
    /// directly.
    pub principal: Option<PublicKey>,
    /// True when the owner makes the request on a principal's behalf: that
    /// act is the authorization, so grants are not consulted.
    pub owner_act: bool,
    /// The connection's session.
    pub session: Option<InstanceId>,
}

impl Actor {
    /// True for a viewer, whose requests store nothing.
    pub fn is_viewer(&self) -> bool {
        matches!(self.caller, Caller::Viewer(_))
    }

    /// The principal the request acts as, or `Invalid` for the owner asking
    /// directly where a principal is needed.
    pub fn principal(&self) -> Result<PublicKey, ApiError> {
        self.principal.ok_or_else(|| {
            ApiError::new(
                ErrorCode::Invalid,
                "the owner makes this request on behalf of a principal",
            )
        })
    }

    /// The connection's session, or `Invalid` when the hello carried none.
    pub fn session(&self) -> Result<InstanceId, ApiError> {
        self.session.ok_or_else(|| {
            ApiError::new(
                ErrorCode::Invalid,
                "this request needs a session in the hello",
            )
        })
    }
}

fn denied(message: &'static str) -> ApiError {
    ApiError::new(ErrorCode::Denied, message)
}

/// Resolves who `frame` acts as on a connection of `caller`.
pub(super) fn resolve(
    principals: &Principals,
    caller: Caller,
    session: Option<InstanceId>,
    frame: &RequestFrame,
) -> Result<Actor, ApiError> {
    let operation = frame.request.operation();
    let actor = |principal, owner_act| Actor {
        caller,
        principal,
        owner_act,
        session,
    };
    match caller {
        Caller::Owner => match (operation.audience, frame.on_behalf) {
            (Audience::Owner, None) => Ok(actor(None, false)),
            (Audience::Owner, Some(_)) => Err(ApiError::new(
                ErrorCode::Invalid,
                "an owner-only request is not made on behalf of a principal",
            )),
            (_, Some(principal)) => {
                if principals.active(&principal).is_none() {
                    return Err(ApiError::new(
                        ErrorCode::NotFound,
                        "no enrolled principal has that key",
                    ));
                }
                if principals
                    .active(&principal)
                    .is_some_and(|p| p.record.author_only)
                    && operation.audience != Audience::Author
                {
                    return Err(denied("an authoring principal cannot act in goals"));
                }
                Ok(actor(Some(principal), true))
            }
            (_, None) => {
                let direct =
                    operation.read_only || matches!(frame.request, Request::SessionDrop { .. });
                if direct {
                    Ok(actor(None, false))
                } else {
                    Err(ApiError::new(
                        ErrorCode::Invalid,
                        "the owner makes this request on behalf of a principal",
                    ))
                }
            }
        },
        Caller::Agent(principal) => {
            if frame.on_behalf.is_some() {
                return Err(denied("only the owner acts on behalf of a principal"));
            }
            if principals.active(&principal).is_none() {
                return Err(denied("the credential was revoked"));
            }
            if operation.audience == Audience::Owner {
                return Err(denied("this request is the owner's to make"));
            }
            Ok(actor(Some(principal), false))
        }
        Caller::Author(principal) => {
            if frame.on_behalf.is_some()
                || session.is_some()
                || operation.audience != Audience::Author
            {
                return Err(denied(
                    "this credential only accesses its private blueprint catalog",
                ));
            }
            if principals.active(&principal).is_none() {
                return Err(denied("the credential was revoked"));
            }
            Ok(actor(Some(principal), false))
        }
        Caller::Viewer(principal) => {
            if frame.on_behalf.is_some() {
                return Err(denied("a viewer acts on behalf of nobody"));
            }
            if principals.active(&principal).is_none() {
                return Err(denied("the credential was revoked"));
            }
            if !operation.read_only
                || matches!(operation.audience, Audience::Author | Audience::Owner)
            {
                return Err(denied("a viewer makes read-only requests only"));
            }
            Ok(actor(Some(principal), false))
        }
    }
}
