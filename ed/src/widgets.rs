use arcana::{Name, validate_name};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NewNameWidgetState {
    AlwaysVisible,
    Visible,
    Collapsed,
}

#[derive(Debug, Clone)]
pub struct NewNameWidget {
    name: String,
    problem: String,
    hint: String,
    state: NewNameWidgetState,
}

impl NewNameWidget {
    pub fn new() -> Self {
        NewNameWidget {
            name: String::new(),
            problem: String::new(),
            hint: String::new(),
            state: NewNameWidgetState::AlwaysVisible,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    pub fn collapsible(self) -> Self {
        NewNameWidget {
            state: NewNameWidgetState::Collapsed,
            ..self
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<Name> {
        self.show_with_predicate(ui, |_| Ok(()))
    }

    pub fn show_with_predicate(
        &mut self,
        ui: &mut egui::Ui,
        check_name: impl Fn(&str) -> Result<(), String>,
    ) -> Option<Name> {
        if self.state == NewNameWidgetState::Collapsed {
            let mut r = ui.button(egui_phosphor::regular::PLUS);

            if !self.hint.is_empty() {
                r = r.on_hover_text(&self.hint);
            }

            if r.clicked() {
                self.state = NewNameWidgetState::Visible;
            }
        }

        if self.state == NewNameWidgetState::Visible
            || self.state == NewNameWidgetState::AlwaysVisible
        {
            let edit = egui::TextEdit::singleline(&mut self.name)
                .lock_focus(true)
                .hint_text(&self.hint);

            let r = ui.add(edit);

            if r.changed() {
                match (check_name(&self.name), validate_name(&self.name)) {
                    (Ok(_), Ok(_)) => {
                        self.problem.clear();
                    }
                    (Err(err), _) => {
                        self.problem = err;
                    }
                    (_, Err(err)) => {
                        self.problem = err.to_string();
                    }
                }
            }

            if r.lost_focus() {
                let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));

                if enter_pressed {
                    match (check_name(&self.name), validate_name(&self.name)) {
                        (Ok(_), Ok(_)) => {
                            self.problem.clear();
                        }
                        (Err(err), _) => {
                            self.problem = err;
                        }
                        (_, Err(err)) => {
                            self.problem = err.to_string();
                        }
                    }

                    if !self.problem.is_empty() {
                        r.request_focus();
                    } else {
                        let name = Name::from_string(std::mem::take(&mut self.name)).unwrap();

                        if self.state == NewNameWidgetState::Visible {
                            self.state = NewNameWidgetState::Collapsed;
                        }
                        self.problem.clear();

                        return Some(name);
                    }
                } else {
                    self.state = NewNameWidgetState::Collapsed;
                }
            }

            if !self.problem.is_empty() {
                ui.weak(egui::RichText::new(&self.problem).color(ui.style().visuals.warn_fg_color));
            }
        }

        None
    }
}
