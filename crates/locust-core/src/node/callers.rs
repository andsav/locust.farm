//! Connections and who a request acts as.
//!
//! The hello's credential fixes the [`Caller`] of a connection. Each request
//! is then resolved to an [`Actor`]: the principal it acts as, and whether the
//! owner's direct act stands in for that principal's local level. The rules are
//! those of `locust_proto::api` ("Who is asking").

use locust_proto::api::{
    Act, ApiError, Audience, Caller, ErrorCode, Refused, Request, RequestFrame, Why,
};
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
    /// act skips the local level, while shared rules still apply.
    pub owner_act: bool,
    /// The connection's session.
    pub session: Option<InstanceId>,
}

impl Actor {
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

fn only_you(
    principals: &Principals,
    principal: PublicKey,
    frame: &RequestFrame,
    host: bool,
) -> ApiError {
    let agent_name = principals
        .get(&principal)
        .map(|principal| principal.record.name.clone())
        .unwrap_or_else(|| principal.to_string().chars().take(8).collect());
    let act = match &frame.request {
        Request::GoalCreate { .. } => Act::Start,
        Request::GoalJoin { .. } => Act::Join,
        Request::GoalLeave { .. } => Act::Leave,
        Request::GoalInvite { .. } => Act::Invite,
        Request::MemberRemove { .. } => Act::RemoveMember,
        Request::RulesBind { .. } => Act::ChangeRules,
        Request::TaskRevise { .. } => Act::Revise,
        Request::WorkspaceConnect { .. } => Act::ConnectFolder,
        Request::FarmOn { .. } => Act::Publish,
        _ => Act::PersonCommand,
    };
    let refused = Refused {
        agent: principal,
        agent_name,
        member_name: None,
        goal: frame.request.goal(),
        goal_title: None,
        act,
        task: None,
        task_title: None,
        why: Why::OnlyYou {
            operation: frame.request.name().into(),
            host,
        },
    };
    denied("this request is the owner's to make")
        .with_details(serde_json::to_value(refused).expect("refusal serializes"))
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
            (Audience::Owner | Audience::Host, None) => Ok(actor(None, false)),
            (Audience::Owner | Audience::Host, Some(_)) => Err(ApiError::new(
                ErrorCode::Invalid,
                "an owner-only request is not made on behalf of a principal",
            )),
            (_, Some(principal)) => {
                if principals.active(&principal).is_none() {
                    return Err(ApiError::new(
                        ErrorCode::NotFound,
                        principals.inactive_message(&principal),
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
                return Err(ApiError::new(
                    ErrorCode::Denied,
                    principals.inactive_message(&principal),
                ));
            }
            if matches!(operation.audience, Audience::Owner | Audience::Host) {
                return Err(only_you(
                    principals,
                    principal,
                    frame,
                    operation.audience == Audience::Host,
                ));
            }
            Ok(actor(Some(principal), false))
        }
        Caller::Author(principal) => {
            if frame.on_behalf.is_some()
                || session.is_some()
                || operation.audience != Audience::Author
            {
                return Err(denied(
                    "this credential only accesses its private formation catalog",
                ));
            }
            if principals.active(&principal).is_none() {
                return Err(ApiError::new(
                    ErrorCode::Denied,
                    principals.inactive_message(&principal),
                ));
            }
            Ok(actor(Some(principal), false))
        }
    }
}
