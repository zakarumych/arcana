use winit::window::{Window, WindowId};

use crate::render::TargetId;

pub struct BobWindow {
    window: Window,
    surface: mev::Surface,
}

impl BobWindow {
    pub fn new(window: Window, surface: mev::Surface, target: TargetId) -> Self {
        BobWindow {
            window,
            surface,
            target,
        }
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn winit(&self) -> &Window {
        &self.window
    }

    pub fn target(&self) -> TargetId {
        self.target
    }

    pub fn surface(&self) -> &mev::Surface {
        &self.surface
    }

    pub fn surface_mut(&mut self) -> &mut mev::Surface {
        &mut self.surface
    }
}

pub struct Windows {
    pub windows: Vec<BobWindow>,
}
