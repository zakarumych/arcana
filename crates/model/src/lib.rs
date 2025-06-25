mod model;
mod stids;
mod value;

use arcana_id::HasStid;

pub use self::{
    model::{ColorModel, Model},
    value::{ColorValue, Value},
};

/// Trait for types that matches some data model.
///
/// It is expected that values of the type can be converted to and from [`Value`] with that model.
/// If type implements [`Serialize`], it should be able to serialize to a [`Value`] with that model.
/// If type implements [`Deserialize`], it should be able to deserialize from a [`Value`] with that model.
pub trait TypeModel: HasStid + Clone + Send + Sync + 'static {
    /// Returns data model that describes the type.
    fn model() -> Model
    where
        Self: Sized;

    /// Returns data model that describes the type.
    fn model_dyn(&self) -> Model;

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
