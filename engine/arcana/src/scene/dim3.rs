use super::*;
use arcana_ecs::query::Entities;
use athena::{Motor3, Point3, Rotor3, Translator3, Vector3};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Component)]
pub struct LocalTransform(pub Motor3<f64>);
#[derive(Clone, Copy, Debug, Component)]
pub struct Transform(pub Motor3<f64>);
arcana_id::has_stid!(LocalTransform = "arcana::scene::dim3::LocalTransform" @ arcana_id);
arcana_id::has_stid!(Transform = "arcana::scene::dim3::Transform" @ arcana_id);
#[derive(Clone, Copy, Debug, Component)]
pub struct Cuboid {
    pub min: Vector3<f64>,
    pub max: Vector3<f64>,
}
arcana_id::has_stid!(Cuboid = "arcana::scene::dim3::Cuboid" @ arcana_id);
impl Default for Cuboid {
    fn default() -> Self {
        Self {
            min: Vector3::new(-0.5, -0.5, -0.5),
            max: Vector3::new(0.5, 0.5, 0.5),
        }
    }
}
reflect_record!(Cuboid {min:Vector3<f64>,max:Vector3<f64>});

/// Camera looks along negative Z, positive Y is up. Clip depth is 0..1.
#[derive(Clone, Copy, Debug, Component)]
pub struct Camera {
    /// Perspective when true, orthographic otherwise.
    pub perspective: bool,
    pub vertical_fov: f64,
    pub height: f64,
    pub near: f64,
    pub far: f64,
}
arcana_id::has_stid!(Camera = "arcana::scene::dim3::Camera" @ arcana_id);
impl Default for Camera {
    fn default() -> Self {
        Self {
            perspective: true,
            vertical_fov: std::f64::consts::FRAC_PI_3,
            height: 10.0,
            near: 0.1,
            far: 1000.0,
        }
    }
}
reflect_record!(Camera {
    perspective: bool,
    vertical_fov: f64,
    height: f64,
    near: f64,
    far: f64
});
impl Camera {
    pub fn matrices(self, transform: Transform, aspect: f64) -> Option<CameraMatrices> {
        if !aspect.is_finite()
            || aspect <= 0.0
            || !self.near.is_finite()
            || !self.far.is_finite()
            || self.near <= 0.0
            || self.far <= self.near
        {
            return None;
        }
        let projection = if self.perspective {
            if !self.vertical_fov.is_finite()
                || self.vertical_fov <= 0.0
                || self.vertical_fov >= std::f64::consts::PI
            {
                return None;
            }
            let y = 1.0 / (self.vertical_fov * 0.5).tan();
            let z = self.far / (self.near - self.far);
            Matrix4::from_column_arrays([
                [y / aspect, 0.0, 0.0, 0.0],
                [0.0, y, 0.0, 0.0],
                [0.0, 0.0, z, -1.0],
                [0.0, 0.0, self.near * z, 0.0],
            ])
        } else {
            if !self.height.is_finite() || self.height <= 0.0 {
                return None;
            }
            Matrix4::from_column_arrays([
                [2.0 / (self.height * aspect), 0.0, 0.0, 0.0],
                [0.0, 2.0 / self.height, 0.0, 0.0],
                [0.0, 0.0, 1.0 / (self.near - self.far), 0.0],
                [0.0, 0.0, self.near / (self.near - self.far), 1.0],
            ])
        };
        Some(CameraMatrices {
            view: super::inverse_rigid(transform.matrix()),
            projection,
        })
    }
}
pub fn camera_matrices(world: &World, entity: EntityId, aspect: f64) -> Option<CameraMatrices> {
    let cameras = world.view::<&Camera>();
    let camera = *cameras.try_get(entity).ok()?;
    let transforms = world.view::<&Transform>();
    let transform = transforms.try_get(entity).copied().unwrap_or_default();
    camera.matrices(transform, aspect)
}
macro_rules! transform_impl {
    ($ty:ident) => {
        impl Default for $ty {
            fn default() -> Self {
                Self(Motor3::from(Rotor3::IDENTITY))
            }
        }
        impl $ty {
            pub fn new(position: Vector3<f64>, rotation_vector: Vector3<f64>) -> Self {
                let angle = (rotation_vector.x * rotation_vector.x
                    + rotation_vector.y * rotation_vector.y
                    + rotation_vector.z * rotation_vector.z)
                    .sqrt();
                let rotor = if angle > 0.0 {
                    Rotor3::from_axis_angle(
                        Point3::at(rotation_vector.x, rotation_vector.y, rotation_vector.z),
                        angle,
                    )
                } else {
                    Rotor3::IDENTITY
                };
                let translation =
                    Motor3::from(Translator3::from_offset(position.x, position.y, position.z));

                Self(translation * Motor3::from(rotor))
            }
            pub fn position(self) -> Vector3<f64> {
                self.0.move_point(Point3::ORIGIN).into()
            }
            pub fn rotation_vector(self) -> Vector3<f64> {
                let b = self.0.bivector();
                let mut w = self.0.scalar().0;
                let mut v = Vector3::new(-b.e23, -b.e31, -b.e12);
                if w < 0.0 {
                    w = -w;
                    v = v * -1.0;
                }
                let length = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
                if length < 1e-12 {
                    Vector3::new(0.0, 0.0, 0.0)
                } else {
                    v * (2.0 * length.atan2(w) / length)
                }
            }
            pub fn matrix(self) -> Matrix4<f64> {
                self.0.into()
            }
            pub fn inverse(self) -> Self {
                let rotation = self.rotation_vector() * -1.0;
                let inverse_rotation = Self::new(Vector3::new(0.0, 0.0, 0.0), rotation);
                Self::new(
                    inverse_rotation.transform_point(self.position() * -1.0),
                    rotation,
                )
            }
            pub fn transform_point(self, point: Vector3<f64>) -> Vector3<f64> {
                self.0.move_point(point.into()).into()
            }
        }
        impl Reflect for $ty {
            fn model() -> Model {
                Pose::model()
            }
            fn to_value(&self) -> Value {
                Pose {
                    position: self.position(),
                    rotation: self.rotation_vector(),
                }
                .to_value()
            }
            fn from_value(value: Value) -> Result<Self, ValueError> {
                let p = Pose::from_value(value)?;
                if ![
                    p.position.x,
                    p.position.y,
                    p.position.z,
                    p.rotation.x,
                    p.rotation.y,
                    p.rotation.z,
                ]
                .into_iter()
                .all(f64::is_finite)
                {
                    return Err(ValueError::Custom("Transform must be finite".into()));
                }
                Ok(Self::new(p.position, p.rotation))
            }
        }
    };
}
struct Pose {
    position: Vector3<f64>,
    rotation: Vector3<f64>,
}
reflect_record!(Pose {position:Vector3<f64>,rotation:Vector3<f64>});
transform_impl!(LocalTransform);
transform_impl!(Transform);
pub(super) fn derive_transforms(
    world: &World,
) -> Result<Vec<(EntityId, Transform)>, TransformError> {
    let locals: HashMap<_, _> = world
        .view::<(Entities, &LocalTransform)>()
        .iter()
        .map(|(e, t)| (e.id(), t.0))
        .collect();
    let mut resolved: HashMap<_, _> = world
        .view::<(Entities, &Transform)>()
        .iter()
        .filter(|(e, _)| !locals.contains_key(&e.id()))
        .map(|(e, t)| (e.id(), t.0))
        .collect();
    let mut active = HashSet::new();
    fn resolve(
        e: EntityId,
        world: &World,
        locals: &HashMap<EntityId, Motor3<f64>>,
        resolved: &mut HashMap<EntityId, Motor3<f64>>,
        active: &mut HashSet<EntityId>,
    ) -> Result<Motor3<f64>, TransformError> {
        if let Some(t) = resolved.get(&e) {
            return Ok(*t);
        }
        let local = *locals
            .get(&e)
            .ok_or(TransformError::MissingParentTransform(e))?;
        if !active.insert(e) {
            return Err(TransformError::Cycle(e));
        }
        let t = match parent(world, e) {
            Some(p) => resolve(p, world, locals, resolved, active)? * local,
            None => local,
        };
        active.remove(&e);
        resolved.insert(e, t);
        Ok(t)
    }
    let mut result = Vec::with_capacity(locals.len());
    for &e in locals.keys() {
        result.push((
            e,
            Transform(resolve(e, world, &locals, &mut resolved, &mut active)?),
        ));
    }
    Ok(result)
}

/// Applies a world-space edit to the authored local transform when present.
pub fn set_world_transform(
    world: &mut World,
    entity: EntityId,
    transform: Transform,
) -> Result<(), TransformError> {
    if world.view::<&LocalTransform>().try_get(entity).is_ok() {
        let local = if let Some(p) = parent(world, entity) {
            let basis = world
                .view::<&Transform>()
                .try_get(p)
                .copied()
                .map_err(|_| TransformError::MissingParentTransform(p))?;
            LocalTransform(basis.inverse().0 * transform.0)
        } else {
            LocalTransform(transform.0)
        };
        world
            .insert(entity, local)
            .map_err(|_| TransformError::MissingEntity(entity))?;
    }
    world
        .insert(entity, transform)
        .map_err(|_| TransformError::MissingEntity(entity))?;
    Ok(())
}
