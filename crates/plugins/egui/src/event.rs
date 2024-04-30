use arcana::events::{
    ElementState, KeyCode, ModifiersState, MouseButton, MouseScrollDelta, PhysicalKey,
    ViewportEvent,
};
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
        MouseButton::Back => None,
        MouseButton::Forward => None,
    }
}

fn translate_key_code(key: KeyCode) -> Option<egui::Key> {
    Some(match key {
        KeyCode::ArrowDown => egui::Key::ArrowDown,
        KeyCode::ArrowLeft => egui::Key::ArrowLeft,
        KeyCode::ArrowRight => egui::Key::ArrowRight,
        KeyCode::ArrowUp => egui::Key::ArrowUp,

        KeyCode::Escape => egui::Key::Escape,
        KeyCode::Tab => egui::Key::Tab,
        KeyCode::Backspace => egui::Key::Backspace,
        KeyCode::Enter | KeyCode::NumpadEnter => egui::Key::Enter,
        KeyCode::Space => egui::Key::Space,

        KeyCode::Insert => egui::Key::Insert,
        KeyCode::Delete => egui::Key::Delete,
        KeyCode::Home => egui::Key::Home,
        KeyCode::End => egui::Key::End,
        KeyCode::PageUp => egui::Key::PageUp,
        KeyCode::PageDown => egui::Key::PageDown,

        KeyCode::Minus | KeyCode::NumpadSubtract => egui::Key::Minus,
        // Using Mac the key with the Plus sign on it is reported as the Equals key
        // (with both English and Swedish keyboard).
        KeyCode::Equal => egui::Key::Equals,
        KeyCode::NumpadAdd => egui::Key::Plus,

        KeyCode::Digit0 | KeyCode::Numpad0 => egui::Key::Num0,
        KeyCode::Digit1 | KeyCode::Numpad1 => egui::Key::Num1,
        KeyCode::Digit2 | KeyCode::Numpad2 => egui::Key::Num2,
        KeyCode::Digit3 | KeyCode::Numpad3 => egui::Key::Num3,
        KeyCode::Digit4 | KeyCode::Numpad4 => egui::Key::Num4,
        KeyCode::Digit5 | KeyCode::Numpad5 => egui::Key::Num5,
        KeyCode::Digit6 | KeyCode::Numpad6 => egui::Key::Num6,
        KeyCode::Digit7 | KeyCode::Numpad7 => egui::Key::Num7,
        KeyCode::Digit8 | KeyCode::Numpad8 => egui::Key::Num8,
        KeyCode::Digit9 | KeyCode::Numpad9 => egui::Key::Num9,

        KeyCode::KeyA => egui::Key::A,
        KeyCode::KeyB => egui::Key::B,
        KeyCode::KeyC => egui::Key::C,
        KeyCode::KeyD => egui::Key::D,
        KeyCode::KeyE => egui::Key::E,
        KeyCode::KeyF => egui::Key::F,
        KeyCode::KeyG => egui::Key::G,
        KeyCode::KeyH => egui::Key::H,
        KeyCode::KeyI => egui::Key::I,
        KeyCode::KeyJ => egui::Key::J,
        KeyCode::KeyK => egui::Key::K,
        KeyCode::KeyL => egui::Key::L,
        KeyCode::KeyM => egui::Key::M,
        KeyCode::KeyN => egui::Key::N,
        KeyCode::KeyO => egui::Key::O,
        KeyCode::KeyP => egui::Key::P,
        KeyCode::KeyQ => egui::Key::Q,
        KeyCode::KeyR => egui::Key::R,
        KeyCode::KeyS => egui::Key::S,
        KeyCode::KeyT => egui::Key::T,
        KeyCode::KeyU => egui::Key::U,
        KeyCode::KeyV => egui::Key::V,
        KeyCode::KeyW => egui::Key::W,
        KeyCode::KeyX => egui::Key::X,
        KeyCode::KeyY => egui::Key::Y,
        KeyCode::KeyZ => egui::Key::Z,

        KeyCode::F1 => egui::Key::F1,
        KeyCode::F2 => egui::Key::F2,
        KeyCode::F3 => egui::Key::F3,
        KeyCode::F4 => egui::Key::F4,
        KeyCode::F5 => egui::Key::F5,
        KeyCode::F6 => egui::Key::F6,
        KeyCode::F7 => egui::Key::F7,
        KeyCode::F8 => egui::Key::F8,
        KeyCode::F9 => egui::Key::F9,
        KeyCode::F10 => egui::Key::F10,
        KeyCode::F11 => egui::Key::F11,
        KeyCode::F12 => egui::Key::F12,
        KeyCode::F13 => egui::Key::F13,
        KeyCode::F14 => egui::Key::F14,
        KeyCode::F15 => egui::Key::F15,
        KeyCode::F16 => egui::Key::F16,
        KeyCode::F17 => egui::Key::F17,
        KeyCode::F18 => egui::Key::F18,
        KeyCode::F19 => egui::Key::F19,
        KeyCode::F20 => egui::Key::F20,

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
            ViewportEvent::KeyboardInput { ref event, .. } => {
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    let pressed = event.state == ElementState::Pressed;

                    if let Some(key) = translate_key_code(keycode) {
                        self.raw_input.events.push(egui::Event::Key {
                            key,
                            pressed,
                            repeat: false, // egui will fill this in for us!
                            modifiers: self.raw_input.modifiers,
                            physical_key: None,
                        });
                    }
                }

                // TODO: Check if `logical_key` matched to `Character` is better here.
                if let Some(text) = &event.text {
                    self.raw_input
                        .events
                        .push(egui::Event::Text(text.to_string()));
                }

                self.cx.wants_keyboard_input()
            }
            ViewportEvent::ModifiersChanged(modifiers) => {
                self.raw_input.modifiers = egui::Modifiers {
                    alt: modifiers.state().contains(ModifiersState::ALT),
                    ctrl: modifiers.state().contains(ModifiersState::CONTROL),
                    shift: modifiers.state().contains(ModifiersState::SHIFT),
                    command: if cfg!(target_os = "macos") {
                        modifiers.state().contains(ModifiersState::SUPER)
                    } else {
                        modifiers.state().contains(ModifiersState::CONTROL)
                    },
                    mac_cmd: cfg!(target_os = "macos")
                        && modifiers.state().contains(ModifiersState::SUPER),
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
