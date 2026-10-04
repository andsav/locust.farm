//! Lazy response cursors. At most one frame is awaiting transport capacity.
use std::collections::VecDeque;

use locust_proto::event::AuthorPoint;
use locust_proto::id::{BlobHash, EventId, PublicKey};
use locust_proto::limits::{BLOB_CHUNK_BYTES, MAX_EVENTS_PER_BATCH, MAX_PEER_FRAME_BYTES};
use locust_proto::sync::SyncMessage;

use super::Replica;
use super::batch::encoded_len;

#[derive(Debug)]
pub(super) enum Work {
    Frame(SyncMessage),
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
            let mut done = true;
            let frame = match work {
                Work::Frame(frame) => Some(frame.clone()),
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
