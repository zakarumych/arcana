use std::{
    num::{NonZeroU16, NonZeroU64},
    time::{Duration, Instant, SystemTime},
};

use rand::RngCore;

use parking_lot::Mutex;

use crate::id::{GenId, GenUid};

const MAX_SLEEP: Duration = Duration::from_secs(10);

fn counter_next(counter: NonZeroU16) -> Option<NonZeroU16> {
    let c = counter.get();
    if c >= 0x3ff {
        None
    } else {
        Some(counter.saturating_add(1))
    }
}

/// Generates pseudo-unique IDs.
///
/// The IDs are generated with following scheme:
///
/// 34 bits - seconds since epoch.
/// 20 bits - random.
/// 10 bits - counter.
pub struct Generator {
    state: Mutex<State>,

    /// Monotonic clock.
    start: Instant,

    /// Seconds since epoch before start.
    from_epoch: u64,
}

struct State {
    // Last seconds since epoch.
    last_secs: u64,
    counter: NonZeroU16,
}

impl Generator {
    /// Returns default epoch.
    pub fn default_epoch() -> SystemTime {
        SystemTime::UNIX_EPOCH
    }

    /// Creates a new generator with default epoch.
    pub fn new() -> Self {
        let epoch = Self::default_epoch();
        Generator::with_epoch(epoch)
    }

    /// Creates a new generator with given epoch.
    pub fn with_epoch(epoch: SystemTime) -> Self {
        let now = SystemTime::now();

        let from_epoch = now
            .duration_since(epoch)
            .expect("Epoch is in the future")
            .as_secs();

        Generator {
            state: Mutex::new(State {
                counter: NonZeroU16::MIN,
                last_secs: 0,
            }),
            start: Instant::now(),
            from_epoch,
        }
    }

    /// Generates a new pseudo-unique ID.
    /// The generated ID is guaranteed to be unique only within
    /// the same instance of the generator.
    ///
    /// For multiple instances of the generator, the IDs may collide with
    /// low probability.
    ///
    /// # Panics
    ///
    /// Panics if seconds since epoch is greater than 2^34 - 557+ years.
    pub fn generate(&self) -> NonZeroU64 {
        /// The maximum number of seconds since epoch is 2^34 - 557+ years.
        const MAX_SECONDS: u64 = 2 << 34;

        let mut now = Instant::now();
        let mut seconds = now
            .duration_since(self.start)
            .as_secs()
            .saturating_add(self.from_epoch);
        let counter;

        loop {
            if seconds >= MAX_SECONDS {
                panic!("Time overflow");
            }

            let mut state = self.state.lock();

            // Bump the seconds so it won't be decreased in any case.
            // It shouldn't be decreased anyway, but just in case.
            seconds = seconds.max(state.last_secs);
            if state.last_secs == seconds {
                match counter_next(state.counter) {
                    None => {
                        let next_second = self.start + Duration::from_secs(state.last_secs + 1);
                        let dur = next_second.duration_since(now);
                        drop(state);

                        if dur > MAX_SLEEP {
                            panic!("Time based ID generator requires long sleep. This should not happen.");
                        }

                        std::thread::sleep(dur);

                        // Update timer after sleep.
                        now = Instant::now();
                        seconds = now
                            .duration_since(self.start)
                            .as_secs()
                            .saturating_add(self.from_epoch);
                        continue;
                    }
                    Some(counter) => state.counter = counter,
                }
            } else {
                state.last_secs = seconds;
                state.counter = NonZeroU16::MIN;
            }

            counter = state.counter;
            drop(state);
            break;
        }

        let random = u64::from(rand::rng().next_u32() & 0xfffff);
        let counter = u64::from(counter.get());
        debug_assert!(counter <= 0x3ff);

        // unwrap: `counter` is non-zero u16, thus it results in non-zero u64 here.
        NonZeroU64::new((seconds << 30) | (counter << 20) | random).unwrap()
    }
}

impl GenId for Generator {
    type Value = NonZeroU64;

    fn generate(&mut self) -> NonZeroU64 {
        Self::generate(&*self)
    }
}

impl GenUid for Generator {}

impl GenId for &Generator {
    type Value = NonZeroU64;

    fn generate(&mut self) -> NonZeroU64 {
        Generator::generate(self)
    }
}

impl GenUid for &Generator {}
