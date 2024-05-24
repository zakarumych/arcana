//! OS events handling.

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
pub enum Input {
    /// Event emitted from a viewport.
    ViewportInput { input: ViewportInput },

    /// Event emitted from a device.
    DeviceInput {
        device: DeviceId,
        event: DeviceInput,
    },
}

#[derive(Clone)]
pub enum ViewportInput {
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
}

pub struct UnsupportedEvent;

impl TryFrom<&WindowEvent> for ViewportInput {
    type Error = UnsupportedEvent;

    fn try_from(event: &WindowEvent) -> Result<Self, UnsupportedEvent> {
        match *event {
            WindowEvent::Resized(size) => {
                let width = size.width;
                let height = size.height;
                Ok(ViewportInput::Resized { width, height })
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                Ok(ViewportInput::ScaleFactorChanged {
                    scale_factor: scale_factor as f32,
                })
            }
            WindowEvent::KeyboardInput {
                device_id,
                ref event,
                ..
            } => {
                let device_id = DeviceId::from(device_id);
                Ok(ViewportInput::KeyboardInput {
                    device_id,
                    event: event.clone(),
                })
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                Ok(ViewportInput::ModifiersChanged(modifiers))
            }
            WindowEvent::CursorMoved {
                device_id,
                position,
                ..
            } => {
                let device_id = DeviceId::from(device_id);
                let x = position.x as f32;
                let y = position.y as f32;
                Ok(ViewportInput::CursorMoved { device_id, x, y })
            }
            WindowEvent::CursorEntered { device_id } => {
                let device_id = DeviceId::from(device_id);
                Ok(ViewportInput::CursorEntered { device_id })
            }
            WindowEvent::CursorLeft { device_id } => {
                let device_id = DeviceId::from(device_id);
                Ok(ViewportInput::CursorLeft { device_id })
            }
            WindowEvent::MouseWheel {
                device_id,
                delta,
                phase: _,
            } => {
                let device_id = DeviceId::from(device_id);
                let delta = delta;
                Ok(ViewportInput::MouseWheel { device_id, delta })
            }
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => {
                let device_id = DeviceId::from(device_id);
                let state = state;
                let button = button;
                Ok(ViewportInput::MouseInput {
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
pub enum DeviceInput {}

impl TryFrom<&winit::event::DeviceEvent> for DeviceInput {
    type Error = UnsupportedEvent;

    #[inline(always)]
    fn try_from(_value: &winit::event::DeviceEvent) -> Result<Self, UnsupportedEvent> {
        Err(UnsupportedEvent)
    }
}

pub trait InputFilter: 'static {
    /// Returns `true` if the event is consumed.
    fn filter(&mut self, blink: &Blink, world: &mut World, event: &Input) -> bool;
}

pub trait IntoInputFilter<M> {
    type InputFilter: InputFilter;

    fn into_input_filter(self) -> Self::InputFilter;
}

pub struct IsInputFilter;

impl<F> IntoInputFilter<IsInputFilter> for F
where
    F: InputFilter,
{
    type InputFilter = F;

    #[inline(always)]
    fn into_input_filter(self) -> F {
        self
    }
}

pub struct InputFilterFn<F>(F);

impl<F> InputFilter for InputFilterFn<F>
where
    F: FnMut(&Input) -> bool + 'static,
{
    #[inline(always)]
    fn filter(&mut self, _blink: &Blink, _world: &mut World, event: &Input) -> bool {
        self.0(event)
    }
}

impl<F> IntoInputFilter<()> for F
where
    F: FnMut(&Input) -> bool + 'static,
{
    type InputFilter = InputFilterFn<F>;

    #[inline(always)]
    fn into_input_filter(self) -> InputFilterFn<F> {
        InputFilterFn(self)
    }
}

pub struct InputFilterWorldFn<F>(F);

impl<F> InputFilter for InputFilterWorldFn<F>
where
    F: FnMut(&mut World, &Input) -> bool + 'static,
{
    #[inline(always)]
    fn filter(&mut self, _blink: &Blink, world: &mut World, event: &Input) -> bool {
        self.0(world, event)
    }
}

impl<F> IntoInputFilter<(&mut World,)> for F
where
    F: FnMut(&mut World, &Input) -> bool + 'static,
{
    type InputFilter = InputFilterWorldFn<F>;

    #[inline(always)]
    fn into_input_filter(self) -> InputFilterWorldFn<F> {
        InputFilterWorldFn(self)
    }
}

pub struct InputFilterBlinkFn<F>(F);

impl<F> InputFilter for InputFilterBlinkFn<F>
where
    F: FnMut(&Blink, &Input) -> bool + 'static,
{
    #[inline(always)]
    fn filter(&mut self, blink: &Blink, _world: &mut World, event: &Input) -> bool {
        self.0(blink, event)
    }
}

impl<F> IntoInputFilter<(&Blink,)> for F
where
    F: FnMut(&Blink, &Input) -> bool + 'static,
{
    type InputFilter = InputFilterBlinkFn<F>;

    #[inline(always)]
    fn into_input_filter(self) -> InputFilterBlinkFn<F> {
        InputFilterBlinkFn(self)
    }
}

pub struct InputFilterBlinkWorldFn<F>(F);

impl<F> InputFilter for InputFilterBlinkWorldFn<F>
where
    F: FnMut(&Blink, &mut World, &Input) -> bool + 'static,
{
    #[inline(always)]
    fn filter(&mut self, blink: &Blink, world: &mut World, event: &Input) -> bool {
        self.0(blink, world, event)
    }
}

impl<F> IntoInputFilter<(&Blink, &mut World)> for F
where
    F: FnMut(&Blink, &mut World, &Input) -> bool + 'static,
{
    type InputFilter = InputFilterBlinkWorldFn<F>;

    #[inline(always)]
    fn into_input_filter(self) -> InputFilterBlinkWorldFn<F> {
        InputFilterBlinkWorldFn(self)
    }
}

pub struct InputFunnel {
    pub filters: Vec<Box<dyn InputFilter>>,
}

impl InputFunnel {
    pub const fn new() -> Self {
        InputFunnel {
            filters: Vec::new(),
        }
    }

    #[cfg_attr(inline_more, inline(always))]
    pub fn add<F>(&mut self, filter: F)
    where
        F: InputFilter,
    {
        self.filters.push(Box::new(filter));
    }

    #[cfg_attr(inline_more, inline(always))]
    pub fn add_boxed(&mut self, filter: Box<dyn InputFilter>) {
        self.filters.push(filter);
    }

    #[cfg_attr(inline_more, inline(always))]
    pub fn filter(&mut self, blink: &Blink, world: &mut World, event: &Input) -> bool {
        for filter in self.filters.iter_mut() {
            if filter.filter(blink, world, event) {
                return true;
            }
        }
        false
    }
}

/// Allow composing `InputFunnel` into super-funnels.
impl InputFilter for InputFunnel {
    #[inline(always)]
    fn filter(&mut self, blink: &Blink, world: &mut World, event: &Input) -> bool {
        self.filter(blink, world, event)
    }
}

// fn is_printable_char(chr: char) -> bool {
//     let is_in_private_use_area = '\u{e000}' <= chr && chr <= '\u{f8ff}'
//         || '\u{f0000}' <= chr && chr <= '\u{ffffd}'
//         || '\u{100000}' <= chr && chr <= '\u{10fffd}';

//     !is_in_private_use_area && !chr.is_ascii_control()
// }
