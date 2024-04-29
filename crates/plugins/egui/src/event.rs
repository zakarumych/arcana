use arcana::events::{ElementState, Key, MouseButton, MouseScrollDelta, ViewportEvent};
use egui::{pos2, vec2, MouseWheelUnit};

use crate::Egui;

// fn is_cut_command(modifiers: egui::Modifiers, keycode: KeyCode) -> bool {
//     (modifiers.command && keycode == KeyCode::X)
//         || (cfg!(target_os = "windows") && modifiers.shift && keycode == KeyCode::Delete)
// }

// fn is_copy_command(modifiers: egui::Modifiers, keycode: KeyCode) -> bool {
//     (modifiers.command && keycode == KeyCode::C)
//         || (cfg!(target_os = "windows") && modifiers.ctrl && keycode == KeyCode::Insert)
// }

// fn is_paste_command(modifiers: egui::Modifiers, keycode: KeyCode) -> bool {
//     (modifiers.command && keycode == KeyCode::V)
//         || (cfg!(target_os = "windows") && modifiers.shift && keycode == KeyCode::Insert)
// }

fn translate_mouse_button(button: MouseButton) -> Option<egui::PointerButton> {
    match button {
        MouseButton::Left => Some(egui::PointerButton::Primary),
        MouseButton::Right => Some(egui::PointerButton::Secondary),
        MouseButton::Middle => Some(egui::PointerButton::Middle),
        MouseButton::Other(1) => Some(egui::PointerButton::Extra1),
        MouseButton::Other(2) => Some(egui::PointerButton::Extra2),
        MouseButton::Other(_) => None,
    }
}

fn translate_key_code(key: Key) -> Option<egui::Key> {
    use egui::Key;

    Some(match key {
        KeyCode::ArrowDown => Key::ArrowDown,
        KeyCode::ArrowLeft => Key::ArrowLeft,
        KeyCode::ArrowRight => Key::ArrowRight,
        KeyCode::ArrowUp => Key::ArrowUp,

        KeyCode::Escape => Key::Escape,
        KeyCode::Tab => Key::Tab,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
        KeyCode::Space => Key::Space,

        KeyCode::Insert => Key::Insert,
        KeyCode::Delete => Key::Delete,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,

        KeyCode::Minus | KeyCode::NumpadSubtract => Key::Minus,
        // Using Mac the key with the Plus sign on it is reported as the Equals key
        // (with both English and Swedish keyboard).
        KeyCode::Equal => Key::Equals,
        KeyCode::Plus | KeyCode::NumpadAdd => Key::Plus,

        KeyCode::Key0 | KeyCode::Numpad0 => Key::Num0,
        KeyCode::Key1 | KeyCode::Numpad1 => Key::Num1,
        KeyCode::Key2 | KeyCode::Numpad2 => Key::Num2,
        KeyCode::Key3 | KeyCode::Numpad3 => Key::Num3,
        KeyCode::Key4 | KeyCode::Numpad4 => Key::Num4,
        KeyCode::Key5 | KeyCode::Numpad5 => Key::Num5,
        KeyCode::Key6 | KeyCode::Numpad6 => Key::Num6,
        KeyCode::Key7 | KeyCode::Numpad7 => Key::Num7,
        KeyCode::Key8 | KeyCode::Numpad8 => Key::Num8,
        KeyCode::Key9 | KeyCode::Numpad9 => Key::Num9,

        KeyCode::A => Key::A,
        KeyCode::B => Key::B,
        KeyCode::C => Key::C,
        KeyCode::D => Key::D,
        KeyCode::E => Key::E,
        KeyCode::F => Key::F,
        KeyCode::G => Key::G,
        KeyCode::H => Key::H,
        KeyCode::I => Key::I,
        KeyCode::J => Key::J,
        KeyCode::K => Key::K,
        KeyCode::L => Key::L,
        KeyCode::M => Key::M,
        KeyCode::N => Key::N,
        KeyCode::O => Key::O,
        KeyCode::P => Key::P,
        KeyCode::Q => Key::Q,
        KeyCode::R => Key::R,
        KeyCode::S => Key::S,
        KeyCode::T => Key::T,
        KeyCode::U => Key::U,
        KeyCode::V => Key::V,
        KeyCode::W => Key::W,
        KeyCode::X => Key::X,
        KeyCode::Y => Key::Y,
        KeyCode::Z => Key::Z,

        KeyCode::F1 => Key::F1,
        KeyCode::F2 => Key::F2,
        KeyCode::F3 => Key::F3,
        KeyCode::F4 => Key::F4,
        KeyCode::F5 => Key::F5,
        KeyCode::F6 => Key::F6,
        KeyCode::F7 => Key::F7,
        KeyCode::F8 => Key::F8,
        KeyCode::F9 => Key::F9,
        KeyCode::F10 => Key::F10,
        KeyCode::F11 => Key::F11,
        KeyCode::F12 => Key::F12,
        KeyCode::F13 => Key::F13,
        KeyCode::F14 => Key::F14,
        KeyCode::F15 => Key::F15,
        KeyCode::F16 => Key::F16,
        KeyCode::F17 => Key::F17,
        KeyCode::F18 => Key::F18,
        KeyCode::F19 => Key::F19,
        KeyCode::F20 => Key::F20,

        _ => {
            return None;
        }
    })
}

