//! Asset loading and typed asset requests for ECS entities.

pub use arcana_assets::*;

/// Metadata for importers available without a plugin.
pub fn builtin_importers() -> Vec<crate::plugin::ImporterInfo> {
    use arcana_assets::import::Importer;
    let desc = crate::graphics::ImageImporter::desc();
    vec![crate::plugin::ImporterInfo {
        id: crate::graphics::ImageImporter::id(),
        name: desc.name,
        desc,
        location: None,
    }]
}

/// Ensures built-in importers exist while retaining existing instances.
pub fn register_builtin_importers(hub: &mut crate::plugin::PluginsHub) {
    hub.importers
        .entry(crate::graphics::ImageImporter::id())
        .or_insert_with(|| Box::new(crate::graphics::ImageImporter));
}

use std::{fmt, marker::PhantomData, task::Poll};

use crate::{
    ecs::{
        action::{ActionEncoder, LocalActionEncoder},
        component::Component,
        entity::EntityId,
        query::{Entities, QueryItem},
    },
    error::Error,
    unfold::{ResourceItem, Unfold},
};

/// Requests an asset from the world's [`Assets`] resource and installs it as a component.
///
/// Register this type with [`crate::unfold::register_unfold`] for each asset type.
/// Missing resources and pending requests retry on subsequent unfolding passes.
/// The marker remains after success or failure; modifying or reinserting it requests
/// the asset again. Cache invalidation alone does not refresh installed components.
///
/// This marker owns the installed `A` and [`AssetLoadError<A>`]. Dropping or replacing
/// it removes both. As with other ECS hooks, extracting it with `World::remove`
/// bypasses this cleanup.
pub struct AssetRef<A: Asset + Component> {
    pub id: AssetId,
    marker: PhantomData<fn() -> A>,
}

impl<A: Asset + Component> AssetRef<A> {
    pub const fn new(id: AssetId) -> Self {
        Self {
            id,
            marker: PhantomData,
        }
    }
}

impl<A: Asset + Component> Copy for AssetRef<A> {}

impl<A: Asset + Component> Clone for AssetRef<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Asset + Component> fmt::Debug for AssetRef<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetRef").field("id", &self.id).finish()
    }
}

impl<A: Asset + Component> Component for AssetRef<A> {
    fn on_drop(&mut self, entity: EntityId, mut actions: LocalActionEncoder) {
        actions.drop::<A>(entity);
        actions.drop::<AssetLoadError<A>>(entity);
    }
}

/// A terminal failure for an entity's typed [`AssetRef`].
pub struct AssetLoadError<A: Asset + Component> {
    pub id: AssetId,
    pub error: Error,
    marker: PhantomData<fn() -> A>,
}

impl<A: Asset + Component> fmt::Debug for AssetLoadError<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetLoadError")
            .field("id", &self.id)
            .field("error", &self.error)
            .finish()
    }
}

impl<A: Asset + Component> Component for AssetLoadError<A> {}

impl<A: Asset + Component> Unfold for AssetRef<A> {
    type Query = Entities;
    type Resources = Option<&'static Assets>;

