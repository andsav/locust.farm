//! Blocking client for the local daemon API.
//!
//! [`Client`] speaks the framing and handshake of [`crate::api`] over any
//! byte stream: a Unix socket in the CLI, the bridge and the adapters, an
//! in-memory stream in tests. It makes one call at a time: a request frame
//! out, the response frame with the same `id` back. It opens no sockets,
//! reads no files and starts no threads; the caller connects the stream and
//! reads the credential. A caller that still needs the stream, for a read
//! timeout or a shutdown, opens the client over a reference to it.
//!
//! After any error other than [`ClientError::Api`] and
//! [`ClientError::TooLarge`] the stream is no longer in step with the daemon
//! and the client must be dropped.

use std::fmt;
use std::io::{self, Read, Write};

use crate::API_VERSION;
use crate::api::{
    ApiError, Caller, ClientHello, Credential, Request, RequestFrame, Response, ResponseFrame,
    ServerHello, SessionSecret,
};
use crate::codec::{self, FRAME_PREFIX_BYTES};
use crate::id::{IdempotencyKey, PublicKey};
use crate::limits::{MAX_HELLO_FRAME_BYTES, MAX_LOCAL_FRAME_BYTES};

/// Buffer capacity kept between calls. A larger buffer, left by a call that
/// carried content, is released instead of held for the life of the client.
const KEPT_BUFFER_BYTES: usize = 64 * 1024;

