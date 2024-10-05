use std::{
    num::{NonZeroU16, NonZeroU64},
    time::{Duration, SystemTime},
};

use rand::RngCore;

use parking_lot::Mutex;

const ONE: NonZeroU16 = match NonZeroU16::new(1) {
    None => unreachable!(),
    Some(value) => value,
};

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
    epoch: SystemTime,
}

struct State {
    // Last seconds since epoch.
    last_secs: u64,
    counter: NonZeroU16,
}

impl Generator {
    /// Returns default epoch.
    pub fn default_epoch() -> SystemTime {
        /// 2023-04-07 21:51:12 UTC as seconds since UNIX epoch.
        /// This is the time when the value was defined.
        const DEFAULT_EPOCH: u64 = 1680904272;

        SystemTime::UNIX_EPOCH + Duration::from_secs(DEFAULT_EPOCH)
    }

    /// Creates a new generator with default epoch.
    pub fn new() -> Self {
        let epoch = Self::default_epoch();
        Generator::with_epoch(epoch)
    }

    /// Creates a new generator with given epoch.
    pub const fn with_epoch(epoch: SystemTime) -> Self {
        Generator {
            state: Mutex::new(State {
                counter: ONE,
                last_secs: 0,
            }),
            epoch,
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
        loop {
            let mut state = self.state.lock();
            let now = SystemTime::now();
            let since_epoch = now.duration_since(self.epoch).unwrap();
            let mut seconds = since_epoch.as_secs();

            if seconds >= 2 << 34 {
                panic!("Time overflow");
            }

            seconds = seconds.max(state.last_secs);
            if state.last_secs == seconds {
                match counter_next(state.counter) {
                    None => {
                        let next_second = self.epoch + Duration::from_secs(state.last_secs + 1);
                        let dur = next_second.duration_since(now).unwrap();
                        drop(state);
                        std::thread::sleep(dur);
                        continue;
                    }
                    Some(counter) => state.counter = counter,
                }
                continue;
            } else {
                state.last_secs = seconds;
                state.counter = ONE;
            }

            let counter = state.counter;
            drop(state);

            let mut r = [0u8; 4];
            rand::thread_rng().fill_bytes(&mut r[..3]);
            let r = u32::from_le_bytes(r);

            return (seconds << 30) | ((r as u64 & 0xfffff) << 10) | NonZeroU64::from(counter);
        }
    }
}
