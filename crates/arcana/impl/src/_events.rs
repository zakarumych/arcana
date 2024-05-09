//! Async event loop.

use std::{
    borrow::Borrow, cell::Cell, future::Future, ops::Deref, ptr::NonNull, rc::Rc, sync::Arc,
    task::Poll, time::Instant,
};

use parking_lot::Mutex;

use winit::{event_loop::EventLoopWindowTarget, window::WindowId};

pub use winit::event::*;

#[cfg(target_os = "windows")]
use winit::platform::windows::EventLoopBuilderExtWindows as _;

/// Runtime unique identifier for a device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct DeviceId(DeviceIdKind);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum DeviceIdKind {
    Winit(winit::event::DeviceId),
    Gilrs(gilrs::GamepadId),
}

impl PartialEq<winit::event::DeviceId> for DeviceId {
    fn eq(&self, other: &winit::event::DeviceId) -> bool {
        match &self.0 {
            DeviceIdKind::Winit(id) => *id == *other,
            DeviceIdKind::Gilrs(_) => false,
        }
    }
}

impl From<winit::event::DeviceId> for DeviceId {
    fn from(id: winit::event::DeviceId) -> Self {
        DeviceId(DeviceIdKind::Winit(id))
    }
}

impl PartialEq<gilrs::GamepadId> for DeviceId {
    fn eq(&self, other: &gilrs::GamepadId) -> bool {
        match &self.0 {
            DeviceIdKind::Winit(_) => false,
            DeviceIdKind::Gilrs(id) => *id == *other,
        }
    }
}

impl From<gilrs::GamepadId> for DeviceId {
    fn from(id: gilrs::GamepadId) -> Self {
        DeviceId(DeviceIdKind::Gilrs(id))
    }
}

/// Event emitted by the event loop.
#[derive(Debug)]
pub enum Event {
    /// Emitted once in the beginning of the event loop iteration.
    ///
    /// Main loop should run next update phase upon receiving this event.
    Update,

    /// Emitted when the OS sends an event for a winit window.
    WindowEvent {
        window_id: WindowId,
        event: WindowEvent<'static>,
    },

    /// Emitted when the OS sends an event for a device.
    DeviceEvent {
        device_id: DeviceId,
        event: DeviceEvent,
    },

    /// Emitted when the OS sends an event for a gamepad.
    GamepadEvent {
        device_id: DeviceId,
        event: gilrs::EventType,
    },

    /// Emitted when the OS requests a redraw for a window.
    RedrawRequested(WindowId),
}

pub enum UserEvent {
    Wake,
}

struct EventLoopWaker {
    proxy: Mutex<winit::event_loop::EventLoopProxy<UserEvent>>,
}

impl std::task::Wake for EventLoopWaker {
    #[cfg_attr(inline_more, inline(always))]
    fn wake(self: Arc<Self>) {
        EventLoopWaker::wake_by_ref(&self)
    }

    #[cfg_attr(inline_more, inline(always))]
    fn wake_by_ref(self: &Arc<Self>) {
        // This send will never block.
        let _ = self.proxy.lock().send_event(UserEvent::Wake);
    }
}

struct Shared {
    target: Cell<Option<NonNull<EventLoopWindowTarget<UserEvent>>>>,
    deadline: Cell<Option<Instant>>,
}

struct SharedGuard<'a> {
    shared: &'a Shared,
}

impl<'a> SharedGuard<'a> {
    fn new(shared: &'a Shared, target: &'a EventLoopWindowTarget<UserEvent>) -> Self {
        let guard = SharedGuard { shared };
        guard.shared.target.set(Some(NonNull::from(&*target)));
        guard
    }
}

impl Drop for SharedGuard<'_> {
    fn drop(&mut self) {
        self.shared.target.set(None);
    }
}

pub struct EventLoopBuilder {
    _private: (),
}

impl EventLoopBuilder {
    pub const fn new() -> Self {
        EventLoopBuilder { _private: () }
    }

    /// Runs main application future on the event loop.
    /// Initializes tokio runtime and uses it to run the future inside winit event loop.
    ///
    /// Due to library limitations this function never returns.
    /// This function can be called right away in the `main`.
    pub fn run<F, Fut>(&self, f: F) -> !
    where
        F: FnOnce(EventLoop) -> Fut,
        Fut: Future + 'static,
    {
        EventLoop::run_impl(f)
    }
}

/// Async event loop.
pub struct EventLoop {
    events: flume::Receiver<Event>,
    shared: Rc<Shared>,
}

