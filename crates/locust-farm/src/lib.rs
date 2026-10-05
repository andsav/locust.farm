//! Signed public farm service. Only validated public snapshots enter this store.

use rusqlite::Connection;
use std::{
    collections::HashMap,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Semaphore, broadcast, watch};

/// Operator policy, independent of any uploader-controlled fields.
#[derive(Clone, Debug)]
pub struct Config {
    pub max_body_bytes: usize,
    pub min_mutation_interval_ms: u64,
    pub max_streams: usize,
    pub max_requests_per_second: u32,
    pub public_enrollment: bool,
    pub retention_ms: u64,
    pub abuse_contact: String,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            max_body_bytes: 256 * 1024,
            min_mutation_interval_ms: 1000,
            max_streams: 512,
            max_requests_per_second: 500,
            public_enrollment: false,
            retention_ms: 30 * 24 * 60 * 60 * 1000,
            abuse_contact: "Contact the deployment operator".into(),
        }
    }
}

/// Lightweight per-viewer and request-rate bookkeeping. Kept separate from the
/// DB lock so request-rate accounting and stream teardown never block behind a
/// long SQLite operation running on a blocking thread.
struct Meta {
    streams: usize,
    request_window: u64,
    request_count: u32,
    channels: HashMap<String, broadcast::Sender<String>>,
}

/// The SQLite database and publisher rate-limit instants. Access is serialized
/// for async HTTP callers by a single-permit semaphore and offloaded to a
/// blocking thread, so the current_thread async executor stays free while SQLite
/// works. Sync public APIs (CLI/tests) lock the mutex directly.
struct Db {
    db: Connection,
    last_mutation: HashMap<String, std::time::Instant>,
}

#[derive(Clone)]
pub struct Service {
    config: Config,
    meta: Arc<Mutex<Meta>>,
    db: Arc<Mutex<Db>>,
    /// One permit: at most one async HTTP DB operation runs at a time, offloaded
    /// to a blocking thread. Sync public APIs bypass this and lock `db` directly.
    db_permit: Arc<Semaphore>,
    /// Earliest retention deadline (closed_at + retention_ms) among available
    /// closed farms, or u64::MAX when none remain. Atomic so the async maintain
    /// check is lock-free and never blocks behind a DB operation.
    next_expire: Arc<AtomicU64>,
    shutdown: watch::Sender<bool>,
}

impl Service {
    pub fn open(path: impl AsRef<Path>, config: Config) -> Result<Self, rusqlite::Error> {
        let db = Connection::open(path)?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version != 0 && version != 1 {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let count: i64 = db.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table'",
            [],
            |r| r.get(0),
        )?;
        if version == 0 && count != 0 {
            return Err(rusqlite::Error::InvalidQuery);
        }

        db.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS enrollment (farm_id TEXT PRIMARY KEY);
            CREATE TABLE IF NOT EXISTS farms (
                farm_id TEXT PRIMARY KEY, public_key TEXT NOT NULL,
                seq INTEGER NOT NULL, version INTEGER NOT NULL,
                state TEXT NOT NULL, listed INTEGER NOT NULL,
                snapshot TEXT, received_at INTEGER NOT NULL,
                semantic_at INTEGER NOT NULL, closed_at INTEGER);
            CREATE TABLE IF NOT EXISTS receipts (
                farm_id TEXT NOT NULL, seq INTEGER NOT NULL, digest TEXT NOT NULL,
                receipt TEXT NOT NULL, PRIMARY KEY (farm_id,seq));
            CREATE INDEX IF NOT EXISTS gallery ON farms (listed,state,semantic_at,farm_id);
            CREATE TABLE IF NOT EXISTS gallery_history (position INTEGER PRIMARY KEY AUTOINCREMENT,farm_id TEXT NOT NULL,semantic_at INTEGER NOT NULL,listed INTEGER NOT NULL);
            CREATE INDEX IF NOT EXISTS gallery_history_farm ON gallery_history (farm_id,position);
            CREATE INDEX IF NOT EXISTS expire ON farms (closed_at,farm_id) WHERE state='available' AND closed_at IS NOT NULL;
            PRAGMA user_version=1;",
        )?;
        let earliest_closed: Option<i64> = db.query_row(
            "SELECT min(closed_at) FROM farms WHERE state='available' AND closed_at IS NOT NULL",
            [],
            |r| r.get::<_, Option<i64>>(0),
        )?;
        let next_expire = earliest_closed.map_or(u64::MAX, |t| {
            (t.max(0) as u64).saturating_add(config.retention_ms)
        });
        Ok(Self {
            shutdown: watch::channel(false).0,
            config,
            meta: Arc::new(Mutex::new(Meta {
                streams: 0,
                request_window: 0,
                request_count: 0,
                channels: HashMap::new(),
            })),
            db: Arc::new(Mutex::new(Db {
                db,
                last_mutation: HashMap::new(),
            })),
            db_permit: Arc::new(Semaphore::new(1)),
            next_expire: Arc::new(AtomicU64::new(next_expire)),
        })
    }
    pub fn enroll(&self, farm_id: &str) -> Result<(), ServiceError> {
        FarmId(farm_id.into())
            .validate()
            .map_err(|e| ServiceError::new(StatusCode::BAD_REQUEST, e))?;
        self.db.lock().expect("farm db poisoned").db.execute(
            "INSERT OR IGNORE INTO enrollment (farm_id) VALUES (?)",
            [farm_id],
        )?;
        Ok(())
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as u64
}

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path as RoutePath, Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post, put},
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;

use locust_proto::farm::{
    FarmAvailability, FarmGoalState, FarmId, FarmOperation, FarmUploadBody, FarmVisibility,
    SignedFarmRequest,
};
pub use locust_proto::farm::{FarmReceipt as Receipt, FarmServiceView as PublicState};
#[derive(Debug)]
pub struct ServiceError {
    status: StatusCode,
    message: String,
}
impl ServiceError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
    fn db(error: rusqlite::Error) -> Self {
        eprintln!("farm database error: {error}");
        Self::new(StatusCode::SERVICE_UNAVAILABLE, "service unavailable")
    }
}
impl From<rusqlite::Error> for ServiceError {
    fn from(error: rusqlite::Error) -> Self {
        Self::db(error)
    }
}
impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        (
            self.status,
            [("cache-control", "no-store")],
            Json(serde_json::json!({"error":self.message})),
        )
            .into_response()
    }
}
impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ServiceError {}

