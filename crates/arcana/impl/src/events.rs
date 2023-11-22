use std::fmt;

use blink_alloc::Blink;
use edict::{EntityId, World};
use winit::event::WindowEvent;

pub use winit::{
    event::{
        ElementState, KeyboardInput, ModifiersState, MouseButton, MouseScrollDelta, ScanCode,
        VirtualKeyCode,
    },
    window::CursorIcon,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum DeviceIdKind {
    Emulated,
    Winit(winit::event::DeviceId),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceId {
    kind: DeviceIdKind,
}

impl fmt::Debug for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            DeviceIdKind::Emulated => write!(f, "Emulated"),
            DeviceIdKind::Winit(id) => write!(f, "winit::DeviceId({:?})", id),
        }
    }
}

impl From<winit::event::DeviceId> for DeviceId {
    fn from(id: winit::event::DeviceId) -> Self {
        DeviceId {
            kind: DeviceIdKind::Winit(id),
        }
    }
}

impl DeviceId {
    pub fn emulated() -> Self {
        DeviceId {
            kind: DeviceIdKind::Emulated,
        }
    }
}

/// Event emitted from outside the game.
///
/// Viewport and device events fall into this category.
#[derive(Clone)]
pub enum Event {
    /// Event emitted from a viewport.
    ViewportEvent { event: ViewportEvent },

    /// Event emitted from a device.
    DeviceEvent {
        device: DeviceId,
        event: DeviceEvent,
    },
}

#[derive(Clone)]
pub enum ViewportEvent {
    Resized {
        width: u32,
        height: u32,
    },
    ScaleFactorChanged {
        scale_factor: f32,
    },
    KeyboardInput {
        device_id: DeviceId,
        input: KeyboardInput,
    },
    ModifiersChanged(ModifiersState),
    CursorMoved {
        device_id: DeviceId,
        x: f32,
        y: f32,
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
                Ok(ViewportEvent::ScaleFactorChanged {
                    scale_factor: scale_factor as f32,
                })
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
                let x = position.x as f32;
                let y = position.y as f32;
                Ok(ViewportEvent::CursorMoved { device_id, x, y })
            }
            WindowEvent::CursorEntered { device_id } => {
                let device_id = DeviceId::from(device_id);
                Ok(ViewportEvent::CursorEntered { device_id })
            }
            WindowEvent::CursorLeft { device_id } => {
                let device_id = DeviceId::from(device_id);
                Ok(ViewportEvent::CursorLeft { device_id })
            }
            WindowEvent::MouseWheel {
                device_id,
                delta,
                phase,
                modifiers,
            } => {
                let device_id = DeviceId::from(device_id);
                let delta = delta;
                Ok(ViewportEvent::MouseWheel { device_id, delta })
            }
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
                modifiers,
            } => {
                let device_id = DeviceId::from(device_id);
                let state = state;
                let button = button;
                Ok(ViewportEvent::MouseInput {
                    device_id,
                    state,
                    button,
                })
            }
            _ => Err(UnsupportedEvent),
        }
    }
}

#[derive(Clone)]
pub enum DeviceEvent {}

impl TryFrom<&winit::event::DeviceEvent> for DeviceEvent {
    type Error = UnsupportedEvent;

    fn try_from(value: &winit::event::DeviceEvent) -> Result<Self, UnsupportedEvent> {
        Err(UnsupportedEvent)
    }
}

pub trait EventFilter: 'static {
    fn filter(&mut self, blink: &Blink, world: &mut World, event: &Event) -> bool;
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

    #[inline(never)]
    pub fn add<F>(&mut self, filter: F)
    where
        F: EventFilter,
    {
        self.filters.push(Box::new(filter));
    }

    #[inline(never)]
    pub fn add_boxed(&mut self, filter: Box<dyn EventFilter>) {
        self.filters.push(filter);
    }

    #[inline(never)]
    pub fn filter(&mut self, blink: &Blink, world: &mut World, event: &Event) -> bool {
        for filter in self.filters.iter_mut() {
            if filter.filter(blink, world, event) {
                return true;
            }
        }
        false
    }
}

impl EventFilter for EventFunnel {
    #[inline(never)]
    fn filter(&mut self, blink: &Blink, world: &mut World, event: &Event) -> bool {
        self.filter(blink, world, event)
    }
}
