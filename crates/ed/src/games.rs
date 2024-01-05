//! This module implements main Ed tool - game.
//! Game Tool is responsible for managing game's plugins
//! and run instances of the game.

use std::{cmp::Reverse, collections::BinaryHeap, sync::Arc};

use arcana::{
    const_format,
    edict::world::WorldLocal,
    events::{Event, EventFunnel, ViewportEvent},
    game::{Game, GameInit},
    mev,
    plugin::{ArcanaPlugin, PluginsHub},
    project::{Ident, Project},
    texture::Texture,
    Blink, ClockStep, Component, Entities, EntityId, World,
};
use hashbrown::HashMap;
use parking_lot::Mutex;
use winit::{
    event::WindowEvent,
    window::{Window, WindowId},
};

use crate::{
    data::ProjectData,
    systems::{Category, Systems},
    Tab,
};

use super::plugins::Plugins;

/// Game instances.
pub struct Games {
    /// Currently focused game.
    ///
    /// If set, consumes viewport events and sends them to the game.
    /// Cursor events are transformed by viewport and ignored if cursor is outside of the viewport.
    focus: Option<GameFocus>,
}

struct GameFocus {
    window_id: WindowId,
    rect: egui::Rect,
    pixel_per_point: f32,
    entity: EntityId,
    cursor_inside: bool,
    lock_pointer: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[repr(transparent)]
pub struct GameId {
    entity: EntityId,
}

struct LaunchGame;

impl Component for LaunchGame {
    fn name() -> &'static str {
        "LaunchGame"
    }
}

impl Games {
    pub fn new() -> Self {
        Games { focus: None }
    }

    fn is_focused(&self, id: GameId) -> bool {
        self.focus.as_ref().map_or(false, |f| f.entity == id.entity)
    }

    pub fn new_game(world: &WorldLocal) -> GameId {
        let mut games = world.expect_resource_mut::<Games>();
        games._new_game(world)
    }

    fn _new_game(&mut self, world: &WorldLocal) -> GameId {
        let entity = world.allocate().id();
        world.insert_defer(entity, LaunchGame);
        tracing::info!("Game {entity} starting");
        GameId { entity }
    }

    pub fn detach(world: &WorldLocal, id: GameId) {
        let mut games = world.expect_resource_mut::<Games>();

        if games.is_focused(id) {
            games.focus = None;
        }
    }

    pub fn stop(world: &WorldLocal, id: GameId) {
        let mut games = world.expect_resource_mut::<Games>();
        games._stop(world, id);
    }

    fn _stop(&mut self, world: &WorldLocal, id: GameId) {
        if self.is_focused(id) {
            self.focus = None;
        }

        world.despawn_defer(id.entity);
        tracing::info!("Game {} stopped", id.entity);
    }

    /// Launches requested games.
    pub fn launch_games(world: &mut WorldLocal) {
        if world.new_view_mut().with::<LaunchGame>().iter().count() == 0 {
            return;
        }

        tracing::info!("Launching games");

        let plugins = world.expect_resource_mut::<Plugins>();

        let Some(active_plugins) = plugins.active_plugins() else {
            return;
        };
        let active_plugins = active_plugins.collect::<Vec<_>>();

        let project = world.expect_resource_mut::<Project>();
        let data = world.expect_resource::<ProjectData>();
        let device = world.expect_resource::<mev::Device>();
        let queue = world.expect_resource::<Arc<Mutex<mev::Queue>>>();
        let systems = world.expect_resource::<Systems>();

        let games = world
            .view::<Entities>()
            .with::<LaunchGame>()
            .into_iter()
            .map(|e| {
                tracing::info!("Launching game {e}");

                let mut hub = PluginsHub::new();

                let init = |game_world: &mut World| {
                    for (_, plugin) in &active_plugins {
                        plugin.init(game_world, &mut hub);
                    }

                    let mut scheduler = systems.scheduler(&data, hub.systems);

                    let scheduler = Box::new(move |game_world: &mut World, fixed: bool| {
                        let cat = match fixed {
                            true => Category::Fix,
                            false => Category::Var,
                        };
                        scheduler(game_world, cat);
                    });

                    let funnel = Box::new(
                        move |blink: &Blink, game_world: &mut World, event: &Event| {
                            for (_, filter) in &mut hub.filters {
                                if filter.filter(blink, game_world, event) {
                                    return true;
                                }
                            }
                            false
                        },
                    );

                    GameInit { scheduler, funnel }
                };

                let game = Game::launch(init, (*device).clone(), (*queue).clone(), None);

                (e.id(), game)
            })
            .collect::<Vec<_>>();

        drop((project, plugins, data, device, queue, systems));

        for (e, game) in games {
            let _ = world.drop::<LaunchGame>(e);
            let _ = world.insert(e, game);
        }
    }

