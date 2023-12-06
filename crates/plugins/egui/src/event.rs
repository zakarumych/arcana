use arcana::events::{ElementState, MouseButton, MouseScrollDelta, ViewportEvent, VirtualKeyCode};
use egui::{pos2, vec2, MouseWheelUnit};

use crate::Egui;

// fn is_cut_command(modifiers: egui::Modifiers, keycode: VirtualKeyCode) -> bool {
//     (modifiers.command && keycode == VirtualKeyCode::X)
//         || (cfg!(target_os = "windows") && modifiers.shift && keycode == VirtualKeyCode::Delete)
// }

// fn is_copy_command(modifiers: egui::Modifiers, keycode: VirtualKeyCode) -> bool {
//     (modifiers.command && keycode == VirtualKeyCode::C)
//         || (cfg!(target_os = "windows") && modifiers.ctrl && keycode == VirtualKeyCode::Insert)
// }

// fn is_paste_command(modifiers: egui::Modifiers, keycode: VirtualKeyCode) -> bool {
//     (modifiers.command && keycode == VirtualKeyCode::V)
//         || (cfg!(target_os = "windows") && modifiers.shift && keycode == VirtualKeyCode::Insert)
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

fn translate_virtual_key_code(key: VirtualKeyCode) -> Option<egui::Key> {
    use egui::Key;

    Some(match key {
        VirtualKeyCode::Down => Key::ArrowDown,
        VirtualKeyCode::Left => Key::ArrowLeft,
        VirtualKeyCode::Right => Key::ArrowRight,
        VirtualKeyCode::Up => Key::ArrowUp,

        VirtualKeyCode::Escape => Key::Escape,
        VirtualKeyCode::Tab => Key::Tab,
        VirtualKeyCode::Back => Key::Backspace,
        VirtualKeyCode::Return | VirtualKeyCode::NumpadEnter => Key::Enter,
        VirtualKeyCode::Space => Key::Space,

        VirtualKeyCode::Insert => Key::Insert,
        VirtualKeyCode::Delete => Key::Delete,
        VirtualKeyCode::Home => Key::Home,
        VirtualKeyCode::End => Key::End,
        VirtualKeyCode::PageUp => Key::PageUp,
        VirtualKeyCode::PageDown => Key::PageDown,

        VirtualKeyCode::Minus | VirtualKeyCode::NumpadSubtract => Key::Minus,
        // Using Mac the key with the Plus sign on it is reported as the Equals key
        // (with both English and Swedish keyboard).
        VirtualKeyCode::Equals | VirtualKeyCode::Plus | VirtualKeyCode::NumpadAdd => {
            Key::PlusEquals
        }

        VirtualKeyCode::Key0 | VirtualKeyCode::Numpad0 => Key::Num0,
        VirtualKeyCode::Key1 | VirtualKeyCode::Numpad1 => Key::Num1,
        VirtualKeyCode::Key2 | VirtualKeyCode::Numpad2 => Key::Num2,
        VirtualKeyCode::Key3 | VirtualKeyCode::Numpad3 => Key::Num3,
        VirtualKeyCode::Key4 | VirtualKeyCode::Numpad4 => Key::Num4,
        VirtualKeyCode::Key5 | VirtualKeyCode::Numpad5 => Key::Num5,
        VirtualKeyCode::Key6 | VirtualKeyCode::Numpad6 => Key::Num6,
        VirtualKeyCode::Key7 | VirtualKeyCode::Numpad7 => Key::Num7,
        VirtualKeyCode::Key8 | VirtualKeyCode::Numpad8 => Key::Num8,
        VirtualKeyCode::Key9 | VirtualKeyCode::Numpad9 => Key::Num9,

        VirtualKeyCode::A => Key::A,
        VirtualKeyCode::B => Key::B,
        VirtualKeyCode::C => Key::C,
        VirtualKeyCode::D => Key::D,
        VirtualKeyCode::E => Key::E,
        VirtualKeyCode::F => Key::F,
        VirtualKeyCode::G => Key::G,
        VirtualKeyCode::H => Key::H,
        VirtualKeyCode::I => Key::I,
        VirtualKeyCode::J => Key::J,
        VirtualKeyCode::K => Key::K,
        VirtualKeyCode::L => Key::L,
        VirtualKeyCode::M => Key::M,
        VirtualKeyCode::N => Key::N,
        VirtualKeyCode::O => Key::O,
        VirtualKeyCode::P => Key::P,
        VirtualKeyCode::Q => Key::Q,
        VirtualKeyCode::R => Key::R,
        VirtualKeyCode::S => Key::S,
        VirtualKeyCode::T => Key::T,
        VirtualKeyCode::U => Key::U,
        VirtualKeyCode::V => Key::V,
        VirtualKeyCode::W => Key::W,
        VirtualKeyCode::X => Key::X,
        VirtualKeyCode::Y => Key::Y,
        VirtualKeyCode::Z => Key::Z,

        VirtualKeyCode::F1 => Key::F1,
        VirtualKeyCode::F2 => Key::F2,
        VirtualKeyCode::F3 => Key::F3,
        VirtualKeyCode::F4 => Key::F4,
        VirtualKeyCode::F5 => Key::F5,
        VirtualKeyCode::F6 => Key::F6,
        VirtualKeyCode::F7 => Key::F7,
        VirtualKeyCode::F8 => Key::F8,
        VirtualKeyCode::F9 => Key::F9,
        VirtualKeyCode::F10 => Key::F10,
        VirtualKeyCode::F11 => Key::F11,
        VirtualKeyCode::F12 => Key::F12,
        VirtualKeyCode::F13 => Key::F13,
        VirtualKeyCode::F14 => Key::F14,
        VirtualKeyCode::F15 => Key::F15,
        VirtualKeyCode::F16 => Key::F16,
        VirtualKeyCode::F17 => Key::F17,
        VirtualKeyCode::F18 => Key::F18,
        VirtualKeyCode::F19 => Key::F19,
        VirtualKeyCode::F20 => Key::F20,

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
            ViewportEvent::KeyboardInput { input, .. } => {
                if let Some(keycode) = input.virtual_keycode {
                    let pressed = input.state == ElementState::Pressed;

                    if let Some(key) = translate_virtual_key_code(keycode) {
                        self.raw_input.events.push(egui::Event::Key {
                            key,
                            pressed,
                            repeat: false, // egui will fill this in for us!
                            modifiers: self.raw_input.modifiers,
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