fn unavailable(id: &str, version: u64, time: u64) -> PublicState {
    PublicState {
        farm_id: FarmId(id.into()),
        stream_version: version,
        service_time_ms: time,
        status: FarmAvailability::Unavailable,
        received_at_ms: None,
        snapshot: None,
        visibility: None,
    }
}
fn current(db: &Db, id: &str, time: u64) -> Result<PublicState, ServiceError> {
    let row = db
        .db
        .query_row(
            "SELECT version,state,snapshot,received_at,listed FROM farms WHERE farm_id=?",
            [id],
            |r| {
                Ok((
                    (r.get::<_, i64>(0)? as u64),
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    (r.get::<_, i64>(3)? as u64),
                    r.get::<_, bool>(4)?,
                ))
            },
        )
        .optional()
        .map_err(ServiceError::db)?;
    match row {
        Some((version, state, Some(snapshot), received, listed)) if state == "available" => {
            Ok(PublicState {
                farm_id: FarmId(id.into()),
                stream_version: version,
                service_time_ms: time,
                status: FarmAvailability::Available,
                received_at_ms: Some(received),
                visibility: Some(if listed {
                    FarmVisibility::Listed
                } else {
                    FarmVisibility::Link
                }),
                snapshot: Some(serde_json::from_str(&snapshot).map_err(|_| {
                    ServiceError::new(StatusCode::SERVICE_UNAVAILABLE, "service unavailable")
                })?),
            })
        }
        Some((version, _, _, _, _)) => Ok(unavailable(id, version, time)),
        None => Ok(unavailable(id, 0, time)),
    }
}
/// Reads the current public state and pushes it to any open viewers. Called
/// while the DB is already locked; takes the meta lock only to reach the channel.
/// Lock order is always DB -> meta, so request-rate and stream teardown (which
/// take only meta) never deadlock against it.
fn publish(db: &Db, meta: &Arc<Mutex<Meta>>, id: &str, time: u64) -> Result<(), ServiceError> {
    let state = current(db, id, time)?;
    if let Some(channel) = meta.lock().expect("farm meta poisoned").channels.get(id) {
        let _ = channel.send(serde_json::to_string(&state).expect("public state JSON"));
    }
    Ok(())
}
impl Service {
    /// Close stream responses during process shutdown. Public state remains
    /// available in SQLite for reconnection to the next service process.
    pub fn shutdown(&self) {
        self.shutdown.send_replace(true);
    }
    pub fn router(&self) -> Router {
        Router::new()
            .route("/health", get(health))
            .route("/api/farms", get(gallery))
            .route("/api/farms/{id}", put(upload).get(read).delete(delete))
            .route("/api/farms/{id}/check-in", post(check_in))
            .route("/api/farms/{id}/suspend", post(suspend))
            .route("/api/farms/{id}/events", get(events))
            .layer(DefaultBodyLimit::max(self.config.max_body_bytes))
            .layer(axum::middleware::from_fn_with_state(
                self.clone(),
                request_rate,
            ))
            .with_state(self.clone())
    }
    pub fn read(&self, id: &str) -> Result<PublicState, ServiceError> {
        self.expire_if_needed()?;
        current(&self.db.lock().expect("farm db poisoned"), id, now_ms())
    }
    pub fn take_down(&self, id: &str) -> Result<(), ServiceError> {
        FarmId(id.into())
            .validate()
            .map_err(|e| ServiceError::new(StatusCode::BAD_REQUEST, e))?;
        let store = self.db.lock().expect("farm db poisoned");
        let time = now_ms();
        // Unknown ids also get a permanent tombstone; no future enrollment bypasses it.
        store.db.execute("INSERT INTO farms (farm_id,public_key,seq,version,state,listed,snapshot,received_at,semantic_at) VALUES (?,'',0,1,'deleted',0,NULL,?,?) ON CONFLICT(farm_id) DO UPDATE SET version=version+1,state='deleted',listed=0,snapshot=NULL,closed_at=NULL",params![id,time as i64,time as i64]).map_err(ServiceError::db)?;
        publish(&store, &self.meta, id, time)
    }
    pub fn expire(&self) -> Result<(), ServiceError> {
        let mut store = self.db.lock().expect("farm db poisoned");
        run_expire(
            &mut store,
            &self.meta,
            &self.next_expire,
            self.config.retention_ms,
        )
    }
    /// Deduped, indexed expiration: skips the scan entirely while no farm is due,
    /// so a read or viewer poll does not touch the database each time. When a
    /// farm is due, the partial `expire` index serves the scan and the deadline
    /// is recomputed from the earliest remaining closed farm.
    pub fn expire_if_needed(&self) -> Result<(), ServiceError> {
        let now = now_ms();
        if now < self.next_expire.load(Ordering::Acquire) {
            return Ok(());
        }
        let mut store = self.db.lock().expect("farm db poisoned");
        run_expire(
            &mut store,
            &self.meta,
            &self.next_expire,
            self.config.retention_ms,
        )
    }
    /// Async maintenance hook for HTTP handlers: checks the lock-free deadline
    /// and, only when a farm is due, offloads the indexed expire scan to a
    /// blocking thread through the serialized DB helper.
    pub async fn maintain(&self) -> Result<(), ServiceError> {
        let now = now_ms();
        if now < self.next_expire.load(Ordering::Acquire) {
            return Ok(());
        }
        let retention = self.config.retention_ms;
        let next_expire = self.next_expire.clone();
        self.with_db(move |db, meta| run_expire(db, meta, &next_expire, retention))
            .await
    }
    /// Serialized async DB access for HTTP handlers: acquires the single DB
    /// permit, then runs the closure on a blocking thread holding the DB lock,
    /// so the current_thread async executor never blocks on SQLite.
    async fn with_db<F, R>(&self, f: F) -> Result<R, ServiceError>
    where
        F: FnOnce(&mut Db, &Arc<Mutex<Meta>>) -> Result<R, ServiceError> + Send + 'static,
        R: Send + 'static,
    {
        let permit = self.db_permit.clone().acquire_owned().await.map_err(|_| {
            ServiceError::new(StatusCode::SERVICE_UNAVAILABLE, "service unavailable")
        })?;
        let db = self.db.clone();
        let meta = self.meta.clone();
        tokio::task::spawn_blocking(move || {
            // Cancellation cannot release capacity while this work still runs.
            let _permit = permit;
            let mut guard = db.lock().expect("farm db poisoned");
            f(&mut guard, &meta)
        })
        .await
        .map_err(|_| ServiceError::new(StatusCode::SERVICE_UNAVAILABLE, "service unavailable"))?
    }
    /// Async read for HTTP and SSE: maintains expiration then reads the current
    /// state through the serialized blocking DB helper.
    async fn read_state_async(&self, id: String) -> Result<PublicState, ServiceError> {
        self.maintain().await?;
        self.with_db(move |db, _meta| current(db, &id, now_ms()))
            .await
    }
    /// Async mutation for HTTP: maintains expiration then applies the signed
    /// request through the serialized blocking DB helper.
    async fn mutate_async(
        &self,
        id: String,
        operation: FarmOperation,
        request: SignedFarmRequest,
    ) -> Result<Receipt, ServiceError> {
        self.maintain().await?;
        let config = self.config.clone();
        let next_expire = self.next_expire.clone();
        self.with_db(move |db, meta| {
            mutate_inner(db, meta, &next_expire, &config, &id, operation, &request)
        })
        .await
    }
}

/// Transitions closed farms past their retention window to deleted, publishes
/// the change to any open viewers, and recomputes the next deadline. The
/// `expire` partial index makes the scan touch only due farms, not every farm.
fn run_expire(
    db: &mut Db,
    meta: &Arc<Mutex<Meta>>,
    next_expire: &AtomicU64,
    retention_ms: u64,
) -> Result<(), ServiceError> {
    let time = now_ms();
    let cutoff = time.saturating_sub(retention_ms);
    let ids = {
        let mut query = db
            .db
            .prepare("SELECT farm_id FROM farms WHERE state='available' AND closed_at IS NOT NULL AND closed_at<=?")
            .map_err(ServiceError::db)?;
        query
            .query_map([cutoff as i64], |r| r.get::<_, String>(0))
            .map_err(ServiceError::db)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(ServiceError::db)?
    };
    for id in &ids {
        let changed = db.db.execute("UPDATE farms SET version=version+1,state='deleted',listed=0,snapshot=NULL,closed_at=NULL WHERE farm_id=? AND state='available' AND closed_at IS NOT NULL AND closed_at<=?",params![id,cutoff as i64]).map_err(ServiceError::db)?;
        if changed > 0 {
            publish(db, meta, id, time)?;
        }
    }
    let earliest: Option<i64> = db
        .db
        .query_row(
            "SELECT min(closed_at) FROM farms WHERE state='available' AND closed_at IS NOT NULL",
            [],
            |r| r.get::<_, Option<i64>>(0),
        )
        .map_err(ServiceError::db)?;
    next_expire.store(
        earliest.map_or(u64::MAX, |t| (t.max(0) as u64).saturating_add(retention_ms)),
        Ordering::Release,
    );
    Ok(())
}
async fn request_rate(
    State(service): State<Service>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<Response, ServiceError> {
    // Request-rate bookkeeping takes only the lightweight meta lock, never the
    // DB lock, so a long SQLite operation on a blocking thread cannot stall the
    // event loop's rate limiter.
    {
        let mut meta = service.meta.lock().expect("farm meta poisoned");
        let window = now_ms() / 1000;
        if window != meta.request_window {
            meta.request_window = window;
            meta.request_count = 0;
        }
        if meta.request_count >= service.config.max_requests_per_second {
            return Err(ServiceError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "service request rate exceeded",
            ));
        }
        meta.request_count += 1;
    }
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}
async fn health(State(service): State<Service>) -> Result<Response, ServiceError> {
    service
        .with_db(|db, _meta| {
            db.db
                .query_row("SELECT 1", [], |r| r.get::<_, u32>(0))
                .map_err(ServiceError::db)?;
            Ok(())
        })
        .await?;
    Ok(([("cache-control","no-store")],Json(serde_json::json!({"status":"ok","max_body_bytes":service.config.max_body_bytes,"min_mutation_interval_ms":service.config.min_mutation_interval_ms,"max_streams":service.config.max_streams,"max_requests_per_second":service.config.max_requests_per_second,"public_enrollment":service.config.public_enrollment,"abuse_contact":service.config.abuse_contact}))).into_response())
}
async fn read(
    State(service): State<Service>,
    RoutePath(id): RoutePath<String>,
) -> Result<Response, ServiceError> {
    let state = service.read_state_async(id).await?;
    let status = if state.status == FarmAvailability::Available {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    };
    Ok((status, [("cache-control", "no-store")], Json(state)).into_response())
}
struct StreamGuard {
    meta: Arc<Mutex<Meta>>,
    id: String,
    receiver: Option<broadcast::Receiver<String>>,
}
impl Drop for StreamGuard {
    fn drop(&mut self) {
        // Stream teardown takes only the lightweight meta lock, never the DB
        // lock, so a dropped viewer never blocks behind a SQLite operation.
        self.receiver.take();
        let mut meta = self.meta.lock().expect("farm meta poisoned");
        meta.streams -= 1;
        if meta
            .channels
            .get(&self.id)
            .is_some_and(|sender| sender.receiver_count() == 0)
        {
            meta.channels.remove(&self.id);
        }
    }
}
async fn events(
    State(service): State<Service>,
    RoutePath(id): RoutePath<String>,
    _headers: HeaderMap,
) -> Result<Response, ServiceError> {
    let mut shutdown = service.shutdown.subscribe();
    // Reserve a stream slot under the lightweight meta lock before any DB work.
    let mut guard = {
        let mut meta = service.meta.lock().expect("farm meta poisoned");
        if meta.streams >= service.config.max_streams {
            return Err(ServiceError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "stream capacity reached",
            ));
        }
        meta.streams += 1;
        StreamGuard {
            meta: service.meta.clone(),
            id: id.clone(),
            receiver: None,
        }
    };
    // Read the current state through the serialized blocking DB helper. If this
    // fails, the guard drops and releases the reserved slot.
    let initial = service.read_state_async(id.clone()).await?;
    guard.receiver = Some({
        let mut meta = service.meta.lock().expect("farm meta poisoned");
        if initial.status == FarmAvailability::Available {
            meta.channels
                .entry(id.clone())
                .or_insert_with(|| broadcast::channel(32).0)
                .subscribe()
        } else {
            broadcast::channel::<String>(1).1
        }
    });
    let stream = async_stream::stream! {
        let mut guard=guard;
        let mut state=initial;
        if *shutdown.borrow() {return;}
        loop {
            yield Ok::<Event,Infallible>(Event::default().event("state").id(state.stream_version.to_string()).data(serde_json::to_string(&state).expect("state JSON")));
            if state.status==FarmAvailability::Unavailable {break;}
            loop {
            let update=tokio::select! {
                biased;
                _=shutdown.changed()=>return,
                result=guard.receiver.as_mut().expect("subscribed stream").recv()=>match result {
                    Ok(json)=>serde_json::from_str(&json).ok(),
                    Err(broadcast::error::RecvError::Lagged(_))=>service.read_state_async(id.clone()).await.ok(),
                    Err(broadcast::error::RecvError::Closed)=>None,
                },
                _=tokio::time::sleep(std::time::Duration::from_secs(1))=>service.read_state_async(id.clone()).await.ok(),
            };
            match update {
                Some(next) if next.stream_version>state.stream_version =>{state=next;break;},
                Some(_)=>continue,
                None=>return,
            }
            }
        }
    };
    Ok((
        [("cache-control", "no-store"), ("x-accel-buffering", "no")],
        Sse::new(stream).keep_alive(KeepAlive::default()),
    )
        .into_response())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GalleryQuery {
    page_size: Option<usize>,
    cursor: Option<String>,
    filter: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    anchor: i64,
    last_time: i64,
    last_id: String,
    filter: String,
}
#[derive(Debug, Serialize)]
struct Gallery {
    farms: Vec<PublicState>,
    next_cursor: Option<String>,
}
fn encode_cursor(cursor: &Cursor) -> String {
    serde_json::to_vec(cursor)
        .expect("cursor JSON")
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn decode_cursor(text: &str) -> Result<Cursor, ServiceError> {
    let bad = || ServiceError::new(StatusCode::BAD_REQUEST, "invalid cursor");
    if text.len() > 8192
        || !text.len().is_multiple_of(2)
        || !text.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(bad());
    }
    let bytes = (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).map_err(|_| bad()))
        .collect::<Result<Vec<_>, _>>()?;
    serde_json::from_slice(&bytes).map_err(|_| bad())
}
/// The gallery query. Drives from currently listed/available farms and, for
/// each, finds its frozen gallery_history entry at or before the cursor anchor
/// via a correlated index lookup on `gallery_history_farm`. This is a per-farm
/// SEARCH (one index seek per current farm) rather than a SCAN of the
/// ever-growing history. The frozen semantic time, listed check, filter and
/// cursor ordering are unchanged from the original grouped subquery.
const GALLERY_QUERY: &str = "SELECT h.farm_id,h.semantic_at FROM farms f JOIN gallery_history h ON h.position=(SELECT position FROM gallery_history WHERE farm_id=f.farm_id AND position<=? ORDER BY position DESC LIMIT 1) WHERE h.listed=1 AND f.listed=1 AND f.state='available' AND (h.semantic_at<? OR (h.semantic_at=? AND h.farm_id>?)) AND (?='all' OR (?='ended' AND f.closed_at IS NOT NULL) OR (?='receiving' AND f.closed_at IS NULL AND f.received_at>=?) OR (?='quiet' AND f.closed_at IS NULL AND f.received_at<?)) ORDER BY h.semantic_at DESC,h.farm_id LIMIT ?";

