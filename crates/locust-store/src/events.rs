//! The event log: appending inside a commit, and reading events back.
//!
//! Positions are assigned here. Each goal's `last_position` is read once per
//! commit, advanced for each event actually inserted, and written back once,
//! all in the commit's transaction; a held event or a repeat within the
//! commit hits the unique `id` and takes no position. The unique
//! `(goal, position)` index turns any gap-or-repeat bug into a failed
//! commit instead of a corrupted log.

use locust_proto::event::{AuthorPoint, Event};
use locust_proto::id::{EventId, GoalId, PublicKey, Signature};
use locust_proto::store::StoreError;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::columns::{blob, count, fixed, int, limit};
use crate::error::{corrupted, sql};

const INSERT: &str = "INSERT INTO events (id, goal, position, author, seq, signature, header) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT (id) DO NOTHING";
const LAST_POSITION: &str = "SELECT last_position FROM goals WHERE goal = ?1";
const SET_LAST_POSITION: &str = "INSERT INTO goals (goal, last_position) VALUES (?1, ?2) \
     ON CONFLICT (goal) DO UPDATE SET last_position = excluded.last_position";
const BY_ID: &str = "SELECT id, signature, header FROM events WHERE id = ?1";
const HAS: &str = "SELECT 1 FROM events WHERE id = ?1";
const LOG: &str = "SELECT position, id, signature, header FROM events \
     WHERE goal = ?1 AND position > ?2 ORDER BY position LIMIT ?3";
const AUTHOR_LOG: &str = "SELECT seq, id, signature, header FROM events \
     WHERE goal = ?1 AND author = ?2 AND (seq, id) > (?3, ?4) ORDER BY seq, id LIMIT ?5";
const GOALS: &str = "SELECT goal FROM goals ORDER BY goal";

/// A goal's newest position before and during one commit.
struct Tip {
    goal: GoalId,
    stored: u64,
    last: u64,
}

/// Appends the events not already held, in order, inside the caller's
/// transaction.
pub(crate) fn append(tx: &Connection, events: &[Event]) -> Result<(), StoreError> {
    if events.is_empty() {
        return Ok(());
    }
    let mut insert = tx.prepare_cached(INSERT).map_err(sql)?;
    let mut tips: Vec<Tip> = Vec::new();
    for event in events {
        let header = event.header();
        let index = match tips.iter().position(|tip| tip.goal == header.goal) {
            Some(index) => index,
            None => {
                let stored = last_position(tx, &header.goal)?;
                tips.push(Tip {
                    goal: header.goal,
                    stored,
                    last: stored,
                });
                tips.len() - 1
            }
        };
        let tip = &mut tips[index];
        let position = tip.last + 1;
        let inserted = insert
            .execute(params![
                event.id().as_bytes(),
                header.goal.as_bytes(),
                int(position)?,
                header.author.as_bytes(),
                int(header.seq)?,
                event.signature().as_bytes(),
                event.header_bytes(),
            ])
            .map_err(sql)?;
        if inserted == 1 {
            tip.last = position;
        }
    }
    let mut record = tx.prepare_cached(SET_LAST_POSITION).map_err(sql)?;
    for tip in tips.iter().filter(|tip| tip.last != tip.stored) {
        record
            .execute(params![tip.goal.as_bytes(), int(tip.last)?])
            .map_err(sql)?;
    }
    Ok(())
}

fn last_position(conn: &Connection, goal: &GoalId) -> Result<u64, StoreError> {
    let mut statement = conn.prepare_cached(LAST_POSITION).map_err(sql)?;
    let last = statement
        .query_row([goal.as_bytes()], |row| row.get::<_, i64>(0))
        .optional()
        .map_err(sql)?;
    match last {
        None => Ok(0),
        Some(last) => u64::try_from(last)
            .map_err(|_| corrupted(format_args!("goal {goal} has last position {last}"))),
    }
}

