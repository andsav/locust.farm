//! Local requests: resolution of the caller, idempotency, and dispatch to the
//! module that owns each family of operations.

mod daemon;

use locust_proto::api::{ApiError, ErrorCode, Request, RequestFrame, Response, ResponseFrame};
use locust_proto::engine::{ConnId, Entropy, Parked, Step};
use locust_proto::store::Store;

use super::Node;
use super::callers::{self, Actor};
use super::commit::{Tx, request_digest};

/// A request planned against the node as it is: the answer, and what must be
/// durable before the answer is released.
pub(super) struct Planned {
    pub response: Response,
    pub tx: Tx,
}

pub(super) type Plan = Result<Planned, ApiError>;

/// An answer that changes nothing.
pub(super) fn answer(response: Response) -> Plan {
    Ok(Planned {
        response,
        tx: Tx::none(),
    })
}

/// The refusal for an operation this build does not implement yet.
pub(super) fn not_implemented(request: &Request) -> ApiError {
    ApiError::new(
        ErrorCode::Unavailable,
        format!("{} is not implemented in this build", request.name()),
    )
}

impl<S: Store, E: Entropy> Node<S, E> {
    /// Handles one request frame from a welcomed connection.
    pub(super) fn handle(&mut self, conn: ConnId, frame: RequestFrame, now_ms: u64) -> Step {
        let id = frame.id;
        let result = self.respond(conn, frame, now_ms);
        match result {
            Ok(Step::Park(parked)) => Step::Park(parked),
            Ok(Step::Reply(frame)) => Step::Reply(frame),
            Err(error) => Step::Reply(ResponseFrame {
                id,
                result: Err(error),
            }),
        }
    }

    fn respond(
        &mut self,
        conn: ConnId,
        frame: RequestFrame,
        now_ms: u64,
    ) -> Result<Step, ApiError> {
        let Some(state) = self.conns.get(&conn) else {
            return Err(ApiError::new(
                ErrorCode::Denied,
                "the connection has not been welcomed",
            ));
        };
        frame.request.check()?;
        let actor = callers::resolve(&self.principals, state.caller, state.session, &frame)?;
        let id = frame.id;
        let reply = |response| {
            Ok(Step::Reply(ResponseFrame {
                id,
                result: Ok(response),
            }))
        };

        // A read changes nothing, so running it again is its own idempotency;
        // only requests that write are recorded under their key.
        let key = frame
            .idempotency
            .filter(|_| !frame.request.is_read_only() && !actor.is_viewer());
        let digest = key.map(|_| request_digest(frame.on_behalf, &frame.request));
        if let (Some(key), Some(digest)) = (&key, &digest)
            && let Some(response) = self.replayed(actor.caller, key, digest)?
        {
            return reply(response);
        }

        let Planned { response, mut tx } = self.plan(&actor, frame.request, now_ms)?;
        if let (Some(key), Some(digest)) = (&key, digest) {
            tx.local(Self::remember(actor.caller, key, digest, &response));
        }
        self.land(tx)?;
        reply(response)
    }

    /// Plans one request without changing anything.
    fn plan(&self, actor: &Actor, request: Request, _now_ms: u64) -> Plan {
        match request {
            Request::Status => self.status(actor),
            Request::Shutdown => self.shutdown(),
            Request::AgentEnroll {
                name,
                grants,
                credential,
            } => self.agent_enroll(name, grants, credential),
            other => Err(not_implemented(&other)),
        }
    }

    /// Revisits a parked wait.
    pub(super) fn resume_wait(
        &mut self,
        _conn: ConnId,
        parked: &Parked,
        _timed_out: bool,
        _now_ms: u64,
    ) -> Step {
        Step::Reply(ResponseFrame {
            id: parked.request_id,
            result: Err(ApiError::new(
                ErrorCode::Unavailable,
                "wait is not implemented in this build",
            )),
        })
    }
}
