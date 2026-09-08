use std::{cell::Cell, error::Error, panic::AssertUnwindSafe, rc::Rc};

use arcana::{
    ecs::{EntityError, world::World},
    id::HasStid,
    model::{Model, Reflect, Value, ValueError},
    plugin::{ArcanaPlugin, ComponentDesc, ComponentError, PluginsHub},
    render::{RenderGraphId, Renderer},
};

arcana::declare_plugin!();

#[derive(arcana::Component, arcana::HasStid)]
#[stid("dummy::Counter")]
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

mod factories {
    use super::*;

    #[arcana::component]
    fn counter() -> Counter {
        Counter(Rc::new(Cell::new(7)))
    }
}

#[derive(arcana::Component, arcana::HasStid)]
#[stid("dummy::LocalState")]
struct LocalState {
    value: i64,
    edits: Rc<Cell<usize>>,
}

impl Reflect for LocalState {
    fn model() -> Model {
        Model::Int
    }

    fn to_value(&self) -> Value {
        Value::Int(self.value)
    }

    fn from_value(value: Value) -> Result<Self, ValueError> {
        Ok(Self {
            value: i64::from_value(value)?,
            edits: Rc::new(Cell::new(0)),
        })
    }

    fn apply(&mut self, value: Value) -> Result<(), ValueError> {
        self.value = i64::from_value(value)?;
        self.edits.set(self.edits.get() + 1);
        Ok(())
    }
}

#[arcana::component]
fn local_state() -> LocalState {
    LocalState {
        value: 3,
        edits: Rc::new(Cell::new(0)),
    }
}

fn descriptor<T: HasStid>(plugin: &ArcanaPlugin) -> &ComponentDesc {
    plugin
        .components()
        .iter()
        .find(|desc| desc.id() == T::stid())
        .expect("component is registered by its factory")
}

#[test]
fn plugin_components_are_editable_and_reconstructible() -> Result<(), Box<dyn Error>> {
    let mut plugin = arcana_plugin::get();
    assert_eq!(plugin.components().len(), 2);
    let declared = descriptor::<Counter>(&plugin);
    assert_eq!(declared.name, arcana::name!(counter));
    assert_eq!(declared.model, Model::Int);
    assert!(declared.location.as_ref().is_some_and(|location| {
        location.file.ends_with("components.rs") && location.line > 0 && location.column > 0
    }));

    plugin.add_filter(
        arcana::plugin::FilterInfo {
            id: arcana::id::hash_id!("component-order"),
            name: arcana::name!(component_order),
            location: None,
        },
        |hub| assert!(hub.components.contains_key(&Counter::stid())),
    );

    let mut world = World::new();
    let mut hub = PluginsHub::new();
    plugin.init(&mut world, &mut hub);
    let desc = hub.components.get(&Counter::stid()).expect("hub component");
    let entity = world.spawn_empty().id();
    desc.insert(
        &mut world,
        entity,
        desc.initial_value()
            .expect("factory provides component data"),
    )?;
    assert_eq!(desc.read(&mut world, entity)?, Value::Int(7));
    desc.apply(&mut world, entity, Value::Int(11))?;
    assert!(matches!(
        desc.apply(&mut world, entity, Value::Bool(false)),
        Err(ComponentError::Value(_))
    ));
    assert_eq!(desc.read(&mut world, entity)?, Value::Int(11));

    let mut second_world = World::new();
    let second_entity = second_world.spawn_empty().id();
    desc.insert(
        &mut second_world,
        second_entity,
        desc.read(&mut world, entity)?,
    )?;
    desc.apply(&mut second_world, second_entity, Value::Int(20))?;
    assert_eq!(desc.read(&mut world, entity)?, Value::Int(11));
    assert_eq!(desc.read(&mut second_world, second_entity)?, Value::Int(20));
    desc.insert(&mut world, entity, Value::Int(30))?;
    assert_eq!(desc.read(&mut world, entity)?, Value::Int(30));
    desc.remove(&mut world, entity)?;
    desc.remove(&mut world, entity)?;
    assert!(matches!(
        desc.read(&mut world, entity),
        Err(ComponentError::Entity(EntityError::Mismatch))
    ));
    Ok(())
}