async fn gallery(
    State(service): State<Service>,
    Query(query): Query<GalleryQuery>,
) -> Result<Response, ServiceError> {
    let size = query.page_size.unwrap_or(24).clamp(1, 100);
    let filter = query.filter.unwrap_or_else(|| "all".into());
    if !["all", "receiving", "quiet", "ended"].contains(&filter.as_str()) {
        return Err(ServiceError::new(StatusCode::BAD_REQUEST, "invalid filter"));
    }
    let gallery = service
        .with_db(move |db, _meta| {
            let time = now_ms();
            let mut cursor = match query.cursor {
                Some(text) => decode_cursor(&text)?,
                None => Cursor {
                    anchor: db
                        .db
                        .query_row(
                            "SELECT coalesce(max(position),0) FROM gallery_history",
                            [],
                            |r| r.get(0),
                        )
                        .map_err(ServiceError::db)?,
                    last_time: i64::MAX,
                    last_id: String::new(),
                    filter: filter.clone(),
                },
            };
            if cursor.filter != filter {
                return Err(ServiceError::new(
                    StatusCode::BAD_REQUEST,
                    "cursor filter mismatch",
                ));
            }
            let mut statement = db.db.prepare(GALLERY_QUERY).map_err(ServiceError::db)?;
            let quiet_cutoff = time.saturating_sub(120000);
            let rows = statement
                .query_map(
                    params![
                        cursor.anchor,
                        cursor.last_time,
                        cursor.last_time,
                        cursor.last_id,
                        filter,
                        filter,
                        filter,
                        quiet_cutoff as i64,
                        filter,
                        quiet_cutoff as i64,
                        (size + 1) as i64
                    ],
                    |r| Ok((r.get::<_, String>(0)?, (r.get::<_, i64>(1)? as u64))),
                )
                .map_err(ServiceError::db)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(ServiceError::db)?;
            let more = rows.len() > size;
            let mut entries = Vec::new();
            for (id, semantic) in rows.into_iter().take(size) {
                entries.push(current(db, &id, time)?);
                cursor.last_id = id;
                cursor.last_time = semantic as i64;
            }
            Ok(Gallery {
                farms: entries,
                next_cursor: more.then(|| encode_cursor(&cursor)),
            })
        })
        .await?;
    Ok(([("cache-control", "no-store")], Json(gallery)).into_response())
}

impl Service {
    /// Apply a signed request and persist its receipt atomically. Replays return
    /// the original receipt even after a later mutation or an operator takedown.
    /// Sync public API for CLI/tests; HTTP handlers use `mutate_async`.
    pub fn mutate(
        &self,
        id: &str,
        operation: FarmOperation,
        request: &SignedFarmRequest,
    ) -> Result<Receipt, ServiceError> {
        self.expire_if_needed()?;
        mutate_inner(
            &mut self.db.lock().expect("farm db poisoned"),
            &self.meta,
            &self.next_expire,
            &self.config,
            id,
            operation,
            request,
        )
    }
}