// fn translate_cursor(cursor_icon: egui::CursorIcon) -> Option<CursorIcon> {
//     match cursor_icon {
//         egui::CursorIcon::None => None,

//         egui::CursorIcon::Alias => Some(CursorIcon::Alias),
//         egui::CursorIcon::AllScroll => Some(CursorIcon::AllScroll),
//         egui::CursorIcon::Cell => Some(CursorIcon::Cell),
//         egui::CursorIcon::ContextMenu => Some(CursorIcon::ContextMenu),
//         egui::CursorIcon::Copy => Some(CursorIcon::Copy),
//         egui::CursorIcon::Crosshair => Some(CursorIcon::Crosshair),
//         egui::CursorIcon::Default => Some(CursorIcon::Default),
//         egui::CursorIcon::Grab => Some(CursorIcon::Grab),
//         egui::CursorIcon::Grabbing => Some(CursorIcon::Grabbing),
//         egui::CursorIcon::Help => Some(CursorIcon::Help),
//         egui::CursorIcon::Move => Some(CursorIcon::Move),
//         egui::CursorIcon::NoDrop => Some(CursorIcon::NoDrop),
//         egui::CursorIcon::NotAllowed => Some(CursorIcon::NotAllowed),
//         egui::CursorIcon::PointingHand => Some(CursorIcon::Hand),
//         egui::CursorIcon::Progress => Some(CursorIcon::Progress),

//         egui::CursorIcon::ResizeHorizontal => Some(CursorIcon::EwResize),
//         egui::CursorIcon::ResizeNeSw => Some(CursorIcon::NeswResize),
//         egui::CursorIcon::ResizeNwSe => Some(CursorIcon::NwseResize),
//         egui::CursorIcon::ResizeVertical => Some(CursorIcon::NsResize),

//         egui::CursorIcon::ResizeEast => Some(CursorIcon::EResize),
//         egui::CursorIcon::ResizeSouthEast => Some(CursorIcon::SeResize),
//         egui::CursorIcon::ResizeSouth => Some(CursorIcon::SResize),
//         egui::CursorIcon::ResizeSouthWest => Some(CursorIcon::SwResize),
//         egui::CursorIcon::ResizeWest => Some(CursorIcon::WResize),
//         egui::CursorIcon::ResizeNorthWest => Some(CursorIcon::NwResize),
//         egui::CursorIcon::ResizeNorth => Some(CursorIcon::NResize),
//         egui::CursorIcon::ResizeNorthEast => Some(CursorIcon::NeResize),
//         egui::CursorIcon::ResizeColumn => Some(CursorIcon::ColResize),
//         egui::CursorIcon::ResizeRow => Some(CursorIcon::RowResize),

//         egui::CursorIcon::Text => Some(CursorIcon::Text),
//         egui::CursorIcon::VerticalText => Some(CursorIcon::VerticalText),
//         egui::CursorIcon::Wait => Some(CursorIcon::Wait),
//         egui::CursorIcon::ZoomIn => Some(CursorIcon::ZoomIn),
//         egui::CursorIcon::ZoomOut => Some(CursorIcon::ZoomOut),
//     }
// }

