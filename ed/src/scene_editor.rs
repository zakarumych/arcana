//! Authored scene data and independent simulation sessions.

use arcana::{
    ecs::{entity::EntityId, flow::init_flows, query::Entities, world::World},
    error::Error,
    gametime::{ClockStep, TimeSpan, TimeStamp},
    hash::HashSet,
    id::{HasStid, Stid},
    model::{Model, Reflect, Value},
    plugin::{ComponentDesc, PluginsHub},
    render::{RenderGraphId, Renderer},
};
use winit::{event::WindowEvent, window::WindowId};

use crate::{
    entity_ids::EntityIds,
    model::{ReferenceChoices, ValueProbe},
    plugins::Plugins,
    render::RenderManager,
    render_view::RenderView,
    simulation::{Simulation, SimulationWidget},
    tool::{Tool, ToolContext, ToolTemplate},
    ui::UserTextures,
};

/// Presence marks an authored entity; only listed components belong to its scene data.
#[derive(arcana::Component, Default)]
pub struct SceneComponents {
    components: HashSet<Stid>,
}

#[derive(Debug, PartialEq)]
struct SceneEntity {
    entity: EntityId,
    components: Vec<(Stid, Value)>,
}

#[derive(Debug, PartialEq)]
struct SceneSnapshot {
    entities: Vec<SceneEntity>,
}

impl SceneSnapshot {
    fn capture(world: &mut World, hub: &PluginsHub) -> Result<Self, Error> {
        let mut entities: Vec<_> = world
            .view::<(Entities, &SceneComponents)>()
            .iter()
            .map(|(entity, marker)| {
                let mut components: Vec<_> = marker.components.iter().copied().collect();
                components.sort_unstable();
                (entity.id(), components)
            })
            .collect();
        entities.sort_unstable_by_key(|(entity, _)| *entity);

        let entities = entities
            .into_iter()
            .map(|(entity, ids)| {
                let components = ids
                    .into_iter()
                    .map(|id| {
                        let desc = component_desc(hub, id)?;
                        let value = desc.read(world, entity).map_err(Error::wrap)?;
                        Ok((id, value))
                    })
                    .collect::<Result<_, Error>>()?;
                Ok(SceneEntity { entity, components })
            })
            .collect::<Result<_, Error>>()?;
        Ok(Self { entities })
    }

    fn restore(&self, world: &mut World, hub: &PluginsHub) -> Result<(), Error> {
        for entry in &self.entities {
            if world.is_alive(entry.entity) {
                return Err(Error::msg(format!(
                    "Scene entity {} is already present in the initialized world",
                    entry.entity
                )));
            }
            for (id, _) in &entry.components {
                component_desc(hub, *id)?;
            }
        }

        // All authored IDs must exist before component reconstruction can resolve references.
        for entry in &self.entities {
            world.spawn_at(
                entry.entity,
                (SceneComponents {
                    components: entry.components.iter().map(|(id, _)| *id).collect(),
                },),
            );
        }
        for entry in &self.entities {
            for (id, value) in &entry.components {
                component_desc(hub, *id)?
                    .insert(world, entry.entity, value.clone())
                    .map_err(Error::wrap)?;
            }
        }

        // Component hooks may remove entities or other authored components.
        for entry in &self.entities {
            world
                .get::<&SceneComponents>(entry.entity)
                .map_err(Error::wrap)?;
            for (id, _) in &entry.components {
                component_desc(hub, *id)?
                    .read(world, entry.entity)
                    .map_err(Error::wrap)?;
            }
        }
        Ok(())
    }
}

fn component_desc(hub: &PluginsHub, id: Stid) -> Result<&ComponentDesc, Error> {
    hub.components
        .get(&id)
        .ok_or_else(|| Error::msg(format!("Authored component {id} is not registered")))
}

/// Shared editable data. Running worlds belong to individual scene-editor tools.
pub struct SceneEditor {
    world: World,
    hub: PluginsHub,
    ids: EntityIds,
    plugin_revision: u64,
    renderer_order: Vec<EntityId>,

    // Plugin callbacks and values in the world and hub must be dropped first.
    plugins: Option<Plugins>,
}

impl SceneEditor {
    pub fn new(ids: EntityIds) -> Self {
        let mut world = ids.new_world();
        init_flows(&mut world);
        world.insert_resource(ClockStep {
            now: TimeStamp::start(),
            step: TimeSpan::ZERO,
        });
        let mut hub = PluginsHub::new();
        hub.add_component(ComponentDesc::without_default::<Renderer>(arcana::name!(
            Renderer
        )));
        Self {
            world,
            hub,
            ids,
            plugin_revision: 0,
            renderer_order: Vec::new(),
            plugins: None,
        }
    }

