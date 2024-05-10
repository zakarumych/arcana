use std::fmt;

use blink_alloc::Blink;
use edict::World;
use winit::event::WindowEvent;

pub use winit::{
    event::{ElementState, KeyEvent, Modifiers, MouseButton, MouseScrollDelta},
    keyboard::{Key, KeyCode, ModifiersState, NamedKey, NativeKey, NativeKeyCode, PhysicalKey},
    window::CursorIcon,
};

use crate::make_id;

make_id!(pub FilterId);

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
        event: KeyEvent,
    },
    ModifiersChanged(Modifiers),
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
    // Text {
    //     text: String,
    // },
}

pub struct UnsupportedEvent;

impl TryFrom<&WindowEvent> for ViewportEvent {
    type Error = UnsupportedEvent;

    fn try_from(event: &WindowEvent) -> Result<Self, UnsupportedEvent> {
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
                device_id,
                ref event,
                ..
            } => {
                let device_id = DeviceId::from(device_id);
                Ok(ViewportEvent::KeyboardInput {
                    device_id,
                    event: event.clone(),
                })
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
                phase: _,
            } => {
                let device_id = DeviceId::from(device_id);
                let delta = delta;
                Ok(ViewportEvent::MouseWheel { device_id, delta })
            }
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
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

    #[inline(always)]
    fn try_from(_value: &winit::event::DeviceEvent) -> Result<Self, UnsupportedEvent> {
        Err(UnsupportedEvent)
    }
}

pub trait EventFilter: 'static {
    /// Returns `true` if the event is consumed.
    fn filter(&mut self, blink: &Blink, world: &mut World, event: &Event) -> bool;
}

pub trait IntoEventFilter<M> {
    type EventFilter: EventFilter;

    fn into_event_filter(self) -> Self::EventFilter;
}

pub struct IsEventFilter;

impl<F> IntoEventFilter<IsEventFilter> for F
where
    F: EventFilter,
{
    type EventFilter = F;

    #[inline(always)]
    fn into_event_filter(self) -> F {
        self
    }
}

pub struct EventFilterFn<F>(F);

impl<F> EventFilter for EventFilterFn<F>
where
    F: FnMut(&Event) -> bool + 'static,
{
    #[inline(always)]
    fn filter(&mut self, _blink: &Blink, _world: &mut World, event: &Event) -> bool {
        self.0(event)
    }
}

impl<F> IntoEventFilter<()> for F
where
    F: FnMut(&Event) -> bool + 'static,
{
    type EventFilter = EventFilterFn<F>;

    #[inline(always)]
    fn into_event_filter(self) -> EventFilterFn<F> {
        EventFilterFn(self)
    }
}

pub struct EventFilterWorldFn<F>(F);

impl<F> EventFilter for EventFilterWorldFn<F>
where
    F: FnMut(&mut World, &Event) -> bool + 'static,
{
    #[inline(always)]
    fn filter(&mut self, _blink: &Blink, world: &mut World, event: &Event) -> bool {
        self.0(world, event)
    }
}

impl<F> IntoEventFilter<(&mut World,)> for F
where
    F: FnMut(&mut World, &Event) -> bool + 'static,
{
    type EventFilter = EventFilterWorldFn<F>;

    #[inline(always)]
    fn into_event_filter(self) -> EventFilterWorldFn<F> {
        EventFilterWorldFn(self)
    }
}

pub struct EventFilterBlinkFn<F>(F);

impl<F> EventFilter for EventFilterBlinkFn<F>
where
    F: FnMut(&Blink, &Event) -> bool + 'static,
{
    #[inline(always)]
    fn filter(&mut self, blink: &Blink, _world: &mut World, event: &Event) -> bool {
        self.0(blink, event)
    }
}

impl<F> IntoEventFilter<(&Blink,)> for F
where
    F: FnMut(&Blink, &Event) -> bool + 'static,
{
    type EventFilter = EventFilterBlinkFn<F>;

    #[inline(always)]
    fn into_event_filter(self) -> EventFilterBlinkFn<F> {
        EventFilterBlinkFn(self)
    }
}

pub struct EventFilterBlinkWorldFn<F>(F);

impl<F> EventFilter for EventFilterBlinkWorldFn<F>
where
    F: FnMut(&Blink, &mut World, &Event) -> bool + 'static,
{
    #[inline(always)]
    fn filter(&mut self, blink: &Blink, world: &mut World, event: &Event) -> bool {
        self.0(blink, world, event)
    }
}

impl<F> IntoEventFilter<(&Blink, &mut World)> for F
where
    F: FnMut(&Blink, &mut World, &Event) -> bool + 'static,
{
    type EventFilter = EventFilterBlinkWorldFn<F>;

    #[inline(always)]
    fn into_event_filter(self) -> EventFilterBlinkWorldFn<F> {
        EventFilterBlinkWorldFn(self)
    }
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

    #[cfg_attr(inline_more, inline(always))]
    pub fn add<F>(&mut self, filter: F)
    where
        F: EventFilter,
    {
        self.filters.push(Box::new(filter));
    }

    #[cfg_attr(inline_more, inline(always))]
    pub fn add_boxed(&mut self, filter: Box<dyn EventFilter>) {
        self.filters.push(filter);
    }

    #[cfg_attr(inline_more, inline(always))]
    pub fn filter(&mut self, blink: &Blink, world: &mut World, event: &Event) -> bool {
        for filter in self.filters.iter_mut() {
            if filter.filter(blink, world, event) {
                return true;
            }
        }
        false
    }
}

/// Allow composing `EventFunnel` into super-funnels.
impl EventFilter for EventFunnel {
    #[inline(always)]
    fn filter(&mut self, blink: &Blink, world: &mut World, event: &Event) -> bool {
        self.filter(blink, world, event)
    }
}

// fn is_printable_char(chr: char) -> bool {
//     let is_in_private_use_area = '\u{e000}' <= chr && chr <= '\u{f8ff}'
//         || '\u{f0000}' <= chr && chr <= '\u{ffffd}'
//         || '\u{100000}' <= chr && chr <= '\u{10fffd}';

//     !is_in_private_use_area && !chr.is_ascii_control()
// }
