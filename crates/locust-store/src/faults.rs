//! Per-test-thread durability faults. Hooks run immediately before the real
//! operation; an armed fault is consumed exactly once and tests assert it ran.

use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Point {
    Recovery,
    FileSync,
    DirectorySync,
    Discard,
}

#[derive(Default)]
struct State {
    fail: Option<(Point, PathBuf)>,
    fired: bool,
    seen: Vec<(Point, PathBuf)>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::default();
}

pub(crate) fn arm(point: Point, path: &Path) {
    STATE.with(|state| {
        *state.borrow_mut() = State {
            fail: Some((point, path.to_owned())),
            ..State::default()
        };
    });
}

pub(crate) fn assert_fired() {
    STATE.with(|state| assert!(state.borrow().fired, "the injected fault never fired"));
}

pub(crate) fn take_trace() -> Vec<(Point, PathBuf)> {
    STATE.with(|state| std::mem::take(&mut state.borrow_mut().seen))
}

pub(crate) fn check(point: Point, path: &Path) -> io::Result<()> {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.seen.push((point, path.to_owned()));
        if state
            .fail
            .as_ref()
            .is_some_and(|(p, name)| *p == point && name == path)
        {
            state.fail = None;
            state.fired = true;
            Err(io::Error::other("injected durability failure"))
        } else {
            Ok(())
        }
    })
}