/// Applies a signed request and persists its receipt atomically. Replays return
/// the original receipt even after a later mutation or an operator takedown.
/// Caller holds the DB lock; takes the meta lock only to publish, in DB -> meta
/// order, so it never contends with request-rate or stream teardown.
fn mutate_inner(
    db: &mut Db,
    meta: &Arc<Mutex<Meta>>,
    next_expire: &AtomicU64,
    config: &Config,
    id: &str,
    operation: FarmOperation,
    request: &SignedFarmRequest,
) -> Result<Receipt, ServiceError> {
    if request.farm_id.0 != id
        || request.operation != operation
        || request.sequence > i64::MAX as u64
    {
        return Err(ServiceError::new(
            StatusCode::BAD_REQUEST,
            "request route mismatch",
        ));
    }
    request
        .verify()
        .map_err(|_| ServiceError::new(StatusCode::UNAUTHORIZED, "invalid signed request"))?;
    if request.body.len() > config.max_body_bytes {
        return Err(ServiceError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "request too large",
        ));
    }
    let digest = request.request_digest();
    let last_mutation = db.last_mutation.get(id).copied();
    let tx = db.db.transaction().map_err(ServiceError::db)?;
    let prior = tx
        .query_row(
            "SELECT digest,receipt FROM receipts WHERE farm_id=? AND seq=?",
            params![id, request.sequence as i64],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(ServiceError::db)?;
    if let Some((previous, receipt)) = prior {
        if previous != digest {
            return Err(ServiceError::new(StatusCode::CONFLICT, "sequence conflict"));
        }
        return serde_json::from_str(&receipt).map_err(|_| {
            ServiceError::new(StatusCode::SERVICE_UNAVAILABLE, "service unavailable")
        });
    }
    let old=tx.query_row("SELECT public_key,seq,version,state,listed,snapshot,received_at,closed_at FROM farms WHERE farm_id=?",[id],|r|Ok((r.get::<_,String>(0)?,(r.get::<_, i64>(1)? as u64),(r.get::<_, i64>(2)? as u64),r.get::<_,String>(3)?,r.get::<_,bool>(4)?,r.get::<_,Option<String>>(5)?,(r.get::<_, i64>(6)? as u64),r.get::<_, Option<i64>>(7)?.map(|n|n as u64)))).optional().map_err(ServiceError::db)?;
    if let Some((key, sequence, _, state, _, _, _, _)) = &old {
        // The owning publisher still needs a durable deletion receipt after
        // operator takedown or retention. Verification above binds this key
        // to the farm ID, including tombstones created before enrollment.
        let tombstone_delete = state == "deleted" && operation == FarmOperation::Delete;
        if state == "deleted" && !tombstone_delete {
            return Err(ServiceError::new(StatusCode::GONE, "unavailable"));
        }
        if key != &request.public_key.to_string() && !(key.is_empty() && tombstone_delete) {
            return Err(ServiceError::new(
                StatusCode::UNAUTHORIZED,
                "invalid signed request",
            ));
        }
        if request.sequence <= *sequence {
            return Err(ServiceError::new(StatusCode::CONFLICT, "old sequence"));
        }
    } else if !config.public_enrollment
        && !tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM enrollment WHERE farm_id=?)",
                [id],
                |r| r.get::<_, bool>(0),
            )
            .map_err(ServiceError::db)?
    {
        return Err(ServiceError::new(
            StatusCode::FORBIDDEN,
            "publisher not enrolled",
        ));
    }
    let time = now_ms();
    let version = old.as_ref().map_or(1, |row| row.2 + 1);
    if matches!(operation, FarmOperation::Upload | FarmOperation::CheckIn)
        && last_mutation.map_or_else(
            || {
                old.as_ref().is_some_and(|row| {
                    time >= row.6 && time - row.6 < config.min_mutation_interval_ms
                })
            },
            |instant| instant.elapsed().as_millis() < (config.min_mutation_interval_ms as u128),
        )
    {
        return Err(ServiceError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "publisher request rate exceeded",
        ));
    }
    let (state, listed, snapshot, closed_at) = match operation {
        FarmOperation::Upload => {
            let body: FarmUploadBody = serde_json::from_str(&request.body)
                .map_err(|_| ServiceError::new(StatusCode::BAD_REQUEST, "invalid snapshot"))?;
            if matches!(
                body.snapshot.goal_state,
                FarmGoalState::Unavailable | FarmGoalState::Disputed
            ) {
                return Err(ServiceError::new(
                    StatusCode::BAD_REQUEST,
                    "snapshot unavailable",
                ));
            }
            let closed = if body.snapshot.goal_state == FarmGoalState::Ended {
                old.as_ref().and_then(|r| r.7).or(Some(time))
            } else {
                None
            };
            (
                "available",
                body.visibility == FarmVisibility::Listed,
                Some(serde_json::to_string(&body.snapshot).expect("snapshot JSON")),
                closed,
            )
        }
        FarmOperation::CheckIn => {
            let row = old
                .as_ref()
                .filter(|r| r.3 == "available")
                .ok_or_else(|| ServiceError::new(StatusCode::CONFLICT, "no available snapshot"))?;
            ("available", row.4, row.5.clone(), row.7)
        }
        FarmOperation::Suspend => ("suspended", false, None, None),
        FarmOperation::Delete => ("deleted", false, None, None),
    };
    let semantic = old.as_ref().is_none_or(|r| {
        r.3 != state
            || r.4 != listed
            || semantic_snapshot(r.5.as_deref()) != semantic_snapshot(snapshot.as_deref())
    });
    let receipt = Receipt {
        farm_id: request.farm_id.clone(),
        sequence: request.sequence,
        request_digest: digest.clone(),
        stream_version: version,
        received_at_ms: time,
    };
    tx.execute("INSERT INTO farms (farm_id,public_key,seq,version,state,listed,snapshot,received_at,semantic_at,closed_at) VALUES (?,?,?,?,?,?,?,?,?,?) ON CONFLICT(farm_id) DO UPDATE SET public_key=excluded.public_key,seq=excluded.seq,version=excluded.version,state=excluded.state,listed=excluded.listed,snapshot=excluded.snapshot,received_at=excluded.received_at,semantic_at=CASE WHEN ? THEN excluded.semantic_at ELSE farms.semantic_at END,closed_at=excluded.closed_at",params![id,request.public_key.to_string(),request.sequence as i64,version as i64,state,listed,snapshot,time as i64,time as i64,closed_at.map(|n|n as i64),semantic]).map_err(ServiceError::db)?;
    tx.execute(
        "INSERT INTO receipts (farm_id,seq,digest,receipt) VALUES (?,?,?,?)",
        params![
            id,
            request.sequence as i64,
            digest,
            serde_json::to_string(&receipt).expect("receipt JSON")
        ],
    )
    .map_err(ServiceError::db)?;
    if semantic {
        tx.execute(
            "INSERT INTO gallery_history (farm_id,semantic_at,listed) VALUES (?,?,?)",
            params![id, time as i64, listed],
        )
        .map_err(ServiceError::db)?;
    }
    tx.commit().map_err(ServiceError::db)?;
    if let Some(closed) = closed_at {
        // A farm just closed: advance the retention deadline so the next read
        // does not need to scan to discover it is due.
        let deadline = closed.saturating_add(config.retention_ms);
        next_expire.store(
            next_expire.load(Ordering::Acquire).min(deadline),
            Ordering::Release,
        );
    }
    if operation == FarmOperation::Delete {
        db.last_mutation.remove(id);
    } else {
        db.last_mutation
            .insert(id.into(), std::time::Instant::now());
    }
    publish(db, meta, id, time)?;
    Ok(receipt)
}
async fn upload(
    State(service): State<Service>,
    RoutePath(id): RoutePath<String>,
    Json(request): Json<SignedFarmRequest>,
) -> Result<Response, ServiceError> {
    mutation_async(service, id, FarmOperation::Upload, request).await
}
async fn check_in(
    State(service): State<Service>,
    RoutePath(id): RoutePath<String>,
    Json(request): Json<SignedFarmRequest>,
) -> Result<Response, ServiceError> {
    mutation_async(service, id, FarmOperation::CheckIn, request).await
}
async fn suspend(
    State(service): State<Service>,
    RoutePath(id): RoutePath<String>,
    Json(request): Json<SignedFarmRequest>,
) -> Result<Response, ServiceError> {
    mutation_async(service, id, FarmOperation::Suspend, request).await
}
async fn delete(
    State(service): State<Service>,
    RoutePath(id): RoutePath<String>,
    Json(request): Json<SignedFarmRequest>,
) -> Result<Response, ServiceError> {
    mutation_async(service, id, FarmOperation::Delete, request).await
}
async fn mutation_async(
    service: Service,
    id: String,
    operation: FarmOperation,
    request: SignedFarmRequest,
) -> Result<Response, ServiceError> {
    let receipt = service.mutate_async(id, operation, request).await?;
    Ok(([("cache-control", "no-store")], Json(receipt)).into_response())
}
fn semantic_snapshot(json: Option<&str>) -> Option<serde_json::Value> {
    fn strip(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(fields) => {
                fields.remove("observed_at_ms");
                fields.remove("last_sync_at_ms");
                for value in fields.values_mut() {
                    strip(value);
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    strip(value);
                }
            }
            _ => {}
        }
    }
    json.map(|json| {
        let mut value = serde_json::from_str(json).expect("validated stored snapshot");
        strip(&mut value);
        value
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use locust_proto::{crypto::Keypair, farm::FarmSnapshot};
    use tower::ServiceExt;

    fn config() -> Config {
        Config {
            public_enrollment: true,
            min_mutation_interval_ms: 0,
            ..Config::default()
        }
    }
    fn snapshot(key: &Keypair) -> FarmSnapshot {
        FarmSnapshot {
            version: 1,
            farm_id: FarmId::from_key(key.public()),
            title: Some("Public test".into()),
            formation: "Test formation".into(),
            goal_state: FarmGoalState::Open,
            observed_at_ms: None,
            agents: vec![],
            groups: vec![],
            stages: vec![],
            tasks: vec![],
            attempts: vec![],
            candidates: vec![],
            changes: vec![],
            omitted_changes: 0,
        }
    }
    fn upload_request(key: &Keypair, seq: u64, listed: bool) -> SignedFarmRequest {
        SignedFarmRequest::sign(
            key,
            FarmOperation::Upload,
            seq,
            serde_json::to_string(&FarmUploadBody {
                visibility: if listed {
                    FarmVisibility::Listed
                } else {
                    FarmVisibility::Link
                },
                snapshot: snapshot(key),
            })
            .unwrap(),
        )
    }
    fn control(key: &Keypair, seq: u64, op: FarmOperation) -> SignedFarmRequest {
        SignedFarmRequest::sign(key, op, seq, "{}".into())
    }
    fn apply(service: &Service, request: &SignedFarmRequest) -> Receipt {
        service
            .mutate(&request.farm_id.0, request.operation, request)
            .unwrap()
    }

    #[test]
    fn lost_ack_retry_and_restart_preserve_receipt_and_ordering() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("farm.sqlite");
        let key = Keypair::from_seed([1; 32]);
        let request = upload_request(&key, 1, true);
        let receipt = {
            let service = Service::open(&path, config()).unwrap();
            apply(&service, &request)
        };
        let service = Service::open(&path, config()).unwrap();
        assert_eq!(apply(&service, &request), receipt);
        assert_eq!(service.read(&request.farm_id.0).unwrap().stream_version, 1);
        let newer = apply(&service, &control(&key, 3, FarmOperation::CheckIn));
        assert_eq!(newer.stream_version, 2);
        assert_eq!(apply(&service, &request), receipt);
        assert!(
            service
                .mutate(
                    &request.farm_id.0,
                    FarmOperation::CheckIn,
                    &control(&key, 2, FarmOperation::CheckIn)
                )
                .is_err()
        );
        let mut conflict = upload_request(&key, 1, false);
        conflict.body = conflict.body.replace("Public test", "Changed");
        conflict = SignedFarmRequest::sign(&key, FarmOperation::Upload, 1, conflict.body);
        assert_eq!(
            service
                .mutate(&request.farm_id.0, FarmOperation::Upload, &conflict)
                .unwrap_err()
                .status,
            StatusCode::CONFLICT
        );
    }
    #[test]
    fn suspension_clears_snapshot_and_delete_is_permanent() {
        let service = Service::open(":memory:", config()).unwrap();
        let key = Keypair::from_seed([2; 32]);
        let request = upload_request(&key, 1, true);
        apply(&service, &request);
        apply(&service, &control(&key, 2, FarmOperation::Suspend));
        assert!(service.read(&request.farm_id.0).unwrap().snapshot.is_none());
        apply(&service, &upload_request(&key, 3, true));
        apply(&service, &control(&key, 4, FarmOperation::Delete));
        assert_eq!(
            service
                .mutate(
                    &request.farm_id.0,
                    FarmOperation::Upload,
                    &upload_request(&key, 5, true)
                )
                .unwrap_err()
                .status,
            StatusCode::GONE
        );
        assert_eq!(
            service.read(&request.farm_id.0).unwrap().status,
            FarmAvailability::Unavailable
        );
    }
    #[test]
    fn enrollment_and_signed_routes_are_enforced() {
        let service = Service::open(":memory:", Config::default()).unwrap();
        let key = Keypair::from_seed([3; 32]);
        let request = upload_request(&key, 1, true);
        assert_eq!(
            service
                .mutate(&request.farm_id.0, request.operation, &request)
                .unwrap_err()
                .status,
            StatusCode::FORBIDDEN
        );
        service.enroll(&request.farm_id.0).unwrap();
        apply(&service, &request);
        assert_eq!(
            service
                .mutate(&request.farm_id.0, FarmOperation::Delete, &request)
                .unwrap_err()
                .status,
            StatusCode::BAD_REQUEST
        );
        let other = Keypair::from_seed([4; 32]);
        assert!(
            service
                .mutate(
                    &request.farm_id.0,
                    FarmOperation::Delete,
                    &control(&other, 2, FarmOperation::Delete)
                )
                .is_err()
        );
    }
    #[tokio::test]
    async fn oversized_http_body_rejected_before_json_and_revocation_bypasses_rate() {
        let service = Service::open(
            ":memory:",
            Config {
                max_body_bytes: 2048,
                min_mutation_interval_ms: 60000,
                public_enrollment: true,
                ..Config::default()
            },
        )
        .unwrap();
        let key = Keypair::from_seed([5; 32]);
        let request = upload_request(&key, 1, true);
        apply(&service, &request);
        let response = service
            .router()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/farms/{}", request.farm_id))
                    .header("content-type", "application/json")
                    .body(Body::from("{".repeat(2049)))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(
            service
                .mutate(
                    &request.farm_id.0,
                    FarmOperation::CheckIn,
                    &control(&key, 2, FarmOperation::CheckIn)
                )
                .unwrap_err()
                .status,
            StatusCode::TOO_MANY_REQUESTS
        );
        apply(&service, &control(&key, 2, FarmOperation::Suspend));
        apply(&service, &control(&key, 3, FarmOperation::Delete));
    }
    #[tokio::test]
    async fn sse_starts_from_current_then_invalidates_open_viewer_and_reconnects() {
        let service = Service::open(":memory:", config()).unwrap();
        let key = Keypair::from_seed([6; 32]);
        let request = upload_request(&key, 1, true);
        apply(&service, &request);
        let response = service
            .router()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/farms/{}/events", request.farm_id))
                    .header("last-event-id", "999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let mut body = response.into_body();
        let first = body.frame().await.unwrap().unwrap().into_data().unwrap();
        assert!(
            std::str::from_utf8(&first)
                .unwrap()
                .contains("\"stream_version\":1")
        );
        apply(&service, &control(&key, 2, FarmOperation::Suspend));
        let invalidated = body.frame().await.unwrap().unwrap().into_data().unwrap();
        let text = std::str::from_utf8(&invalidated).unwrap();
        assert!(text.contains("unavailable"));
        assert!(!text.contains("Public test"));
        assert!(body.frame().await.is_none());
        apply(&service, &upload_request(&key, 3, true));
        let response = service
            .router()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/farms/{}/events", request.farm_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let first = response
            .into_body()
            .frame()
            .await
            .unwrap()
            .unwrap()
            .into_data()
            .unwrap();
        assert!(
            std::str::from_utf8(&first)
                .unwrap()
                .contains("\"stream_version\":3")
        );
    }
    #[tokio::test]
    async fn gallery_is_listed_only_and_visit_order_survives_updates() {
        let service = Service::open(":memory:", config()).unwrap();
        let a = Keypair::from_seed([7; 32]);
        let b = Keypair::from_seed([8; 32]);
        let hidden = Keypair::from_seed([9; 32]);
        apply(&service, &upload_request(&a, 1, true));
        apply(&service, &upload_request(&b, 1, true));
        apply(&service, &upload_request(&hidden, 1, false));
        let response = gallery(
            State(service.clone()),
            Query(GalleryQuery {
                page_size: Some(1),
                cursor: None,
                filter: None,
            }),
        )
        .await
        .unwrap();
        let first: serde_json::Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        let first_id = first["farms"][0]["farm_id"].as_str().unwrap();
        let remaining = if first_id == FarmId::from_key(a.public()).0 {
            &b
        } else {
            &a
        };
        let mut request = upload_request(remaining, 2, true);
        request.body = request.body.replace("Public test", "Updated public");
        request = SignedFarmRequest::sign(remaining, FarmOperation::Upload, 2, request.body);
        apply(&service, &request);
        let response = gallery(
            State(service.clone()),
            Query(GalleryQuery {
                page_size: Some(1),
                cursor: Some(first["next_cursor"].as_str().unwrap().into()),
                filter: None,
            }),
        )
        .await
        .unwrap();
        let second: serde_json::Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(second["farms"][0]["farm_id"], request.farm_id.0);
        assert!(second["next_cursor"].is_null());
        service.take_down(&request.farm_id.0).unwrap();
        assert!(service.read(&request.farm_id.0).unwrap().snapshot.is_none());
    }
    #[test]
    fn closure_retention_tombstones_and_reopen_clears_timer() {
        let service = Service::open(
            ":memory:",
            Config {
                retention_ms: 0,
                ..config()
            },
        )
        .unwrap();
        let key = Keypair::from_seed([10; 32]);
        let mut value = snapshot(&key);
        value.goal_state = FarmGoalState::Ended;
        let ended = SignedFarmRequest::sign(
            &key,
            FarmOperation::Upload,
            1,
            serde_json::to_string(&FarmUploadBody {
                visibility: FarmVisibility::Listed,
                snapshot: value,
            })
            .unwrap(),
        );
        apply(&service, &ended);
        service.expire().unwrap();
        assert_eq!(
            service.read(&ended.farm_id.0).unwrap().status,
            FarmAvailability::Unavailable
        );
        assert!(
            service
                .mutate(
                    &ended.farm_id.0,
                    FarmOperation::Upload,
                    &upload_request(&key, 2, true)
                )
                .is_err()
        );
        let service = Service::open(":memory:", config()).unwrap();
        apply(&service, &ended);
        apply(&service, &upload_request(&key, 2, true));
        let closed: Option<i64> = service
            .db
            .lock()
            .unwrap()
            .db
            .query_row("SELECT closed_at FROM farms", [], |r| r.get(0))
            .unwrap();
        assert_eq!(closed, None);
    }
}

#[cfg(test)]
mod adversarial_tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use locust_proto::{crypto::Keypair, farm::FarmSnapshot};
    use tower::ServiceExt;

    fn upload(key: &Keypair, sequence: u64, observed: Option<u64>) -> SignedFarmRequest {
        let value = FarmSnapshot {
            version: 1,
            farm_id: FarmId::from_key(key.public()),
            title: None,
            formation: "Test".into(),
            goal_state: FarmGoalState::Open,
            observed_at_ms: observed,
            agents: vec![],
            groups: vec![],
            stages: vec![],
            tasks: vec![],
            attempts: vec![],
            candidates: vec![],
            changes: vec![],
            omitted_changes: 0,
        };
        SignedFarmRequest::sign(
            key,
            FarmOperation::Upload,
            sequence,
            serde_json::to_string(&FarmUploadBody {
                visibility: FarmVisibility::Listed,
                snapshot: value,
            })
            .unwrap(),
        )
    }
    #[test]
    fn hostile_cursor_is_an_error_and_unknown_database_rejected() {
        assert!(decode_cursor("aéa").is_err());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("unknown.sqlite");
        Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE secrets (value TEXT);")
            .unwrap();
        assert!(Service::open(&path, Config::default()).is_err());
    }
    #[test]
    fn freshness_only_upload_does_not_change_gallery_order() {
        let service = Service::open(
            ":memory:",
            Config {
                public_enrollment: true,
                min_mutation_interval_ms: 0,
                ..Config::default()
            },
        )
        .unwrap();
        let key = Keypair::from_seed([11; 32]);
        for sequence in 1..=2 {
            let request = upload(&key, sequence, Some(sequence));
            service
                .mutate(&request.farm_id.0, request.operation, &request)
                .unwrap();
        }
        let count: i64 = service
            .db
            .lock()
            .unwrap()
            .db
            .query_row("SELECT count(*) FROM gallery_history", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
    #[tokio::test]
    async fn external_takedown_invalidates_stream_and_unknown_ids_do_not_leak_channels() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("farm.sqlite");
        let config = Config {
            public_enrollment: true,
            min_mutation_interval_ms: 0,
            ..Config::default()
        };
        let service = Service::open(&path, config.clone()).unwrap();
        let key = Keypair::from_seed([12; 32]);
        let request = upload(&key, 1, None);
        service
            .mutate(&request.farm_id.0, request.operation, &request)
            .unwrap();
        let response = service
            .router()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/farms/{}/events", request.farm_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let mut body = response.into_body();
        body.frame().await.unwrap().unwrap();
        Service::open(&path, config)
            .unwrap()
            .take_down(&request.farm_id.0)
            .unwrap();
        let frame = tokio::time::timeout(std::time::Duration::from_secs(3), body.frame())
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .into_data()
            .unwrap();
        assert!(std::str::from_utf8(&frame).unwrap().contains("unavailable"));
        drop(body);
        for index in 0..20 {
            let response = service
                .router()
                .oneshot(
                    Request::builder()
                        .uri(format!("/api/farms/{index}/events"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            response.into_body().collect().await.unwrap();
        }
        let store = service.meta.lock().unwrap();
        assert_eq!(store.channels.len(), 0);
        assert_eq!(store.streams, 0);
    }
}

#[cfg(test)]
mod clock_tests {
    use super::*;
    use locust_proto::{crypto::Keypair, farm::FarmSnapshot};
    #[test]
    fn backwards_wall_clock_does_not_freeze_mutations_after_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("farm.sqlite");
        let key = Keypair::from_seed([13; 32]);
        let snapshot = FarmSnapshot {
            version: 1,
            farm_id: FarmId::from_key(key.public()),
            title: None,
            formation: "Clock".into(),
            goal_state: FarmGoalState::Open,
            observed_at_ms: None,
            agents: vec![],
            groups: vec![],
            stages: vec![],
            tasks: vec![],
            attempts: vec![],
            candidates: vec![],
            changes: vec![],
            omitted_changes: 0,
        };
        let request = SignedFarmRequest::sign(
            &key,
            FarmOperation::Upload,
            1,
            serde_json::to_string(&FarmUploadBody {
                visibility: FarmVisibility::Link,
                snapshot,
            })
            .unwrap(),
        );
        {
            let service = Service::open(
                &path,
                Config {
                    public_enrollment: true,
                    ..Config::default()
                },
            )
            .unwrap();
            service
                .mutate(&request.farm_id.0, request.operation, &request)
                .unwrap();
            service
                .db
                .lock()
                .unwrap()
                .db
                .execute("UPDATE farms SET received_at=?", [i64::MAX])
                .unwrap();
        }
        let service = Service::open(&path, Config::default()).unwrap();
        let request = SignedFarmRequest::sign(&key, FarmOperation::CheckIn, 2, "{}".into());
        assert!(
            service
                .mutate(&request.farm_id.0, request.operation, &request)
                .is_ok()
        );
        let next = SignedFarmRequest::sign(&key, FarmOperation::CheckIn, 3, "{}".into());
        assert_eq!(
            service
                .mutate(&next.farm_id.0, next.operation, &next)
                .unwrap_err()
                .status,
            StatusCode::TOO_MANY_REQUESTS
        );
    }
}

#[cfg(test)]
mod tombstone_ack_tests {
    use super::*;
    use locust_proto::{crypto::Keypair, farm::FarmSnapshot};
    fn upload(key: &Keypair, sequence: u64, ended: bool) -> SignedFarmRequest {
        let snapshot = FarmSnapshot {
            version: 1,
            farm_id: FarmId::from_key(key.public()),
            title: None,
            formation: "Tombstone".into(),
            goal_state: if ended {
                FarmGoalState::Ended
            } else {
                FarmGoalState::Open
            },
            observed_at_ms: None,
            agents: vec![],
            groups: vec![],
            stages: vec![],
            tasks: vec![],
            attempts: vec![],
            candidates: vec![],
            changes: vec![],
            omitted_changes: 0,
        };
        SignedFarmRequest::sign(
            key,
            FarmOperation::Upload,
            sequence,
            serde_json::to_string(&FarmUploadBody {
                visibility: FarmVisibility::Listed,
                snapshot,
            })
            .unwrap(),
        )
    }
    #[test]
    fn deleted_farms_acknowledge_new_owner_delete_without_resurrection() {
        for operator_takedown in [true, false] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("farm.sqlite");
            let key = Keypair::from_seed([14; 32]);
            let id = FarmId::from_key(key.public());
            let config = Config {
                public_enrollment: true,
                min_mutation_interval_ms: 0,
                retention_ms: if operator_takedown { 86400000 } else { 0 },
                ..Config::default()
            };
            let receipt = {
                let service = Service::open(&path, config.clone()).unwrap();
                let request = upload(&key, 1, !operator_takedown);
                service.mutate(&id.0, request.operation, &request).unwrap();
                if operator_takedown {
                    service.take_down(&id.0).unwrap();
                } else {
                    service.expire().unwrap();
                }
                let deletion = SignedFarmRequest::sign(&key, FarmOperation::Delete, 2, "{}".into());
                let receipt = service
                    .mutate(&id.0, FarmOperation::Delete, &deletion)
                    .unwrap();
                assert_eq!(receipt.stream_version, 3);
                assert_eq!(
                    service.read(&id.0).unwrap().status,
                    FarmAvailability::Unavailable
                );
                receipt
            };
            let service = Service::open(&path, config).unwrap();
            let deletion = SignedFarmRequest::sign(&key, FarmOperation::Delete, 2, "{}".into());
            assert_eq!(
                service
                    .mutate(&id.0, FarmOperation::Delete, &deletion)
                    .unwrap(),
                receipt
            );
            for operation in [FarmOperation::CheckIn, FarmOperation::Suspend] {
                let request = SignedFarmRequest::sign(&key, operation, 3, "{}".into());
                assert_eq!(
                    service
                        .mutate(&id.0, operation, &request)
                        .unwrap_err()
                        .status,
                    StatusCode::GONE
                );
            }
            let request = upload(&key, 3, false);
            assert_eq!(
                service
                    .mutate(&id.0, FarmOperation::Upload, &request)
                    .unwrap_err()
                    .status,
                StatusCode::GONE
            );
        }
    }
    #[test]
    fn unbound_operator_tombstone_only_acknowledges_its_derived_owner() {
        let service = Service::open(":memory:", Config::default()).unwrap();
        let owner = Keypair::from_seed([15; 32]);
        let id = FarmId::from_key(owner.public());
        service.take_down(&id.0).unwrap();
        let other = Keypair::from_seed([16; 32]);
        let wrong = SignedFarmRequest::sign(&other, FarmOperation::Delete, 1, "{}".into());
        assert!(
            service
                .mutate(&id.0, FarmOperation::Delete, &wrong)
                .is_err()
        );
        // An unenrolled, unknown ID still cannot allocate a new farm record.
        assert_eq!(
            service
                .mutate(&wrong.farm_id.0, FarmOperation::Delete, &wrong)
                .unwrap_err()
                .status,
            StatusCode::FORBIDDEN
        );
        let deletion = SignedFarmRequest::sign(&owner, FarmOperation::Delete, 1, "{}".into());
        let receipt = service
            .mutate(&id.0, FarmOperation::Delete, &deletion)
            .unwrap();
        assert_eq!(
            service
                .mutate(&id.0, FarmOperation::Delete, &deletion)
                .unwrap(),
            receipt
        );
        let stored: String = service
            .db
            .lock()
            .unwrap()
            .db
            .query_row(
                "SELECT public_key FROM farms WHERE farm_id=?",
                [&id.0],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored, owner.public().to_string());
        let count: i64 = service
            .db
            .lock()
            .unwrap()
            .db
            .query_row("SELECT count(*) FROM farms", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}

#[cfg(test)]
mod shutdown_tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use locust_proto::{crypto::Keypair, farm::FarmSnapshot};
    use tower::ServiceExt;
    #[tokio::test]
    async fn shutdown_closes_active_stream_without_invalidating_snapshot() {
        let service = Service::open(
            ":memory:",
            Config {
                public_enrollment: true,
                ..Config::default()
            },
        )
        .unwrap();
        let key = Keypair::from_seed([17; 32]);
        let id = FarmId::from_key(key.public());
        let snapshot = FarmSnapshot {
            version: 1,
            farm_id: id.clone(),
            title: None,
            formation: "Shutdown".into(),
            goal_state: FarmGoalState::Open,
            observed_at_ms: None,
            agents: vec![],
            groups: vec![],
            stages: vec![],
            tasks: vec![],
            attempts: vec![],
            candidates: vec![],
            changes: vec![],
            omitted_changes: 0,
        };
        let request = SignedFarmRequest::sign(
            &key,
            FarmOperation::Upload,
            1,
            serde_json::to_string(&FarmUploadBody {
                visibility: FarmVisibility::Listed,
                snapshot,
            })
            .unwrap(),
        );
        service.mutate(&id.0, request.operation, &request).unwrap();
        let response = service
            .router()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/farms/{id}/events"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let mut body = response.into_body();
        let first = body.frame().await.unwrap().unwrap().into_data().unwrap();
        assert!(std::str::from_utf8(&first).unwrap().contains("available"));
        service.shutdown();
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), body.frame())
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            service.read(&id.0).unwrap().status,
            FarmAvailability::Available
        );
        let store = service.meta.lock().unwrap();
        assert_eq!(store.streams, 0);
        assert_eq!(store.channels.len(), 0);
    }
}

