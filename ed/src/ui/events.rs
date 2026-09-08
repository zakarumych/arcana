use arboard::Clipboard;
use arcana::input::{
    ElementState, KeyCode, ModifiersState, MouseButton, MouseScrollDelta, PhysicalKey, ViewInput,
};

use super::{Ui, UiViewport};

impl Ui {
    pub fn has_requested_repaint_for(&self, viewport: &UiViewport) -> bool {
        self.cx.has_requested_repaint_for(&viewport.id)
    }

    pub fn handle_event(
        &mut self,
        viewport: &mut UiViewport,
        clipboard: &mut Clipboard,
        event: &ViewInput,
    ) -> bool {
        let modifiers = self.modifiers;

        match *event {
            ViewInput::Resized { width, height } => {
                viewport.size = egui::vec2(width as f32, height as f32);
                viewport.raw_input.screen_rect = Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    viewport.size / viewport.scale_factor,
                ));

                let vp_info = viewport.raw_input.viewports.entry(viewport.id).or_default();
                vp_info.inner_rect = viewport.raw_input.screen_rect;
                false
            }
            ViewInput::ScaleFactorChanged { scale_factor } => {
                viewport.scale_factor = scale_factor;
                viewport.raw_input.screen_rect = Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    viewport.size / viewport.scale_factor,
                ));

                let vp_info = viewport.raw_input.viewports.entry(viewport.id).or_default();
                vp_info.native_pixels_per_point = Some(scale_factor);
                vp_info.inner_rect = viewport.raw_input.screen_rect;

                false
            }
            ViewInput::KeyboardInput { ref event, .. } => {
                let logical_key = key_from_winit_key(&event.logical_key);
                let physical_key = if let PhysicalKey::Code(keycode) = event.physical_key {
                    key_from_key_code(keycode)
                } else {
                    None
                };
                let pressed = event.state == ElementState::Pressed;

                if let Some(key) = logical_key.or(physical_key) {
                    if pressed && is_cut_command(modifiers, key) {
                        viewport.raw_input.events.push(egui::Event::Cut);
                    } else if pressed && is_copy_command(modifiers, key) {
                        viewport.raw_input.events.push(egui::Event::Copy);
                    } else if pressed && is_paste_command(modifiers, key) {
                        match clipboard.get_text() {
                            Ok(content) => {
                                viewport.raw_input.events.push(egui::Event::Text(content))
                            }
                            Err(error) => {
                                tracing::error!("Failed to get text from clipboard: {:?}", error);
                            }
                        }
                    } else {
                        viewport.raw_input.events.push(egui::Event::Key {
                            key,
                            pressed,
                            repeat: false, // egui will fill this in for us!
                            modifiers: modifiers,
                            physical_key: None,
                        });

                        if pressed {
                            let is_cmd = modifiers.ctrl || modifiers.command || modifiers.mac_cmd;

                            if !is_cmd {
                                if let Some(text) = &event.text {
                                    if text.chars().all(is_printable_char) {
                                        viewport
                                            .raw_input
                                            .events
                                            .push(egui::Event::Text(text.to_string()));
                                    }
                                }
                            }
                        }
                    }
                }

                // When pressing the Tab key, egui focuses the first focusable element, hence Tab always consumes.
                self.cx.egui_wants_keyboard_input()
                    || event.logical_key
                        == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Tab)
            }
            ViewInput::ModifiersChanged(modifiers) => {
                self.modifiers = egui::Modifiers {
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

                viewport
                    .raw_input
                    .events
                    .push(egui::Event::ModifiersChanged(self.modifiers));
                false
            }
            ViewInput::CursorMoved { x, y, .. } => {
                viewport.mouse_pos = egui::pos2(
                    x as f32 / viewport.scale_factor,
                    y as f32 / viewport.scale_factor,
                );
                viewport
                    .raw_input
                    .events
                    .push(egui::Event::PointerMoved(viewport.mouse_pos));
                false
            }
            ViewInput::CursorEntered { .. } => false,
            ViewInput::CursorLeft { .. } => {
                viewport.raw_input.events.push(egui::Event::PointerGone);
                false
            }
            ViewInput::MouseWheel { delta, .. } => {
                {
                    let (unit, delta) = match delta {
                        MouseScrollDelta::LineDelta(x, y) => {
                            (egui::MouseWheelUnit::Line, egui::vec2(x, y))
                        }
                        MouseScrollDelta::PixelDelta(pos) => (
                            egui::MouseWheelUnit::Point,
                            egui::vec2(pos.x as f32, pos.y as f32) / viewport.scale_factor,
                        ),
                    };

                    viewport.raw_input.events.push(egui::Event::MouseWheel {
                        unit,
                        delta,
                        modifiers: modifiers,
                        phase: egui::TouchPhase::Move,
                    });
                }

                // let delta = match delta {
                //     MouseScrollDelta::LineDelta(x, y) => {
                //         let points_per_scroll_line = 50.0; // Scroll speed decided by consensus: https://github.com/emilk/egui/issues/461
                //         egui::vec2(x, y) * points_per_scroll_line
                //     }
                //     MouseScrollDelta::PixelDelta(delta) => {
                //         egui::vec2(delta.x as f32, delta.y as f32) / viewport.scale_factor
                //     }
                // };

                // if modifiers.ctrl || modifiers.command {
                //     // Treat as zoom instead:
                //     let factor = (delta.y / 200.0).exp();
                //     viewport.raw_input.events.push(egui::Event::Zoom(factor));
                // } else if modifiers.shift {
                //     // Treat as horizontal scrolling.
                //     // Note: one Mac we already get horizontal scroll events when shift is down.
                //     viewport
                //         .raw_input
                //         .events
                //         .push(egui::Event::Scroll(egui::vec2(delta.x + delta.y, 0.0)));
                // } else {
                //     viewport
                //         .raw_input
                //         .events
                //         .push(egui::Event::MouseWheel(delta));
                // }

                self.cx.egui_wants_pointer_input()
            }
            ViewInput::MouseInput { state, button, .. } => {
                if let Some(button) = translate_mouse_button(button) {
                    let pressed = state == ElementState::Pressed;

                    viewport.raw_input.events.push(egui::Event::PointerButton {
                        pos: viewport.mouse_pos,
                        button,
                        pressed,
                        modifiers: modifiers,
                    });
                }

                self.cx.egui_wants_pointer_input()
            }
        }
    }
}

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

