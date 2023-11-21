//! This module implements main Ed tool - game.
//! Game Tool is responsible for managing game's plugins
//! and run instances of the game.

use std::{cmp::Reverse, collections::BinaryHeap, sync::Arc};

use arcana::{
    const_format,
    edict::world::WorldLocal,
    game::Game,
    mev,
    plugin::ArcanaPlugin,
    project::{Ident, Item, Project},
    texture::Texture,
    ClockStep, Component, Entities, EntityId, World,
};
use parking_lot::Mutex;
use winit::{event::WindowEvent, window::WindowId};

use crate::Tab;

use super::plugins::Plugins;

/// Game instances.
pub struct Games {
    free_ids: BinaryHeap<Reverse<u16>>,
    last_id: u16,
    name_offset: u16,

    /// List of games without viewer.
    headless: Vec<GameId>,
}

#[must_use]
pub struct GameId {
    // Entity of the Game component.
    // Expected to be alive.
    entity: EntityId,

    // Unique game id.
    id: u16,
}

struct LaunchGame;

impl Component for LaunchGame {
    fn name() -> &'static str {
        "LaunchGame"
    }
}

impl Games {
    pub fn new() -> Self {
        Games {
            free_ids: BinaryHeap::new(),
            last_id: 0,
            name_offset: 0,
            headless: Vec::new(),
        }
    }

    fn alloc_id(&mut self) -> u16 {
        match self.free_ids.pop() {
            None => {
                if self.last_id < u16::MAX {
                    self.last_id += 1;
                    self.last_id
                } else {
                    panic!("Can't allocate new game id");
                }
            }
            Some(Reverse(id)) => id,
        }
    }

    fn free_id(&mut self, id: u16) {
        debug_assert!(id <= self.last_id);

        if self.last_id == id {
            self.last_id -= 1;

            // Remove free ids that are at the end of the range.
            while let Some(Reverse(id)) = self.free_ids.peek() {
                debug_assert!(*id <= self.last_id);
                if *id != self.last_id {
                    break;
                }
                self.free_ids.pop();
                self.last_id -= 1;
            }
        } else {
            self.free_ids.push(Reverse(id));
        }
    }

    pub fn new_game(world: &WorldLocal) -> GameId {
        let mut games = world.expect_resource_mut::<Games>();
        let id = games.alloc_id();
        let entity = world.allocate().id();
        let game_id = GameId { entity, id };
        world.insert_defer(entity, LaunchGame);
        game_id
    }

    pub fn detach(world: &WorldLocal, id: GameId) {
        let mut games = world.expect_resource_mut::<Games>();
        games.headless.push(id);
    }

    pub fn stop(world: &WorldLocal, id: GameId) {
        world.despawn_defer(id.entity);
        let mut games = world.expect_resource_mut::<Games>();
        games.free_id(id.id);
    }

    /// Launches a new game instance in its own window.
    fn _launch(world: &mut World) {
        let project = world.expect_resource_mut::<Project>();
        let plugins = world.expect_resource_mut::<Plugins>();
        let Some(active_plugins) = plugins.active_plugins() else {
            return;
        };

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

        let device = world.clone_resource::<mev::Device>();
        let queue = world.clone_resource::<Arc<Mutex<mev::Queue>>>();

        let games = world
            .view::<Entities>()
            .with::<LaunchGame>()
            .into_iter()
            .map(|e| {
                let game = Game::launch(
                    active_plugins.clone(),
                    active_filters.clone(),
                    active_systems.clone(),
                    device.clone(),
                    queue.clone(),
                    None,
                );

                (e.id(), game)
            })
            .collect::<Vec<_>>();

        drop(project);
        drop(plugins);

        for (e, game) in games {
            let _ = world.drop::<LaunchGame>(e);
            let _ = world.insert(e, game);
        }
    }