#[test]
fn custom_apply_preserves_runtime_state() -> Result<(), Box<dyn Error>> {
    let plugin = arcana_plugin::get();
    let desc = descriptor::<LocalState>(&plugin);
    let mut world = World::new();
    let edits = Rc::new(Cell::new(4));
    let entity = world
        .spawn_one(LocalState {
            value: 1,
            edits: edits.clone(),
        })
        .id();
    desc.apply(&mut world, entity, Value::Int(8))?;
    assert_eq!(edits.get(), 5);
    assert!(Rc::ptr_eq(&world.get::<&LocalState>(entity)?.edits, &edits));
    assert!(desc.apply(&mut world, entity, Value::Unit).is_err());
    assert_eq!(edits.get(), 5);
    assert_eq!(desc.read(&mut world, entity)?, Value::Int(8));
    Ok(())
}

#[test]
fn distinguishes_missing_components_and_entities() -> Result<(), Box<dyn Error>> {
    let plugin = arcana_plugin::get();
    let desc = descriptor::<Counter>(&plugin);
    let mut world = World::new();
    let entity = world.spawn_empty().id();
    assert!(matches!(
        desc.apply(&mut world, entity, Value::Int(1)),
        Err(ComponentError::Entity(EntityError::Mismatch))
    ));
    world.despawn(entity)?;
    assert!(matches!(
        desc.read(&mut world, entity),
        Err(ComponentError::Entity(EntityError::NoSuchEntity))
    ));
    for result in [
        desc.apply(&mut world, entity, Value::Int(1)),
        desc.insert(&mut world, entity, Value::Int(1)),
        desc.remove(&mut world, entity),
    ] {
        assert!(matches!(
            result,
            Err(ComponentError::Entity(EntityError::NoSuchEntity))
        ));
    }
    Ok(())
}

#[derive(arcana::Component, arcana::HasStid)]
#[stid("dummy::Counter")]
struct Collision;

impl Reflect for Collision {
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
fn duplicate_stable_ids_do_not_overwrite_registrations() {
    let mut plugin = arcana_plugin::get();
    let collision = ComponentDesc::new::<Collision>(arcana::name!(collision), || Value::Unit);
    assert!(
        std::panic::catch_unwind(AssertUnwindSafe(|| {
            plugin.add_component::<Collision>(arcana::name!(collision), || Value::Unit);
        }))
        .is_err()
    );
    assert_eq!(plugin.components().len(), 2);
    assert_eq!(descriptor::<Counter>(&plugin).name, arcana::name!(counter));

    let mut hub = PluginsHub::new();
    hub.add_component(descriptor::<Counter>(&plugin).clone());
    assert!(
        std::panic::catch_unwind(AssertUnwindSafe(|| {
            hub.add_component(collision);
        }))
        .is_err()
    );
    assert_eq!(hub.components.len(), 1);
    assert_eq!(
        hub.components[&Counter::stid()].name,
        arcana::name!(counter)
    );
}

arcana::id::make_uid! {
    /// Reference type defined by a downstream crate with only Arcana as a dependency.
    pub PluginResourceId;
}

#[test]
fn downstream_generated_ids_reflect_without_additional_dependencies() -> Result<(), ValueError> {
    let id = PluginResourceId::new(std::num::NonZeroU64::MAX);
    assert_eq!(
        PluginResourceId::model(),
        Model::Reference(arcana::ident!(PluginResourceId))
    );
    assert_eq!(
        id.to_value(),
        Value::Reference(arcana::ident!(PluginResourceId), u64::MAX)
    );
    assert_eq!(PluginResourceId::from_value(id.to_value())?, id);
    assert!(PluginResourceId::from_value(PluginResourceId::model().default_value()).is_err());
    assert!(
        PluginResourceId::from_value(Value::Reference(arcana::ident!(RenderGraphId), 1)).is_err()
    );
    Ok(())
}

#[test]
fn renderer_requires_an_explicit_graph_and_round_trips_as_a_reference() -> Result<(), Box<dyn Error>>
{
    let graph = RenderGraphId::new(std::num::NonZeroU64::MAX);
    let renderer = Renderer { graph };
    assert_eq!(Renderer::model(), RenderGraphId::model());
    assert_eq!(
        renderer.to_value(),
        Value::Reference(arcana::ident!(RenderGraphId), u64::MAX)
    );
    assert_eq!(Renderer::from_value(renderer.to_value())?.graph, graph);
    assert!(Renderer::from_value(Renderer::model().default_value()).is_err());

    let desc = ComponentDesc::without_default::<Renderer>(arcana::name!(Renderer));
    assert_eq!(desc.id(), Renderer::stid());
    assert!(desc.initial_value().is_none());
    let mut world = World::new();
    let entity = world.spawn_empty().id();
    desc.insert(&mut world, entity, renderer.to_value())?;
    assert_eq!(world.get::<&Renderer>(entity)?.graph, graph);
    assert_eq!(desc.read(&mut world, entity)?, renderer.to_value());
    Ok(())
}