    pub fn handle_event<'a>(
        world: &mut World,
        window_id: WindowId,
        event: &WindowEvent<'a>,
    ) -> bool {
        let world = world.local();
        for game in world.view_mut::<&mut Game>() {
            if game.window_id() == Some(window_id) {
                if let Ok(event) = ViewportEvent::try_from(event) {
                    if game.on_event(&Event::ViewportEvent { event }) {
                        return true;
                    }
                }

                if let WindowEvent::CloseRequested = event {
                    game.quit();
                    return true;
                }
            }
        }

        let mut games = world.expect_resource_mut::<Games>();
        if let Some(focus) = &mut games.focus {
            if focus.window_id == window_id {
                if let Ok(event) = ViewportEvent::try_from(event) {
                    let mut consume = true;
                    let mut game_view = world.try_view_one::<&mut Game>(focus.entity).unwrap();
                    let game = game_view.get_mut().unwrap();

                    match event {
                        ViewportEvent::CursorEntered { .. } => return false,
                        ViewportEvent::CursorLeft { .. } => return false,
                        ViewportEvent::CursorMoved { device_id, x, y } => {
                            let px = x / focus.pixel_per_point;
                            let py = y / focus.pixel_per_point;

                            let gx = px - focus.rect.min.x;
                            let gy = py - focus.rect.min.y;

                            if focus.rect.contains(egui::pos2(px, py)) {
                                if !focus.cursor_inside {
                                    focus.cursor_inside = true;

                                    game.on_event(&Event::ViewportEvent {
                                        event: ViewportEvent::CursorEntered { device_id },
                                    });
                                }

                                game.on_event(&Event::ViewportEvent {
                                    event: ViewportEvent::CursorMoved {
                                        device_id,
                                        x: gx,
                                        y: gy,
                                    },
                                });
                            } else {
                                consume = false;

                                if focus.cursor_inside {
                                    focus.cursor_inside = false;
                                    game.on_event(&Event::ViewportEvent {
                                        event: ViewportEvent::CursorLeft { device_id },
                                    });
                                }
                            }
                        }
                        ViewportEvent::MouseWheel { .. } if !focus.cursor_inside => {
                            consume = false;
                        }
                        ViewportEvent::MouseInput { .. } if !focus.cursor_inside => {
                            consume = false;
                        }
                        ViewportEvent::Resized { .. }
                        | ViewportEvent::ScaleFactorChanged { .. } => {
                            consume = false;
                        }
                        ViewportEvent::KeyboardInput { input, .. }
                            if input.virtual_keycode
                                == Some(winit::event::VirtualKeyCode::Escape) =>
                        {
                            games.focus = None;
                        }
                        event => {
                            game.on_event(&Event::ViewportEvent { event });
                        }
                    }

                    return consume;
                }
            }
        }

        false
    }

    pub fn render(world: &mut World) {
        for game in world.view_mut::<&mut Game>() {
            return game.render();
        }
    }

    pub fn tick(world: &mut World, step: ClockStep) {
        Self::launch_games(world.local());

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

    pub fn show(&mut self, ui: &mut egui::Ui, world: &WorldLocal, window: &Window) {
        let mut games = world.expect_resource_mut::<Games>();

        let mut game_view;
        let mut game: Option<&mut Game> = match &self.id {
            None => None,
            Some(id) => {
                game_view = world.try_view_one::<&mut Game>(id.entity).unwrap();
                game_view.get_mut()
            }
        };

        let mut stop = false;
        let r = ui.horizontal_top(|ui| {
            let cbox = egui::ComboBox::from_id_source("games-list");

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
        });

        if stop {
            games._stop(world, self.id.take().unwrap());
            game = None;
        }

        // let r = ui.horizontal_top(|ui| {
        //     let r = ui.button(const_format!(
        //         "{} {}",
        //         egui_phosphor::regular::CURSOR,
        //         egui_phosphor::regular::LOCK
        //     ));

        //     if r.clicked() {

        //     }
        // });

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
                            self.id = Some(games._new_game(world));
                        }
                    });
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.spinner();
                    });
                }
            }
            Some(game) => {
                let id = self.id.unwrap();

                let focused = games.is_focused(id);

                let game_frame = egui::Frame::none()
                    .rounding(egui::Rounding::same(5.0))
                    .stroke(egui::Stroke::new(
                        1.0,
                        if focused {
                            egui::Color32::LIGHT_GRAY
                        } else {
                            egui::Color32::DARK_GRAY
                        },
                    ))
                    .inner_margin(egui::Margin::same(10.0));

                game_frame.show(ui, |ui| {
                    let size = ui.available_size();
                    let extent = mev::Extent2::new(size.x as u32, size.y as u32);
                    let Ok(image) = game.render_with_texture(extent) else {
                        ui.centered_and_justified(|ui| {
                            ui.label("GPU OOM");
                        });
                        return;
                    };

                    world.insert_defer(id.entity, Texture { image });

                    let image = egui::Image::new(egui::load::SizedTexture {
                        id: egui::TextureId::User(id.entity.bits()),
                        size: size.into(),
                    });

                    let r = ui.add(image.sense(egui::Sense::click()));

                    if focused {
                        if !r.has_focus() {
                            tracing::info!("Game {} lost focus", id.entity);
                            games.focus = None;
                        } else {
                            let focus = games.focus.as_mut().unwrap();

                            focus.rect = r.rect;
                            focus.pixel_per_point = ui.ctx().pixels_per_point();
                        }
                    } else {
                        if r.has_focus() {
                            r.surrender_focus();
                        }

                        let mut make_focused = false;
                        if r.clicked() {
                            r.request_focus();
                            make_focused = !focused
                        }

                        if make_focused {
                            tracing::info!("Game {} focused", id.entity);

                            games.focus = Some(GameFocus {
                                window_id: window.id(),
                                rect: r.rect,
                                pixel_per_point: ui.ctx().pixels_per_point(),
                                entity: id.entity,
                                cursor_inside: false,
                                lock_pointer: false,
                            });
                        }
                    }
                });
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