/// Why a call did not return a response.
#[derive(Debug)]
pub enum ClientError {
    /// The stream failed, or the daemon announced a frame above the admitted
    /// size.
    Io(io::Error),
    /// The daemon closed the connection instead of answering.
    Closed,
    /// The daemon answered with something this client cannot accept: bytes
    /// that do not decode, another request's `id`, or a response of the
    /// wrong kind for the request.
    Protocol(&'static str),
    /// The daemon refused the hello. Carries its reason and the versions it
    /// reported.
    Refused {
        /// Why the daemon refused.
        error: ApiError,
        /// The API version the daemon speaks.
        api_version: u16,
        /// The daemon's own version.
        daemon_version: String,
    },
    /// The request would not fit a local frame. Nothing was sent and the
    /// connection is still usable.
    TooLarge,
    /// The daemon answered the request with an error. The connection is
    /// still usable.
    Api(ApiError),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "connection to the daemon failed: {error}"),
            Self::Closed => f.write_str("the daemon closed the connection"),
            Self::Protocol(what) => write!(f, "unexpected answer from the daemon: {what}"),
            Self::Refused {
                error,
                api_version,
                daemon_version,
            } => write!(
                f,
                "the daemon ({daemon_version}, API version {api_version}) refused the connection: {error}"
            ),
            Self::TooLarge => f.write_str("the request is too large for one frame"),
            Self::Api(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ClientError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for ClientError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// One open connection to the daemon, past the hello.
pub struct Client<S> {
    stream: S,
    last_id: u64,
    buffer: Vec<u8>,
    caller: Caller,
    daemon_version: String,
    max_blob_bytes: u64,
}

impl<S: Read + Write> Client<S> {
    /// Sends the hello over a connected stream and reads the daemon's answer.
    /// `session` makes the connection an execution session, which is what
    /// lets it hold claims and write its session record.
    pub fn open(
        mut stream: S,
        credential: Credential,
        session: Option<SessionSecret>,
    ) -> Result<Self, ClientError> {
        let hello = ClientHello {
            api_version: API_VERSION,
            credential,
            session,
        };
        let mut buffer = Vec::new();
        codec::encode_frame(&hello, &mut buffer)
            .map_err(|_| ClientError::Protocol("hello could not be encoded"))?;
        stream.write_all(&buffer)?;
        stream.flush()?;

        let frame =
            codec::read_frame(&mut stream, MAX_HELLO_FRAME_BYTES)?.ok_or(ClientError::Closed)?;
        match codec::decode(&frame) {
            Ok(ServerHello::Welcome {
                api_version: API_VERSION,
                daemon_version,
                caller,
                max_blob_bytes,
            }) => Ok(Self {
                stream,
                last_id: 0,
                buffer,
                caller,
                daemon_version,
                max_blob_bytes,
            }),
            Ok(ServerHello::Welcome { .. }) => Err(ClientError::Protocol(
                "daemon welcomed the connection with another API version",
            )),
            Ok(ServerHello::Refused {
                error,
                api_version,
                daemon_version,
            }) => Err(ClientError::Refused {
                error,
                api_version,
                daemon_version,
            }),
            Err(_) => Err(ClientError::Protocol("hello answer does not decode")),
        }
    }

    /// Who the daemon resolved the credential to.
    pub fn caller(&self) -> Caller {
        self.caller
    }

    /// The daemon's version, as it reported it in the hello.
    pub fn daemon_version(&self) -> &str {
        &self.daemon_version
    }

    /// Largest content object the daemon stores, in bytes.
    pub fn max_blob_bytes(&self) -> u64 {
        self.max_blob_bytes
    }

    /// Makes one request and waits for its response. A blocking request such
    /// as `wait` blocks here for as long as the daemon holds it.
    pub fn call(&mut self, request: Request) -> Result<Response, ClientError> {
        self.call_with(request, None, None)
    }

    /// Like [`Client::call`], with an idempotency key, a principal the owner
    /// acts on behalf of, or both; see [`RequestFrame`].
    pub fn call_with(
        &mut self,
        request: Request,
        idempotency: Option<IdempotencyKey>,
        on_behalf: Option<PublicKey>,
    ) -> Result<Response, ClientError> {
        let frame = RequestFrame {
            id: self.last_id.wrapping_add(1),
            idempotency,
            on_behalf,
            request,
        };
        self.exchange(&frame, MAX_LOCAL_FRAME_BYTES)
    }

    fn exchange(
        &mut self,
        frame: &RequestFrame,
        max_frame: usize,
    ) -> Result<Response, ClientError> {
        self.buffer.clear();
        let encoded = codec::encode_frame(frame, &mut self.buffer);
        let fits = encoded.is_ok() && self.buffer.len() - FRAME_PREFIX_BYTES <= max_frame;
        let sent = if fits {
            self.stream
                .write_all(&self.buffer)
                .and_then(|()| self.stream.flush())
        } else {
            Ok(())
        };
        if self.buffer.capacity() > KEPT_BUFFER_BYTES {
            self.buffer = Vec::new();
        }
        if !fits {
            return Err(ClientError::TooLarge);
        }
        sent?;
        self.last_id = frame.id;

        let answer = codec::read_frame(&mut self.stream, MAX_LOCAL_FRAME_BYTES)?
            .ok_or(ClientError::Closed)?;
        let answer: ResponseFrame = codec::decode(&answer)
            .map_err(|_| ClientError::Protocol("response does not decode"))?;
        if answer.id != frame.id {
            return Err(ClientError::Protocol("response answers another request"));
        }
        match answer.result {
            Ok(response) if frame.request.is_answered_by(&response) => Ok(response),
            Ok(_) => Err(ClientError::Protocol(
                "response is of the wrong kind for the request",
            )),
            Err(error) => Err(ClientError::Api(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use serde::Serialize;

    use super::*;
    use crate::api::{ErrorCode, WaitOutcome};
    use crate::id::{BlobHash, EventId, GoalId};

    const CREDENTIAL: Credential = Credential([0x5a; 32]);
    const SESSION: SessionSecret = SessionSecret([0x6b; 32]);

    /// An in-memory stream whose far side is a scripted daemon: every whole
    /// frame the client writes is handed to `daemon`, and the bytes it
    /// returns are what the client reads next. No bytes left means the
    /// daemon closed the connection.
    struct Duplex<D> {
        written: Vec<u8>,
        readable: VecDeque<u8>,
        daemon: D,
    }

    impl<D: FnMut(Vec<u8>) -> Vec<u8>> Duplex<D> {
        fn new(daemon: D) -> Self {
            Self {
                written: Vec::new(),
                readable: VecDeque::new(),
                daemon,
            }
        }
    }

    impl<D: FnMut(Vec<u8>) -> Vec<u8>> Write for Duplex<D> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.written.extend_from_slice(bytes);
            while let Some(payload) = codec::read_frame(&mut self.written.as_slice(), usize::MAX)
                .ok()
                .flatten()
            {
                self.written.drain(..FRAME_PREFIX_BYTES + payload.len());
                let reply = (self.daemon)(payload);
                self.readable.extend(reply);
            }
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<D> Read for Duplex<D> {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            // One byte at a time: the client must cope with short reads.
            match (self.readable.pop_front(), out.first_mut()) {
                (Some(byte), Some(slot)) => {
                    *slot = byte;
                    Ok(1)
                }
                (Some(byte), None) => {
                    self.readable.push_front(byte);
                    Ok(0)
                }
                (None, _) => Ok(0),
            }
        }
    }

    fn frame<T: Serialize>(value: &T) -> Vec<u8> {
        let mut bytes = Vec::new();
        codec::encode_frame(value, &mut bytes).unwrap();
        bytes
    }

    fn welcome() -> Vec<u8> {
        frame(&ServerHello::Welcome {
            api_version: API_VERSION,
            daemon_version: "0.1.0".to_string(),
            caller: Caller::Agent(PublicKey([2; 32])),
            max_blob_bytes: 1024,
        })
    }

    fn answer(id: u64, result: Result<Response, ApiError>) -> Vec<u8> {
        frame(&ResponseFrame { id, result })
    }

    /// A daemon that welcomes the hello and then answers each request with
    /// what `respond` returns for it.
    fn daemon(
        mut respond: impl FnMut(RequestFrame) -> Vec<u8>,
    ) -> Duplex<impl FnMut(Vec<u8>) -> Vec<u8>> {
        let mut greeted = false;
        Duplex::new(move |payload: Vec<u8>| {
            if !greeted {
                greeted = true;
                assert!(ClientHello::decode(&payload).is_ok());
                return welcome();
            }
            respond(codec::decode(&payload).unwrap())
        })
    }

    fn goal() -> GoalId {
        GoalId([1; 32])
    }

    fn pending() -> Request {
        Request::Pending { goal: goal() }
    }

    #[test]
    fn open_presents_the_credential_and_session_and_learns_the_caller() {
        let mut hellos = Vec::new();
        let stream = Duplex::new(|payload: Vec<u8>| {
            assert!(payload.len() <= MAX_HELLO_FRAME_BYTES);
            hellos.push(ClientHello::decode(&payload).unwrap());
            welcome()
        });
        let client = Client::open(stream, CREDENTIAL, Some(SESSION)).unwrap();
        assert_eq!(client.caller(), Caller::Agent(PublicKey([2; 32])));
        assert_eq!(client.daemon_version(), "0.1.0");
        assert_eq!(client.max_blob_bytes(), 1024);
        drop(client);
        assert_eq!(
            hellos,
            [ClientHello {
                api_version: API_VERSION,
                credential: CREDENTIAL,
                session: Some(SESSION),
            }]
        );
    }

    #[test]
    fn a_refused_hello_reports_the_daemons_reason_and_versions() {
        let stream = Duplex::new(|_: Vec<u8>| {
            frame(&ServerHello::Refused {
                error: ApiError::new(ErrorCode::Denied, "credential is not known"),
                api_version: 3,
                daemon_version: "0.9.0".to_string(),
            })
        });
        let Err(ClientError::Refused {
            error,
            api_version,
            daemon_version,
        }) = Client::open(stream, CREDENTIAL, None)
        else {
            panic!("expected a refusal");
        };
        assert_eq!(error.code, ErrorCode::Denied);
        assert_eq!((api_version, daemon_version.as_str()), (3, "0.9.0"));
    }

    #[test]
    fn a_hello_answer_that_cannot_be_accepted_is_an_error() {
        let closed = Duplex::new(|_: Vec<u8>| Vec::new());
        assert!(matches!(
            Client::open(closed, CREDENTIAL, None),
            Err(ClientError::Closed)
        ));

        let garbage = Duplex::new(|_: Vec<u8>| frame(&(250u8, 7u8)));
        assert!(matches!(
            Client::open(garbage, CREDENTIAL, None),
            Err(ClientError::Protocol(_))
        ));

        let other_version = Duplex::new(|_: Vec<u8>| {
            frame(&ServerHello::Welcome {
                api_version: API_VERSION + 1,
                daemon_version: "9.0.0".to_string(),
                caller: Caller::Owner,
                max_blob_bytes: 0,
            })
        });
        assert!(matches!(
            Client::open(other_version, CREDENTIAL, None),
            Err(ClientError::Protocol(_))
        ));

        // A hello answer is read with the hello limit, before allocating.
        let oversized = Duplex::new(|_: Vec<u8>| {
            let announced = MAX_HELLO_FRAME_BYTES as u32 + 1;
            announced.to_le_bytes().to_vec()
        });
        let Err(ClientError::Io(error)) = Client::open(oversized, CREDENTIAL, None) else {
            panic!("expected the oversized frame to be refused");
        };
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);

        let truncated = Duplex::new(|_: Vec<u8>| {
            let mut bytes = welcome();
            bytes.truncate(bytes.len() - 1);
            bytes
        });
        let Err(ClientError::Io(error)) = Client::open(truncated, CREDENTIAL, None) else {
            panic!("expected the truncated frame to be an error");
        };
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn a_call_is_one_request_and_the_response_with_its_id() {
        let mut seen = Vec::new();
        let stream = daemon(|request: RequestFrame| {
            let reply = answer(request.id, Ok(Response::Pending(Default::default())));
            seen.push(request);
            reply
        });
        let mut client = Client::open(stream, CREDENTIAL, None).unwrap();
        for _ in 0..3 {
            assert_eq!(
                client.call(pending()).unwrap(),
                Response::Pending(Default::default())
            );
        }
        drop(client);
        let ids: Vec<u64> = seen.iter().map(|frame| frame.id).collect();
        assert_eq!(ids, [1, 2, 3]);
        assert!(seen.iter().all(|frame| {
            frame.request == pending() && frame.idempotency.is_none() && frame.on_behalf.is_none()
        }));
    }

    #[test]
    fn an_idempotency_key_and_a_principal_travel_in_the_frame() {
        let mut seen = Vec::new();
        let stream = daemon(|request: RequestFrame| {
            let reply = answer(
                request.id,
                Ok(Response::Recorded {
                    event: EventId([9; 32]),
                }),
            );
            seen.push(request);
            reply
        });
        let mut client = Client::open(stream, CREDENTIAL, None).unwrap();
        let request = Request::WorkDecline {
            goal: goal(),
            offer: EventId([3; 32]),
        };
        let key = IdempotencyKey([4; 16]);
        let principal = PublicKey([5; 32]);
        client
            .call_with(request.clone(), Some(key), Some(principal))
            .unwrap();
        drop(client);
        assert_eq!(
            seen,
            [RequestFrame {
                id: 1,
                idempotency: Some(key),
                on_behalf: Some(principal),
                request,
            }]
        );
    }

    #[test]
    fn a_daemon_error_is_returned_and_the_connection_stays_usable() {
        let mut calls = 0;
        let stream = daemon(|request: RequestFrame| {
            calls += 1;
            if calls == 1 {
                answer(
                    request.id,
                    Err(ApiError::new(ErrorCode::ClaimHeld, "another session")),
                )
            } else {
                answer(request.id, Ok(Response::Waited(WaitOutcome::NoEvent)))
            }
        });
        let mut client = Client::open(stream, CREDENTIAL, Some(SESSION)).unwrap();
        let claim = Request::AttemptStart {
            goal: goal(),
            task: crate::event::TaskId::Authored(EventId([3; 32])),
            offer: None,
        };
        let Err(ClientError::Api(error)) = client.call(claim) else {
            panic!("expected the daemon's error");
        };
        assert_eq!(error.code, ErrorCode::ClaimHeld);

        let wait = Request::Wait {
            goal: goal(),
            seen: 4,
            timeout_ms: 0,
        };
        assert_eq!(
            client.call(wait).unwrap(),
            Response::Waited(WaitOutcome::NoEvent)
        );
    }

    #[test]
    fn a_response_with_another_id_is_a_protocol_error() {
        let stream = daemon(|request: RequestFrame| answer(request.id + 1, Ok(Response::Done)));
        let mut client = Client::open(stream, CREDENTIAL, None).unwrap();
        assert!(matches!(
            client.call(Request::Shutdown),
            Err(ClientError::Protocol("response answers another request"))
        ));
    }

    #[test]
    fn a_response_of_the_wrong_kind_is_a_protocol_error() {
        let stream = daemon(|request: RequestFrame| answer(request.id, Ok(Response::Done)));
        let mut client = Client::open(stream, CREDENTIAL, None).unwrap();
        assert!(matches!(
            client.call(pending()),
            Err(ClientError::Protocol(
                "response is of the wrong kind for the request"
            ))
        ));
    }

    #[test]
    fn a_response_that_does_not_decode_or_never_comes_is_an_error() {
        let stream = daemon(|_: RequestFrame| frame(&(0u8, 250u8)));
        let mut client = Client::open(stream, CREDENTIAL, None).unwrap();
        assert!(matches!(
            client.call(pending()),
            Err(ClientError::Protocol("response does not decode"))
        ));

        let stream = daemon(|_: RequestFrame| Vec::new());
        let mut client = Client::open(stream, CREDENTIAL, None).unwrap();
        assert!(matches!(client.call(pending()), Err(ClientError::Closed)));

        let stream = daemon(|_: RequestFrame| {
            let announced = MAX_LOCAL_FRAME_BYTES as u32 + 1;
            announced.to_le_bytes().to_vec()
        });
        let mut client = Client::open(stream, CREDENTIAL, None).unwrap();
        let Err(ClientError::Io(error)) = client.call(pending()) else {
            panic!("expected the oversized frame to be refused");
        };
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn a_request_that_does_not_fit_a_frame_is_refused_before_sending() {
        let mut received = 0;
        let stream = daemon(|request: RequestFrame| {
            received += 1;
            answer(
                request.id,
                Ok(Response::BlobStored {
                    hash: BlobHash([7; 32]),
                }),
            )
        });
        let mut client = Client::open(stream, CREDENTIAL, None).unwrap();
        let put = |id| RequestFrame {
            id,
            idempotency: None,
            on_behalf: None,
            request: Request::BlobPut {
                goal: goal(),
                bytes: vec![0; 200_000],
            },
        };
        let size = codec::encode(&put(1)).unwrap().len();

        assert!(matches!(
            client.exchange(&put(1), size - 1),
            Err(ClientError::TooLarge)
        ));
        // Nothing was sent, the id was not used, and the large buffer is gone.
        assert_eq!(client.last_id, 0);
        assert!(client.buffer.capacity() <= KEPT_BUFFER_BYTES);

        // The same request within the limit goes through on the same connection.
        assert!(client.exchange(&put(1), size).is_ok());
        assert_eq!(client.last_id, 1);
        assert!(client.buffer.capacity() <= KEPT_BUFFER_BYTES);
        drop(client);
        assert_eq!(received, 1);
    }

    #[test]
    fn errors_read_as_sentences() {
        assert_eq!(
            ClientError::Api(ApiError::new(
                ErrorCode::Superseded,
                "generation 1 is stale"
            ))
            .to_string(),
            "superseded: generation 1 is stale"
        );
        assert_eq!(
            ClientError::Refused {
                error: ApiError::new(ErrorCode::UnsupportedVersion, "client is too old"),
                api_version: 2,
                daemon_version: "0.4.0".to_string(),
            }
            .to_string(),
            "the daemon (0.4.0, API version 2) refused the connection: unsupported_version: client is too old"
        );
        assert_eq!(
            ClientError::Closed.to_string(),
            "the daemon closed the connection"
        );
    }
}