impl Egui {
    pub fn handle_event(&mut self, event: &ViewportEvent) -> bool {
        match *event {
            ViewportEvent::Resized { width, height } => {
                self.size = vec2(width as f32, height as f32);
                let rect =
                    egui::Rect::from_min_size(egui::Pos2::ZERO, self.size / self.scale_factor);
                self.raw_input.screen_rect = Some(rect);
                self.raw_input
                    .viewports
                    .get_mut(&self.raw_input.viewport_id)
                    .unwrap()
                    .inner_rect = Some(rect);
                false
            }
            ViewportEvent::ScaleFactorChanged { scale_factor } => {
                self.scale_factor = scale_factor;
                self.raw_input.screen_rect = Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    self.size / self.scale_factor,
                ));
                self.raw_input
                    .viewports
                    .get_mut(&self.raw_input.viewport_id)
                    .unwrap()
                    .native_pixels_per_point = Some(scale_factor);
                false
            }
            ViewportEvent::KeyboardInput { event, .. } => {
                if let Some(keycode) = event.physical_key {
                    let pressed = event.state == ElementState::Pressed;

                    if let Some(key) = translate_virtual_key_code(keycode) {
                        self.raw_input.events.push(egui::Event::Key {
                            key,
                            pressed,
                            repeat: false, // egui will fill this in for us!
                            modifiers: self.raw_input.modifiers,
                            physical_key: None,
                        });
                    }
                }

                self.cx.wants_keyboard_input()
            }
            ViewportEvent::Text { ref text } => {
                self.raw_input.events.push(egui::Event::Text(text.clone()));
                self.cx.wants_keyboard_input()
            }
            ViewportEvent::ModifiersChanged(modifiers) => {
                self.raw_input.modifiers = egui::Modifiers {
                    alt: modifiers.alt(),
                    ctrl: modifiers.ctrl(),
                    shift: modifiers.shift(),
                    command: if cfg!(target_os = "macos") {
                        modifiers.logo()
                    } else {
                        modifiers.ctrl()
                    },
                    mac_cmd: cfg!(target_os = "macos") && modifiers.logo(),
                };
                false
            }
            ViewportEvent::CursorMoved { x, y, .. } => {
                self.mouse_pos = pos2(x as f32 / self.scale_factor, y as f32 / self.scale_factor);
                self.raw_input
                    .events
                    .push(egui::Event::PointerMoved(self.mouse_pos));
                false
            }
            ViewportEvent::CursorEntered { .. } => false,
            ViewportEvent::CursorLeft { .. } => {
                self.raw_input.events.push(egui::Event::PointerGone);
                false
            }
            ViewportEvent::MouseWheel { delta, .. } => {
                {
                    let (unit, delta) = match delta {
                        MouseScrollDelta::LineDelta(x, y) => {
                            (MouseWheelUnit::Line, egui::vec2(x, y))
                        }
                        MouseScrollDelta::PixelDelta(pos) => (
                            MouseWheelUnit::Point,
                            vec2(pos.x as f32, pos.y as f32) / self.scale_factor,
                        ),
                    };

                    self.raw_input.events.push(egui::Event::MouseWheel {
                        unit,
                        delta,
                        modifiers: self.raw_input.modifiers,
                    });
                }

                let delta = match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        let points_per_scroll_line = 50.0; // Scroll speed decided by consensus: https://github.com/emilk/egui/issues/461
                        egui::vec2(x, y) * points_per_scroll_line
                    }
                    MouseScrollDelta::PixelDelta(delta) => {
                        egui::vec2(delta.x as f32, delta.y as f32) / self.scale_factor
                    }
                };

                if self.raw_input.modifiers.ctrl || self.raw_input.modifiers.command {
                    // Treat as zoom instead:
                    let factor = (delta.y / 200.0).exp();
                    self.raw_input.events.push(egui::Event::Zoom(factor));
                } else if self.raw_input.modifiers.shift {
                    // Treat as horizontal scrolling.
                    // Note: one Mac we already get horizontal scroll events when shift is down.
                    self.raw_input
                        .events
                        .push(egui::Event::Scroll(egui::vec2(delta.x + delta.y, 0.0)));
                } else {
                    self.raw_input.events.push(egui::Event::Scroll(delta));
                }

                self.cx.wants_pointer_input()
            }
            ViewportEvent::MouseInput { state, button, .. } => {
                if let Some(button) = translate_mouse_button(button) {
                    let pressed = state == ElementState::Pressed;

                    self.raw_input.events.push(egui::Event::PointerButton {
                        pos: self.mouse_pos,
                        button,
                        pressed,
                        modifiers: self.raw_input.modifiers,
                    });
                }

                self.cx.wants_pointer_input()
            }
        }
    }
}
