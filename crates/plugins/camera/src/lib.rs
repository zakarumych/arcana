use arcana::{
    edict::{self, Component},
    export_arcana_plugin, na,
};

export_arcana_plugin! {
    CameraPlugin {
        components: [Camera2],
    }
}

#[derive(Clone, Copy, Component)]
pub struct Camera2 {
    /// Viewport of the camera.
    pub viewport: ViewRect,

    /// Parallax applied to the layers this camera renders.
    pub parallax: f32,
}

#[derive(Clone, Copy)]
pub enum ViewRect {
    /// ViewRect that honors aspect ratio of the target resolution.
    FovY(f32),

    /// ViewRect with fixed aspect ratio.
    FovXY(f32, f32),
}

impl ViewRect {
    pub fn transform(&self, scale: f32, ratio: f32) -> na::Affine2<f32> {
        let (x, y) = match *self {
            ViewRect::FovY(y) => (y * ratio, y),
            ViewRect::FovXY(x, y) => (x, y),
        };

        let scaling = na::Vector3::new(x * scale, y * scale, 1.0);
        let scaling = na::Matrix3::from_diagonal(&scaling);
        na::Affine2::from_matrix_unchecked(scaling)
    }
}

impl Camera2 {
    pub const fn new() -> Self {
        Camera2 {
            viewport: ViewRect::FovY(1.0),
            parallax: 1.0,
        }
    }

    pub const fn with_fovy(mut self, fov_y: f32) -> Self {
        self.viewport = ViewRect::FovY(fov_y);
        self
    }

    pub const fn with_fovxy(mut self, fov_x: f32, fov_y: f32) -> Self {
        self.viewport = ViewRect::FovXY(fov_x, fov_y);
        self
    }

    pub const fn with_viewport(mut self, viewport: ViewRect) -> Self {
        self.viewport = viewport;
        self
    }

    pub const fn with_parallax(mut self, parallax: f32) -> Self {
        self.parallax = parallax;
        self
    }
}
