//! This module provide unfolding engine system.
//!
//! Unfolding is a process of expanding special transferable component into multiple components and entities.
//! For example one can make a component `Unit` that will unfold into `Mesh`, `Material` other components
//! required to represent a unit in the game.
//!
//! This system works for unfolding loaded assets into game-ready components (including loading more assets).
//! As well as for unfolding synced components when they are first received or updated from the network.

use std::any::TypeId;

use arcana_hash::NoHashBuilder;
use edict::{
    action::{ActionBuffer, ActionBufferSliceExt, ActionEncoder},
    component::Component,
    query::{Alt, Entities, Modified, QueryItem},
    system::{IntoSystem, QueryArg, State, System},
    world::World,
};
use hashbrown::HashSet;

mod resource;
pub use resource::{ResourceItem, ResourceQuery};

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
/// Implement this trait explicitly; no unfolding derive macro is exported.
pub trait Unfold: Component + Send + 'static {
    /// Query type used to unfold the component.
    ///
    /// This is typically a tuple of `Option<&mut T>`.
    /// Where `T`s are components this one unfolds into.
    ///
    /// If some `T` is missing it can be inserted into the world using `actions`.
    /// The query's `before` hook runs while requested resource guards are held;
    /// conflicting resource access there panics through world borrow checks.
    type Query: QueryArg;

    /// Resources used by the callback, or `()` when none are needed.
    /// Required resources must exist before callbacks run. Missing required
    /// resources leave the modification checkpoint unchanged for a later pass.
    type Resources: ResourceQuery;

    /// Unfold bundle into the query item.
    /// Insert components and spawn new entities with `actions`.
    ///
    /// Returns `true` if unfolded successfully,
    /// `false` if this function should be called again
    /// (e.g. if some asset is not loaded yet).
    fn unfold(
        &self,
        item: QueryItem<Self::Query>,
        resources: ResourceItem<'_, Self::Resources>,
        actions: ActionEncoder,
    ) -> bool;
}

struct UnfoldTypeState<U: Unfold> {
    query: (Entities, Modified<Alt<U>>, U::Query),
}

impl<U: Unfold> Default for UnfoldTypeState<U> {
    fn default() -> Self {
        Self {
            query: QueryArg::new(),
        }
    }
}

/// System that unfolds bundles.
fn unfold_type_system<U>(
    world: &mut World,
    mut state: State<UnfoldTypeState<U>>,
    mut actions: ActionEncoder,
) where
    U: Unfold,
{
    state.query.before(world);
    let Some(mut resources) = U::Resources::fetch(world) else {
        return;
    };
    for (entity, bundle, item) in world.view_with(state.query) {
        if !U::unfold(
            &bundle,
            item,
            U::Resources::reborrow(&mut resources),
            actions.reborrow(),
        ) {
            // Stamp a fresh epoch after the modified query records its checkpoint.
            let entity = entity.id();
            actions.closure(move |world| {
                let _ = world.get::<&mut U>(entity);
            });
        }
    }
    drop(resources);
    state.query.after(world);
}

struct UnfoldRegistrar {
    registered: HashSet<TypeId, NoHashBuilder>,
    systems: Vec<Box<dyn System + Send>>,
}

