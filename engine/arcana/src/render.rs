use arcana_ecs::{component::Component, entity::EntityId};
use arcana_id::make_uid;
use arcana_model::{Model, Reflect, Value, ValueError};

make_uid! {
    /// ID of the render graph.
    pub RenderGraphId;
}

/// Component for the entity that is responsible for rendering.
///
/// Each viewport chooses a renderer entity which is passed to all render jobs.
/// Jobs decide which auxiliary data to read from that entity, such as a camera.
#[derive(Clone, Copy, Debug, Component)]
#[repr(transparent)]
pub struct Renderer {
    pub graph: RenderGraphId,
}

arcana_id::has_stid!(Renderer = "arcana::render::Renderer" @ arcana_id);

impl Reflect for Renderer {
    fn model() -> Model {
        RenderGraphId::model()
    }

    fn to_value(&self) -> Value {
        self.graph.to_value()
    }

    fn from_value(value: Value) -> Result<Self, ValueError> {
        Ok(Self {
            graph: RenderGraphId::from_value(value)?,
        })
    }
}

/// Resource to hold the current renderer.
/// When rendering starts, the current renderer is set to the renderer of the viewport.
///
/// Jobs may use the current renderer to access associated data.
///
/// For example a `Camera` component may be attached to the renderer entity
/// and jobs that needs a camera may access it through the current renderer.
pub struct CurrentRenderer {
    pub entity: EntityId,
}
