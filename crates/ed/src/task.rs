use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Waker},
};

use parking_lot::Mutex;
use smallvec::SmallVec;

/// Array of wakers that can be used to wake up multiple tasks.
///
/// Use this when implementing futures manually and multiple tasks may wait for the same future.
#[derive(Default)]
pub struct WakerArray {
    /// Array of wakers.
    ///
    /// 8 is picked as a reasonable default capacity.
    wakers: SmallVec<[Waker; 4]>,
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

/// Simple task queue that accepts tasks of some type `T` and returns futures with results of some type `U`.
///
/// Use this for communication between coroutines and ordinary functions.
/// It can be freely cloned to share between tasks and other parts of the process.
#[derive(Clone)]
pub struct TaskQueue<T, U> {
    requests: Arc<Mutex<Requests<T>>>,
    responses: Arc<Mutex<Responses<U>>>,
}

impl<T, U> TaskQueue<T, U> {
    /// Returns new task queue instance.
    pub fn new() -> Self {
        TaskQueue {
            requests: Arc::new(Mutex::new(Requests {
                array: VecDeque::new(),
            })),
            responses: Arc::new(Mutex::new(Responses {
                offset: 0,
                array: Vec::new(),
            })),
        }
    }

    /// Pushes new task into the queue and returns a future that resolves to the result of the task.
    pub fn push(&self, task: T) -> TaskFuture<U> {
        let mut responses = self.responses.lock();

        let id = responses.offset + responses.array.len() as u64;
        responses.array.push(Response::Pending {
            wakers: WakerArray::new(),
        });
        drop(responses);

        let mut requests = self.requests.lock();
        requests.array.push_back((task, id));
        drop(requests);

        TaskFuture {
            id,
            responses: self.responses.clone(),
        }
    }

    /// Processes all tasks in the queue.
    /// This requires the closure to be callable multiple times.
    /// If need to use `FnOnce` closure, use `process` instead.
    pub fn process_all(&self, mut f: impl FnMut(T) -> U) {
        let mut exit = false;
        while !exit {
            exit = true;
            self.process(|task| {
                exit = false;
                f(task)
            });
        }
    }

    /// Processes up to one task in the queue.
    pub fn process(&self, f: impl FnOnce(T) -> U) {
        let mut requests = self.requests.lock();
        let Some((task, id)) = requests.array.pop_front() else {
            return;
        };

        let mut responses = self.responses.lock();
        let index = id - responses.offset;
        assert!(index < responses.array.len() as u64);

        match responses.array[index as usize] {
            Response::Pending { ref mut wakers } => {
                let result = f(task);
                wakers.wake();
                responses.array[index as usize] = Response::Ready { result };
            }
            _ => {
                panic!("Task is not pending");
            }
        }
    }
}

struct Requests<T> {
    array: VecDeque<(T, u64)>,
}

enum Response<U> {
    Pending { wakers: WakerArray },
    Ready { result: U },
    Taken,
}

struct Responses<U> {
    offset: u64,
    array: Vec<Response<U>>,
}

pub struct TaskFuture<U> {
    id: u64,
    responses: Arc<Mutex<Responses<U>>>,
}

impl<U> TaskFuture<U> {
    pub fn is_ready(&self) -> bool {
        let responses = self.responses.lock();

        let index = self.id - responses.offset;
        assert!(index < responses.array.len() as u64);

        match unsafe { responses.array.get_unchecked(index as usize) } {
            Response::Ready { .. } => true,
            _ => false,
        }
    }

    pub fn poll(&mut self) -> Option<U> {
        let mut responses = self.responses.lock();

        let index = self.id - responses.offset;
        assert!(index < responses.array.len() as u64);

        match unsafe { responses.array.get_unchecked_mut(index as usize) } {
            response @ Response::Ready { .. } => {
                let response = std::mem::replace(response, Response::Taken);
                let Response::Ready { result } = response else {
                    unsafe {
                        std::hint::unreachable_unchecked();
                    }
                };

                Some(result)
            }
            Response::Pending { .. } => None,
            Response::Taken => {
                panic!("Response already taken");
            }
        }
    }
}

impl<U> Future for TaskFuture<U> {
    type Output = U;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let me = self.get_mut();

        let mut responses = me.responses.lock();

        let index = me.id - responses.offset;
        assert!(index < responses.array.len() as u64);

        match unsafe { responses.array.get_unchecked_mut(index as usize) } {
            response @ Response::Ready { .. } => {
                let response = std::mem::replace(response, Response::Taken);
                let Response::Ready { result } = response else {
                    unsafe {
                        std::hint::unreachable_unchecked();
                    }
                };

                Poll::Ready(result)
            }
            Response::Pending { wakers } => {
                wakers.register(cx.waker());
                Poll::Pending
            }
            Response::Taken => {
                panic!("Response already taken");
            }
        }
    }
}