    fn unfold(
        &self,
        entity: QueryItem<Self::Query>,
        assets: ResourceItem<'_, Self::Resources>,
        mut actions: ActionEncoder,
    ) -> bool {
        match assets.map(|assets| assets.get::<A>(self.id)) {
            None | Some(Poll::Pending) => {
                actions.drop::<A>(entity);
                actions.drop::<AssetLoadError<A>>(entity);
                false
            }
            Some(Poll::Ready(Ok(asset))) => {
                actions.drop::<AssetLoadError<A>>(entity);
                actions.insert(entity, asset);
                true
            }
            Some(Poll::Ready(Err(error))) => {
                actions.drop::<A>(entity);
                actions.insert(
                    entity,
                    AssetLoadError::<A> {
                        id: self.id,
                        error,
                        marker: PhantomData,
                    },
                );
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ecs::{
            system::{IntoSystem, System},
            world::World,
        },
        id::{HasStid, Stid},
        unfold::{register_unfold, unfold_system},
    };
    use std::num::NonZeroU64;

    macro_rules! test_asset {
        ($name:ident, $stid:literal) => {
            #[derive(Clone)]
            struct $name;
            impl Component for $name {}
            impl HasStid for $name {
                fn stid() -> Stid {
                    Stid::new(NonZeroU64::new($stid).unwrap())
                }
                fn stid_dyn(&self) -> Stid {
                    Self::stid()
                }
            }
            impl AssetVersion for $name {
                fn version() -> u64 {
                    1
                }
            }
            impl Asset for $name {
                type Loaded = Self;
                async fn load(_: Box<[u8]>, _: &Assets) -> Result<Self, AssetError> {
                    Ok(Self)
                }
                fn build(loaded: Self, _: &mut AssetBuilder) -> Result<Self, AssetError> {
                    Ok(loaded)
                }
            }
        };
    }

    test_asset!(First, 101);
    test_asset!(Second, 102);

    fn id(value: u64) -> AssetId {
        AssetId::new(NonZeroU64::new(value).unwrap())
    }

    fn error<A: Asset + Component>(id: AssetId) -> AssetLoadError<A> {
        AssetLoadError {
            id,
            error: Error::msg("test failure".into()),
            marker: PhantomData,
        }
    }

    fn has<T: 'static>(world: &mut World, entity: EntityId) -> bool {
        world.get::<Option<&T>>(entity).unwrap().is_some()
    }

    #[test]
    fn missing_resource_retries_and_clears_owned_components() {
        let mut world = World::new();
        register_unfold::<AssetRef<First>>(&mut world);
        let entity = world
            .spawn((AssetRef::<First>::new(id(1)), First, error::<First>(id(1))))
            .id();
        let mut unfold = unfold_system.into_system();
        unfold.run_alone(&mut world);
        assert!(has::<AssetRef<First>>(&mut world, entity));
        assert!(!has::<First>(&mut world, entity));
        assert!(!has::<AssetLoadError<First>>(&mut world, entity));

        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(async {
            world.insert_resource(Assets::new([]));
            unfold.run_alone(&mut world);
            assert!(!has::<AssetLoadError<First>>(&mut world, entity));
            tokio::task::yield_now().await;
            unfold.run_alone(&mut world);
            assert!(has::<AssetLoadError<First>>(&mut world, entity));
        });
    }

    #[test]
    fn failures_stop_retrying_until_marker_changes() {
        let mut world = World::new();
        register_unfold::<AssetRef<First>>(&mut world);
        world.insert_resource(Assets::new([]));
        let entity = world.spawn((AssetRef::<First>::new(id(1)),)).id();
        let mut unfold = unfold_system.into_system();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(async {
            unfold.run_alone(&mut world);
            tokio::task::yield_now().await;
            unfold.run_alone(&mut world);
            assert_eq!(
                world.get::<&AssetLoadError<First>>(entity).unwrap().id,
                id(1)
            );
            world.drop::<AssetLoadError<First>>(entity).unwrap();
            unfold.run_alone(&mut world);
            assert!(!has::<AssetLoadError<First>>(&mut world, entity));

            world.get::<&mut AssetRef<First>>(entity).unwrap().id = id(2);
            world.insert(entity, First).unwrap();
            unfold.run_alone(&mut world);
            assert!(!has::<First>(&mut world, entity));
            tokio::task::yield_now().await;
            unfold.run_alone(&mut world);
            assert_eq!(
                world.get::<&AssetLoadError<First>>(entity).unwrap().id,
                id(2)
            );
            assert!(has::<AssetRef<First>>(&mut world, entity));
        });
    }

    #[test]
    fn replacement_and_drop_clean_owned_components_only() {
        let mut world = World::new();
        let entity = world
            .spawn((
                AssetRef::<First>::new(id(1)),
                First,
                error::<First>(id(1)),
                AssetRef::<Second>::new(id(1)),
                Second,
                error::<Second>(id(1)),
            ))
            .id();
        world.insert(entity, AssetRef::<First>::new(id(2))).unwrap();
        assert!(!has::<First>(&mut world, entity));
        assert!(!has::<AssetLoadError<First>>(&mut world, entity));
        assert!(has::<Second>(&mut world, entity));
        assert!(has::<AssetLoadError<Second>>(&mut world, entity));
        world.insert(entity, First).unwrap();
        world.insert(entity, error::<First>(id(2))).unwrap();
        world.drop::<AssetRef<First>>(entity).unwrap();
        assert!(!has::<First>(&mut world, entity));
        assert!(!has::<AssetLoadError<First>>(&mut world, entity));
        assert!(has::<AssetRef<Second>>(&mut world, entity));
        assert!(has::<Second>(&mut world, entity));
        assert!(has::<AssetLoadError<Second>>(&mut world, entity));
    }

    #[test]
    fn distinct_asset_types_keep_distinct_failures() {
        let mut world = World::new();
        register_unfold::<AssetRef<First>>(&mut world);
        register_unfold::<AssetRef<Second>>(&mut world);
        world.insert_resource(Assets::new([]));
        let entity = world
            .spawn((
                AssetRef::<First>::new(id(1)),
                AssetRef::<Second>::new(id(2)),
            ))
            .id();
        let mut unfold = unfold_system.into_system();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(async {
            unfold.run_alone(&mut world);
            tokio::task::yield_now().await;
            unfold.run_alone(&mut world);
            assert_eq!(
                world.get::<&AssetLoadError<First>>(entity).unwrap().id,
                id(1)
            );
            assert_eq!(
                world.get::<&AssetLoadError<Second>>(entity).unwrap().id,
                id(2)
            );
        });
    }
}
