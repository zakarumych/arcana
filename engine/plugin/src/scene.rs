use arcana_id::{HasStid, Stid};
use edict::component::Component;
use hashbrown::HashSet;

/// Presence marks an authored entity; only listed components belong to its scene data.
#[derive(Component, Default)]
pub struct SceneComponents {
    pub components: HashSet<Stid>,
}

impl SceneComponents {
    /// Includes a component in the authored scene data.
    pub fn with<T: HasStid>(mut self) -> Self {
        self.components.insert(T::stid());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArcanaPlugin, ComponentDesc, PluginsHub};
    use arcana_model::{Model, Reflect, Value, ValueError};
    use edict::{entity::EntityId, world::World};

    #[derive(Component)]
    struct Number(i64);
    arcana_id::has_stid!(Number = "arcana-plugin::tests::Number" @ arcana_id);
    impl Reflect for Number {
        fn model() -> Model {
            Model::Int
        }
        fn to_value(&self) -> Value {
            self.0.to_value()
        }
        fn from_value(value: Value) -> Result<Self, ValueError> {
            Ok(Self(i64::from_value(value)?))
        }
    }
    #[derive(Component)]
    struct Companion(i64);
    arcana_id::has_stid!(Companion = "arcana-plugin::tests::Companion" @ arcana_id);
    fn initialize(world: &mut World, entity: EntityId) {
        if world.get::<&Companion>(entity).is_err() {
            world
                .insert(entity, Companion(7))
                .expect("live authored entity");
            world
                .get::<&mut SceneComponents>(entity)
                .expect("authored marker")
                .components
                .insert(Companion::stid());
        }
    }

    #[test]
    fn snapshot_insert_does_not_initialize_companions() {
        let mut desc = ComponentDesc::new::<Number>(arcana_intern::name!(Number), || Value::Int(1));
        desc.with_add_callback(initialize);
        let mut world = World::new();
        let entity = world.spawn_one(SceneComponents::default()).id();
        desc.insert(&mut world, entity, Value::Int(3))
            .expect("snapshot insert");
        assert!(world.get::<&Companion>(entity).is_err());
        world
            .get::<&mut SceneComponents>(entity)
            .expect("marker")
            .components
            .insert(Number::stid());
        desc.initialize_added(&mut world, entity);
        assert_eq!(world.get::<&Companion>(entity).expect("companion").0, 7);
        assert!(
            world
                .get::<&SceneComponents>(entity)
                .expect("marker")
                .components
                .contains(&Companion::stid())
        );
        world.get::<&mut Companion>(entity).expect("companion").0 = 42;
        desc.initialize_added(&mut world, entity);
        assert_eq!(
            world
                .get::<&Companion>(entity)
                .expect("companion preserved")
                .0,
            42
        );
    }

    #[test]
    fn runtime_initialization_does_not_seed_authored_defaults() {
        let mut plugin = ArcanaPlugin::new(None);
        plugin.add_init(|world| {
            world.insert_resource(42_i64);
        });
        plugin.add_scene_init(|world| {
            world.spawn((Number(1), SceneComponents::default().with::<Number>()));
        });
        let mut world = World::new();
        let mut hub = PluginsHub::new();
        plugin.init(&mut world, &mut hub);
        assert_eq!(
            *world.get_resource::<i64>().expect("runtime initialized"),
            42
        );
        assert_eq!(world.view::<&SceneComponents>().iter().count(), 0);
        plugin.seed_scene(&mut world);
        assert_eq!(world.view::<&SceneComponents>().iter().count(), 1);
    }
}
