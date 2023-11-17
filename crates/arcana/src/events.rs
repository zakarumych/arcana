use blink_alloc::Blink;
use edict::{EntityId, World};
use winit::event::{
    ElementState, KeyboardInput, ModifiersState, MouseButton, MouseScrollDelta, WindowEvent,
};

enum DeviceIdKind {
    Winit(winit::event::DeviceId),
}

pub struct DeviceId {
    kind: DeviceIdKind,
}

impl From<winit::event::DeviceId> for DeviceId {
    fn from(id: winit::event::DeviceId) -> Self {
        DeviceId {
            kind: DeviceIdKind::Winit(id),
        }
    }
}

/// Event emitted from outside the game.
///
/// Viewport and device events fall into this category.
pub enum Event {
    /// Event emitted from a viewport.
    ViewportEvent {
        viewport: EntityId,
        event: ViewportEvent,
    },

    /// Event emitted from a device.
    DeviceEvent {
        device: DeviceId,
        event: DeviceEvent,
    },
}

pub enum ViewportEvent {
    Resized {
        width: u32,
        height: u32,
    },
    ScaleFactorChanged {
        scale_factor: f64,
    },
    KeyboardInput {
        device_id: DeviceId,
        input: KeyboardInput,
    },
    ModifiersChanged(ModifiersState),
    CursorMoved {
        device_id: DeviceId,
        x: f64,
        y: f64,
    },
    CursorEntered {
        device_id: DeviceId,
    },
    CursorLeft {
        device_id: DeviceId,
    },
    MouseWheel {
        device_id: DeviceId,
        delta: MouseScrollDelta,
    },
    MouseInput {
        device_id: DeviceId,
        state: ElementState,
        button: MouseButton,
    },
}

pub struct UnsupportedEvent;

impl TryFrom<&WindowEvent<'_>> for ViewportEvent {
    type Error = UnsupportedEvent;

    fn try_from(event: &WindowEvent<'_>) -> Result<Self, UnsupportedEvent> {
        match *event {
            WindowEvent::Resized(size) => {
                let width = size.width;
                let height = size.height;
                Ok(ViewportEvent::Resized { width, height })
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                Ok(ViewportEvent::ScaleFactorChanged { scale_factor })
            }
            WindowEvent::KeyboardInput {
                device_id, input, ..
            } => {
                let device_id = DeviceId::from(device_id);
                Ok(ViewportEvent::KeyboardInput { device_id, input })
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                Ok(ViewportEvent::ModifiersChanged(modifiers))
            }
            WindowEvent::CursorMoved {
                device_id,
                position,
                ..
            } => {
                let device_id = DeviceId::from(device_id);
                let x = position.x;
                let y = position.y;
                Ok(ViewportEvent::CursorMoved { device_id, x, y })
            }
            WindowEvent::CursorEntered { device_id, .. } => {
                let device_id = DeviceId::from(device_id);
                Ok(ViewportEvent::CursorEntered { device_id })
            }
            _ => Err(UnsupportedEvent),
        }
    }
}

pub enum DeviceEvent {}

impl TryFrom<&winit::event::DeviceEvent> for DeviceEvent {
    type Error = UnsupportedEvent;

    fn try_from(value: &winit::event::DeviceEvent) -> Result<Self, UnsupportedEvent> {
        Err(UnsupportedEvent)
    }
}

pub trait EventFilter: Send + Sync + 'static {
    fn filter(&mut self, blink: &Blink, world: &mut World, event: Event) -> Option<Event>;
}

pub struct EventFunnel {
    pub filters: Vec<Box<dyn EventFilter>>,
}

impl EventFunnel {
    pub const fn new() -> Self {
        EventFunnel {
            filters: Vec::new(),
        }
    }

    #[inline]
    pub fn add<F>(&mut self, filter: F)
    where
        F: EventFilter,
    {
        self.filters.push(Box::new(filter));
    }

    #[inline]
    pub fn add_boxed(&mut self, filter: Box<dyn EventFilter>) {
        self.filters.push(filter);
    }

    #[inline]
    pub fn filter(&mut self, blink: &Blink, world: &mut World, mut event: Event) -> Option<Event> {
        for filter in self.filters.iter_mut() {
            event = filter.filter(blink, world, event)?;
        }
        Some(event)
    }
}

impl EventFilter for EventFunnel {
    #[inline]
    fn filter(&mut self, blink: &Blink, world: &mut World, event: Event) -> Option<Event> {
        self.filter(blink, world, event)
    }
}
