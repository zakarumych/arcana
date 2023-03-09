//! Basic windowing.

/// A window component.
/// Links to the actual window via ID.
pub struct Window {
    #[cfg(feature = "winit")]
    window: winit::window::Window,
}
