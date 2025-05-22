mod model;
mod value;

pub use self::{
    model::{ColorModel, Model},
    value::{ColorValue, Value},
};

/// Trait for types that matches some data model.
///
/// It is expected that values of the type can be converted to and from [`Value`] with that model.
/// If type implements [`Serialize`], it should be able to serialize to a [`Value`] with that model.
/// If type implements [`Deserialize`], it should be able to deserialize from a [`Value`] with that model.
pub trait TypeModel {
    /// Returns data model that describes the type.
    fn model() -> Model
    where
        Self: Sized;

    /// Returns data model that describes the type.
    fn model_dyn(&self) -> Model;
}
