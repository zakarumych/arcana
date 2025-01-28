use edict::{component::Component, entity::EntityId};

use crate::make_uid;

make_uid! {
    /// ID of the render graph.
    pub RenderGraphId;
}

/// Component for the entity that is responsible for rendering.
///
/// Each viewport may chose a renderer entity which will be passed to all render jobs.
#[derive(Clone, Copy, Debug, Component)]
#[repr(transparent)]
pub struct Renderer {
    pub graph: RenderGraphId,
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
