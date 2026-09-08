use std::{error::Error, fmt};

use arcana_id::{HasStid, Stid};
use arcana_intern::Name;
use arcana_model::{Model, Reflect, Value, ValueError};
use edict::{EntityError, component::Component, entity::EntityId, world::World};

use crate::Location;

/// Failure to access a component or convert its editable data.
#[derive(Debug)]
pub enum ComponentError {
    Value(ValueError),
    Entity(EntityError),
}

impl fmt::Display for ComponentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(error) => fmt::Display::fmt(error, f),
            Self::Entity(error) => fmt::Display::fmt(error, f),
        }
    }
}

impl Error for ComponentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Value(error) => Some(error),
            Self::Entity(error) => Some(error),
        }
    }
}

impl From<ValueError> for ComponentError {
    fn from(error: ValueError) -> Self {
        Self::Value(error)
    }
}

impl From<EntityError> for ComponentError {
    fn from(error: EntityError) -> Self {
        Self::Entity(error)
    }
}

/// Editable component metadata and operations declared by a plugin.
///
/// Descriptors and their clones contain callbacks into the plugin. The plugin
/// library must remain loaded until all descriptors and components created
/// through them have been dropped.
#[derive(Clone)]
pub struct ComponentDesc {
    id: Stid,
    pub name: Name,
    pub model: Model,
    pub location: Option<Location>,
    initial_value: Option<fn() -> Value>,
    read: fn(&mut World, EntityId) -> Result<Value, ComponentError>,
    apply: fn(&mut World, EntityId, Value) -> Result<(), ComponentError>,
    insert: fn(&mut World, EntityId, Value) -> Result<(), ComponentError>,
    remove: fn(&mut World, EntityId) -> Result<(), ComponentError>,
}

impl ComponentDesc {
    /// Describes `T` using its stable ID and reflected representation.
    /// `initial_value` must produce a value accepted by `T::from_value`.
    pub fn new<T>(name: Name, initial_value: fn() -> Value) -> Self
    where
        T: Component + Reflect + HasStid,
    {
        Self {
            initial_value: Some(initial_value),
            ..Self::without_default::<T>(name)
        }
    }

    /// Describes a component that requires explicitly supplied data when added.
    pub fn without_default<T>(name: Name) -> Self
    where
        T: Component + Reflect + HasStid,
    {
        Self {
            id: T::stid(),
            name,
            model: T::model(),
            location: None,
            initial_value: None,
            read: |world, entity| Ok(world.get::<&T>(entity)?.to_value()),
            apply: |world, entity, value| {
                world.get::<&mut T>(entity)?.apply(value)?;
                Ok(())
            },
            insert: |world, entity, value| {
                let component = T::from_value(value)?;
                world.insert(entity, component).map_err(EntityError::from)?;
                Ok(())
            },
            remove: |world, entity| {
                world.drop::<T>(entity).map_err(EntityError::from)?;
                Ok(())
            },
        }
    }

    pub fn id(&self) -> Stid {
        self.id
    }

    /// Creates editable data for adding a component, if a factory is available.
    pub fn initial_value(&self) -> Option<Value> {
        self.initial_value.map(|initial_value| initial_value())
    }

    /// Reads an existing component. Missing entities and components are distinct errors.
    pub fn read(&self, world: &mut World, entity: EntityId) -> Result<Value, ComponentError> {
        (self.read)(world, entity)
    }

    /// Applies editable data to an existing component through `Reflect::apply`.
    /// The component remains unchanged when conversion fails.
    pub fn apply(
        &self,
        world: &mut World,
        entity: EntityId,
        value: Value,
    ) -> Result<(), ComponentError> {
        (self.apply)(world, entity, value)
    }

    /// Decodes before inserting, or replacing an existing component.
    /// Uses `World::insert` and its ECS replacement-hook semantics.
    pub fn insert(
        &self,
        world: &mut World,
        entity: EntityId,
        value: Value,
    ) -> Result<(), ComponentError> {
        (self.insert)(world, entity, value)
    }

    /// Drops a component through the ECS, invoking its drop hook.
    /// A missing component is a no-op; a missing entity is an error.
    pub fn remove(&self, world: &mut World, entity: EntityId) -> Result<(), ComponentError> {
        (self.remove)(world, entity)
    }
}