/// Rebuilds the event in columns `first..first + 3` (id, signature, header).
/// A row whose bytes no longer hash to its identifier, or no longer decode,
/// is reported, never returned as some other event.
fn event_at(row: &Row<'_>, first: usize) -> Result<Event, StoreError> {
    let id = EventId(fixed(row, first)?);
    let signature = Signature(fixed(row, first + 1)?);
    let header = blob(row, first + 2)?.to_vec();
    Event::from_stored(id, header, signature)
        .map_err(|error| corrupted(format_args!("stored event {id}: {error}")))
}

pub(crate) fn event(conn: &Connection, id: &EventId) -> Result<Option<Event>, StoreError> {
    let mut statement = conn.prepare_cached(BY_ID).map_err(sql)?;
    let mut rows = statement.query([id.as_bytes()]).map_err(sql)?;
    match rows.next().map_err(sql)? {
        Some(row) => event_at(row, 0).map(Some),
        None => Ok(None),
    }
}

pub(crate) fn has_event(conn: &Connection, id: &EventId) -> Result<bool, StoreError> {
    let mut statement = conn.prepare_cached(HAS).map_err(sql)?;
    statement.exists([id.as_bytes()]).map_err(sql)
}

pub(crate) fn log(
    conn: &Connection,
    goal: &GoalId,
    after: u64,
    max: usize,
) -> Result<Vec<(u64, Event)>, StoreError> {
    // No position exceeds i64::MAX, so nothing follows a larger cursor.
    let Ok(after) = i64::try_from(after) else {
        return Ok(Vec::new());
    };
    let mut statement = conn.prepare_cached(LOG).map_err(sql)?;
    let mut rows = statement
        .query(params![goal.as_bytes(), after, limit(max)])
        .map_err(sql)?;
    let mut events = Vec::new();
    while let Some(row) = rows.next().map_err(sql)? {
        let position = count(row, 0)?;
        let event = event_at(row, 1)?;
        if event.header().goal != *goal {
            return Err(corrupted(format_args!(
                "position {position} of goal {goal} holds an event of another goal"
            )));
        }
        events.push((position, event));
    }
    Ok(events)
}

pub(crate) fn author_log(
    conn: &Connection,
    goal: &GoalId,
    author: &PublicKey,
    after: Option<AuthorPoint>,
    max: usize,
) -> Result<Vec<Event>, StoreError> {
    // (-1, empty) precedes every stored point; no stored seq exceeds i64::MAX.
    let (seq, id): (i64, &[u8]) = match &after {
        None => (-1, &[]),
        Some(point) => match i64::try_from(point.seq) {
            Ok(seq) => (seq, point.id.as_bytes()),
            Err(_) => return Ok(Vec::new()),
        },
    };
    let mut statement = conn.prepare_cached(AUTHOR_LOG).map_err(sql)?;
    let mut rows = statement
        .query(params![
            goal.as_bytes(),
            author.as_bytes(),
            seq,
            id,
            limit(max)
        ])
        .map_err(sql)?;
    let mut events = Vec::new();
    while let Some(row) = rows.next().map_err(sql)? {
        let seq = count(row, 0)?;
        let event = event_at(row, 1)?;
        let header = event.header();
        if header.goal != *goal || header.author != *author || header.seq != seq {
            return Err(corrupted(format_args!(
                "event {} is indexed under another author position",
                event.id()
            )));
        }
        events.push(event);
    }
    Ok(events)
}

pub(crate) fn goals(conn: &Connection) -> Result<Vec<GoalId>, StoreError> {
    let mut statement = conn.prepare_cached(GOALS).map_err(sql)?;
    let mut rows = statement.query([]).map_err(sql)?;
    let mut goals = Vec::new();
    while let Some(row) = rows.next().map_err(sql)? {
        goals.push(GoalId(fixed(row, 0)?));
    }
    Ok(goals)
}
