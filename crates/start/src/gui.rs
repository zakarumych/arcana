use std::path::PathBuf;

use arcana::{
    app::Application,
    blink_alloc::BlinkAlloc,
    edict::World,
    egui::{self, EguiRender, EguiResource},
    events::{Event, EventLoop},
    init_mev, mev,
    render::{render, RenderGraph, RenderResources},
};
use winit::window::Window;

struct Recent {
    name: String,
    path: PathBuf,
}

/// Arcn app instance.
pub struct App {
    /// Windows opened in the editor.
    window: Window,
    recent: Vec<Recent>,
    should_quit: bool,

    graph: RenderGraph,
    resources: RenderResources,
    device: mev::Device,
    queue: mev::Queue,
    world: World,

    blink: BlinkAlloc,
}

impl Application for App {
    fn on_event(&mut self, event: Event) -> Option<Event> {
        match event {
            Event::WindowEvent {
                event: winit::event::WindowEvent::CloseRequested,
                ..
            } => {
                self.should_quit = true;
                None
            }
            _ => Some(event),
        }
    }

    fn render(&mut self) {
        render(
            &mut self.graph,
            &self.device,
            &mut self.queue,
            &self.blink,
            None,
            std::iter::once(&self.window),
            &self.world,
            &mut self.resources,
        );
    }

    fn should_quit(&self) -> bool {
        self.should_quit
    }

    fn tick(&mut self, _events: &EventLoop) {
        let mut egui = self
            .world
            .remove_resource::<EguiResource>()
            .expect("EguiResource must be present");

        egui.run(&self.window, |cx| {
            if !self.recent.is_empty() {
                egui::SidePanel::left("Recent").show(cx, |ui| {});
            }

            egui::CentralPanel::default().show(cx, |ui| {
                ui.heading("Arcana");
            });
        });

        self.world.insert_resource(egui);
    }
}

fn main() -> miette::Result<()> {
    arcana::app::try_run(|events, _collector| {
        let window = winit::window::WindowBuilder::new()
            .with_title("Arcana")
            .build(events)
            .unwrap();

        let mut egui = EguiResource::new();
        egui.add_window(&window, events);

        let mut graph = RenderGraph::new();
        let target =
            EguiRender::build(&mut graph, window.id(), mev::ClearColor(0.2, 0.2, 0.2, 1.0));
        graph.present(target, window.id());

        let mut world = World::new();
        world.insert_resource(egui);

        let (device, queue) = init_mev();

        App {
            window,
            recent: Vec::new(),
            should_quit: false,

            graph,
            resources: RenderResources::default(),
            device,
            queue,
            world,

            blink: BlinkAlloc::new(),
        }
    })
}
