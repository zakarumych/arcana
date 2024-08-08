use std::task::Waker;

use smallvec::SmallVec;

/// Array of wakers that can be used to wake up multiple tasks.
///
/// Use this when implementing futures manually and multiple tasks may wait for the same future.
pub struct WakerArray {
    /// Array of wakers.
    ///
    /// 8 is picked as a reasonable default capacity.
    wakers: SmallVec<[Waker; 8]>,
}

impl WakerArray {
    /// Creates a new empty waker array.
    ///
    /// Call [`WakerArray::register`] to register wakers and [`WakerArray::wake`] to all registered wakers.
    pub fn new() -> Self {
        WakerArray {
            wakers: SmallVec::new(),
        }
    }

    /// Registers a waker to be woken up when [`WakerArray::wake`] is called.
    pub fn register(&mut self, waker: &Waker) {
        if self.wakers.iter().any(|w| w.will_wake(&waker)) {
            return;
        }
        self.wakers.push(waker.clone());
    }

    /// Wakes up all registered wakers.
    ///
    /// All registered wakers are woken up and removed from the array.
    pub fn wake(&mut self) {
        for waker in self.wakers.drain(..) {
            waker.wake();
        }
    }
}
