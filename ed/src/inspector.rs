use arcana::{
    ecs::world::WorldLocal,
    model::{Model, Value},
};
use egui::Ui;

/// Trait for components that can be inspected in the inspector.
///
/// Components defined by plugins should implement this trait
/// to allow them to be displayed and edited within the inspector.
pub trait Inspect {
    /// Returns the model associated with this component type.
    fn model(&self) -> Model;

    /// Returns the current value of this component.
    fn value(&self) -> Value;

    /// Updates the component with the given value.
    fn update(&mut self, value: Value);
}

pub struct Inspector;

impl Inspector {
    pub fn show(world: &WorldLocal, ui: &mut Ui) {}
}