impl EventLoop {
    pub fn run<F, Fut>(f: F) -> !
    where
        F: FnOnce(EventLoop) -> Fut,
        Fut: Future + 'static,
    {
        EventLoopBuilder::new().run(f)
    }

    fn run_impl<F, Fut>(f: F) -> !
    where
        F: FnOnce(EventLoop) -> Fut,
        Fut: Future + 'static,
    {
        let runtime = runtime();

        let mut el = winit::event_loop::EventLoopBuilder::<UserEvent>::with_user_event();

        #[cfg(target_os = "windows")]
        el.with_dpi_aware(true);

        #[cfg(target_os = "windows")]
        el.with_any_thread(true);

        let el = el.build();

        let (event_tx, event_rx) = flume::unbounded();

        let shared = Rc::new(Shared {
            target: Cell::new(None),
            deadline: Cell::new(None),
        });

        let instance = EventLoop {
            events: event_rx,
            shared: shared.clone(),
        };

        let guard = SharedGuard::new(&shared, &el);
        let app_future = runtime.block_on(async move { f(instance) });
        let mut app_future_opt = Some(Box::pin(app_future));
        drop(guard);

        let waker = std::task::Waker::from(Arc::new(EventLoopWaker {
            proxy: Mutex::new(el.create_proxy()),
        }));

        el.run(move |event, target, flow| match event {
            winit::event::Event::NewEvents(_) => {}
            winit::event::Event::Suspended | winit::event::Event::Resumed => {}
            winit::event::Event::UserEvent(_) => {}
            winit::event::Event::WindowEvent { window_id, event } => {
                if let Some(event) = event.to_static() {
                    let _ = event_tx.send(Event::WindowEvent { window_id, event });
                }
            }
            winit::event::Event::DeviceEvent { device_id, event } => {
                let _ = event_tx.send(Event::DeviceEvent {
                    device_id: device_id.into(),
                    event,
                });
            }
            winit::event::Event::MainEventsCleared => {}
            winit::event::Event::RedrawRequested(window_id) => {
                let _ = event_tx.send(Event::RedrawRequested(window_id));
            }
            winit::event::Event::RedrawEventsCleared => {
                // Run the app future.

                if let Some(app_future) = &mut app_future_opt {
                    let guard = SharedGuard::new(&shared, target);

                    let mut ctx = std::task::Context::from_waker(&waker);

                    let runtime_guard = runtime.enter();
                    if let Poll::Ready(_) = app_future.as_mut().poll(&mut ctx) {
                        tracing::info!("App future completed");
                        *flow = winit::event_loop::ControlFlow::Exit;
                        app_future_opt = None;
                        return;
                    }
                    drop(runtime_guard);

                    match guard.shared.deadline.take() {
                        None => *flow = winit::event_loop::ControlFlow::Wait,
                        Some(deadline) => {
                            *flow = winit::event_loop::ControlFlow::WaitUntil(deadline)
                        }
                    }
                }
            }
            winit::event::Event::LoopDestroyed => {
                // Destroy app if it's still running.
                app_future_opt.take();
            }
        });
    }

    /// Collects new events and returns iterator over them.
    pub async fn next(&self, deadline: Option<Instant>) -> impl Iterator<Item = Event> + '_ {
        self.shared.deadline.set(deadline);
        futures::pending!();
        self.events.try_iter()
    }

    fn target(&self) -> &EventLoopWindowTarget<UserEvent> {
        let target = self
            .shared
            .target
            .get()
            .expect("Target must be set whenever app is running");
        unsafe { target.as_ref() }
    }
}

impl Deref for EventLoop {
    type Target = EventLoopWindowTarget<UserEvent>;

    fn deref(&self) -> &Self::Target {
        self.target()
    }
}

#[cfg(not(feature = "multi-thread"))]
fn runtime_builder() -> tokio::runtime::Builder {
    tokio::runtime::Builder::new_current_thread()
}

#[cfg(feature = "multi-thread")]
fn runtime_builder() -> tokio::runtime::Builder {
    tokio::runtime::Builder::new_multi_thread()
}

fn runtime() -> tokio::runtime::Runtime {
    tracing::info!("Building tokio runtime");

    match runtime_builder().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => {
            tracing::error!("Failed to build tokio runtime with IO enabled: {}", err);

            tracing::info!("Building tokio runtime with IO disabled");
            runtime_builder()
                .enable_time()
                .build()
                .expect("Failed to build tokio runtime")
        }
    }
}
