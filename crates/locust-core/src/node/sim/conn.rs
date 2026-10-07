//! Connections between shells, the shell's own deadlines, and what happens
//! to both when a machine stops, sleeps or wakes.
//!
//! Each end of a connection has its own belief about whether it is alive.
//! A shell opens exchanges over the newest connection it believes alive, so
//! after the other end vanished without a word an exchange opens, carries
//! nothing, and ends at the shell's idle deadline. That is how the daemon
//! behaves over QUIC, and it is where most recovery time goes.

use locust_proto::engine::{ExchangeId, PeerInput};

use super::machine::Power;
use super::world::{Ev, MS, Micros, World};
use super::{CONN_IDLE_MS, IO_IDLE_MS};

pub(super) struct Conn {
    /// The machine that connected, then the one that accepted.
    pub ends: [usize; 2],
    pub boots: [u32; 2],
    /// Each end's belief that the connection is alive.
    pub alive: [bool; 2],
    /// When each end last could have heard from the other.
    last_ok: [Micros; 2],
    /// Whether each end's engine admitted an exchange on it. A shell closes
    /// a connection that is not admitted by its deadline, or whose exchange
    /// ends first while no exchange the shell opened is in flight on it.
    pub admitted: [bool; 2],
    created: Micros,
}

impl Conn {
    pub fn end_of(&self, m: usize) -> usize {
        usize::from(self.ends[1] == m)
    }

    pub fn other(&self, m: usize) -> usize {
        self.ends[1 - self.end_of(m)]
    }

    pub fn alive_both(&self) -> bool {
        self.alive == [true; 2]
    }
}

impl World {
    /// The newest connection from `m` to `to` that `m`'s shell believes alive.
    pub(super) fn believed_conn(&self, m: usize, to: usize) -> Option<usize> {
        let boot = self.machines[m].boot;
        self.net.conns.iter().rposition(|conn| {
            let end = conn.end_of(m);
            conn.ends[end] == m
                && conn.ends[1 - end] == to
                && conn.boots[end] == boot
                && conn.alive[end]
        })
    }

    fn connection_input(&mut self, m: usize, peer: usize, connected: bool) {
        let at = self.now + self.local_delay();
        let boot = self.machines[m].boot;
        let endpoint = self.machines[peer].endpoint;
        let input = PeerInput::Connection {
            endpoint,
            connected,
        };
        self.schedule(at, Ev::Local { m, boot, input });
    }

    /// A connection attempt is tried: it gives a connection when the other
    /// machine runs and packets pass both ways, is tried again a little
    /// later otherwise, and ends with `OpenFailed` at its deadline.
    pub(super) fn connect(
        &mut self,
        m: usize,
        boot: u32,
        exchange: ExchangeId,
        to: Option<usize>,
        deadline: Micros,
    ) {
        let machine = &self.machines[m];
        if machine.boot != boot || machine.power == Power::Stopped {
            return;
        }
        let again = |deadline| Ev::Connect {
            m,
            boot,
            exchange,
            to,
            deadline,
        };
        if machine.power == Power::Asleep {
            return self.deferred.push(again(deadline));
        }
        let unlucky = self.rng.chance(self.net.open_fail_rate, 64);
        let reachable = to.filter(|to| {
            !unlucky
                && self.machines[*to].running()
                && self.net.reach(m, *to)
                && self.net.reach(*to, m)
        });
        let Some(to) = reachable else {
            let retry = self.now + self.rng.range(200, 1_500) * MS;
            if retry < deadline {
                return self.schedule(retry, again(deadline));
            }
            let input = PeerInput::OpenFailed(exchange);
            return self.schedule(deadline, Ev::Local { m, boot, input });
        };
        // A newly authenticated connection replaces old connections, while
        // preserving simultaneous dials younger than the production grace.
        for (local, remote) in [(m, to), (to, m)] {
            let boot = self.machines[local].boot;
            for index in 0..self.net.conns.len() {
                let conn = &self.net.conns[index];
                let end = conn.end_of(local);
                if conn.ends[end] == local
                    && conn.ends[1 - end] == remote
                    && conn.boots[end] == boot
                    && conn.alive[end]
                    && self.now - conn.created >= super::REPLACED_AFTER_MS * MS
                {
                    self.close_conn(index, end);
                }
            }
        }
        // A shell keeps at most two connections to a peer, which is what
        // two simultaneous dials need, and closes any further one.
        if self.believed_conns(m, to) >= 2 {
            let input = PeerInput::OpenFailed(exchange);
            let at = self.now + self.local_delay();
            return self.schedule(at, Ev::Local { m, boot, input });
        }
        let refused = self.believed_conns(to, m) >= 2;
        let conn = self.net.conns.len();
        self.net.conns.push(Conn {
            ends: [m, to],
            boots: [boot, self.machines[to].boot],
            alive: [true, !refused],
            last_ok: [self.now; 2],
            admitted: [false; 2],
            created: self.now,
        });
        self.connection_input(m, to, true);
        if refused {
            let at = self.now + self.latency(to, m);
            self.schedule(at, Ev::ConnClose { conn, to: 0 });
        } else {
            self.connection_input(to, m, true);
        }
        self.new_stream(conn, m, exchange);
    }

