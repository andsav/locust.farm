//! What the shell supplies to the engine from the operating system: the
//! time and randomness. The engine reads neither on its own.

use std::time::{SystemTime, UNIX_EPOCH};

use locust_proto::engine::Entropy;

/// The system clock in Unix milliseconds.
pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// The operating system's random source.
pub(crate) struct OsEntropy;

impl Entropy for OsEntropy {
    fn fill(&mut self, bytes: &mut [u8]) {
        // Without randomness no key or secret can be made; there is nothing
        // to fall back to.
        getrandom::fill(bytes).expect("the operating system's random source failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_is_in_milliseconds_and_entropy_is_not_constant() {
        // 2026-01-01 in Unix milliseconds.
        assert!(now_ms() > 1_767_225_600_000);
        let (mut first, mut second) = ([0u8; 32], [0u8; 32]);
        OsEntropy.fill(&mut first);
        OsEntropy.fill(&mut second);
        assert_ne!(first, second);
    }
}