fn key_from_winit_key(key: &winit::keyboard::Key) -> Option<egui::Key> {
    match key {
        winit::keyboard::Key::Named(named_key) => key_from_named_key(*named_key),
        winit::keyboard::Key::Character(str) => egui::Key::from_name(str.as_str()),
        winit::keyboard::Key::Unidentified(_) | winit::keyboard::Key::Dead(_) => None,
    }
}

fn key_from_named_key(named_key: winit::keyboard::NamedKey) -> Option<egui::Key> {
    use egui::Key;
    use winit::keyboard::NamedKey;

    Some(match named_key {
        NamedKey::Enter => Key::Enter,
        NamedKey::Tab => Key::Tab,
        NamedKey::ArrowDown => Key::ArrowDown,
        NamedKey::ArrowLeft => Key::ArrowLeft,
        NamedKey::ArrowRight => Key::ArrowRight,
        NamedKey::ArrowUp => Key::ArrowUp,
        NamedKey::End => Key::End,
        NamedKey::Home => Key::Home,
        NamedKey::PageDown => Key::PageDown,
        NamedKey::PageUp => Key::PageUp,
        NamedKey::Backspace => Key::Backspace,
        NamedKey::Delete => Key::Delete,
        NamedKey::Insert => Key::Insert,
        NamedKey::Escape => Key::Escape,
        NamedKey::Cut => Key::Cut,
        NamedKey::Copy => Key::Copy,
        NamedKey::Paste => Key::Paste,

        NamedKey::Space => Key::Space,

        NamedKey::F1 => Key::F1,
        NamedKey::F2 => Key::F2,
        NamedKey::F3 => Key::F3,
        NamedKey::F4 => Key::F4,
        NamedKey::F5 => Key::F5,
        NamedKey::F6 => Key::F6,
        NamedKey::F7 => Key::F7,
        NamedKey::F8 => Key::F8,
        NamedKey::F9 => Key::F9,
        NamedKey::F10 => Key::F10,
        NamedKey::F11 => Key::F11,
        NamedKey::F12 => Key::F12,
        NamedKey::F13 => Key::F13,
        NamedKey::F14 => Key::F14,
        NamedKey::F15 => Key::F15,
        NamedKey::F16 => Key::F16,
        NamedKey::F17 => Key::F17,
        NamedKey::F18 => Key::F18,
        NamedKey::F19 => Key::F19,
        NamedKey::F20 => Key::F20,
        NamedKey::F21 => Key::F21,
        NamedKey::F22 => Key::F22,
        NamedKey::F23 => Key::F23,
        NamedKey::F24 => Key::F24,
        NamedKey::F25 => Key::F25,
        NamedKey::F26 => Key::F26,
        NamedKey::F27 => Key::F27,
        NamedKey::F28 => Key::F28,
        NamedKey::F29 => Key::F29,
        NamedKey::F30 => Key::F30,
        NamedKey::F31 => Key::F31,
        NamedKey::F32 => Key::F32,
        NamedKey::F33 => Key::F33,
        NamedKey::F34 => Key::F34,
        NamedKey::F35 => Key::F35,

        NamedKey::BrowserBack => Key::BrowserBack,
        _ => {
            return None;
        }
    })
}

fn key_from_key_code(key: KeyCode) -> Option<egui::Key> {
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

fn is_printable_char(chr: char) -> bool {
    let is_in_private_use_area = '\u{e000}' <= chr && chr <= '\u{f8ff}'
        || '\u{f0000}' <= chr && chr <= '\u{ffffd}'
        || '\u{100000}' <= chr && chr <= '\u{10fffd}';

    !is_in_private_use_area && !chr.is_ascii_control()
}

fn is_cut_command(modifiers: egui::Modifiers, keycode: egui::Key) -> bool {
    keycode == egui::Key::Cut
        || (modifiers.command && keycode == egui::Key::X)
        || (cfg!(target_os = "windows") && modifiers.shift && keycode == egui::Key::Delete)
}

fn is_copy_command(modifiers: egui::Modifiers, keycode: egui::Key) -> bool {
    keycode == egui::Key::Copy
        || (modifiers.command && keycode == egui::Key::C)
        || (cfg!(target_os = "windows") && modifiers.ctrl && keycode == egui::Key::Insert)
}

fn is_paste_command(modifiers: egui::Modifiers, keycode: egui::Key) -> bool {
    keycode == egui::Key::Paste
        || (modifiers.command && keycode == egui::Key::V)
        || (cfg!(target_os = "windows") && modifiers.shift && keycode == egui::Key::Insert)
}
