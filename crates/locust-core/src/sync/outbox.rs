//! Lazy response cursors. At most one frame is awaiting transport capacity.
use std::collections::VecDeque;

use locust_proto::event::AuthorPoint;
use locust_proto::id::{BlobHash, EventId, PublicKey};
use locust_proto::limits::{
    BLOB_CHUNK_BYTES, MAX_EVENTS_PER_BATCH, MAX_INVENTORY_POINTS, MAX_PEER_FRAME_BYTES,
};
use locust_proto::sync::{Frontier, SyncMessage};

use super::Replica;
use super::batch::encoded_len;

#[derive(Debug)]
pub(super) enum Work {
    Frame(SyncMessage),
    Inventory {
        author: PublicKey,
        after: Option<AuthorPoint>,
    },
    /// Retains the request and advances one author at a time. Expanding a
    /// frontier must not enqueue a materialized answer per author.
    Frontier {
        theirs: Frontier,
        mine: Frontier,
        next: usize,
    },
    Events {
        author: PublicKey,
        after: Option<AuthorPoint>,
        end: AuthorPoint,
        exclude: Vec<AuthorPoint>,
    },
    Requested(VecDeque<EventId>),
    Blob {
        hash: BlobHash,
        offset: u64,
        total: u64,
    },
}

#[derive(Debug, Default)]
pub(super) struct Outbox {
    work: VecDeque<Work>,
    waiting: bool,
}

impl Outbox {
    pub fn push(&mut self, frame: SyncMessage) {
        self.work.push_back(Work::Frame(frame));
    }
    pub fn task(&mut self, work: Work) {
        self.work.push_back(work);
    }
    pub fn pending(&self) -> bool {
        self.waiting || !self.work.is_empty()
    }
    /// Intake resumes only after the complete answer has been written.
    pub fn readable(&self) -> bool {
        !self.pending()
    }
    pub fn writable(&mut self) {
        self.waiting = false;
    }
    pub fn clear(&mut self) {
        self.work.clear();
    }
    pub fn events(
        &mut self,
        replica: &dyn Replica,
        author: PublicKey,
        after: Option<AuthorPoint>,
        end: Option<AuthorPoint>,
        exclude: Vec<AuthorPoint>,
    ) {
        if let Some(end) = end.or_else(|| replica.points(&author).last().copied())
            && after.is_none_or(|after| after < end)
        {
            self.task(Work::Events {
                author,
                after,
                end,
                exclude,
            });
        }
    }
    pub fn pump_frame(&mut self, out: &mut Vec<SyncMessage>) {
        if !self.waiting
            && matches!(self.work.front(), Some(Work::Frame(_)))
            && let Some(Work::Frame(frame)) = self.work.pop_front()
        {
            out.push(frame);
            self.waiting = true;
        }
    }
    pub fn pump(&mut self, replica: &dyn Replica, out: &mut Vec<SyncMessage>) {
        if self.waiting {
            return;
        }
        while let Some(work) = self.work.front_mut() {
            if let Work::Frontier { theirs, mine, next } = work {
                if let Some(entry) = mine.authors.get(*next) {
                    let author = entry.author;
                    let ours = *entry;
                    let theirs = theirs.get(&author);
                    *next += 1;
                    let points = replica.points(&author);
                    let following = if replica.extends(&theirs) {
                        points
                            .last()
                            .copied()
                            .filter(|last| last.seq >= theirs.next_seq)
                            .map(|end| {
                                let from =
                                    points.partition_point(|point| point.seq < theirs.next_seq);
                                Work::Events {
                                    author,
                                    after: from.checked_sub(1).map(|i| points[i]),
                                    end,
                                    exclude: Vec::new(),
                                }
                            })
                    } else if ours.next_seq < theirs.next_seq
                        && points.last().is_none_or(|point| point.seq < ours.next_seq)
                    {
                        // We cannot verify a longer prefix. Send ours and let
                        // the initiator either push its suffix or ask for an
                        // inventory if the prefixes actually diverge.
                        None
                    } else {
                        Some(Work::Inventory {
                            author,
                            after: None,
                        })
                    };
                    if let Some(following) = following {
                        self.work.push_front(following);
                    }
                    continue;
                }
                let frame = SyncMessage::Frontier(mine.clone());
                self.work.pop_front();
                out.push(frame);
                self.waiting = true;
                return;
            }
            let mut done = true;
            let frame = match work {
                Work::Frame(frame) => Some(frame.clone()),
                Work::Inventory { author, after } => {
                    let points = replica.points(author);
                    let start =
                        after.map_or(0, |after| points.partition_point(|point| *point <= after));
                    let end = points.len().min(start + MAX_INVENTORY_POINTS);
                    Some(SyncMessage::Inventory {
                        author: *author,
                        points: points[start..end].to_vec(),
                        more: end < points.len(),
                    })
                }
                Work::Frontier { .. } => unreachable!("expanded above"),
                Work::Events {
                    author,
                    after,
                    end,
                    exclude,
                } => {
                    let points = replica.points(author);
                    let start =
                        after.map_or(0, |after| points.partition_point(|point| *point <= after));
                    let mut events = Vec::new();
                    let mut bytes = 3;
                    for point in &points[start..] {
                        if point > end {
                            break;
                        }
                        if exclude.binary_search(point).is_ok() {
                            *after = Some(*point);
                            continue;
                        }
                        if let Some(event) = replica.wire_event(&point.id) {
                            let size = encoded_len(&event);
                            if events.len() == MAX_EVENTS_PER_BATCH
                                || bytes + size > MAX_PEER_FRAME_BYTES
                            {
                                done = false;
                                break;
                            }
                            bytes += size;
                            events.push(event);
                        }
                        *after = Some(*point);
                    }
                    (!events.is_empty()).then_some(SyncMessage::Events(events))
                }
                Work::Requested(ids) => {
                    if ids.is_empty() {
                        self.work.pop_front();
                        continue;
                    }
                    let events = ids
                        .drain(..ids.len().min(MAX_EVENTS_PER_BATCH))
                        .filter_map(|id| replica.wire_event(&id))
                        .collect();
                    done = ids.is_empty();
                    Some(SyncMessage::Events(events))
                }
                Work::Blob {
                    hash,
                    offset,
                    total,
                } => {
                    let len = (*total - *offset).min(BLOB_CHUNK_BYTES as u64) as usize;
                    match replica.blob_range(hash, *offset, len) {
                        Some(bytes) if bytes.len() == len => {
                            let frame = SyncMessage::BlobChunk {
                                hash: *hash,
                                offset: *offset,
                                total: *total,
                                bytes,
                            };
                            *offset += len as u64;
                            done = *offset == *total;
                            Some(frame)
                        }
                        _ => Some(SyncMessage::BlobUnavailable(*hash)),
                    }
                }
            };
            if done {
                self.work.pop_front();
            }
            if let Some(frame) = frame {
                out.push(frame);
                self.waiting = true;
                return;
            }
        }
    }
}
