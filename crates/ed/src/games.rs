//! This module implements main Ed tool - game.
//! Game Tool is responsible for managing game's plugins
//! and run instances of the game.

use std::sync::Arc;

use arcana::{
    game::Game,
    mev,
    plugin::ArcanaPlugin,
    project::{Ident, Item, Project},
    texture::Texture,
    ClockStep, Entities, EntityId, World,
};
use parking_lot::Mutex;
use winit::{
    event::WindowEvent,
    window::{WindowBuilder, WindowId},
};

use crate::{app::EventLoop, Tab};

use super::plugins::Plugins;

/// Game instances.
pub struct Games;

pub struct LaunchGame;

impl Games {
    /// Launches a new game instance in its own window.
    pub fn launch(
        events: &EventLoop,
        world: &mut World,
        device: &mev::Device,
        queue: &Arc<Mutex<mev::Queue>>,
        windowed: bool,
    ) -> Option<EntityId> {
        let world = world.local();
        let project = world.expect_resource_mut::<Project>();
        let plugins = world.expect_resource_mut::<Plugins>();
        let active_plugins = plugins.active_plugins()?;

        let active = |exported: fn(&dyn ArcanaPlugin) -> &[&Ident]| {
            let plugins = &plugins;
            move |item: &&Item| {
                if !item.enabled {
                    return false;
                }

                if !plugins.is_active(&item.plugin) {
                    return false;
                }

                let plugin = plugins.get_plugin(&item.plugin).unwrap();
                if !exported(plugin).iter().any(|name| **name == *item.name) {
                    return false;
                }
                true
            }
        };

        let active_systems = project
            .manifest()
            .systems
            .iter()
            .filter(active(|p| p.systems()))
            .map(|i| (&*i.plugin, &*i.name));

        let active_filters = project
            .manifest()
            .filters
            .iter()
            .filter(active(|p| p.filters()))
            .map(|i| (&*i.plugin, &*i.name));

        let window = match windowed {
            false => None,
            true => Some(
                WindowBuilder::new()
                    .with_title("Arcana Game")
                    .build(events)
                    .unwrap(),
            ),
        };

        let game = Game::launch(
            active_plugins,
            active_filters,
            active_systems,
            device.clone(),
            queue.clone(),
            window,
        );

        Some(world.spawn_one(game).id())
    }

    pub fn handle_event(
        world: &mut World,
        window_id: WindowId,
        event: WindowEvent<'static>,
    ) -> Option<WindowEvent<'static>> {
        for game in world.view_mut::<&mut Game>() {
            if game.window_id() == Some(window_id) {
                // return game.handle_event(event);
                return None;
            }
        }
        Some(event)
    }

    pub fn render(world: &mut World) {
        for game in world.view_mut::<&mut Game>() {
            if game.window_id().is_some() {
                return game.render();
            }
        }
    }

    pub fn tick(world: &mut World, step: ClockStep) {
        let mut to_remove = Vec::new();
        for (e, game) in world.view_mut::<(Entities, &mut Game)>() {
            game.tick(step);

            if game.should_quit() {
                to_remove.push(e.id());
            }
        }

        for e in to_remove {
            world.despawn(e);
        }
    }

    pub fn show(world: &mut World, ui: &mut egui::Ui) {
        let world = world.local();

        let mut id = None;
        for (e, g) in world.view_mut::<(Entities, &Game)>() {
            if g.window_id().is_none() {
                id = Some(e.id());
                break;
            }
        }

        let Some(e) = id else {
            let r = ui.horizontal_top(|ui| {
                let mut stop = false;

                let r = ui.button(egui_phosphor::regular::PLAY);
                if r.clicked() {
                    world.insert_resource(LaunchGame);
                }

                let was_enabled = ui.is_enabled();
                ui.set_enabled(false);
                ui.button(egui_phosphor::regular::PAUSE);
                ui.button(egui_phosphor::regular::STOP);
                ui.button(egui_phosphor::regular::FAST_FORWARD);
                let value = egui::Slider::new(&mut 0.0, 0.0..=10.0);
                ui.add(value);
                ui.set_enabled(was_enabled);
            });
            return;
        };

        let game = world.get::<&mut Game>(e).unwrap();

        let r = ui.horizontal_top(|ui| {
            let mut stop = false;

            let r = ui.button(egui_phosphor::regular::PLAY);
            if r.clicked() {
                game.set_rate_ratio(1, 1);
            }
            let r = ui.button(egui_phosphor::regular::PAUSE);
            if r.clicked() {
                game.pause();
            }
            let r = ui.button(egui_phosphor::regular::STOP);
            if r.clicked() {
                stop = true;
            }
            let r = ui.button(egui_phosphor::regular::FAST_FORWARD);
            if r.clicked() {
                game.set_rate_ratio(2, 1);
            }

            let mut rate = game.get_rate();

            let value = egui::Slider::new(&mut rate, 0.0..=10.0);
            let r = ui.add(value);
            if r.changed() {
                game.set_rate(rate as f32);
            }
            stop
        });

        let size = ui.available_size();
        let extent = mev::Extent2::new(size.x as u32, size.y as u32);
        let Ok(image) = game.render_with_texture(extent) else {
            ui.centered_and_justified(|ui| {
                ui.label("GPU OOM");
            });
            return;
        };

        world.insert(e, Texture { image });

        ui.add(egui::Image::new(egui::load::SizedTexture {
            id: egui::TextureId::User(e.bits()),
            size: size.into(),
        }));

        if r.inner {
            world.despawn(e);
        }
    }

    pub fn tab() -> Tab {
        Tab::Game
    }
}
