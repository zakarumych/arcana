pub use ::egui::*;
use blink_alloc::Blink;
use edict::World;
use hashbrown::HashMap;
use winit::{event_loop::EventLoopWindowTarget, window::WindowId};

use crate::{events::Event, funnel::Filter};

struct EguiInstance {
    ctx: Context,
    state: egui_winit::State,
    textures_delta: epaint::textures::TexturesDelta,
    shapes: Vec<epaint::ClippedShape>,
}

pub struct EguiResource {
    instances: HashMap<WindowId, EguiInstance>,
}

impl EguiResource {
    pub fn new() -> Self {
        EguiResource {
            instances: HashMap::new(),
        }
    }

    pub fn add_window<T>(&mut self, window_id: WindowId, event_loop: &EventLoopWindowTarget<T>) {
        self.instances.insert(
            window_id,
            EguiInstance {
                ctx: Context::default(),
                state: egui_winit::State::new(event_loop),
                textures_delta: epaint::textures::TexturesDelta::default(),
                shapes: Vec::new(),
            },
        );
    }

    pub fn run(&mut self, window: &winit::window::Window, run_ui: impl FnOnce(&Context)) {
        let Some(instance) = self.instances.get_mut(&window.id()) else { return; };

        let raw_input = instance.state.take_egui_input(window);
        instance.ctx.begin_frame(raw_input);
        run_ui(&instance.ctx);
        let output = instance.ctx.end_frame();

        instance
            .state
            .handle_platform_output(window, &instance.ctx, output.platform_output);

        instance.textures_delta.append(output.textures_delta);
        instance.shapes = output.shapes;
    }

    pub fn render(&mut self, window_id: WindowId) -> (TexturesDelta, Vec<epaint::ClippedShape>) {
        let instance = self.instances.get_mut(&window_id).unwrap();

        let textures_delta = std::mem::take(&mut instance.textures_delta);
        let shapes = std::mem::take(&mut instance.shapes);

        (textures_delta, shapes)
    }
}

pub struct EguiFunnel;

impl Filter for EguiFunnel {
    fn filter(&mut self, _blink: &Blink, world: &mut World, event: Event) -> Option<Event> {
        let egui = &mut *world.expect_resource_mut::<EguiResource>();

        if let Event::WindowEvent { window_id, event } = &event {
            if let Some(instance) = egui.instances.get_mut(window_id) {
                let response = instance.state.on_event(&instance.ctx, event);

                if response.consumed {
                    return None;
                }
            }
        }

        Some(event)
    }
}
