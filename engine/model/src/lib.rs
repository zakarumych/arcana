//! Data model for type-erased and runtime-typed values.
//!
//! Supports serialization and UI editing for all modelled types.

mod model;
mod value;

use athena::{Matrix2, Matrix3, Matrix4, Vector2, Vector3, Vector4};
use edict::entity::EntityId;
use gametime::TimeSpan;
use palette::{Hsv, Hsva, LinLuma, LinLumaa, Srgb, Srgba};
use smol_str::SmolStr;

pub use self::{
    model::{ColorModel, Model},
    value::{ColorValue, Value, ValueError, serialize_to_value},
};

/// A type described by a data model and convertible to and from [`Value`].
///
/// [`Reflect::to_value`] must produce a value matching [`Reflect::model`], and
/// passing that value to [`Reflect::from_value`] must reconstruct an equivalent
/// value of the type. Reflection does not require a serde implementation.
pub trait Reflect: Sized {
    /// Returns the data model that describes the type.
    fn model() -> Model;

    /// Converts this value to its model representation.
    fn to_value(&self) -> Value;

    /// Reconstructs a value, returning an error if the representation is invalid.
    fn from_value(value: Value) -> Result<Self, ValueError>;

    /// Reconstructs a value from a borrowed model representation.
    fn clone_from_value(value: &Value) -> Result<Self, ValueError> {
        Self::from_value(value.clone())
    }

    /// Applies a model representation to this value.
    ///
    /// Implementations must leave `self` unchanged when returning an error.
    fn apply(&mut self, value: Value) -> Result<(), ValueError> {
        *self = Self::from_value(value)?;
        Ok(())
    }
}

fn unexpected_value(value: &Value, expected: &str) -> ValueError {
    let actual = match value {
        Value::Color(color) => format!("Color({})", color.kind()),
        _ => value.kind().to_owned(),
    };
    ValueError::Custom(format!("Expected {expected}, found {actual}"))
}

impl Reflect for () {
    fn model() -> Model {
        Model::Unit
    }

    fn to_value(&self) -> Value {
        Value::Unit
    }

    fn from_value(value: Value) -> Result<Self, ValueError> {
        match value {
            Value::Unit => Ok(()),
            value => Err(unexpected_value(&value, "Unit")),
        }
    }
}

macro_rules! reflect_native {
    ($ty:ty, $variant:ident) => {
        impl Reflect for $ty {
            fn model() -> Model {
                Model::$variant
            }

            fn to_value(&self) -> Value {
                Value::$variant(self.clone())
            }

            fn from_value(value: Value) -> Result<Self, ValueError> {
                match value {
                    Value::$variant(value) => Ok(value),
                    value => Err(unexpected_value(&value, stringify!($variant))),
                }
            }
        }
    };
}

reflect_native!(bool, Bool);
reflect_native!(i64, Int);
reflect_native!(f64, Float);
reflect_native!(SmolStr, String);
reflect_native!(TimeSpan, TimeSpan);
reflect_native!(Vector2<f64>, Vec2);
reflect_native!(Vector3<f64>, Vec3);
reflect_native!(Vector4<f64>, Vec4);
reflect_native!(Matrix2<f64>, Mat2);
reflect_native!(Matrix3<f64>, Mat3);
reflect_native!(Matrix4<f64>, Mat4);

impl Reflect for EntityId {
    fn model() -> Model {
        Model::Reference(arcana_intern::ident!(EntityId))
    }

    fn to_value(&self) -> Value {
        Value::Reference(arcana_intern::ident!(EntityId), self.bits())
    }

    fn from_value(value: Value) -> Result<Self, ValueError> {
        match value {
            Value::Reference(tag, bits) if tag == arcana_intern::ident!(EntityId) => {
                Self::from_bits(bits).ok_or_else(|| {
                    ValueError::Custom("EntityId reference cannot be zero".to_owned())
                })
            }
            value => Err(unexpected_value(&value, "EntityId reference")),
        }
    }
}

macro_rules! reflect_color {
    ($ty:ty, $variant:ident) => {
        impl Reflect for $ty {
            fn model() -> Model {
                Model::Color(ColorModel::$variant)
            }

            fn to_value(&self) -> Value {
                Value::Color(ColorValue::$variant(*self))
            }

            fn from_value(value: Value) -> Result<Self, ValueError> {
                match value {
                    Value::Color(ColorValue::$variant(value)) => Ok(value),
                    value => Err(unexpected_value(
                        &value,
                        concat!("Color(", stringify!($variant), ")"),
                    )),
                }
            }
        }
    };
}

reflect_color!(LinLuma, Luma);
reflect_color!(LinLumaa, Lumaa);
reflect_color!(Srgb, Srgb);
reflect_color!(Srgba, Srgba);
reflect_color!(Hsv, Hsv);
reflect_color!(Hsva, Hsva);

impl<T> Reflect for Option<T>
where
    T: Reflect,
{
    fn model() -> Model {
        Model::Option(Some(Box::new(T::model())))
    }

    fn to_value(&self) -> Value {
        Value::Option(self.as_ref().map(|value| Box::new(value.to_value())))
    }

    fn from_value(value: Value) -> Result<Self, ValueError> {
        match value {
            Value::Option(Some(value)) => T::from_value(*value)
                .map(Some)
                .map_err(|error| ValueError::Custom(format!("Option value: {error}"))),
            Value::Option(None) => Ok(None),
            value => Err(unexpected_value(&value, "Option")),
        }
    }
}

impl<T> Reflect for Vec<T>
where
    T: Reflect,
{
    fn model() -> Model {
        Model::Array {
            elem: Some(Box::new(T::model())),
            len: None,
        }
    }

    fn to_value(&self) -> Value {
        Value::Array(self.iter().map(T::to_value).collect())
    }

    fn from_value(value: Value) -> Result<Self, ValueError> {
        match value {
            Value::Array(values) => values
                .into_iter()
                .enumerate()
                .map(|(index, value)| {
                    T::from_value(value).map_err(|error| {
                        ValueError::Custom(format!("Array element {index}: {error}"))
                    })
                })
                .collect(),
            value => Err(unexpected_value(&value, "Array")),
        }
    }
}
