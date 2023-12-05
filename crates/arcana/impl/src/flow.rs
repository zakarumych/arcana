use std::{
    cmp::Ordering,
    collections::{binary_heap::PeekMut, BinaryHeap},
    future::poll_fn,
    task::{Poll, Waker},
};

pub use edict::flow::*;
use edict::{Res, ResMut, Scheduler, World};
use gametime::{ClockStep, TimeSpan, TimeStamp};

/// Causes flow to sleep for the specified duration.
pub async fn sleep(duration: TimeSpan, world: FlowWorld<'_>) {
    if duration == TimeSpan::ZERO {
        return;
    }

    let deadline = world
        .poll_fn(|world, cx| {
            let now = world.expect_resource::<ClockStep>().now;
            let deadline = now + duration;

            world
                .expect_resource_mut::<Timers>()
                .add_timer(cx.waker().clone(), deadline);
            Poll::Ready(deadline)
        })
        .await;

    world
        .poll_fn(|world, cx| {
            let now = world.expect_resource::<ClockStep>().now;

            if now >= deadline {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await
}

/// Causes flow to sleep untile specified time.
pub async fn sleep_until(deadline: TimeStamp, world: FlowWorld<'_>) {
    let ready = world
        .poll_fn(|world, cx| {
            let now = world.expect_resource::<ClockStep>().now;
            if now >= deadline {
                return Poll::Ready(true);
            }

            world
                .expect_resource_mut::<Timers>()
                .add_timer(cx.waker().clone(), deadline);
            Poll::Ready(false)
        })
        .await;

    if ready {
        return;
    }

    world
        .poll_fn(|world, cx| {
            let now = world.expect_resource::<ClockStep>().now;

            if now >= deadline {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await
}

struct Timer {
    when: TimeStamp,
    waker: Waker,
}

impl PartialEq for Timer {
    fn eq(&self, other: &Self) -> bool {
        self.when == other.when
    }
}

impl Eq for Timer {}

impl PartialOrd for Timer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.when.cmp(&other.when).reverse())
    }

    fn le(&self, other: &Self) -> bool {
        self.when >= other.when
    }

    fn ge(&self, other: &Self) -> bool {
        self.when <= other.when
    }

    fn gt(&self, other: &Self) -> bool {
        self.when < other.when
    }

    fn lt(&self, other: &Self) -> bool {
        self.when > other.when
    }
}

impl Ord for Timer {
    fn cmp(&self, other: &Self) -> Ordering {
        self.when.cmp(&other.when).reverse()
    }
}

/// Resource that contains wakers with timers when to wake them.
struct Timers {
    timers_heap: BinaryHeap<Timer>,
}

impl Timers {
    fn new() -> Self {
        Timers {
            timers_heap: BinaryHeap::new(),
        }
    }

    fn add_timer(&mut self, waker: Waker, when: TimeStamp) {
        self.timers_heap.push(Timer { when, waker });
    }

    fn wake_until(&mut self, now: TimeStamp) {
        while let Some(top) = self.timers_heap.peek() {
            if top.when > now {
                break;
            }
            self.timers_heap.pop().unwrap().waker.wake();
        }
    }
}

pub fn init_flows(world: &mut World) {
    world.insert_resource(Timers::new());
}

pub fn run_flows(world: &mut World) {
    let mut times = world.expect_resource_mut::<Timers>();
    let clocks = world.expect_resource::<ClockStep>();

    times.wake_until(clocks.now);
}

pub trait FlowWorldExt {}

impl FlowWorldExt for FlowWorld<'_> {}
