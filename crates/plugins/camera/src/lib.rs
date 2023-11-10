use arcana::{
    edict::{self, Component, World},
    export_arcana_plugin, na,
    plugin::{ArcanaPlugin, PluginInit},
    plugin_init,
};

export_arcana_plugin!(CameraPlugin);

pub struct CameraPlugin;

impl ArcanaPlugin for CameraPlugin {
    fn init(&self, world: &mut World) -> PluginInit {
        world.ensure_component_registered::<Camera2>();
        plugin_init!()
    }
}

#[derive(Clone, Copy, Component)]
pub struct Camera2 {
    /// Viewport of the camera.
    pub viewport: Viewport2,

    /// Parallax applied to the layers this camera renders.
    pub parallax: f32,
}

#[derive(Clone, Copy)]
pub enum Viewport2 {
    /// Viewport that honors aspect ratio of the target resolution.
    FovY(f32),

    /// Viewport with fixed aspect ratio.
    FovXY(f32, f32),
}

impl Viewport2 {
    pub fn transform(&self, scale: f32, ratio: f32) -> na::Affine2<f32> {
        let (x, y) = match *self {
            Viewport2::FovY(y) => (y * ratio, y),
            Viewport2::FovXY(x, y) => (x, y),
        };

        let scaling = na::Vector3::new(x * scale, y * scale, 1.0);
        let scaling = na::Matrix3::from_diagonal(&scaling);
        na::Affine2::from_matrix_unchecked(scaling)
    }
}

impl Camera2 {
    pub const fn new() -> Self {
        Self {
            viewport: Viewport2::FovY(1.0),
            parallax: 1.0,
        }
    }

    pub const fn with_fovy(mut self, fov_y: f32) -> Self {
        self.viewport = Viewport2::FovY(fov_y);
        self
    }

    pub const fn with_fovxy(mut self, fov_x: f32, fov_y: f32) -> Self {
        self.viewport = Viewport2::FovXY(fov_x, fov_y);
        self
    }

    pub const fn with_viewport(mut self, viewport: Viewport2) -> Self {
        self.viewport = viewport;
        self
    }

    pub const fn with_parallax(mut self, parallax: f32) -> Self {
        self.parallax = parallax;
        self
    }
}