#[cfg(test)]
mod expire_index_tests {
    use super::*;
    use locust_proto::{crypto::Keypair, farm::FarmSnapshot};

    fn upload(key: &Keypair, sequence: u64, ended: bool) -> SignedFarmRequest {
        let snapshot = FarmSnapshot {
            version: 1,
            farm_id: FarmId::from_key(key.public()),
            title: None,
            formation: "Expire".into(),
            goal_state: if ended {
                FarmGoalState::Ended
            } else {
                FarmGoalState::Open
            },
            observed_at_ms: None,
            agents: vec![],
            groups: vec![],
            stages: vec![],
            tasks: vec![],
            attempts: vec![],
            candidates: vec![],
            changes: vec![],
            omitted_changes: 0,
        };
        SignedFarmRequest::sign(
            key,
            FarmOperation::Upload,
            sequence,
            serde_json::to_string(&FarmUploadBody {
                visibility: FarmVisibility::Listed,
                snapshot,
            })
            .unwrap(),
        )
    }

    fn explain(db: &Connection, sql: &str, params: &[&dyn rusqlite::ToSql]) -> Vec<String> {
        let mut statement = db.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
        statement
            .query_map(params, |row| row.get::<_, String>(3))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    }

    #[test]
    fn expire_query_uses_the_partial_index_not_a_full_scan() {
        let service = Service::open(":memory:", config()).unwrap();
        let store = service.db.lock().unwrap();
        let plan = explain(
            &store.db,
            "SELECT farm_id FROM farms WHERE state='available' AND closed_at IS NOT NULL AND closed_at<=?",
            &[&0i64 as &dyn rusqlite::ToSql],
        );
        assert!(
            plan.iter().any(|line| line.contains("expire")),
            "expire scan must use the partial `expire` index: {plan:?}"
        );
        assert!(
            !plan.iter().any(|line| line.to_lowercase().contains("scan")
                && line.contains("farms")
                && !line.contains("expire")),
            "expire scan must not full-scan farms: {plan:?}"
        );
    }