    /// Identifies the accepted plugin generation, independent of authored edits.
    pub fn revision(&self) -> u64 {
        self.plugin_revision
    }

    pub fn update_plugins(&mut self, plugins: &Plugins) -> Result<(), Error> {
        if self.plugins.as_ref() == Some(plugins) {
            return Ok(());
        }
        self.update_plugins_with(plugins, |world, hub| {
            for (_, plugin) in plugins.iter() {
                plugin.init(world, hub);
            }
        })
    }

    fn update_plugins_with(
        &mut self,
        plugins: &Plugins,
        initialize: impl FnOnce(&mut World, &mut PluginsHub),
    ) -> Result<(), Error> {
        self.reconcile_renderers();
        let snapshot = SceneSnapshot::capture(&mut self.world, &self.hub)?;
        let mut replacement = Self::new(self.ids.clone());
        replacement.plugins = Some(plugins.clone());
        initialize(&mut replacement.world, &mut replacement.hub);
        snapshot.restore(&mut replacement.world, &replacement.hub)?;
        replacement.renderer_order = self.renderer_order.clone();
        replacement.reconcile_renderers();
        replacement.plugin_revision = self
            .plugin_revision
            .checked_add(1)
            .expect("scene plugin revision exhausted");
        *self = replacement;
        Ok(())
    }

    pub fn instantiate(&mut self) -> Result<Simulation, Error> {
        let snapshot = SceneSnapshot::capture(&mut self.world, &self.hub)?;
        let mut simulation = Simulation::new(self.ids.clone());
        if let Some(plugins) = &self.plugins {
            simulation.update_plugins(plugins);
        }
        let (world, hub) = simulation.world_and_hub_mut();
        snapshot.restore(world, hub)?;
        simulation.set_default_renderer(self.select_renderer(None));
        Ok(simulation)
    }

    fn new_entity(&mut self) -> EntityId {
        self.world.spawn_one(SceneComponents::default()).id()
    }

    fn add_component(&mut self, entity: EntityId, id: Stid) -> Result<(), Error> {
        let value = component_desc(&self.hub, id)?
            .initial_value()
            .ok_or_else(|| Error::msg("Select an initial value for this component".into()))?;
        self.insert_component(entity, id, value)
    }

    fn add_renderer(
        &mut self,
        entity: EntityId,
        graph: RenderGraphId,
        renders: &RenderManager,
    ) -> Result<(), Error> {
        if renders.get_render_graph(graph).is_none() {
            return Err(Error::msg(format!("Render graph {graph} is unavailable")));
        }
        self.insert_component(entity, Renderer::stid(), graph.to_value())
    }

    fn insert_component(&mut self, entity: EntityId, id: Stid, value: Value) -> Result<(), Error> {
        if id == Renderer::stid() {
            self.reconcile_renderers();
        }
        let marker = self
            .world
            .get::<&SceneComponents>(entity)
            .map_err(Error::wrap)?;
        if marker.components.contains(&id) {
            return Ok(());
        }
        let desc = component_desc(&self.hub, id)?;
        desc.insert(&mut self.world, entity, value)
            .map_err(Error::wrap)?;
        self.world
            .get::<&mut SceneComponents>(entity)
            .map_err(Error::wrap)?
            .components
            .insert(id);
        if id == Renderer::stid() && !self.renderer_order.contains(&entity) {
            self.renderer_order.push(entity);
        }
        Ok(())
    }

    fn remove_component(&mut self, entity: EntityId, id: Stid) -> Result<(), Error> {
        let result = component_desc(&self.hub, id)?
            .remove(&mut self.world, entity)
            .map_err(Error::wrap);
        self.reconcile_renderers();
        result?;
        self.world
            .get::<&mut SceneComponents>(entity)
            .map_err(Error::wrap)?
            .components
            .remove(&id);
        Ok(())
    }

    fn delete_entity(&mut self, entity: EntityId) -> Result<(), Error> {
        let result = self.world.despawn(entity).map_err(Error::wrap);
        self.reconcile_renderers();
        result
    }

    fn reconcile_renderers(&mut self) {
        let mut present: Vec<_> = self
            .world
            .view::<(Entities, &Renderer)>()
            .iter()
            .map(|(entity, _)| entity.id())
            .collect();
        present.sort_unstable();
        self.renderer_order
            .retain(|entity| present.binary_search(entity).is_ok());
        for entity in present {
            if !self.renderer_order.contains(&entity) {
                self.renderer_order.push(entity);
            }
        }
    }

    fn select_renderer(&mut self, selected: Option<EntityId>) -> Option<EntityId> {
        self.reconcile_renderers();
        selected
            .filter(|entity| self.renderer_order.contains(entity))
            .or_else(|| self.renderer_order.first().copied())
    }
}

