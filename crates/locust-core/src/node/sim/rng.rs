//! The simulator's only source of randomness: a seeded SplitMix64 stream.
//!
//! Every choice the simulator makes (latencies, faults, credentials, the
//! bytes a node draws through [`Entropy`]) comes from a stream forked from
//! the run's seed, so a seed reproduces a run exactly. Streams are forked by
//! purpose, so drawing more from one never shifts another.

use locust_proto::engine::Entropy;

#[derive(Clone, Debug)]
pub struct Rng(u64);

fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(mix(seed ^ 0x6C6F_6375_7374_2D73))
    }

    /// An independent stream for one purpose, named by `tag`.
    pub fn fork(&self, tag: u64) -> Self {
        Self(mix(self.0 ^ mix(tag.wrapping_add(0x9E37_79B9_7F4A_7C15))))
    }

    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix(self.0)
    }

    /// Uniform in `0..n`; `n` must not be zero.
    pub fn below(&mut self, n: u64) -> u64 {
        // The bias of a plain remainder is far below anything a few
        // thousand draws per run can observe.
        self.next() % n
    }

    /// Uniform in `lo..=hi`.
    pub fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.below(hi - lo + 1)
    }

    /// True `num` times in `den`.
    pub fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }

    pub fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize]
    }

    pub fn bytes<const N: usize>(&mut self) -> [u8; N] {
        let mut out = [0u8; N];
        self.fill(&mut out);
        out
    }

    pub fn fill(&mut self, bytes: &mut [u8]) {
        for chunk in bytes.chunks_mut(8) {
            let word = self.next().to_le_bytes();
            chunk.copy_from_slice(&word[..chunk.len()]);
        }
    }
}

/// What a node draws its keys, salts and secrets from. One stream per
/// machine and per start of its process: a restarted daemon never repeats
/// the draws of its earlier life, as a real source never does.
pub struct SimEntropy(pub Rng);

impl Entropy for SimEntropy {
    fn fill(&mut self, bytes: &mut [u8]) {
        self.0.fill(bytes);
    }
}

#[test]
fn a_seed_repeats_and_forks_differ() {
    let (mut a, mut b) = (Rng::new(7), Rng::new(7));
    assert_eq!(
        (0..8).map(|_| a.next()).collect::<Vec<_>>(),
        (0..8).map(|_| b.next()).collect::<Vec<_>>()
    );
    let root = Rng::new(7);
    assert_ne!(root.fork(1).next(), root.fork(2).next());
    assert_ne!(Rng::new(7).next(), Rng::new(8).next());
    let mut r = Rng::new(1);
    assert!((0..1000).all(|_| (3..=9).contains(&r.range(3, 9))));
}