    #[test]
    fn gallery_frozen_lookup_uses_correlated_index_search_not_a_history_scan() {
        let service = Service::open(":memory:", config()).unwrap();
        let store = service.db.lock().unwrap();
        // Inspect the actual production query (GALLERY_QUERY), not a synthetic
        // copy, so the test tracks the real SQL.
        let plan = explain(
            &store.db,
            GALLERY_QUERY,
            &[
                &0i64 as &dyn rusqlite::ToSql,
                &i64::MAX,
                &i64::MAX,
                &"",
                &"all",
                &"all",
                &"all",
                &0i64,
                &"all",
                &0i64,
                &24i64,
            ],
        );
        // The correlated predecessor lookup must SEARCH gallery_history via the
        // farm index, never SCAN the ever-growing history table.
        assert!(
            plan.iter()
                .any(|line| line.contains("gallery_history_farm")),
            "frozen lookup must use gallery_history_farm: {plan:?}"
        );
        assert!(
            !plan
                .iter()
                .any(|line| line.contains("SCAN") && line.contains("gallery_history")),
            "gallery must not SCAN gallery_history: {plan:?}"
        );
    }

    #[test]
    fn expire_if_needed_skips_until_the_deadline_and_recomputes_it() {
        let service = Service::open(
            ":memory:",
            Config {
                retention_ms: 1000,
                ..config()
            },
        )
        .unwrap();
        let key = Keypair::from_seed([21; 32]);
        let request = upload(&key, 1, true);
        service
            .mutate(&request.farm_id.0, request.operation, &request)
            .unwrap();
        // The farm is closed but retention has not elapsed: it stays available.
        assert_eq!(
            service.read(&request.farm_id.0).unwrap().status,
            FarmAvailability::Available
        );
        // The deadline was advanced when the farm closed; reads skip the scan.
        let deadline = service.next_expire.load(Ordering::Relaxed);
        assert!(deadline < u64::MAX);
        for _ in 0..50 {
            assert_eq!(
                service.read(&request.farm_id.0).unwrap().status,
                FarmAvailability::Available
            );
        }
        // The deadline is unchanged: no re-scan recomputed it.
        assert_eq!(service.next_expire.load(Ordering::Relaxed), deadline);
    }