pub struct SceneEditorWidget {
    selected: Option<EntityId>,
    renderer: Option<EntityId>,
    preview: RenderView,
    simulation: Option<Simulation>,
    simulation_widget: SimulationWidget,
    source_revision: u64,
    error: Option<String>,
}

impl SceneEditorWidget {
    pub fn new() -> Self {
        Self {
            selected: None,
            renderer: None,
            preview: RenderView::new(),
            simulation: None,
            simulation_widget: SimulationWidget::new(),
            source_revision: 0,
            error: None,
        }
    }

    fn restore(state: Option<serde_json::Value>) -> Result<Self, serde_json::Error> {
        let state: SceneEditorWidgetState = match state {
            Some(state) if !state.is_null() => serde_json::from_value(state)?,
            _ => SceneEditorWidgetState::default(),
        };
        Ok(Self {
            selected: state.selected.and_then(EntityId::from_bits),
            renderer: state.renderer.and_then(EntityId::from_bits),
            ..Self::new()
        })
    }

    fn play(&mut self, editor: &mut SceneEditor) -> Result<(), Error> {
        if self.simulation.is_none() {
            let mut simulation = editor.instantiate()?;
            self.renderer = editor.select_renderer(self.renderer);
            simulation.set_default_renderer(self.renderer);
            self.source_revision = editor.revision();
            self.simulation_widget = SimulationWidget::new();
            self.simulation = Some(simulation);
        }
        Ok(())
    }

    fn stop(&mut self, textures: &mut UserTextures<'_>) {
        if let Some(mut simulation) = self.simulation.take() {
            simulation.clear_textures(textures);
        }
        self.simulation_widget = SimulationWidget::new();
    }

    fn stop_stale(&mut self, revision: u64, textures: &mut UserTextures<'_>) {
        if self.source_revision != revision {
            self.stop(textures);
            self.preview.clear(textures);
            self.source_revision = revision;
        }
    }