    /// How many connections to `to` the shell of `m` believes alive.
    fn believed_conns(&self, m: usize, to: usize) -> usize {
        let boot = self.machines[m].boot;
        let held = self.net.conns.iter().filter(|conn| {
            let end = conn.end_of(m);
            conn.ends[end] == m
                && conn.ends[1 - end] == to
                && conn.boots[end] == boot
                && conn.alive[end]
        });
        held.count()
    }

    /// The shell at one end closes a connection, and the other end hears.
    pub(super) fn close_conn(&mut self, conn: usize, end: usize) {
        if !self.net.conns[conn].alive[end] {
            return;
        }
        self.kill_view(conn, end);
        let (m, peer) = (
            self.net.conns[conn].ends[end],
            self.net.conns[conn].ends[1 - end],
        );
        let at = self.now + self.latency(m, peer);
        self.schedule(at, Ev::ConnClose { conn, to: 1 - end });
    }

    /// One end stops believing in a connection: its exchanges on it close,
    /// and the engine hears `Connection { connected: false }` once the shell
    /// holds no other connection to that endpoint.
    fn kill_view(&mut self, conn: usize, end: usize) {
        if !self.net.conns[conn].alive[end] {
            return;
        }
        self.net.conns[conn].alive[end] = false;
        let (m, peer) = (
            self.net.conns[conn].ends[end],
            self.net.conns[conn].ends[1 - end],
        );
        if self.machines[m].boot != self.net.conns[conn].boots[end] {
            return;
        }
        let streams: Vec<usize> = self.net.live.iter().copied().collect();
        for stream in streams {
            let st = &self.net.streams[stream];
            if st.conn == conn {
                let side = usize::from(st.sides[1].m == m);
                self.end_side(stream, side, false);
            }
        }
        if self.believed_conn(m, peer).is_none() && self.machines[m].power != Power::Stopped {
            self.connection_input(m, peer, false);
        }
    }

    pub(super) fn conn_close(&mut self, conn: usize, to: usize) {
        let c = &self.net.conns[conn];
        let (m, from) = (c.ends[to], c.ends[1 - to]);
        if self.machines[m].boot == c.boots[to]
            && self.machines[m].running()
            && self.net.reach(from, m)
        {
            self.kill_view(conn, to);
        }
    }

    /// A woken machine finds the connections it had before it slept dead.
    pub(super) fn wake_close(&mut self, m: usize, boot: u32, before: usize) {
        if self.machines[m].boot != boot || !self.machines[m].running() {
            return;
        }
        for conn in 0..before.min(self.net.conns.len()) {
            let c = &self.net.conns[conn];
            let end = c.end_of(m);
            if c.ends[end] == m && c.boots[end] == boot && c.alive[end] {
                self.kill_view(conn, end);
                let at = self.now + self.latency(m, self.net.conns[conn].ends[1 - end]);
                self.schedule(at, Ev::ConnClose { conn, to: 1 - end });
            }
        }
    }

    /// The deadlines the shell of a running machine keeps, checked at its
    /// poll: an exchange on which no frame was read or written for the idle
    /// deadline is aborted; a connection the engine did not admit within the
    /// same deadline is closed; a connection that could not have heard from
    /// its other end for the transport's idle timeout is dropped.
    pub(super) fn shell_timers(&mut self, m: usize) {
        let boot = self.machines[m].boot;
        let idle = IO_IDLE_MS * MS;
        let now = self.now;
        let streams: Vec<usize> = self.net.live.iter().copied().collect();
        for stream in streams {
            // A stream nothing can happen on any more is forgotten: each
            // side ended, lost its process, or never heard of the stream
            // before the other side ended it.
            let sides = &self.net.streams[stream].sides;
            let over = (0..2).all(|i| {
                let machine = &self.machines[sides[i].m];
                sides[i].ended
                    || machine.boot != sides[i].boot
                    || machine.power == Power::Stopped
                    || (sides[i].exchange.is_none() && sides[1 - i].ended)
            });
            if over {
                self.net.live.remove(&stream);
                continue;
            }
            for side in 0..2 {
                let s = &self.net.streams[stream].sides[side];
                if s.m != m || s.boot != boot || s.ended || s.exchange.is_none() {
                    continue;
                }
                // One deadline per exchange, pushed back by every frame
                // read or written.
                if now - s.last_recv.max(s.last_send) >= idle {
                    self.abort_side(stream, side);
                }
            }
        }
        for conn in 0..self.net.conns.len() {
            let c = &self.net.conns[conn];
            let end = c.end_of(m);
            if c.ends[end] != m || c.boots[end] != boot || !c.alive[end] {
                continue;
            }
            if !c.admitted[end] && now - c.created >= idle {
                self.close_conn(conn, end);
                continue;
            }
            let c = &self.net.conns[conn];
            let peer = c.ends[1 - end];
            let other = &self.machines[peer];
            let healthy = c.alive[1 - end]
                && other.boot == c.boots[1 - end]
                && other.running()
                && self.net.reach(m, peer)
                && self.net.reach(peer, m);
            if healthy {
                self.net.conns[conn].last_ok[end] = now;
            } else if now - c.last_ok[end] >= CONN_IDLE_MS * MS {
                self.kill_view(conn, end);
            }
        }
    }

