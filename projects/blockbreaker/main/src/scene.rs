use crate::GRAPH_ID;
use arcana::scene::Name;
use arcana::{
    athena::Vector2,
    ecs::{entity::EntityId, world::World},
    id::HasStid,
    model::{Model, Reflect, Value, ValueError},
    plugin::{ArcanaPlugin, ComponentError, SceneComponents},
    scene::dim2::{Camera, LocalTransform, Rect, Transform},
};

macro_rules! property_component {
    ($name:ident, $field:ident, $ty:ty, $default:expr) => {
        #[derive(Clone, Copy, Debug, arcana::Component, arcana::HasStid)]
        #[stid(concat!("blockbreaker::", stringify!($name)))]
        pub struct $name {
            pub $field: $ty,
        }
        impl Default for $name {
            fn default() -> Self {
                Self { $field: $default }
            }
        }
        impl Reflect for $name {
            fn model() -> Model {
                Model::Record(vec![(arcana::ident!($field), Some(Model::Float))])
            }
            fn to_value(&self) -> Value {
                Value::Map(
                    [(stringify!($field).into(), Value::Float(self.$field as f64))]
                        .into_iter()
                        .collect(),
                )
            }
            fn from_value(v: Value) -> Result<Self, ValueError> {
                {
                    let Value::Map(mut fields) = v else {
                        return Err(ValueError::Custom("Expected component fields".into()));
                    };
                    let value =
                        f64::from_value(fields.remove(stringify!($field)).ok_or_else(|| {
                            ValueError::Custom("Missing component field".into())
                        })?)?;
                    if !value.is_finite() || value < 0.0 || value > <$ty>::MAX as f64 {
                        return Err(ValueError::Custom(
                            "Expected finite nonnegative value within component range".into(),
                        ));
                    }
                    Ok(Self {
                        $field: value as $ty,
                    })
                }
            }
        }
    };
}
#[derive(Clone, Copy, Debug, arcana::Component, arcana::HasStid)]
#[stid("blockbreaker::Block")]
pub struct Block {
    pub points: u32,
}
impl Default for Block {
    fn default() -> Self {
        Self { points: 50 }
    }
}
impl Reflect for Block {
    fn model() -> Model {
        Model::Record(vec![(arcana::ident!(points), Some(Model::Int))])
    }
    fn to_value(&self) -> Value {
        Value::Map(
            [("points".into(), Value::Int(self.points as i64))]
                .into_iter()
                .collect(),
        )
    }
    fn from_value(value: Value) -> Result<Self, ValueError> {
        let Value::Map(mut fields) = value else {
            return Err(ValueError::Custom("Expected Block fields".into()));
        };
        let points = i64::from_value(
            fields
                .remove("points")
                .ok_or_else(|| ValueError::Custom("Missing points".into()))?,
        )?;
        Ok(Self {
            points: u32::try_from(points).map_err(|_| {
                ValueError::Custom("Points must be between 0 and 4294967295".into())
            })?,
        })
    }
}
property_component!(Paddle, speed, f32, 610.0);
property_component!(TitleWidget, size, f32, 4.0);
property_component!(ObjectiveWidget, size, f32, 2.0);
property_component!(ScoreWidget, size, f32, 3.0);
property_component!(LivesWidget, size, f32, 3.0);
property_component!(StatusWidget, size, f32, 3.0);
property_component!(InstructionWidget, size, f32, 2.0);