    fn show_authoring(
        &mut self,
        editor: &mut SceneEditor,
        renders: &RenderManager,
        ui: &mut egui::Ui,
    ) {
        ui.horizontal(|ui| {
            if ui.button("New entity").clicked() {
                self.selected = Some(editor.new_entity());
                self.error = None;
            }
            if ui
                .add_enabled(self.selected.is_some(), egui::Button::new("Delete entity"))
                .clicked()
            {
                if let Some(entity) = self.selected.take() {
                    self.error = editor.delete_entity(entity).err().map(|e| e.to_string());
                }
            }
        });

        let mut entities: Vec<_> = editor
            .world
            .view::<(Entities, &SceneComponents)>()
            .iter()
            .map(|(entity, _)| entity.id())
            .collect();
        entities.sort_unstable();
        let mut graphs: Vec<_> = renders
            .render_graphs()
            .map(|(id, graph)| (id, graph.name))
            .collect();
        graphs.sort_unstable_by_key(|(id, name)| (*name, *id));
        let references = [
            ReferenceChoices {
                domain: arcana::ident!(EntityId),
                values: entities
                    .iter()
                    .map(|entity| (entity.bits(), format!("Entity {entity}")))
                    .collect(),
            },
            ReferenceChoices {
                domain: arcana::ident!(RenderGraphId),
                values: graphs
                    .iter()
                    .map(|(id, name)| (id.get(), name.to_string()))
                    .collect(),
            },
        ];
        if self
            .selected
            .is_some_and(|entity| !entities.contains(&entity))
        {
            self.selected = None;
        }

        egui::ScrollArea::vertical()
            .id_salt("scene-entities")
            .max_height(160.0)
            .show(ui, |ui| {
                if entities.is_empty() {
                    ui.weak("Create an entity to begin authoring the scene.");
                }
                for entity in entities {
                    ui.selectable_value(
                        &mut self.selected,
                        Some(entity),
                        format!("Entity {entity}"),
                    );
                }
            });

        let Some(entity) = self.selected else {
            return;
        };
        ui.separator();

        let authored = match editor.world.get::<&SceneComponents>(entity) {
            Ok(marker) => marker.components.clone(),
            Err(error) => {
                self.error = Some(error.to_string());
                self.selected = None;
                return;
            }
        };
        let mut available: Vec<_> = editor
            .hub
            .components
            .values()
            .map(|desc| (desc.id(), desc.name))
            .collect();
        available.sort_unstable_by_key(|(id, name)| (*name, *id));
        let mut add = None;
        let mut add_renderer = None;
        ui.menu_button("Add component", |ui| {
            if available.is_empty() {
                ui.weak("No component types registered by the active plugins.");
            }
            for (id, name) in &available {
                if *id == Renderer::stid() {
                    ui.add_enabled_ui(!authored.contains(id), |ui| {
                        ui.menu_button(name.as_str(), |ui| {
                            if graphs.is_empty() {
                                ui.weak("Create a render graph before adding a Renderer.");
                            }
                            for (graph, name) in &graphs {
                                if ui.button(name.as_str()).clicked() {
                                    add_renderer = Some(*graph);
                                    ui.close();
                                }
                            }
                        });
                    });
                    continue;
                }
                if ui
                    .add_enabled(!authored.contains(id), egui::Button::new(name.as_str()))
                    .clicked()
                {
                    add = Some(*id);
                    ui.close();
                }
            }
        });
        if let Some(id) = add {
            self.error = editor
                .add_component(entity, id)
                .err()
                .map(|e| e.to_string());
        }
        if let Some(graph) = add_renderer {
            self.error = editor
                .add_renderer(entity, graph, renders)
                .err()
                .map(|e| e.to_string());
        }

        let mut authored: Vec<_> = authored.into_iter().collect();
        authored.sort_unstable();
        egui::ScrollArea::vertical()
            .id_salt("scene-components")
            .show(ui, |ui| {
                for id in authored {
                    if !editor.world.is_alive(entity) {
                        break;
                    }
                    ui.push_id(id.get(), |ui| {
                        let desc = match component_desc(&editor.hub, id) {
                            Ok(desc) => desc,
                            Err(error) => {
                                ui.colored_label(egui::Color32::RED, error.to_string());
                                return;
                            }
                        };
                        let mut remove = false;
                        ui.horizontal(|ui| {
                            ui.strong(desc.name.as_str());
                            remove = ui.small_button("Remove").clicked();
                        });
                        if remove {
                            self.error = editor
                                .remove_component(entity, id)
                                .err()
                                .map(|e| e.to_string());
                            return;
                        }
                        match desc.read(&mut editor.world, entity) {
                            Ok(mut value) => {
                                if model_has_editor(&desc.model) {
                                    let mut probe = ValueProbe::new(
                                        Some(&desc.model),
                                        &mut value,
                                        (entity.bits(), id.get()),
                                    ).with_references(&references);
                                    if egui_probe::Probe::new(&mut probe)
                                        .with_header("Value")
                                        .show(ui)
                                        .changed()
                                    {
                                        self.error = desc
                                            .apply(&mut editor.world, entity, value)
                                            .err()
                                            .map(|e| e.to_string());
                                    }
                                } else {
                                    ui.weak("An editor for this component's value model is not implemented.")
                                        .on_hover_text(format!("{:?}", desc.model));
                                    ui.monospace(format!("{value:?}"));
                                }
                            }
                            Err(error) => {
                                ui.colored_label(egui::Color32::RED, error.to_string());
                            }
                        }
                        ui.separator();
                    });
                }
            });
        if !editor.world.is_alive(entity) {
            self.selected = None;
        }
    }

    fn show_preview(
        &mut self,
        editor: &mut SceneEditor,
        textures: &mut UserTextures<'_>,
        ui: &mut egui::Ui,
    ) {
        self.renderer = editor.select_renderer(self.renderer);
        ui.horizontal(|ui| {
            ui.strong("Preview");
            egui::ComboBox::from_id_salt("preview-renderer")
                .selected_text(self.renderer.map_or_else(
                    || "No renderer".to_owned(),
                    |entity| format!("Entity {entity}"),
                ))
                .show_ui(ui, |ui| {
                    for &entity in &editor.renderer_order {
                        ui.selectable_value(
                            &mut self.renderer,
                            Some(entity),
                            format!("Entity {entity}"),
                        );
                    }
                });
        });
        if self.renderer.is_none() {
            self.preview.clear(textures);
            ui.weak("Add a Renderer component to preview this scene.");
        } else {
            self.preview.show(textures, ui);
        }
    }

    fn show_stopped(&mut self, ui: &mut egui::Ui, context: &mut ToolContext<'_>) {
        let available = ui.available_size();
        if available.x >= 640.0 {
            let inspector_width = (available.x * 0.38).clamp(260.0, 400.0);
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(inspector_width, available.y),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_min_width(inspector_width);
                        ui.set_max_width(inspector_width);
                        self.show_authoring(context.scene_editor, context.renders, ui);
                    },
                );
                ui.separator();
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), available.y),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.show_preview(context.scene_editor, &mut context.textures, ui),
                );
            });
        } else {
            let inspector_height = (available.y * 0.45).min(280.0);
            ui.allocate_ui_with_layout(
                egui::vec2(available.x, inspector_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("scene-authoring")
                        .max_height(inspector_height)
                        .show(ui, |ui| {
                            self.show_authoring(context.scene_editor, context.renders, ui);
                        });
                },
            );
            ui.separator();
            self.show_preview(context.scene_editor, &mut context.textures, ui);
        }

        let editor = &mut *context.scene_editor;
        if let Err(error) = self.preview.render(
            &mut editor.world,
            &mut editor.hub,
            editor.plugins.as_ref(),
            self.renderer,
            context.queue,
            context.renders,
            &mut context.textures,
        ) {
            self.error = Some(error.to_string());
        }
    }
}

