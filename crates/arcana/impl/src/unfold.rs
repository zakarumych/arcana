//! This module provide unfolding engine system.
//!
//! Unfolding is a process of expanding special transferable component into multiple components and entities.
//! For example one can make a component `Unit` that will unfold into `Mesh`, `Material` other components
//! required to represent a unit in the game.
//!
//! This system works for unfolding loaded assets into game-ready components (including loading more assets).
//! As well as for unfolding synced components when they are first received or updated from the network.

use std::any::TypeId;

use edict::{
    query::QueryItem, system::QueryArg, ActionBuffer, ActionBufferSliceExt, ActionEncoder,
    Component, IntoSystem, State, System, View, World,
};
use hashbrown::HashSet;

use crate::type_id;

pub trait Unfold: Component + Sync {
    /// Query type for the bundle.
    /// Typical bundle would use query that is matched by any entity.
    /// Otherwise bundle won't unfold for entities that don't match the query.
    type Query: QueryArg;

    // Unfold bundle into the query item.
    // Insert components and spawn new entities with `actions`.
    fn unfold(&self, item: QueryItem<Self::Query>, actions: ActionEncoder);
}

/// System that unfolds bundles.
pub fn unfold_type_system<U>(view: View<(&U, U::Query)>, mut actions: ActionEncoder)
where
    U: Unfold,
{
    view.into_iter().for_each(|(bundle, item)| {
        bundle.unfold(item, actions.reborrow());
    })
}

struct UnfoldRegistrar {
    registered: HashSet<TypeId>,
    systems: Vec<Box<dyn System + Send>>,
}

impl UnfoldRegistrar {
    fn new() -> Self {
        UnfoldRegistrar {
            registered: HashSet::new(),
            systems: Vec::new(),
        }
    }

    fn register<U: Unfold>(&mut self) {
        let type_id = type_id::<U>();
        if self.registered.contains(&type_id) {
            return;
        }

        self.registered.insert(type_id);
        self.systems
            .push(Box::new(unfold_type_system::<U>.into_system()));
    }
}

#[doc(hidden)]
pub struct UnfoldSystemState {
    systems: Vec<Box<dyn System + Send>>,
    buffers: Vec<ActionBuffer>,
}

/// Register unfolding for the given component.
pub fn register_unfold<U: Unfold>(world: &mut World) {
    let registrar = world.with_resource(UnfoldRegistrar::new);
    registrar.register::<U>();
}

/// Unfold all registered components.
pub fn unfold_system(world: &mut World, mut state: State<UnfoldSystemState>) {
    let UnfoldSystemState { systems, buffers } = &mut *state;

    if let Some(mut registrar) = world.get_resource_mut::<UnfoldRegistrar>() {
        systems.extend(registrar.systems.drain(..));
    }

    for system in systems {
        system.run(world, buffers);
    }

    buffers.execute_all(world);
    buffers.clear();
}