pub fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        min: Vector2::new(x, y),
        max: Vector2::new(x + w, y + h),
    }
}
fn defaults(
    world: &mut World,
    id: EntityId,
    position: [f64; 2],
    bounds: Rect,
    name: &str,
) -> Result<(), ComponentError> {
    let mut added = Vec::new();
    if world.get::<&Name>(id).is_err() {
        world
            .insert(id, Name(name.into()))
            .map_err(arcana::ecs::EntityError::from)?;
        added.push(Name::stid());
    }
    if world.get::<&LocalTransform>(id).is_err() && world.get::<&Transform>(id).is_err() {
        world
            .insert(
                id,
                LocalTransform::new(Vector2::new(position[0], position[1]), 0.0),
            )
            .map_err(arcana::ecs::EntityError::from)?;
        added.push(LocalTransform::stid());
    }
    if world.get::<&Rect>(id).is_err() {
        world
            .insert(id, bounds)
            .map_err(arcana::ecs::EntityError::from)?;
        added.push(Rect::stid());
    }
    if world.get::<&SceneComponents>(id).is_err() {
        world
            .insert(id, SceneComponents::default())
            .map_err(arcana::ecs::EntityError::from)?;
    }
    world
        .get::<&mut SceneComponents>(id)?
        .components
        .extend(added);
    Ok(())
}
macro_rules! register {
    ($plugin:ident, $ty:ty, $name:literal, $pos:expr, $bounds:expr) => {
        $plugin
            .add_component::<$ty>(arcana::name!($name), || <$ty>::default().to_value())
            .with_add_callback(|world, id| {
                defaults(world, id, $pos, $bounds, $name)
                    .expect("newly added component owns a live entity");
            });
    };
}
pub fn register(plugin: &mut ArcanaPlugin) {
    register!(
        plugin,
        Block,
        "Block",
        [440.0, 240.0],
        rect(0.0, 0.0, 76.0, 24.0)
    );
    register!(
        plugin,
        Paddle,
        "Paddle",
        [480.0, 624.0],
        rect(-60.0, 0.0, 120.0, 12.0)
    );
    register!(
        plugin,
        TitleWidget,
        "Title widget",
        [40.0, 30.0],
        rect(0.0, 0.0, 306.0, 28.0)
    );
    register!(
        plugin,
        ObjectiveWidget,
        "Objective widget",
        [42.0, 70.0],
        rect(0.0, 0.0, 166.0, 14.0)
    );
    register!(
        plugin,
        ScoreWidget,
        "Score widget",
        [560.0, 28.0],
        rect(0.0, 0.0, 140.0, 42.0)
    );
    register!(
        plugin,
        LivesWidget,
        "Lives widget",
        [786.0, 28.0],
        rect(0.0, 0.0, 120.0, 42.0)
    );
    register!(
        plugin,
        StatusWidget,
        "Status widget",
        [270.0, 410.0],
        rect(0.0, 0.0, 420.0, 90.0)
    );
    register!(
        plugin,
        InstructionWidget,
        "Instruction widget",
        [193.0, 688.0],
        rect(0.0, 0.0, 574.0, 14.0)
    );
    plugin.add_scene_init(seed);
}
fn spawn<T: arcana::ecs::component::Component + HasStid>(
    world: &mut World,
    component: T,
    x: f64,
    y: f64,
    bounds: Rect,
    name: &str,
) {
    world.spawn((
        component,
        Name(name.into()),
        LocalTransform::new(Vector2::new(x, y), 0.0),
        bounds,
        SceneComponents::default()
            .with::<T>()
            .with::<Name>()
            .with::<LocalTransform>()
            .with::<Rect>(),
    ));
}
pub fn seed(world: &mut World) {
    world.spawn((
        Name("Camera".into()),
        arcana::render::Renderer { graph: GRAPH_ID },
        Camera::default(),
        LocalTransform::default(),
        SceneComponents::default()
            .with::<Name>()
            .with::<arcana::render::Renderer>()
            .with::<Camera>()
            .with::<LocalTransform>(),
    ));
    for row in 0..5 {
        for col in 0..10 {
            spawn(
                world,
                Block {
                    points: (5 - row) * 10,
                },
                64.0 + col as f64 * 84.0,
                154.0 + row as f64 * 34.0,
                rect(0.0, 0.0, 76.0, 24.0),
                &format!("Block {}", row * 10 + col + 1),
            );
        }
    }
    spawn(
        world,
        Paddle::default(),
        480.0,
        624.0,
        rect(-60.0, 0.0, 120.0, 12.0),
        "Paddle",
    );
    spawn(
        world,
        TitleWidget::default(),
        40.0,
        30.0,
        rect(0.0, 0.0, 306.0, 28.0),
        "Title",
    );
    spawn(
        world,
        ObjectiveWidget::default(),
        42.0,
        70.0,
        rect(0.0, 0.0, 166.0, 14.0),
        "Objective",
    );
    spawn(
        world,
        ScoreWidget::default(),
        560.0,
        28.0,
        rect(0.0, 0.0, 140.0, 42.0),
        "Score",
    );
    spawn(
        world,
        LivesWidget::default(),
        786.0,
        28.0,
        rect(0.0, 0.0, 120.0, 42.0),
        "Lives",
    );
    spawn(
        world,
        StatusWidget::default(),
        270.0,
        410.0,
        rect(0.0, 0.0, 420.0, 90.0),
        "Status",
    );
    spawn(
        world,
        InstructionWidget::default(),
        193.0,
        688.0,
        rect(0.0, 0.0, 574.0, 14.0),
        "Instructions",
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn additions_author_companions_but_reconstruction_preserves_missing_bounds() {
        let plugin = crate::arcana_plugin::get();
        let desc = plugin
            .components()
            .iter()
            .find(|d| d.id() == Block::stid())
            .expect("Block registered");
        let mut world = World::new();
        let id = world
            .spawn_one(SceneComponents::default().with::<Block>())
            .id();
        desc.insert(&mut world, id, desc.initial_value().expect("Block factory"))
            .expect("insert Block");
        assert!(world.get::<&Rect>(id).is_err());
        desc.initialize_added(&mut world, id);
        assert!(world.get::<&Rect>(id).is_ok());
        assert!(world.get::<&LocalTransform>(id).is_ok());
        assert!(world
            .get::<&SceneComponents>(id)
            .expect("authored marker")
            .components
            .contains(&Rect::stid()));
        let authored = LocalTransform::new(Vector2::new(123.0, 456.0), 0.6);
        world.insert(id, authored).expect("alive");
        desc.initialize_added(&mut world, id);
        assert_eq!(
            world.get::<&LocalTransform>(id).expect("local").position(),
            authored.position()
        );
    }
}