// ValueProbe must support both its header and inner-field traversal for the entire model.
fn model_has_editor(model: &Model) -> bool {
    match model {
        Model::Unit
        | Model::Bool
        | Model::Int
        | Model::Float
        | Model::String
        | Model::Color(_)
        | Model::Reference(_) => true,
        Model::Array {
            elem: Some(elem), ..
        }
        | Model::Map(Some(elem)) => model_has_editor(elem),
        Model::Enum(variants) => variants
            .iter()
            .all(|(_, model)| model.as_ref().is_some_and(model_has_editor)),
        _ => false,
    }
}

impl Tool for SceneEditorWidget {
    fn title(&self) -> &str {
        "Scene Editor"
    }

    fn ui(&mut self, ui: &mut egui::Ui, window: WindowId, context: &mut ToolContext<'_>) {
        self.stop_stale(context.scene_editor.revision(), &mut context.textures);
        if self.simulation.is_some() {
            if ui.button("Stop").clicked() {
                self.stop(&mut context.textures);
                self.error = None;
            }
        } else if ui.button("Play").clicked() {
            self.error = self.play(context.scene_editor).err().map(|e| e.to_string());
            if self.simulation.is_some() {
                self.preview.clear(&mut context.textures);
            }
        }

        if let Some(simulation) = &mut self.simulation {
            self.simulation_widget
                .show(simulation, window, &mut context.textures, ui);
            if let Err(error) = self.simulation_widget.render(
                simulation,
                context.queue,
                context.renders,
                &mut context.textures,
            ) {
                self.error = Some(error.to_string());
            }
        } else {
            self.show_stopped(ui, context);
        }

        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::RED, error);
        }
    }

    fn update(&mut self, step: ClockStep, context: &mut ToolContext<'_>) {
        self.stop_stale(context.scene_editor.revision(), &mut context.textures);
        if let Some(simulation) = &mut self.simulation {
            simulation.tick(context.systems, step);
        }
    }

    fn handle_event(
        &mut self,
        window: WindowId,
        event: &WindowEvent,
        context: &mut ToolContext<'_>,
    ) -> bool {
        self.stop_stale(context.scene_editor.revision(), &mut context.textures);
        match &mut self.simulation {
            Some(simulation) => {
                self.simulation_widget
                    .handle_event(simulation, context.filters, window, event)
            }
            None => false,
        }
    }

    fn close(&mut self, textures: &mut UserTextures<'_>) {
        self.stop(textures);
        self.preview.clear(textures);
    }

    fn save(&self) -> Option<serde_json::Value> {
        serde_json::to_value(SceneEditorWidgetState {
            selected: self.selected.map(|entity| entity.bits()),
            renderer: self.renderer.map(|entity| entity.bits()),
        })
        .ok()
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct SceneEditorWidgetState {
    #[serde(default)]
    selected: Option<u64>,
    #[serde(default)]
    renderer: Option<u64>,
}

pub struct SceneEditorTemplate;

impl ToolTemplate for SceneEditorTemplate {
    fn key(&self) -> &str {
        "scene-editor"
    }

    fn title(&self) -> &str {
        "Scene Editor"
    }

    fn create(&self, state: Option<serde_json::Value>) -> Result<Box<dyn Tool>, serde_json::Error> {
        Ok(Box::new(SceneEditorWidget::restore(state)?))
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use arcana::{
        id::HasStid,
        model::{Reflect, ValueError},
        plugin::ArcanaPlugin,
    };

    use super::*;

    #[derive(arcana::Component, arcana::HasStid)]
    #[stid("arcana-ed::scene-editor::test-counter")]
    struct Counter(Rc<Cell<i64>>);

    impl Reflect for Counter {
        fn model() -> Model {
            Model::Int
        }

        fn to_value(&self) -> Value {
            Value::Int(self.0.get())
        }

        fn from_value(value: Value) -> Result<Self, ValueError> {
            Ok(Self(Rc::new(Cell::new(i64::from_value(value)?))))
        }
    }

    #[derive(arcana::Component)]
    struct Generated;

    fn plugin() -> ArcanaPlugin {
        let mut plugin = ArcanaPlugin::new(None);
        plugin.add_component::<Counter>(arcana::name!(Counter), || Value::Int(7));
        plugin
    }

    fn editor() -> SceneEditor {
        let mut editor = SceneEditor::new(EntityIds::new());
        plugin().init(&mut editor.world, &mut editor.hub);
        editor
    }

    #[test]
    fn snapshot_contains_only_authored_data_and_empty_entities() -> Result<(), Error> {
        let mut editor = editor();
        let authored = editor.new_entity();
        editor.add_component(authored, Counter::stid())?;
        editor
            .world
            .insert(authored, Generated)
            .map_err(Error::wrap)?;
        let empty = editor.new_entity();
        editor
            .world
            .insert(empty, Counter(Rc::new(Cell::new(42))))
            .map_err(Error::wrap)?;
        editor
            .world
            .spawn((Counter(Rc::new(Cell::new(99))), Generated));

        let snapshot = SceneSnapshot::capture(&mut editor.world, &editor.hub)?;
        assert_eq!(snapshot.entities.len(), 2);
        assert_eq!(snapshot.entities[0].entity, authored);
        assert_eq!(
            snapshot.entities[0].components,
            vec![(Counter::stid(), Value::Int(7))]
        );
        assert_eq!(snapshot.entities[1].entity, empty);
        assert!(snapshot.entities[1].components.is_empty());
        Ok(())
    }

    #[test]
    fn same_id_reconstruction_is_independent() -> Result<(), Error> {
        let mut editor = editor();
        let authored = editor.new_entity();
        editor.add_component(authored, Counter::stid())?;
        editor
            .world
            .insert(authored, Generated)
            .map_err(Error::wrap)?;
        let empty = editor.new_entity();
        let snapshot = SceneSnapshot::capture(&mut editor.world, &editor.hub)?;
        let mut target = editor.ids.new_world();
        let mut hub = PluginsHub::new();
        plugin().init(&mut target, &mut hub);
        snapshot.restore(&mut target, &hub)?;

        assert!(target.is_alive(empty));
        assert!(target.get::<&SceneComponents>(empty).is_ok());
        assert!(target.get::<&Generated>(authored).is_err());
        target
            .get::<&Counter>(authored)
            .map_err(Error::wrap)?
            .0
            .set(21);
        assert_eq!(
            editor
                .world
                .get::<&Counter>(authored)
                .map_err(Error::wrap)?
                .0
                .get(),
            7
        );
        assert_eq!(
            target
                .get::<&Counter>(authored)
                .map_err(Error::wrap)?
                .0
                .get(),
            21
        );
        let runtime = target.spawn_empty().id();
        let next_authored = editor.new_entity();
        assert_ne!(runtime, next_authored);
        Ok(())
    }

    #[test]
    fn failed_reload_preserves_the_editable_world() -> Result<(), Error> {
        let mut editor = editor();
        let entity = editor.new_entity();
        editor.add_component(entity, Counter::stid())?;
        let before = SceneSnapshot::capture(&mut editor.world, &editor.hub)?;
        let revision = editor.revision();

        assert!(
            editor
                .update_plugins_with(&Plugins::default(), |_, _| {})
                .is_err()
        );
        assert_eq!(editor.revision(), revision);
        assert_eq!(
            SceneSnapshot::capture(&mut editor.world, &editor.hub)?,
            before
        );
        component_desc(&editor.hub, Counter::stid())?
            .apply(&mut editor.world, entity, Value::Int(12))
            .map_err(Error::wrap)?;
        assert_eq!(
            editor
                .world
                .get::<&Counter>(entity)
                .map_err(Error::wrap)?
                .0
                .get(),
            12
        );
        Ok(())
    }

    #[test]
    fn successful_reload_preserves_authored_ids_and_values() -> Result<(), Error> {
        let mut editor = editor();
        let entity = editor.new_entity();
        editor.add_component(entity, Counter::stid())?;
        editor
            .world
            .insert(entity, Generated)
            .map_err(Error::wrap)?;
        let empty = editor.new_entity();
        let generated = editor.world.spawn_one(Generated).id();
        let counter = editor
            .world
            .get::<&Counter>(entity)
            .map_err(Error::wrap)?
            .0
            .clone();
        counter.set(31);
        let before = SceneSnapshot::capture(&mut editor.world, &editor.hub)?;
        let revision = editor.revision();

        editor.update_plugins_with(&Plugins::default(), |world, hub| {
            plugin().init(world, hub);
        })?;

        assert_eq!(editor.revision(), revision + 1);
        assert_eq!(
            SceneSnapshot::capture(&mut editor.world, &editor.hub)?,
            before
        );
        assert!(editor.world.is_alive(empty));
        assert!(!editor.world.is_alive(generated));
        assert!(editor.world.get::<&Generated>(entity).is_err());
        assert!(!Rc::ptr_eq(
            &counter,
            &editor.world.get::<&Counter>(entity).map_err(Error::wrap)?.0,
        ));
        Ok(())
    }

    #[derive(arcana::Component, arcana::HasStid)]
    #[stid("arcana-ed::scene-editor::test-counter")]
    struct ChangedCounter;

    impl Reflect for ChangedCounter {
        fn model() -> Model {
            Model::Unit
        }

        fn to_value(&self) -> Value {
            Value::Unit
        }

        fn from_value(value: Value) -> Result<Self, ValueError> {
            <()>::from_value(value)?;
            Ok(Self)
        }
    }

    #[test]
    fn failed_reconstruction_preserves_the_editable_world() -> Result<(), Error> {
        let mut editor = editor();
        let entity = editor.new_entity();
        editor.add_component(entity, Counter::stid())?;
        let before = SceneSnapshot::capture(&mut editor.world, &editor.hub)?;
        let revision = editor.revision();
        let result = editor.update_plugins_with(&Plugins::default(), |_, hub| {
            hub.add_component(ComponentDesc::new::<ChangedCounter>(
                arcana::name!(Counter),
                || Value::Unit,
            ));
        });
        assert!(result.is_err());
        assert_eq!(editor.revision(), revision);
        assert!(editor.plugins.is_none());
        assert_eq!(
            SceneSnapshot::capture(&mut editor.world, &editor.hub)?,
            before
        );
        editor.remove_component(entity, Counter::stid())?;
        assert!(
            editor
                .world
                .get::<&SceneComponents>(entity)
                .map_err(Error::wrap)?
                .components
                .is_empty()
        );
        Ok(())
    }

    #[test]
    fn failed_apply_preserves_the_component_and_marker() -> Result<(), Error> {
        let mut editor = editor();
        let entity = editor.new_entity();
        editor.add_component(entity, Counter::stid())?;
        let before = SceneSnapshot::capture(&mut editor.world, &editor.hub)?;
        assert!(
            component_desc(&editor.hub, Counter::stid())?
                .apply(&mut editor.world, entity, Value::Bool(false))
                .is_err()
        );
        assert_eq!(
            SceneSnapshot::capture(&mut editor.world, &editor.hub)?,
            before
        );
        Ok(())
    }

    #[test]
    fn widgets_play_and_stop_independently_and_restore_stopped() -> Result<(), Error> {
        let mut editor = SceneEditor::new(EntityIds::new());
        let entity = editor.new_entity();
        let mut first = SceneEditorWidget::new();
        let mut second = SceneEditorWidget::new();
        first.selected = Some(entity);
        first.play(&mut editor)?;
        second.play(&mut editor)?;
        first
            .simulation
            .as_mut()
            .expect("first widget is playing")
            .world_and_hub_mut()
            .0
            .despawn(entity)
            .map_err(Error::wrap)?;
        assert!(
            second
                .simulation
                .as_mut()
                .expect("second widget is playing")
                .world_and_hub_mut()
                .0
                .is_alive(entity)
        );
        assert!(editor.world.is_alive(entity));

        let saved = first.save();
        let restored = SceneEditorWidget::restore(saved.clone()).map_err(Error::wrap)?;
        assert_eq!(restored.save(), saved);
        assert!(restored.simulation.is_none());
        assert!(
            SceneEditorWidget::restore(None)
                .map_err(Error::wrap)?
                .simulation
                .is_none()
        );
        assert!(
            SceneEditorWidget::restore(Some(serde_json::json!({})))
                .map_err(Error::wrap)?
                .simulation
                .is_none()
        );
        let mut ui = crate::ui::Ui::new();
        first.close(&mut ui.textures());
        assert!(first.simulation.is_none());
        assert!(second.simulation.is_some());
        editor.update_plugins(&Plugins::default())?;
        second.stop_stale(editor.revision(), &mut ui.textures());
        assert!(second.simulation.is_none());
        Ok(())
    }

    #[test]
    fn editor_guard_rejects_models_with_unimplemented_probe_paths() {
        assert!(model_has_editor(&Model::Int));
        assert!(model_has_editor(&Renderer::model()));
        assert!(model_has_editor(&Model::Array {
            elem: Some(Box::new(Model::Bool)),
            len: None
        }));
        assert!(!model_has_editor(&Model::Record(Vec::new())));
        assert!(!model_has_editor(&Model::Option(Some(Box::new(
            Model::Int
        )))));
        assert!(!model_has_editor(&Model::Array {
            elem: Some(Box::new(Model::Vec3)),
            len: None
        }));
        assert!(!model_has_editor(&Model::Map(None)));
    }

    #[test]
    fn renderer_requires_a_graph_and_is_copied_as_authored_data() -> Result<(), Error> {
        let mut editor = SceneEditor::new(EntityIds::new());
        let entity = editor.new_entity();
        let desc = component_desc(&editor.hub, Renderer::stid())?;
        assert!(desc.initial_value().is_none());
        assert_eq!(desc.model, Model::Reference(arcana::ident!(RenderGraphId)));
        assert!(editor.add_component(entity, Renderer::stid()).is_err());
        assert!(editor.world.get::<&Renderer>(entity).is_err());

        let mut renders = RenderManager::new();
        let missing = arcana::id::hash_id!("missing-render-graph");
        assert!(editor.add_renderer(entity, missing, &renders).is_err());
        let graph = renders.new_render(arcana::name!(Main));
        editor.add_renderer(entity, graph, &renders)?;
        let snapshot = SceneSnapshot::capture(&mut editor.world, &editor.hub)?;
        assert_eq!(
            snapshot.entities[0].components,
            vec![(
                Renderer::stid(),
                Value::Reference(arcana::ident!(RenderGraphId), graph.get())
            )]
        );

        let mut simulation = editor.instantiate()?;
        let (world, _) = simulation.world_and_hub_mut();
        assert_eq!(
            world.get::<&Renderer>(entity).map_err(Error::wrap)?.graph,
            graph
        );
        world.despawn(entity).map_err(Error::wrap)?;
        assert!(editor.world.is_alive(entity));
        editor.remove_component(entity, Renderer::stid())?;
        assert!(editor.world.get::<&Renderer>(entity).is_err());
        assert!(
            editor
                .world
                .get::<&SceneComponents>(entity)
                .map_err(Error::wrap)?
                .components
                .is_empty()
        );
        Ok(())
    }

    #[test]
    fn renderer_order_survives_reload_and_tracks_removal_and_external_changes() -> Result<(), Error>
    {
        let mut editor = SceneEditor::new(EntityIds::new());
        let lower = editor.new_entity();
        let higher = editor.new_entity();
        let external = editor.new_entity();
        assert!(lower < higher);
        let mut renders = RenderManager::new();
        let graph = renders.new_render(arcana::name!(Main));
        editor.add_renderer(higher, graph, &renders)?;
        editor.add_renderer(lower, graph, &renders)?;
        assert_eq!(editor.renderer_order, vec![higher, lower]);
        assert_eq!(editor.select_renderer(None), Some(higher));
        assert_eq!(editor.select_renderer(Some(lower)), Some(lower));

        editor.update_plugins(&Plugins::default())?;
        assert_eq!(editor.renderer_order, vec![higher, lower]);
        assert_eq!(editor.select_renderer(None), Some(higher));
        editor
            .world
            .insert(external, Renderer { graph })
            .map_err(Error::wrap)?;
        assert_eq!(editor.select_renderer(None), Some(higher));
        assert_eq!(editor.renderer_order, vec![higher, lower, external]);
        editor.remove_component(higher, Renderer::stid())?;
        assert_eq!(editor.select_renderer(Some(higher)), Some(lower));
        editor.add_renderer(higher, graph, &renders)?;
        assert_eq!(editor.renderer_order, vec![lower, external, higher]);
        editor.delete_entity(lower)?;
        assert_eq!(editor.select_renderer(Some(lower)), Some(external));
        editor
            .world
            .drop::<Renderer>(external)
            .map_err(Error::wrap)?;
        assert_eq!(editor.select_renderer(Some(external)), Some(higher));
        Ok(())
    }

    #[test]
    fn renderer_selection_is_separate_from_inspection_and_restores_stopped() -> Result<(), Error> {
        let mut editor = SceneEditor::new(EntityIds::new());
        let lower = editor.new_entity();
        let higher = editor.new_entity();
        let mut renders = RenderManager::new();
        let graph = renders.new_render(arcana::name!(Main));
        editor.add_renderer(higher, graph, &renders)?;
        editor.add_renderer(lower, graph, &renders)?;

        let mut widget = SceneEditorWidget::new();
        widget.selected = Some(lower);
        widget.play(&mut editor)?;
        assert_eq!(widget.selected, Some(lower));
        assert_eq!(widget.renderer, Some(higher));
        let restored = SceneEditorWidget::restore(widget.save()).map_err(Error::wrap)?;
        assert_eq!(restored.selected, Some(lower));
        assert_eq!(restored.renderer, Some(higher));
        assert!(restored.simulation.is_none());
        let mut ui = crate::ui::Ui::new();
        widget.stop(&mut ui.textures());
        widget.renderer = Some(lower);
        widget.play(&mut editor)?;
        assert_eq!(widget.renderer, Some(lower));
        widget.close(&mut ui.textures());
        Ok(())
    }
}