    #[test]
    fn closed_farm_past_retention_is_immediately_unavailable_on_read() {
        let service = Service::open(
            ":memory:",
            Config {
                retention_ms: 0,
                ..config()
            },
        )
        .unwrap();
        let key = Keypair::from_seed([22; 32]);
        let request = upload(&key, 1, true);
        service
            .mutate(&request.farm_id.0, request.operation, &request)
            .unwrap();
        // retention_ms=0 means the close deadline is already due; a read must not
        // return the stale available snapshot.
        assert_eq!(
            service.read(&request.farm_id.0).unwrap().status,
            FarmAvailability::Unavailable
        );
        // After expiring, the deadline resets to "no closed farms".
        assert_eq!(service.next_expire.load(Ordering::Relaxed), u64::MAX);
    }

    #[tokio::test]
    async fn many_viewers_do_not_each_trigger_an_expire_scan() {
        let service = Service::open(
            ":memory:",
            Config {
                retention_ms: u64::MAX,
                ..config()
            },
        )
        .unwrap();
        let key = Keypair::from_seed([23; 32]);
        let request = upload(&key, 1, false);
        service
            .mutate(&request.farm_id.0, request.operation, &request)
            .unwrap();
        // With retention far in the future, maintain() must not offload any scan.
        for _ in 0..40 {
            service.maintain().await.unwrap();
        }
        // No farm is due, so the deadline stays at "no closed farms" (the open farm
        // never set closed_at) and reads remain available without expiration work.
        assert_eq!(service.next_expire.load(Ordering::Relaxed), u64::MAX);
        assert_eq!(
            service.read(&request.farm_id.0).unwrap().status,
            FarmAvailability::Available
        );
    }

    fn config() -> Config {
        Config {
            public_enrollment: true,
            min_mutation_interval_ms: 0,
            ..Config::default()
        }
    }
}

#[cfg(test)]
mod gallery_pagination_tests {
    use super::*;
    use http_body_util::BodyExt;
    use locust_proto::{crypto::Keypair, farm::FarmSnapshot};

    fn upload(key: &Keypair, sequence: u64) -> SignedFarmRequest {
        let snapshot = FarmSnapshot {
            version: 1,
            farm_id: FarmId::from_key(key.public()),
            title: None,
            formation: "Gallery".into(),
            goal_state: FarmGoalState::Open,
            observed_at_ms: None,
            agents: vec![],
            groups: vec![],
            stages: vec![],
            tasks: vec![],
            attempts: vec![],
            candidates: vec![],
            changes: vec![],
            omitted_changes: 0,
        };
        SignedFarmRequest::sign(
            key,
            FarmOperation::Upload,
            sequence,
            serde_json::to_string(&FarmUploadBody {
                visibility: FarmVisibility::Listed,
                snapshot,
            })
            .unwrap(),
        )
    }

    async fn page(service: &Service, cursor: Option<String>) -> serde_json::Value {
        let response = gallery(
            State(service.clone()),
            Query(GalleryQuery {
                page_size: Some(2),
                cursor,
                filter: None,
            }),
        )
        .await
        .unwrap();
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
    }

