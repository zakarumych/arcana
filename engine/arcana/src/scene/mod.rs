//! Shared rigid spatial components. Column vectors, right-handed 3D, clip depth 0..1.
use arcana_ecs::{
    component::Component,
    entity::EntityId,
    query::With,
    relation::{ChildOf, RelatesExclusive},
    world::World,
};
use arcana_model::{Model, Reflect, Value, ValueError};
use athena::Matrix4;

macro_rules! reflect_record {
    ($ty:ty { $($field:ident : $ft:ty),+ $(,)? }) => {
        impl Reflect for $ty {
            fn model() -> Model {
                Model::Record(vec![$(
                    (arcana_intern::ident!($field), Some(<$ft>::model()))
                ),+])
            }
            fn to_value(&self) -> Value {
                Value::Map([$(
                    (stringify!($field).into(), self.$field.to_value())
                ),+].into_iter().collect())
            }
            fn from_value(value: Value) -> Result<Self, ValueError> {
                let Value::Map(mut fields) = value else {
                    return Err(ValueError::Custom("Expected record".into()));
                };
                Ok(Self { $(
                    $field: <$ft>::from_value(
                        fields.remove(stringify!($field)).ok_or_else(|| {
                            ValueError::Custom(concat!("Missing ", stringify!($field)).into())
                        })?
                    )?
                ),+ })
            }
        }
    };
}

pub mod dim2;
pub mod dim3;

#[derive(Clone, Debug, Default, Component)]
pub struct Name(pub smol_str::SmolStr);
arcana_id::has_stid!(Name = "arcana::scene::Name" @ arcana_id);
impl Reflect for Name {
    fn model() -> Model {
        Model::String
    }
    fn to_value(&self) -> Value {
        self.0.to_value()
    }
    fn from_value(value: Value) -> Result<Self, ValueError> {
        Ok(Self(smol_str::SmolStr::from_value(value)?))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CameraMatrices {
    pub view: Matrix4<f64>,
    pub projection: Matrix4<f64>,
}
impl CameraMatrices {
    pub fn view_projection(self) -> Matrix4<f64> {
        self.projection * self.view
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformError {
    Cycle(EntityId),
    MissingParentTransform(EntityId),
    MissingEntity(EntityId),
}
impl std::fmt::Display for TransformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for TransformError {}

pub fn parent(world: &World, entity: EntityId) -> Option<EntityId> {
    world
        .view::<RelatesExclusive<With<ChildOf>>>()
        .try_get(entity)
        .ok()
        .map(|p| p.id())
}

/// Derives transforms parent first. A parent lacking a matching dimensional transform is an error.
/// Standalone world transforms are preserved. No writes occur unless both hierarchies validate.
pub fn update_transforms(world: &mut World) -> Result<(), TransformError> {
    let dim2 = dim2::derive_transforms(world)?;
    let dim3 = dim3::derive_transforms(world)?;
    for (entity, transform) in dim2 {
        world
            .insert(entity, transform)
            .map_err(|_| TransformError::MissingEntity(entity))?;
    }
    for (entity, transform) in dim3 {
        world
            .insert(entity, transform)
            .map_err(|_| TransformError::MissingEntity(entity))?;
    }
    Ok(())
}

/// Registers engine-owned scene component descriptors in an empty plugin hub.
pub fn register_components(hub: &mut arcana_plugin::PluginsHub) {
    fn add<T: Component + Reflect + arcana_id::HasStid + Default>(
        hub: &mut arcana_plugin::PluginsHub,
        name: &str,
    ) {
        hub.add_component(arcana_plugin::ComponentInfo::new::<T>(
            arcana_intern::Name::from_str(name).expect("valid built-in component name"),
            || T::default().to_value(),
        ));
    }
    add::<Name>(hub, "Name");
    add::<dim2::LocalTransform>(hub, "Local Transform 2D");
    add::<dim2::Transform>(hub, "Transform 2D");
    add::<dim2::Rect>(hub, "Rect");
    add::<dim2::Camera>(hub, "Camera 2D");
    add::<dim3::LocalTransform>(hub, "Local Transform 3D");
    add::<dim3::Transform>(hub, "Transform 3D");
    add::<dim3::Cuboid>(hub, "Cuboid");
    add::<dim3::Camera>(hub, "Camera 3D");
}

/// Inverse of a rigid affine matrix.
fn inverse_rigid(matrix: Matrix4<f64>) -> Matrix4<f64> {
    let a = matrix.arrays();
    let mut result = Matrix4::identity();
    let r = result.arrays_mut();
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = a[j][i];
        }
    }
    for j in 0..3 {
        r[3][j] = -(r[0][j] * a[3][0] + r[1][j] * a[3][1] + r[2][j] * a[3][2]);
    }
    result
}

#[cfg(test)]
mod tests;
