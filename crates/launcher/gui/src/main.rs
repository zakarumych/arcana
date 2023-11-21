use std::{path::PathBuf, sync::Arc};

use arcana::{
    blink_alloc::BlinkAlloc,
    edict::world::WorldLocal,
    events::ViewportEvent,
    game::Quit,
    init_mev, mev,
    project::Project,
    render::{render, RenderGraph, RenderResources},
    Clock, Entities, With, WorldBuilder,
};
use arcana_egui::{Context, Egui, EguiRender, TopBottomPanel, Ui, WidgetText};
use egui::vec2;
use egui_tracing::EventCollector;
use hashbrown::HashMap;
use winit::{
    dpi,
    event::WindowEvent,
    window::{Window, WindowBuilder, WindowId},
};

pub enum UserEvent {}

pub type Event<'a> = winit::event::Event<'a, UserEvent>;
pub type EventLoop = winit::event_loop::EventLoop<UserEvent>;
pub type EventLoopWindowTarget = winit::event_loop::EventLoopWindowTarget<UserEvent>;

/// Editor app instance.
/// Contains state of the editor.
pub struct App {
    // App state is stored in World.
    world: WorldLocal,

    graph: RenderGraph,
    resources: RenderResources,
    device: mev::Device,
    queue: mev::Queue,
    blink: BlinkAlloc,
}

impl App {
    pub fn new(events: &EventLoop, event_collector: EventCollector) -> Self {
        let (device, queue) = init_mev();

        let mut builder = WorldBuilder::new();
        builder.register_external::<Window>();
        builder.register_component::<Egui>();
        builder.register_external::<mev::Surface>();

        let mut world = builder.build_local();

        let mut graph = RenderGraph::new();

        App {
            world,
            graph,
            resources: RenderResources::default(),
            device,
            queue,
            blink: BlinkAlloc::new(),
        }
    }

    pub fn on_event<'a>(&mut self, event: Event<'a>, events: &EventLoopWindowTarget) {
        match event {
            Event::WindowEvent { window_id, event } => {
                let world = self.world.local();

                for (w, egui) in world.view_mut::<(&Window, &mut Egui)>() {
                    if w.id() == window_id {
                        if let Ok(event) = ViewportEvent::try_from(&event) {
                            egui.handle_event(&event);
                        }
                    }
                }

                match event {
                    WindowEvent::CloseRequested => {
                        let mut drop_windows = Vec::new();
                        for (e, w) in world.view_mut::<(Entities, &Window)>() {
                            if w.id() == window_id {
                                drop_windows.push(e.id());
                            }
                        }
                        for e in drop_windows {
                            let _ = world.despawn(e);
                        }
                    }
                    _ => {}
                }
            }
            Event::RedrawEventsCleared => {
                self.render();
            }
            _ => {}
        }
    }

    pub fn render(&mut self) {
        if self.world.view_mut::<With<Window>>().into_iter().count() == 0 {
            return;
        }

        render(
            &mut self.graph,
            &self.device,
            &mut self.queue,
            &self.blink,
            None,
            &mut self.world,
            &mut self.resources,
        );
    }

    pub fn should_quit(&self) -> bool {
        self.world.get_resource::<Quit>().is_some()
    }
}

fn main() {}
