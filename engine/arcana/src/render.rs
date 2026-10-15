use arcana_ecs::{component::Component, entity::EntityId};
use arcana_id::make_uid;
use arcana_model::{Model, Reflect, Value, ValueError};

mod clear;
mod overlay;
pub use clear::{NewColorImage, NewDepthImage};
pub use overlay::{OverlayImage, OverlayScale};

/// Metadata for jobs available without a plugin.
pub fn builtin_jobs() -> Vec<arcana_plugin::JobInfo> {
    use arcana_work_graph::Job;

    vec![
        arcana_plugin::JobInfo {
            id: OverlayImage::id(),
            name: OverlayImage::name(),
            desc: OverlayImage::desc(),
            location: None,
        },
        arcana_plugin::JobInfo {
            id: NewColorImage::id(),
            name: NewColorImage::name(),
            desc: NewColorImage::desc(),
            location: None,
        },
        arcana_plugin::JobInfo {
            id: NewDepthImage::id(),
            name: NewDepthImage::name(),
            desc: NewDepthImage::desc(),
            location: None,
        },
    ]
}

/// Ensures built-in jobs exist while retaining existing job instances and caches.
pub fn register_builtin_jobs(hub: &mut arcana_plugin::PluginsHub) {
    use arcana_work_graph::Job;

    hub.jobs
        .entry(OverlayImage::id())
        .or_insert_with(|| Box::new(OverlayImage::new()));
    hub.jobs
        .entry(NewColorImage::id())
        .or_insert_with(|| Box::new(NewColorImage::new()));
    hub.jobs
        .entry(NewDepthImage::id())
        .or_insert_with(|| Box::new(NewDepthImage::new()));
}

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
