use super::*;
use arcana_ecs::query::Entities;
use athena::{Matrix3, Motor2, Point2, Vector2};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Component)]
pub struct LocalTransform(pub Motor2<f64>);
#[derive(Clone, Copy, Debug, Component)]
pub struct Transform(pub Motor2<f64>);
arcana_id::has_stid!(LocalTransform = "arcana::scene::dim2::LocalTransform" @ arcana_id);
arcana_id::has_stid!(Transform = "arcana::scene::dim2::Transform" @ arcana_id);

#[derive(Clone, Copy, Debug, Component)]
pub struct Rect {
    pub min: Vector2<f64>,
    pub max: Vector2<f64>,
}
arcana_id::has_stid!(Rect = "arcana::scene::dim2::Rect" @ arcana_id);
impl Default for Rect {
    fn default() -> Self {
        Self {
            min: Vector2::new(-0.5, -0.5),
            max: Vector2::new(0.5, 0.5),
        }
    }
}
reflect_record!(Rect { min: Vector2<f64>, max: Vector2<f64> });
impl Rect {
    pub fn contains(self, point: Vector2<f64>) -> bool {
        point.x >= self.min.x
            && point.y >= self.min.y
            && point.x <= self.max.x
            && point.y <= self.max.y
    }
}

#[derive(Clone, Copy, Debug, Component)]
pub struct Camera {
    /// Center of the visible logical rectangle, in camera-local coordinates.
    pub center: Vector2<f64>,
    /// Minimum visible logical extent. Aspect fitting expands one axis.
    pub size: Vector2<f64>,
}
arcana_id::has_stid!(Camera = "arcana::scene::dim2::Camera" @ arcana_id);
impl Default for Camera {
    fn default() -> Self {
        Self {
            center: Vector2::new(480.0, 360.0),
            size: Vector2::new(960.0, 720.0),
        }
    }
}
reflect_record!(Camera { center: Vector2<f64>, size: Vector2<f64> });
impl Camera {
    /// Logical Y points down; clip Y points up. Visible Z is -1..1, mapped to 0..1.
    pub fn matrices(self, transform: Transform, aspect: f64) -> Option<CameraMatrices> {
        if !aspect.is_finite()
            || aspect <= 0.0
            || !self.size.x.is_finite()
            || !self.size.y.is_finite()
            || self.size.x <= 0.0
            || self.size.y <= 0.0
            || !self.center.x.is_finite()
            || !self.center.y.is_finite()
        {
            return None;
        }
        let width = self.size.x.max(self.size.y * aspect);
        let height = self.size.y.max(self.size.x / aspect);
        Some(CameraMatrices {
            view: super::inverse_rigid(transform.matrix()),
            projection: Matrix4::from_column_arrays([
                [2.0 / width, 0.0, 0.0, 0.0],
                [0.0, -2.0 / height, 0.0, 0.0],
                [0.0, 0.0, 0.5, 0.0],
                [
                    -2.0 * self.center.x / width,
                    2.0 * self.center.y / height,
                    0.5,
                    1.0,
                ],
            ]),
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
                Self(Motor2::from_angle(0.0))
            }
        }
        impl $ty {
            pub fn new(position: Vector2<f64>, angle: f64) -> Self {
                Self(
                    Motor2::from_point_to_point(Point2::ORIGIN, position.into())
                        * Motor2::from_angle(angle),
                )
            }
            pub fn position(self) -> Vector2<f64> {
                self.0.move_point(Point2::ORIGIN).into()
            }
            pub fn angle(self) -> f64 {
                let a: Matrix3<f64> = self.0.into();
                a.arrays()[0][1].atan2(a.arrays()[0][0])
            }
            pub fn transform_point(self, point: Vector2<f64>) -> Vector2<f64> {
                self.0.move_point(point.into()).into()
            }
            pub fn matrix(self) -> Matrix4<f64> {
                let a: Matrix3<f64> = self.0.into();
                let a = a.arrays();
                Matrix4::from_column_arrays([
                    [a[0][0], a[0][1], 0.0, 0.0],
                    [a[1][0], a[1][1], 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [a[2][0], a[2][1], 0.0, 1.0],
                ])
            }
            pub fn inverse(self) -> Self {
                let a = -self.angle();
                let p = self.position();
                let (s, c) = a.sin_cos();
                Self::new(Vector2::new(-c * p.x + s * p.y, -s * p.x - c * p.y), a)
            }
        }
        impl Reflect for $ty {
            fn model() -> Model {
                Pose::model()
            }
            fn to_value(&self) -> Value {
                Pose {
                    position: self.position(),
                    angle: self.angle(),
                }
                .to_value()
            }
            fn from_value(value: Value) -> Result<Self, ValueError> {
                let p = Pose::from_value(value)?;
                if !p.position.x.is_finite() || !p.position.y.is_finite() || !p.angle.is_finite() {
                    return Err(ValueError::Custom("Transform must be finite".into()));
                }
                Ok(Self::new(p.position, p.angle))
            }
        }
    };
}
struct Pose {
    position: Vector2<f64>,
    angle: f64,
}
reflect_record!(Pose {position:Vector2<f64>,angle:f64});
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
        locals: &HashMap<EntityId, Motor2<f64>>,
        resolved: &mut HashMap<EntityId, Motor2<f64>>,
        active: &mut HashSet<EntityId>,
    ) -> Result<Motor2<f64>, TransformError> {
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
