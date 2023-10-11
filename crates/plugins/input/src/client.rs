use std::collections::VecDeque;

use arcana::{
    blink_alloc::Blink,
    edict::{EntityId, NoSuchEntity, Scheduler, World},
    events::{
        DeviceId, ElementState, Event, KeyboardInput, MouseButton, ScanCode, VirtualKeyCode,
        WindowEvent,
    },
    funnel::{EventFilter, EventFunnel},
    winit::window::WindowId,
};
use hashbrown::HashMap;

use crate::CommandQueue;

struct InputFilter;

impl EventFilter for InputFilter {
    fn filter(&mut self, _: &Blink, world: &mut World, event: Event) -> Option<Event> {
        world
            .expect_resource_mut::<InputHandler>()
            .handle(world, event)
    }
}

/// Choses which controller to dispatch events to.
pub struct InputHandler {
    /// Dispatch events from this device to this controller.
    device: HashMap<DeviceId, Box<dyn Controller>>,

    /// Dispatch events from this window to this controller.
    window: HashMap<WindowId, Box<dyn Controller>>,

    /// Dispatch any input event to this controller if
    /// no more specific controller is found for it.
    global: Option<Box<dyn Controller>>,
}

impl InputHandler {
    pub fn new() -> Self {
        InputHandler {
            device: HashMap::new(),
            window: HashMap::new(),
            global: None,
        }
    }

    pub fn add_global_controller(&mut self, controller: Box<dyn Controller>) {
        self.global = Some(controller);
    }

    pub fn add_device_controller(&mut self, device: DeviceId, controller: Box<dyn Controller>) {
        self.device.insert(device, controller);
    }

    pub fn add_window_controller(&mut self, window: WindowId, controller: Box<dyn Controller>) {
        self.window.insert(window, controller);
    }

    pub fn handle(&mut self, world: &World, event: Event) -> Option<Event> {
        match event {
            Event::WindowEvent { window_id, event } => match event {
                WindowEvent::KeyboardInput {
                    device_id, input, ..
                } => {
                    if let Some(controller) = self.device.get_mut(&DeviceId::from(device_id)) {
                        controller.on_keyboard_input(world, &input);
                        None
                    } else if let Some(controller) = self.window.get_mut(&window_id) {
                        controller.on_keyboard_input(world, &input);
                        None
                    } else if let Some(controller) = &mut self.global {
                        controller.on_keyboard_input(world, &input);
                        None
                    } else {
                        Some(Event::WindowEvent { window_id, event })
                    }
                }
                _ => Some(Event::WindowEvent { window_id, event }),
            },
            _ => Some(event),
        }
    }
}

/// Consumer of input events.
/// When added to InputHandler it may be associated with
/// a specific device or window.
pub trait Controller: Send {
    fn on_keyboard_input(&mut self, world: &World, input: &KeyboardInput) {
        let _ = (world, input);
    }
    fn on_mouse_button(&mut self, world: &World, button: MouseButton, state: ElementState) {
        let _ = (world, button, state);
    }
    fn on_mouse_move(&mut self, world: &World, x: f64, y: f64) {
        let _ = (world, x, y);
    }
}

pub trait Translator: Send {
    type Action;

    fn on_keyboard_input(&mut self, input: &KeyboardInput) -> Option<Self::Action> {
        let _ = input;
        None
    }
    fn on_mouse_button(
        &mut self,
        button: MouseButton,
        state: ElementState,
    ) -> Option<Self::Action> {
        let _ = (button, state);
        None
    }
    fn on_mouse_move(&mut self, x: f64, y: f64) -> Option<Self::Action> {
        let _ = (x, y);
        None
    }
}

pub struct Mapper<A> {
    keyboard_map: HashMap<VirtualKeyCode, A>,
    scancode_map: HashMap<ScanCode, A>,
    mouse_map: HashMap<MouseButton, A>,
    move_map: fn(f64, f64) -> Option<A>,
}

impl<A> Translator for Mapper<A>
where
    A: Clone + Send,
{
    type Action = A;

    fn on_keyboard_input(&mut self, input: &KeyboardInput) -> Option<A> {
        if let Some(action) = input
            .virtual_keycode
            .and_then(|code| self.keyboard_map.get(&code))
        {
            return Some(action.clone());
        }
        if let Some(action) = self.scancode_map.get(&input.scancode) {
            return Some(action.clone());
        }
        None
    }

    fn on_mouse_button(&mut self, button: MouseButton, state: ElementState) -> Option<A> {
        self.mouse_map.get(&button).cloned()
    }

    fn on_mouse_move(&mut self, x: f64, y: f64) -> Option<A> {
        (self.move_map)(x, y)
    }
}

pub struct Commander<T> {
    translator: T,
    entity: EntityId,
}

impl<T> Commander<T>
where
    T: Translator,
    T::Action: Send + 'static,
{
    fn send(&self, world: &World, action: T::Action) {
        if let Ok(one) = world.try_view_one::<&mut CommandQueue<T::Action>>(self.entity) {
            if let Some(queue) = one.get() {
                queue.actions.push_back(action);
            }
        }
    }
}

impl<T> Controller for Commander<T>
where
    T: Translator,
    T::Action: Send + 'static,
{
    fn on_keyboard_input(&mut self, world: &World, input: &KeyboardInput) {
        if let Some(action) = self.translator.on_keyboard_input(input) {
            self.send(world, action);
        }
    }

    fn on_mouse_button(&mut self, world: &World, button: MouseButton, state: ElementState) {
        if let Some(action) = self.translator.on_mouse_button(button, state) {
            self.send(world, action);
        }
    }

    fn on_mouse_move(&mut self, world: &World, x: f64, y: f64) {
        if let Some(action) = self.translator.on_mouse_move(x, y) {
            self.send(world, action);
        }
    }
}

pub fn new_commander<T>(
    translator: T,
    entity: EntityId,
    world: &mut World,
) -> Result<Commander<T>, NoSuchEntity>
where
    T: Translator,
    T::Action: Send + 'static,
{
    let commander = Commander { translator, entity };
    let queue = CommandQueue::<T::Action> {
        actions: VecDeque::new(),
    };
    world.insert(entity, queue)?;
    Ok(commander)
}

pub fn init(world: &mut World, _scheduler: &mut Scheduler) {
    world.insert_resource(InputHandler::new());
}

pub fn init_funnel(funnel: &mut EventFunnel) {
    funnel.add(InputFilter);
}
