//! This module provide unfolding engine system.
//!
//! Unfolding is a process of expanding special transferable component into multiple components and entities.
//! For example one can make a component `Unit` that will unfold into `Mesh`, `Material` other components
//! required to represent a unit in the game.
//!
//! This system works for unfolding loaded assets into game-ready components (including loading more assets).
//! As well as for unfolding synced components when they are first received or updated from the network.

use std::{any::TypeId, ops::DerefMut};

use edict::{
    action::{ActionBuffer, ActionBufferSliceExt, ActionEncoder},
    component::Component,
    query::{Alt, Modified, QueryItem, RefMut},
    system::{IntoSystem, QueryArg, State, System},
    view::View,
    world::World,
};
use hashbrown::HashSet;

/// Unfold components are components that store packed entity data.
///
/// They can be unpacked into multiple components and event child entities.
/// A system monitors when unfold component is added or changed
/// and performs unfolding operation.
///
/// Unfold component may reference assets and load them as part of unfolding process.
///
/// Some unfold components may unfold into other unfold components.
///
/// This trait can be derived using `#[derive(Unfold)]` macro.
/// When doing so, each field will be treated as a component that this one unfolds into.
/// Fields of type `Option<T>` may be marked as `#[unfold(maybe)]`,
/// causing the `T` component to be inserted/updated if field is `Some` and removed if `None`.
///
/// `#[unfold(skip)]` instructs macro to skip this field entirely in all code-gen.
///
/// `#[unfold(asset: <asset-type>)]` can be placed onto a field of type `AssetId`,
/// causing unfolding process to load the asset of the specified type and insert it as a component.
pub trait Unfold: Component + Send + 'static {
    /// Query type used to unfold the component.
    ///
    /// This is typically a tuple of `Option<&mut T>`.
    /// Where `T`s are components this one unfolds into.
    ///
    /// If some `T` is missing it can be inserted into the world using `actions`.
    type Query: QueryArg;

    /// Unfold bundle into the query item.
    /// Insert components and spawn new entities with `actions`.
    ///
    /// Returns `true` if unfolded successfully,
    /// `false` if this function should be called again
    /// (e.g. if some asset is not loaded yet).
    fn unfold(&self, item: QueryItem<Self::Query>, actions: ActionEncoder) -> bool;
}

/// System that unfolds bundles.
fn unfold_type_system<U>(view: View<(Modified<Alt<U>>, U::Query)>, mut actions: ActionEncoder)
where
    U: Unfold,
{
    view.into_iter().for_each(|(mut bundle, item)| {
        if !U::unfold(&bundle, item, actions.reborrow()) {
            // This will make sure that the `Modified` will not filter out the `bundle`.
            RefMut::deref_mut(&mut bundle);
        }
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
        let type_id = std::any::TypeId::of::<U>();
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