    pub fn handle_event<'a>(
        world: &mut World,
        window_id: WindowId,
        event: WindowEvent<'a>,
    ) -> Option<WindowEvent<'a>> {
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
            return game.render();
        }
    }

    pub fn tick(world: &mut World, step: ClockStep) {
        Self::_launch(world);

        let mut to_remove = Vec::new();
        for (e, game) in world.view_mut::<(Entities, &mut Game)>() {
            game.tick(step);

            if game.should_quit() {
                to_remove.push(e.id());
            }
        }

        for e in to_remove {
            let _ = world.despawn(e);
        }
    }

    pub fn tab() -> Tab {
        Tab::Game {
            tab: GamesTab::default(),
        }
    }
}

enum OnTabClose {
    Stop,
    Detach,
}

pub struct GamesTab {
    id: Option<GameId>,
    on_close: OnTabClose,
}

impl Default for GamesTab {
    fn default() -> Self {
        Self {
            id: None,
            on_close: OnTabClose::Stop,
        }
    }
}

impl GamesTab {
    pub fn new(world: &WorldLocal) -> Self {
        let id = Games::new_game(world);
        GamesTab {
            id: Some(id),
            on_close: OnTabClose::Stop,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, world: &WorldLocal) {
        let mut game_view;
        let mut game: Option<&mut Game> = match &self.id {
            None => None,
            Some(id) => {
                game_view = world.try_view_one::<&mut Game>(id.entity).unwrap();
                game_view.get_mut()
            }
        };

        let r = ui.horizontal_top(|ui| {
            let mut stop = false;
            let was_enabled = ui.is_enabled();
            ui.set_enabled(game.is_some());

            let r = ui.button(egui_phosphor::regular::PLAY);
            if r.clicked() {
                game.as_mut().unwrap().set_rate_ratio(1, 1);
            }
            let r = ui.button(egui_phosphor::regular::PAUSE);
            if r.clicked() {
                game.as_mut().unwrap().pause();
            }
            let r = ui.button(egui_phosphor::regular::STOP);
            if r.clicked() {
                stop = true;
            }
            let r = ui.button(egui_phosphor::regular::FAST_FORWARD);
            if r.clicked() {
                game.as_mut().unwrap().set_rate_ratio(2, 1);
            }

            let mut rate = game.as_ref().map_or(0.0, |g| g.get_rate());

            let value = egui::Slider::new(&mut rate, 0.0..=10.0);
            let r = ui.add(value);
            if r.changed() {
                game.as_mut().unwrap().set_rate(rate as f32);
            }

            ui.set_enabled(was_enabled);
            stop
        });

        match game {
            None => {
                if self.id.is_none() {
                    ui.vertical_centered(|ui| {
                        ui.label("Game is not running");
                        let r = ui.button(const_format!(
                            "Launch new {}",
                            egui_phosphor::regular::ROCKET_LAUNCH
                        ));
                        if r.clicked() {
                            self.id = Some(Games::new_game(world));
                        }
                    });
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.spinner();
                    });
                }
            }
            Some(game) => {
                let id = self.id.as_ref().unwrap();

                let size = ui.available_size();
                let extent = mev::Extent2::new(size.x as u32, size.y as u32);
                let Ok(image) = game.render_with_texture(extent) else {
                    ui.centered_and_justified(|ui| {
                        ui.label("GPU OOM");
                    });
                    return;
                };

                world.insert_defer(id.entity, Texture { image });

                ui.add(egui::Image::new(egui::load::SizedTexture {
                    id: egui::TextureId::User(id.entity.bits()),
                    size: size.into(),
                }));

                if r.inner {
                    world.drop_defer::<Game>(id.entity);
                }
            }
        }
    }

    pub fn on_close(&mut self, world: &WorldLocal) {
        match self.on_close {
            OnTabClose::Stop => {
                if let Some(id) = self.id.take() {
                    Games::stop(world, id);
                }
            }
            OnTabClose::Detach => {
                if let Some(id) = self.id.take() {
                    Games::detach(world, id);
                }
            }
        }
    }
}