impl UnfoldRegistrar {
    fn new() -> Self {
        UnfoldRegistrar {
            registered: HashSet::default(),
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
#[derive(Default)]
pub struct UnfoldSystemState {
    systems: Vec<Box<dyn System + Send>>,
    buffers: Vec<ActionBuffer>,
}

/// Register unfolding for the given component.
pub fn register_unfold<U: Unfold>(world: &mut World) {
    let registrar = world.with_resource(UnfoldRegistrar::new);
    registrar.register::<U>();
}

/// Unfold all registered components sequentially in registration order.
///
/// Callbacks hold resource guards for one component type's pass. Component and
/// resource borrow conflicts panic. All query checkpoints finish and guards
/// drop before deferred actions execute.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct Pending(Arc<AtomicUsize>);
    impl Component for Pending {}
    impl Unfold for Pending {
        type Query = ();
        type Resources = ();
        fn unfold(&self, _: (), _: (), _: ActionEncoder) -> bool {
            self.0.fetch_add(1, Ordering::SeqCst) >= 2
        }
    }

    #[test]
    fn pending_components_retry_then_stop_after_success() {
        let mut world = World::new();
        register_unfold::<Pending>(&mut world);
        let calls = Arc::new(AtomicUsize::new(0));
        world.spawn((Pending(calls.clone()),));
        let mut system = unfold_system.into_system();
        for expected in [1, 2, 3, 3, 3] {
            system.run_alone(&mut world);
            assert_eq!(calls.load(Ordering::SeqCst), expected);
        }
    }

    struct Count(usize);
    struct Factor(usize);
    struct Optional(usize);

    struct UsesResources;
    impl Component for UsesResources {}
    impl Unfold for UsesResources {
        type Query = ();
        type Resources = (
            &'static Factor,
            &'static mut Count,
            Option<&'static mut Optional>,
        );
        fn unfold(
            &self,
            _: (),
            (factor, count, optional): ResourceItem<'_, Self::Resources>,
            _: ActionEncoder,
        ) -> bool {
            count.0 += factor.0;
            if let Some(optional) = optional {
                optional.0 += 1;
            }
            true
        }
    }

    #[test]
    fn required_resources_retry_without_component_changes() {
        let mut world = World::new();
        register_unfold::<UsesResources>(&mut world);
        world.spawn_one(UsesResources);
        world.insert_resource(Count(0));
        let mut system = unfold_system.into_system();
        system.run_alone(&mut world);
        system.run_alone(&mut world);
        assert_eq!(world.expect_resource::<Count>().0, 0);
        world.insert_resource(Factor(3));
        system.run_alone(&mut world);
        system.run_alone(&mut world);
        assert_eq!(world.expect_resource::<Count>().0, 3);
    }

    #[test]
    fn resource_guards_reborrow_for_all_entities() {
        let mut world = World::new();
        register_unfold::<UsesResources>(&mut world);
        world.spawn_one(UsesResources);
        world.spawn_one(UsesResources);
        world.insert_resource(Count(0));
        world.insert_resource(Factor(3));
        world.insert_resource(Optional(0));
        unfold_system.into_system().run_alone(&mut world);
        assert_eq!(world.expect_resource::<Count>().0, 6);
        assert_eq!(world.expect_resource::<Optional>().0, 2);
    }

    struct DeferredResources;
    impl Component for DeferredResources {}
    impl Unfold for DeferredResources {
        type Query = ();
        type Resources = &'static mut Count;
        fn unfold(&self, _: (), count: &mut Count, mut actions: ActionEncoder) -> bool {
            count.0 += 1;
            actions.closure(|world| {
                world.expect_resource_mut::<Count>().0 += 10;
                world.insert_resource(Factor(9));
            });
            true
        }
    }

    #[test]
    fn resource_guards_drop_before_deferred_actions() {
        let mut world = World::new();
        register_unfold::<DeferredResources>(&mut world);
        world.spawn_one(DeferredResources);
        world.insert_resource(Count(0));
        unfold_system.into_system().run_alone(&mut world);
        assert_eq!(world.expect_resource::<Count>().0, 11);
        assert_eq!(world.expect_resource::<Factor>().0, 9);
    }

    #[test]
    fn optional_readonly_resources_and_shared_aliases() {
        let mut world = World::new();
        world.insert_resource(Count(7));
        type Request = (&'static Count, &'static Count, Option<&'static Factor>);
        let mut guard = Request::fetch(&world).unwrap();
        let (a, b, missing) = Request::reborrow(&mut guard);
        assert_eq!(a.0, 7);
        assert!(std::ptr::eq(a, b));
        assert!(missing.is_none());
    }

    #[test]
    #[should_panic]
    fn duplicate_mutable_resources_panic() {
        let mut world = World::new();
        world.insert_resource(Count(0));
        let _guard = <(&'static mut Count, &'static mut Count)>::fetch(&world);
    }

    #[test]
    #[should_panic]
    fn mixed_resource_aliases_panic() {
        let mut world = World::new();
        world.insert_resource(Count(0));
        let _guard = <(&'static Count, &'static mut Count)>::fetch(&world);
    }

    struct ConflictingQuery;
    impl Component for ConflictingQuery {}
    impl Unfold for ConflictingQuery {
        type Query = edict::query::Write<ConflictingQuery>;
        type Resources = ();
        fn unfold(&self, _: &mut Self, _: (), _: ActionEncoder) -> bool {
            true
        }
    }

    #[test]
    #[should_panic]
    fn conflicting_component_queries_panic() {
        let mut world = World::new();
        register_unfold::<ConflictingQuery>(&mut world);
        world.spawn_one(ConflictingQuery);
        unfold_system.into_system().run_alone(&mut world);
    }
}