    #[tokio::test]
    async fn pagination_stays_frozen_across_updates_with_many_farms() {
        let service = Service::open(":memory:", config()).unwrap();
        let mut ids = Vec::new();
        for seed in 1..=6u8 {
            let key = Keypair::from_seed([seed; 32]);
            let request = upload(&key, 1);
            service
                .mutate(&request.farm_id.0, request.operation, &request)
                .unwrap();
            ids.push(request.farm_id.0);
        }
        let first = page(&service, None).await;
        let first_ids: Vec<String> = first["farms"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["farm_id"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(first_ids.len(), 2);
        let cursor = first["next_cursor"].as_str().unwrap().to_string();
        // Update a farm on the first page and a farm not yet seen while paginating.
        let on_page = &first_ids[0];
        let later = ids.iter().find(|id| !first_ids.contains(id)).unwrap();
        let on_key =
            Keypair::from_seed([ids.iter().position(|x| x == on_page).unwrap() as u8 + 1; 32]);
        let later_key =
            Keypair::from_seed([ids.iter().position(|x| x == later).unwrap() as u8 + 1; 32]);
        let mut updated = upload(&on_key, 2);
        updated.body = updated.body.replace("Gallery", "Updated-on-page");
        updated = SignedFarmRequest::sign(&on_key, FarmOperation::Upload, 2, updated.body);
        let on_id = FarmId::from_key(on_key.public()).0;
        service.mutate(&on_id, updated.operation, &updated).unwrap();
        let mut fresh = upload(&later_key, 2);
        fresh.body = fresh.body.replace("Gallery", "Updated-later");
        fresh = SignedFarmRequest::sign(&later_key, FarmOperation::Upload, 2, fresh.body);
        let later_id = FarmId::from_key(later_key.public()).0;
        service.mutate(&later_id, fresh.operation, &fresh).unwrap();
        // The second page must not repeat the first page or jump to the updated farm.
        let second = page(&service, Some(cursor)).await;
        let second_ids: Vec<String> = second["farms"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["farm_id"].as_str().unwrap().to_string())
            .collect();
        assert!(second_ids.iter().all(|id| !first_ids.contains(id)));
        assert!(second["next_cursor"].as_str().is_some());
        // Walk the remaining pages; every farm appears exactly once across the walk.
        let mut seen = first_ids.clone();
        let mut next = second["next_cursor"].as_str().map(String::from);
        seen.extend(second_ids);
        while let Some(cursor) = next {
            let p = page(&service, Some(cursor)).await;
            let page_ids: Vec<String> = p["farms"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| f["farm_id"].as_str().unwrap().to_string())
                .collect();
            assert!(page_ids.iter().all(|id| !seen.contains(id)));
            seen.extend(page_ids);
            next = p["next_cursor"].as_str().map(String::from);
        }
        assert_eq!(seen.len(), ids.len());
    }

    fn config() -> Config {
        Config {
            public_enrollment: true,
            min_mutation_interval_ms: 0,
            ..Config::default()
        }
    }
}

#[cfg(test)]
mod gallery_scaling_tests {
    use super::*;
    use locust_proto::{crypto::Keypair, farm::FarmSnapshot};
    use rusqlite::StatementStatus;

    fn upload(key: &Keypair, sequence: u64, title: &str) -> SignedFarmRequest {
        let snapshot = FarmSnapshot {
            version: 1,
            farm_id: FarmId::from_key(key.public()),
            title: Some(title.into()),
            formation: "Gallery".into(),
            goal_state: FarmGoalState::Open,
            observed_at_ms: None,
            agents: vec![],
            groups: vec![],
            stages: vec![],
            tasks: vec![],
            attempts: vec![],
            candidates: vec![],
            changes: vec![],
            omitted_changes: 0,
        };
        SignedFarmRequest::sign(
            key,
            FarmOperation::Upload,
            sequence,
            serde_json::to_string(&FarmUploadBody {
                visibility: FarmVisibility::Listed,
                snapshot,
            })
            .unwrap(),
        )
    }

    /// Runs the production gallery query once and returns its VM-step count.
    /// VM steps are a deterministic measure of work (opcode executions), not a
    /// timing threshold.
    fn vm_steps(service: &Service) -> i32 {
        let store = service.db.lock().unwrap();
        let mut statement = store.db.prepare(GALLERY_QUERY).unwrap();
        let rows: Vec<(String, i64)> = statement
            .query_map(
                params![
                    i64::MAX,
                    i64::MAX,
                    i64::MAX,
                    "",
                    "all",
                    "all",
                    "all",
                    0i64,
                    "all",
                    0i64,
                    100i64
                ],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
            )
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(rows.len(), 8, "measure populated gallery pages");
        statement.get_status(StatementStatus::VmStep)
    }

    #[test]
    fn gallery_work_stays_bounded_as_history_grows_manyfold() {
        let service = Service::open(":memory:", config()).unwrap();
        let farms: Vec<Keypair> = (1..=8u8).map(|s| Keypair::from_seed([s; 32])).collect();
        // Seed each farm with one listed upload.
        for key in &farms {
            let request = upload(key, 1, "first");
            service
                .mutate(&request.farm_id.0, request.operation, &request)
                .unwrap();
        }
        let steps_small = vm_steps(&service);
        // Grow history ~50x with semantic-only uploads (new title each time) so
        // the frozen result is still the latest entry per farm and the farm count
        // is unchanged.
        for (i, key) in farms.iter().enumerate() {
            for n in 2..=50 {
                let request = upload(key, n as u64, &format!("grow-{i}-{n}"));
                service
                    .mutate(&request.farm_id.0, request.operation, &request)
                    .unwrap();
            }
        }
        let steps_grown = vm_steps(&service);
        // The gallery still returns the same farms; the plan is a correlated index
        // SEARCH per farm, so work grows with farm count (and log history), not with
        // history size. A full history scan would grow ~50x; the indexed lookup must
        // stay well under 10x.
        assert!(
            steps_grown < steps_small * 10,
            "gallery VM steps grew {steps_grown} from {steps_small} after 50x history; \
             expected bounded (indexed) growth, not a scan",
        );
    }

    fn config() -> Config {
        Config {
            public_enrollment: true,
            min_mutation_interval_ms: 0,
            ..Config::default()
        }
    }
}

#[cfg(test)]
mod async_executor_tests {
    use super::*;

    fn config() -> Config {
        Config {
            public_enrollment: true,
            min_mutation_interval_ms: 0,
            ..Config::default()
        }
    }

    #[tokio::test]
    async fn long_db_work_does_not_block_async_tasks() {
        let service = Service::open(":memory:", config()).unwrap();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let release_rx = std::sync::Mutex::new(release_rx);
        // Hold the single DB permit with a blocking operation parked on a
        // blocking thread, not on the current_thread async executor.
        let db_task = {
            let service = service.clone();
            tokio::spawn(async move {
                service
                    .with_db(move |_db, _meta| {
                        release_rx.lock().unwrap().recv().ok();
                        Ok(())
                    })
                    .await
                    .unwrap()
            })
        };
        // An async task that does not touch the DB must still progress while the
        // DB work is parked. Deterministic: no timing threshold.
        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            done_tx.send(()).unwrap();
        });
        done_rx.await.unwrap();
        release_tx.send(()).unwrap();
        db_task.await.unwrap();
    }

    #[tokio::test]
    async fn request_rate_bookkeeping_does_not_block_behind_db_work() {
        let service = Service::open(":memory:", config()).unwrap();
        let (held_tx, held_rx) = tokio::sync::oneshot::channel::<()>();
        let held_tx = std::sync::Mutex::new(Some(held_tx));
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let release_rx = std::sync::Mutex::new(release_rx);
        let db_task = {
            let service = service.clone();
            tokio::spawn(async move {
                service
                    .with_db(move |_db, _meta| {
                        held_tx.lock().unwrap().take().unwrap().send(()).ok();
                        release_rx.lock().unwrap().recv().ok();
                        Ok(())
                    })
                    .await
                    .unwrap()
            })
        };
        // Wait until the DB permit is held on a blocking thread (deterministic).
        held_rx.await.unwrap();
        // Request-rate bookkeeping takes only the lightweight meta lock and must
        // not block behind the DB operation.
        {
            let mut meta = service.meta.lock().unwrap();
            meta.request_count += 1;
        }
        release_tx.send(()).unwrap();
        db_task.await.unwrap();
    }

    #[tokio::test]
    async fn stream_teardown_does_not_block_behind_db_work() {
        let service = Service::open(":memory:", config()).unwrap();
        let (held_tx, held_rx) = tokio::sync::oneshot::channel::<()>();
        let held_tx = std::sync::Mutex::new(Some(held_tx));
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let release_rx = std::sync::Mutex::new(release_rx);
        let db_task = {
            let service = service.clone();
            tokio::spawn(async move {
                service
                    .with_db(move |_db, _meta| {
                        held_tx.lock().unwrap().take().unwrap().send(()).ok();
                        release_rx.lock().unwrap().recv().ok();
                        Ok(())
                    })
                    .await
                    .unwrap()
            })
        };
        held_rx.await.unwrap();
        // Reserve and release a stream slot (meta lock only) while the DB permit
        // is held on a blocking thread; teardown must not block behind it.
        let guard = {
            let mut meta = service.meta.lock().unwrap();
            meta.streams += 1;
            StreamGuard {
                meta: service.meta.clone(),
                id: "test".into(),
                receiver: None,
            }
        };
        drop(guard);
        assert_eq!(service.meta.lock().unwrap().streams, 0);
        release_tx.send(()).unwrap();
        db_task.await.unwrap();
    }

    #[tokio::test]
    async fn cancelled_request_keeps_database_capacity_until_work_finishes() {
        let service = Service::open(":memory:", config()).unwrap();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let request = {
            let service = service.clone();
            tokio::spawn(async move {
                service
                    .with_db(move |_, _| {
                        started_tx.send(()).unwrap();
                        release_rx.recv().unwrap();
                        Ok(())
                    })
                    .await
            })
        };
        started_rx.await.unwrap();
        request.abort();
        assert!(request.await.unwrap_err().is_cancelled());
        let capacity = service.db_permit.available_permits();
        release_tx.send(()).unwrap();
        service.with_db(|_, _| Ok(())).await.unwrap();
        assert_eq!(capacity, 0);
    }

    #[test]
    fn unsubscribed_stream_cleanup_preserves_other_viewers() {
        let service = Service::open(":memory:", config()).unwrap();
        let (sender, receiver) = broadcast::channel(1);
        {
            let mut meta = service.meta.lock().unwrap();
            meta.streams = 2;
            meta.channels.insert("test".into(), sender);
        }
        let cancelled = StreamGuard {
            meta: service.meta.clone(),
            id: "test".into(),
            receiver: None,
        };
        drop(cancelled);
        assert!(service.meta.lock().unwrap().channels.contains_key("test"));
        let subscribed = StreamGuard {
            meta: service.meta.clone(),
            id: "test".into(),
            receiver: Some(receiver),
        };
        drop(subscribed);
        let meta = service.meta.lock().unwrap();
        assert_eq!(meta.streams, 0);
        assert!(meta.channels.is_empty());
    }
}
