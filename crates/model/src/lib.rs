mod model;
mod value;

use athena::{Matrix2, Matrix3, Matrix4, Vector2, Vector3, Vector4};
use edict::entity::EntityId;
use gametime::TimeSpan;
use palette::{Hsv, Hsva, LinLuma, LinLumaa, Srgb, Srgba};
use smol_str::SmolStr;

pub use self::{
    model::{ColorModel, Model},
    value::{ColorValue, Value},
};

/// Trait for types that matches some data model.
///
/// It is expected that values of the type can be converted to and from [`Value`] with that model.
/// If type implements [`Serialize`], it should be able to serialize to a [`Value`] with that model.
/// If type implements [`Deserialize`], it should be able to deserialize from a [`Value`] with that model.
pub trait TypeModel: Clone + Send + Sync + 'static {
    /// Returns data model that describes the type.
    fn model() -> Model
    where
        Self: Sized;

    /// Converts value to a [`Value`] with the model of the type.
    fn try_from_value(value: Value) -> Option<Self>
    where
        Self: Sized;

    /// Converts value to a [`Value`] with the model of the type.
    fn try_clone_from_value(value: &Value) -> Option<Self>
    where
        Self: Sized;

    /// Converts value to a [`Value`] with the model of the type.
    fn from_value(value: Value) -> Self
    where
        Self: Sized,
    {
        Self::try_from_value(value).expect("Value must match the type's model")
    }

    /// Converts value to a [`Value`] with the model of the type.
    fn clone_from_value(value: &Value) -> Self
    where
        Self: Sized,
    {
        Self::try_clone_from_value(value).expect("Value must match the type's model")
    }
}

impl TypeModel for () {
    fn model() -> Model {
        Model::Unit
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::Unit => Some(()),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        Self::try_from_value(value.clone())
    }
}

impl TypeModel for bool {
    fn model() -> Model {
        Model::Bool
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::Bool(b) => Some(b),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        Self::try_from_value(value.clone())
    }
}

impl TypeModel for i64 {
    fn model() -> Model {
        Model::Int
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::Int(i) => Some(i),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        Self::try_from_value(value.clone())
    }
}

impl TypeModel for f64 {
    fn model() -> Model {
        Model::Float
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::Float(f) => Some(f),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        Self::try_from_value(value.clone())
    }
}

impl TypeModel for SmolStr {
    fn model() -> Model {
        Model::String
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::String(s) => Some(s.clone()),
            _ => None,
        }
    }
}

impl TypeModel for LinLuma {
    fn model() -> Model {
        Model::Color(ColorModel::Luma)
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::Color(ColorValue::Luma(luma)) => Some(luma),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Color(ColorValue::Luma(luma)) => Some(luma.clone()),
            _ => None,
        }
    }
}

impl TypeModel for LinLumaa {
    fn model() -> Model {
        Model::Color(ColorModel::Lumaa)
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::Color(ColorValue::Lumaa(lumaa)) => Some(lumaa),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Color(ColorValue::Lumaa(lumaa)) => Some(lumaa.clone()),
            _ => None,
        }
    }
}

impl TypeModel for Srgb {
    fn model() -> Model {
        Model::Color(ColorModel::Srgb)
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::Color(ColorValue::Srgb(srgb)) => Some(srgb),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Color(ColorValue::Srgb(srgb)) => Some(srgb.clone()),
            _ => None,
        }
    }
}

impl TypeModel for Srgba {
    fn model() -> Model {
        Model::Color(ColorModel::Srgba)
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::Color(ColorValue::Srgba(srgba)) => Some(srgba),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Color(ColorValue::Srgba(srgba)) => Some(srgba.clone()),
            _ => None,
        }
    }
}

impl TypeModel for Hsv {
    fn model() -> Model {
        Model::Color(ColorModel::Hsv)
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::Color(ColorValue::Hsv(hsv)) => Some(hsv),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Color(ColorValue::Hsv(hsv)) => Some(hsv.clone()),
            _ => None,
        }
    }
}

impl TypeModel for Hsva {
    fn model() -> Model {
        Model::Color(ColorModel::Hsva)
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::Color(ColorValue::Hsva(hsva)) => Some(hsva),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Color(ColorValue::Hsva(hsva)) => Some(hsva.clone()),
            _ => None,
        }
    }
}

impl TypeModel for TimeSpan {
    fn model() -> Model {
        Model::TimeSpan
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::TimeSpan(ts) => Some(ts),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::TimeSpan(ts) => Some(ts.clone()),
            _ => None,
        }
    }
}

impl TypeModel for EntityId {
    fn model() -> Model {
        Model::Entity
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::Entity(id) => Some(id),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Entity(id) => Some(id.clone()),
            _ => None,
        }
    }
}

impl TypeModel for Vector2<f64> {
    fn model() -> Model {
        Model::Vec2
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::Vec2(v) => Some(v),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Vec2(v) => Some(v.clone()),
            _ => None,
        }
    }
}

impl TypeModel for Vector3<f64> {
    fn model() -> Model {
        Model::Vec3
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::Vec3(v) => Some(v),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Vec3(v) => Some(v.clone()),
            _ => None,
        }
    }
}

impl TypeModel for Vector4<f64> {
    fn model() -> Model {
        Model::Vec4
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::Vec4(v) => Some(v),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Vec4(v) => Some(v.clone()),
            _ => None,
        }
    }
}

impl TypeModel for Matrix2<f64> {
    fn model() -> Model {
        Model::Mat2
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::Mat2(m) => Some(m),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Mat2(m) => Some(m.clone()),
            _ => None,
        }
    }
}

impl TypeModel for Matrix3<f64> {
    fn model() -> Model {
        Model::Mat3
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::Mat3(m) => Some(m),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Mat3(m) => Some(m.clone()),
            _ => None,
        }
    }
}

impl TypeModel for Matrix4<f64> {
    fn model() -> Model {
        Model::Mat4
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::Mat4(m) => Some(m),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Mat4(m) => Some(m.clone()),
            _ => None,
        }
    }
}

impl<T> TypeModel for Option<T>
where
    T: TypeModel,
{
    fn model() -> Model {
        Model::Option(Some(Box::new(T::model())))
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::Option(Some(v)) => T::try_from_value(*v).map(Some),
            Value::Option(None) => Some(None),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Option(Some(v)) => T::try_clone_from_value(&*v).map(Some),
            Value::Option(None) => Some(None),
            _ => None,
        }
    }
}

impl<T> TypeModel for Vec<T>
where
    T: TypeModel,
{
    fn model() -> Model {
        Model::Array {
            elem: Some(Box::new(T::model())),
            len: None,
        }
    }

    fn try_from_value(value: Value) -> Option<Self> {
        match value {
            Value::Array(arr) => arr
                .into_iter()
                .map(T::try_from_value)
                .collect::<Option<Vec<_>>>(),
            _ => None,
        }
    }

    fn try_clone_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Array(arr) => arr
                .iter()
                .map(T::try_clone_from_value)
                .collect::<Option<Vec<_>>>(),
            _ => None,
        }
    }
}