    /// Ends the daemon process of machine `m`. A graceful stop closes its
    /// connections, which reachable peers notice at once; a crash says
    /// nothing, and peers find out at their own deadlines.
    pub fn stop(&mut self, m: usize, graceful: bool) {
        if self.machines[m].power == Power::Stopped {
            return;
        }
        self.note(m, 4, || format!("process stopped (graceful {graceful})"));
        let boot = self.machines[m].boot;
        for conn in 0..self.net.conns.len() {
            let c = &self.net.conns[conn];
            let end = c.end_of(m);
            if c.ends[end] != m || c.boots[end] != boot || !c.alive[end] {
                continue;
            }
            let peer = c.ends[1 - end];
            self.net.conns[conn].alive[end] = false;
            if graceful {
                let at = self.now + self.latency(m, peer);
                self.schedule(at, Ev::ConnClose { conn, to: 1 - end });
            }
        }
        let streams: Vec<usize> = self.net.live.iter().copied().collect();
        for stream in streams {
            for side in 0..2 {
                let s = &mut self.net.streams[stream].sides[side];
                if s.m == m && s.boot == boot {
                    s.ended = true;
                    s.out.clear();
                }
            }
            if self.net.streams[stream].sides.iter().all(|s| s.ended) {
                self.net.live.remove(&stream);
            }
        }
        self.deferred.retain(
            |ev| !matches!(ev, Ev::Local { m: n, .. } | Ev::Connect { m: n, .. } if *n == m),
        );
        if self.machines[m].running() {
            let now_ms = self.wall_ms(m);
            let revisions = !self.machines[m].restore_remembered();
            self.machines[m].last_revisions = revisions;
            self.machines[m].last_view = self.machines[m].visible(now_ms, revisions);
        }
        self.machines[m].stop();
    }

    /// The machine sleeps: its process exists and does not run.
    pub fn sleep(&mut self, m: usize) {
        if self.machines[m].running() {
            self.note(m, 5, || "asleep".into());
            self.machines[m].power = Power::Asleep;
        }
    }

    /// The machine wakes: its clock has jumped by however long it slept,
    /// its timers run again, and it finds its old connections dead some
    /// time within the idle deadline.
    pub fn wake(&mut self, m: usize) {
        if self.machines[m].power != Power::Asleep {
            return;
        }
        self.note(m, 6, || "awake".into());
        self.machines[m].power = Power::Running;
        let boot = self.machines[m].boot;
        let now = self.now;
        // Deadlines are measured on a clock that stood still while asleep.
        for stream in self.net.live.iter().copied().collect::<Vec<_>>() {
            for s in &mut self.net.streams[stream].sides {
                if s.m == m && s.boot == boot {
                    s.last_recv = now;
                    s.last_send = now;
                }
            }
        }
        let mut next = now;
        for ev in std::mem::take(&mut self.deferred) {
            if matches!(&ev, Ev::Local { m: n, .. } | Ev::Connect { m: n, .. } if *n == m) {
                next += 1;
                self.schedule(next, ev);
            } else {
                self.deferred.push(ev);
            }
        }
        let poll = now + self.rng.range(1, 1_000) * MS;
        self.schedule_poll(m, poll);
        let notice = if self.rng.chance(1, 2) {
            self.rng.range(0, 2_000)
        } else {
            self.rng.range(2_000, IO_IDLE_MS)
        };
        let before = self.net.conns.len();
        self.schedule(now + notice * MS, Ev::WakeClose { m, boot, before });
        self.release_held();
    }

    /// Stops packets from `from` to `to`, or lets them pass again.
    pub fn set_blocked(&mut self, from: usize, to: usize, blocked: bool) {
        let count = &mut self.net.blocked[from][to];
        if blocked {
            *count += 1;
        } else {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.release_held();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulated_shell_reuses_newest_and_expires_dead_connections_after_fifteen_seconds() {
        let mut world = World::new(1, 2);
        world.start(0);
        world.start(1);
        for _ in 0..2 {
            world.net.conns.push(Conn {
                ends: [0, 1],
                boots: [1, 1],
                alive: [true, true],
                last_ok: [0; 2],
                admitted: [true; 2],
                created: 0,
            });
        }
        assert_eq!(world.believed_conn(0, 1), Some(1));
        world.stop(1, false);
        world.now = 14_999 * MS;
        world.shell_timers(0);
        assert_eq!(world.believed_conn(0, 1), Some(1));
        world.now = 15_000 * MS;
        world.shell_timers(0);
        assert_eq!(world.believed_conn(0, 1), None);
    }
}
