use std::hash::Hash;

pub use ::arcana::model::{Model, Value};
use arcana::model::{default_value, ColorModel, ColorValue};
use egui::{Id, Response, Ui, Widget};
use egui_probe::{DeleteMe, EguiProbe, Style};
use hashbrown::HashMap;

pub struct ModelProbe<'a> {
    model: &'a mut Model,
    id_source: Id,
    local_id: Id,
}

impl EguiProbe for ModelProbe<'_> {
    fn probe(&mut self, ui: &mut Ui, _style: &Style) -> Response {
        self.local_id = ui.make_persistent_id(self.id_source);
        let mut changed = false;

        let var_name = |m: &Model| match *m {
            Model::Unit => "Unit",
            Model::Bool => "Bool",
            Model::Int => "Int",
            Model::Float => "Float",
            Model::String => "String",
            Model::Color(ColorModel::Srgb) => "Color",
            Model::Option(_) => "Option",
            Model::Array { .. } => "Array",
            Model::Map(_) => "Map",
            Model::Tuple { .. } => "Tuple",
            Model::Record { .. } => "Record",
            _ => todo!(),
        };

        let mut r = egui::ComboBox::from_id_salt(self.local_id)
            .selected_text(var_name(self.model))
            .show_ui(ui, |ui| {
                let r = ui.selectable_label(matches!(self.model, Model::Bool), "Bool");
                if r.clicked() && !matches!(self.model, Model::Bool) {
                    *self.model = Model::Bool;
                    changed = true;
                }
                let r = ui.selectable_label(matches!(self.model, Model::Int), "Int");
                if r.clicked() && !matches!(self.model, Model::Int) {
                    *self.model = Model::Int;
                    changed = true;
                }
                let r = ui.selectable_label(matches!(self.model, Model::Float), "Float");
                if r.clicked() && !matches!(self.model, Model::Float) {
                    *self.model = Model::Float;
                    changed = true;
                }
                let r = ui.selectable_label(matches!(self.model, Model::String), "String");
                if r.clicked() && !matches!(self.model, Model::String) {
                    *self.model = Model::String;
                    changed = true;
                }
                let r = ui.selectable_label(matches!(self.model, Model::Color(_)), "Color");
                if r.clicked() && !matches!(self.model, Model::Color(_)) {
                    *self.model = Model::Color(ColorModel::Srgb);
                    changed = true;
                }
                let r = ui.selectable_label(matches!(self.model, Model::Option(_)), "Option");
                if r.clicked() && !matches!(self.model, Model::Option(_)) {
                    *self.model = Model::Option(None);
                    changed = true;
                }
                let r = ui.selectable_label(matches!(self.model, Model::Array { .. }), "Array");
                if r.clicked() && !matches!(self.model, Model::Array { .. }) {
                    *self.model = Model::Array {
                        elem: None,
                        len: None,
                    };
                    changed = true;
                }
                let r = ui.selectable_label(matches!(self.model, Model::Map(_)), "Map");
                if r.clicked() && !matches!(self.model, Model::Map(_)) {
                    *self.model = Model::Map(None);
                    changed = true;
                }
                let r = ui.selectable_label(matches!(self.model, Model::Tuple { .. }), "Tuple");
                if r.clicked() && !matches!(self.model, Model::Tuple { .. }) {
                    *self.model = Model::Tuple(Vec::new());
                    changed = true;
                }
                let r = ui.selectable_label(matches!(self.model, Model::Record { .. }), "Record");
                if r.clicked() && !matches!(self.model, Model::Record { .. }) {
                    *self.model = Model::Record(Vec::new());
                    changed = true;
                }
            })
            .response;

        if changed {
            r.mark_changed();
        }

        r
    }

    fn iterate_inner(
        &mut self,
        ui: &mut egui::Ui,
        f: &mut dyn FnMut(&str, &mut egui::Ui, &mut dyn EguiProbe),
    ) {
        match *self.model {
            Model::Option(ref mut model) => {
                let mut probe = MaybeModelProbe {
                    model,
                    id_source: self.local_id.with("Model"),
                    local_id: Id::NULL,
                };
                f("model", ui, &mut probe);
            }
            Model::Array { ref mut elem, .. } => {
                let mut probe = MaybeModelProbe {
                    model: elem,
                    id_source: self.local_id.with("Model"),
                    local_id: Id::NULL,
                };
                f("elem", ui, &mut probe);
            }
            Model::Map(ref mut model) => {
                let mut probe = MaybeModelProbe {
                    model: model,
                    id_source: self.local_id.with("Model"),
                    local_id: Id::NULL,
                };
                f("value", ui, &mut probe);
            }
            _ => {}
        }
    }
}

pub struct MaybeModelProbe<'a> {
    model: &'a mut Option<Box<Model>>,
    id_source: Id,
    local_id: Id,
}

impl EguiProbe for MaybeModelProbe<'_> {
    fn probe(&mut self, ui: &mut Ui, _style: &Style) -> Response {
        self.local_id = ui.make_persistent_id(self.id_source);

        let var_name = |m: Option<&Model>| match m {
            None => "None",
            Some(Model::Unit) => "Unit",
            Some(Model::Bool) => "Bool",
            Some(Model::Int) => "Int",
            Some(Model::Float) => "Float",
            Some(Model::String) => "String",
            Some(Model::Color(ColorModel::Srgb)) => "Color",
            Some(Model::Option(_)) => "Option",
            Some(Model::Array { .. }) => "Array",
            Some(Model::Map(_)) => "Map",
            Some(Model::Tuple { .. }) => "Tuple",
            Some(Model::Record { .. }) => "Record",
            _ => todo!(),
        };

        egui::ComboBox::from_id_salt(self.local_id)
            .selected_text(var_name(self.model.as_deref()))
            .show_ui(ui, |ui| {
                let r = ui.selectable_label(matches!(self.model, None), "None");
                if r.clicked() {
                    *self.model = None;
                }
                let r =
                    ui.selectable_label(matches!(self.model.as_deref(), Some(Model::Bool)), "Bool");
                if r.clicked() {
                    *self.model = Some(Box::new(Model::Bool));
                }
                let r =
                    ui.selectable_label(matches!(self.model.as_deref(), Some(Model::Int)), "Int");
                if r.clicked() {
                    *self.model = Some(Box::new(Model::Int));
                }
                let r = ui
                    .selectable_label(matches!(self.model.as_deref(), Some(Model::Float)), "Float");
                if r.clicked() {
                    *self.model = Some(Box::new(Model::Float));
                }
                let r = ui.selectable_label(
                    matches!(self.model.as_deref(), Some(Model::String)),
                    "String",
                );
                if r.clicked() {
                    *self.model = Some(Box::new(Model::String));
                }
                let r = ui.selectable_label(
                    matches!(self.model.as_deref(), Some(Model::Color(_))),
                    "Color",
                );
                if r.clicked() && !matches!(self.model.as_deref(), Some(Model::Color(_))) {
                    *self.model = Some(Box::new(Model::Color(ColorModel::Srgb)));
                }
                let r = ui.selectable_label(
                    matches!(self.model.as_deref(), Some(Model::Option(_))),
                    "Option",
                );
                if r.clicked() && !matches!(self.model.as_deref(), Some(Model::Option(_))) {
                    *self.model = Some(Box::new(Model::Option(None)));
                }
                let r = ui.selectable_label(
                    matches!(self.model.as_deref(), Some(Model::Array { .. })),
                    "Array",
                );
                if r.clicked() && !matches!(self.model.as_deref(), Some(Model::Array { .. })) {
                    *self.model = Some(Box::new(Model::Array {
                        elem: None,
                        len: None,
                    }));
                }
                let r = ui
                    .selectable_label(matches!(self.model.as_deref(), Some(Model::Map(_))), "Map");
                if r.clicked() && !matches!(self.model.as_deref(), Some(Model::Map(_))) {
                    *self.model = Some(Box::new(Model::Map(None)));
                }
                let r = ui.selectable_label(
                    matches!(self.model.as_deref(), Some(Model::Tuple { .. })),
                    "Tuple",
                );
                if r.clicked() && !matches!(self.model.as_deref(), Some(Model::Tuple { .. })) {
                    *self.model = Some(Box::new(Model::Tuple(Vec::new())));
                }
                let r = ui.selectable_label(
                    matches!(self.model.as_deref(), Some(Model::Record { .. })),
                    "Record",
                );
                if r.clicked() && !matches!(self.model.as_deref(), Some(Model::Record { .. })) {
                    *self.model = Some(Box::new(Model::Record(Vec::new())));
                }
            })
            .response
    }

    fn iterate_inner(
        &mut self,
        ui: &mut egui::Ui,
        f: &mut dyn FnMut(&str, &mut egui::Ui, &mut dyn EguiProbe),
    ) {
        match self.model.as_deref_mut() {
            Some(Model::Option(model)) => {
                let mut probe = MaybeModelProbe {
                    model,
                    id_source: self.local_id.with("Model"),
                    local_id: Id::NULL,
                };
                f("model", ui, &mut probe);
            }
            Some(Model::Array { elem, .. }) => {
                let mut probe = MaybeModelProbe {
                    model: elem,
                    id_source: self.local_id.with("Model"),
                    local_id: Id::NULL,
                };
                f("elem", ui, &mut probe);
            }
            Some(Model::Map(elem)) => {
                let mut probe = MaybeModelProbe {
                    model: elem,
                    id_source: self.local_id.with("Model"),
                    local_id: Id::NULL,
                };
                f("value", ui, &mut probe);
            }
            _ => {}
        }
    }
}

pub struct ValueProbe<'a> {
    model: Option<&'a Model>,
    local_id: Id,
    value: &'a mut Value,
    id_source: Id,
}

impl<'a> ValueProbe<'a> {
    pub fn new(model: Option<&'a Model>, value: &'a mut Value, id_source: impl Hash) -> Self {
        ValueProbe {
            model,
            local_id: Id::NULL,
            value,
            id_source: Id::new(id_source),
        }
    }
}

impl EguiProbe for ValueProbe<'_> {
    fn probe(&mut self, ui: &mut Ui, style: &Style) -> Response {
        match self.model {
            None => {
                self.local_id = ui.make_persistent_id(self.id_source);
                local_model(ui, self.local_id, style).0
            }
            Some(&Model::Unit) => ui.weak("Unit"),
            Some(&Model::Bool) => match self.value {
                Value::Bool(value) => value.probe(ui, style),
                _ => {
                    let mut changed = false;
                    let mut r = ui
                        .horizontal(|ui| {
                            ui.strong(format!(
                                "Expected boolean, but is {} instead",
                                self.value.kind()
                            ));
                            if ui.small_button("Reset to false").clicked() {
                                *self.value = Value::Bool(false);
                                changed = true;
                            }
                            ui.strong("?");
                        })
                        .response;

                    if changed {
                        r.mark_changed();
                    }

                    r
                }
            },
            Some(&Model::Int) => match self.value {
                Value::Int(value) => value.probe(ui, style),
                Value::Float(value) => {
                    let mut changed = false;
                    let f = *value as i64;

                    let mut r = ui
                        .horizontal(|ui| {
                            ui.strong(format!(
                                "Expected integer, but is {} instead",
                                self.value.kind()
                            ));

                            if ui.small_button(format!("Convert to {f}")).clicked() {
                                *self.value = Value::Int(f);
                                changed = true;
                            }

                            ui.strong("?");
                        })
                        .response;

                    if changed {
                        r.mark_changed();
                    }

                    r
                }
                _ => {
                    let mut changed = false;

                    let mut r = ui
                        .horizontal(|ui| {
                            ui.strong(format!(
                                "Expected integer, but is {} instead",
                                self.value.kind()
                            ));
                            if ui.small_button(format!("Reset to 0")).clicked() {
                                *self.value = Value::Int(0);
                                changed = true;
                            }
                            ui.strong("?");
                        })
                        .response;

                    if changed {
                        r.mark_changed();
                    }

                    r
                }
            },
            Some(&Model::Float) => match self.value {
                Value::Float(value) => value.probe(ui, style),
                Value::Int(value) => {
                    let mut changed = false;
                    let f = *value as f64;

                    let mut r = ui
                        .horizontal(|ui| {
                            ui.strong(format!(
                                "Expected integer, but is {} instead",
                                self.value.kind()
                            ));

                            if ui.small_button(format!("Convert to {f:0.1}")).clicked() {
                                *self.value = Value::Float(f);
                                changed = true;
                            }

                            ui.strong("?");
                        })
                        .response;

                    if changed {
                        r.mark_changed();
                    }

                    r
                }
                _ => {
                    let mut changed = false;

                    let mut r = ui
                        .horizontal(|ui| {
                            ui.strong(format!(
                                "Expected integer, but is {} instead",
                                self.value.kind()
                            ));
                            if ui.small_button(format!("Reset to 0.0")).clicked() {
                                *self.value = Value::Float(0.0);
                                changed = true;
                            }
                            ui.strong("?");
                        })
                        .response;

                    if changed {
                        r.mark_changed();
                    }

                    r
                }
            },
            Some(&Model::String) => match self.value {
                Value::String(value) => value.probe(ui, style),
                Value::Bool(value) => {
                    let (mut r, s) = convert_to_string(ui, value, "bool");
                    if let Some(s) = s {
                        *self.value = Value::String(s);
                        r.mark_changed();
                    }
                    r
                }
                Value::Int(value) => {
                    let (mut r, s) = convert_to_string(ui, value, "int");
                    if let Some(s) = s {
                        *self.value = Value::String(s);
                        r.mark_changed();
                    }
                    r
                }
                Value::Float(value) => {
                    let (mut r, s) = convert_to_string(ui, value, "float");
                    if let Some(s) = s {
                        *self.value = Value::String(s);
                        r.mark_changed();
                    }
                    r
                }
                _ => {
                    let mut changed = false;
                    let mut r = ui
                        .horizontal(|ui| {
                            ui.strong(format!(
                                "Expected string, but is {} instead",
                                self.value.kind()
                            ));
                            if ui.small_button("Reset to empty string").clicked() {
                                *self.value = Value::String(String::new());
                                changed = true;
                            }
                            ui.strong("?");
                        })
                        .response;

                    if changed {
                        r.mark_changed();
                    }

                    r
                }
            },
            Some(&Model::Color(model)) => match model {
                ColorModel::Luma => match *self.value {
                    Value::Color(ColorValue::Luma(ref mut luma)) => {
                        egui::DragValue::new(&mut luma.luma)
                            .range(0.0..=1.0)
                            .max_decimals(3)
                            .ui(ui)
                    }
                    Value::Color(color) => {
                        let mut changed = false;
                        let mut r = ui
                            .horizontal(|ui| {
                                ui.strong(format!(
                                    "Expected luma color, but is {} instead",
                                    color.kind()
                                ));
                                if ui.small_button("Reset to luma").clicked() {
                                    *self.value = Value::Color(ColorValue::Luma(color.into_luma()));
                                    changed = true;
                                }
                                ui.strong("?");
                            })
                            .response;

                        if changed {
                            r.mark_changed();
                        }

                        r
                    }
                    _ => {
                        let mut changed = false;
                        let mut r = ui
                            .horizontal(|ui| {
                                ui.strong(format!(
                                    "Expected luma color, but is {} instead",
                                    self.value.kind()
                                ));
                                if ui.small_button("Reset to black").clicked() {
                                    *self.value =
                                        Value::Color(ColorValue::Luma(palette::LinLuma::new(0.0)));
                                    changed = true;
                                }
                                ui.strong("?");
                            })
                            .response;

                        if changed {
                            r.mark_changed();
                        }

                        r
                    }
                },
                ColorModel::Lumaa => match *self.value {
                    Value::Color(ColorValue::Lumaa(ref mut lumaa)) => {
                        let mut changed = false;
                        let mut r = ui
                            .vertical(|ui| {
                                changed |= egui::DragValue::new(&mut lumaa.luma)
                                    .range(0.0..=1.0)
                                    .max_decimals(3)
                                    .ui(ui)
                                    .changed();
                                changed |= egui::DragValue::new(&mut lumaa.alpha)
                                    .range(0.0..=1.0)
                                    .max_decimals(3)
                                    .ui(ui)
                                    .changed();
                            })
                            .response;

                        if changed {
                            r.mark_changed();
                        }

                        r
                    }
                    Value::Color(color) => {
                        let mut changed = false;
                        let mut r = ui
                            .horizontal(|ui| {
                                ui.strong(format!(
                                    "Expected luma color, but is {} instead",
                                    color.kind()
                                ));
                                if ui.small_button("Reset to luma").clicked() {
                                    *self.value =
                                        Value::Color(ColorValue::Lumaa(color.into_lumaa()));
                                    changed = true;
                                }
                                ui.strong("?");
                            })
                            .response;

                        if changed {
                            r.mark_changed();
                        }

                        r
                    }
                    _ => {
                        let mut changed = false;
                        let mut r = ui
                            .horizontal(|ui| {
                                ui.strong(format!(
                                    "Expected lumaa color, but is {} instead",
                                    self.value.kind()
                                ));
                                if ui.small_button("Reset to black").clicked() {
                                    *self.value = Value::Color(ColorValue::Lumaa(
                                        palette::LinLumaa::new(0.0, 1.0),
                                    ));
                                    changed = true;
                                }
                                ui.strong("?");
                            })
                            .response;

                        if changed {
                            r.mark_changed();
                        }

                        r
                    }
                },
                ColorModel::Srgb => match *self.value {
                    Value::Color(ColorValue::Srgb(ref mut c)) => {
                        let mut rgba = egui::Rgba::from_rgb(c.red, c.green, c.blue);
                        let r = egui::color_picker::color_edit_button_rgba(
                            ui,
                            &mut rgba,
                            egui::color_picker::Alpha::Opaque,
                        );
                        if r.changed() {
                            *c = palette::Srgb::new(rgba.r(), rgba.g(), rgba.b());
                        }
                        r
                    }
                    Value::Color(color) => {
                        let mut changed = false;
                        let mut r = ui
                            .horizontal(|ui| {
                                ui.strong(format!(
                                    "Expected srgb color, but is {} instead",
                                    color.kind()
                                ));
                                if ui.small_button("Reset to srgb").clicked() {
                                    *self.value = Value::Color(ColorValue::Srgb(color.into_srgb()));
                                    changed = true;
                                }
                                ui.strong("?");
                            })
                            .response;

                        if changed {
                            r.mark_changed();
                        }

                        r
                    }
                    _ => {
                        let mut changed = false;
                        let mut r = ui
                            .horizontal(|ui| {
                                ui.strong(format!(
                                    "Expected srgb color, but is {} instead",
                                    self.value.kind()
                                ));
                                if ui.small_button("Reset to black").clicked() {
                                    *self.value = Value::Color(ColorValue::Srgb(
                                        palette::Srgb::new(0.0, 0.0, 0.0),
                                    ));
                                    changed = true;
                                }
                                ui.strong("?");
                            })
                            .response;

                        if changed {
                            r.mark_changed();
                        }

                        r
                    }
                },
                ColorModel::Srgba => match *self.value {
                    Value::Color(ColorValue::Srgba(ref mut c)) => {
                        let mut rgba =
                            egui::Rgba::from_rgba_unmultiplied(c.red, c.green, c.blue, c.alpha);
                        let r = egui::color_picker::color_edit_button_rgba(
                            ui,
                            &mut rgba,
                            egui::color_picker::Alpha::Opaque,
                        );
                        if r.changed() {
                            let [r, g, b, a] = rgba.to_rgba_unmultiplied();
                            *c = palette::Srgba::new(r, g, b, a);
                        }
                        r
                    }
                    Value::Color(color) => {
                        let mut changed = false;
                        let mut r = ui
                            .horizontal(|ui| {
                                ui.strong(format!(
                                    "Expected srgb color, but is {} instead",
                                    color.kind()
                                ));
                                if ui.small_button("Reset to srgb").clicked() {
                                    *self.value =
                                        Value::Color(ColorValue::Srgba(color.into_srgba()));
                                    changed = true;
                                }
                                ui.strong("?");
                            })
                            .response;

                        if changed {
                            r.mark_changed();
                        }

                        r
                    }
                    _ => {
                        let mut changed = false;
                        let mut r = ui
                            .horizontal(|ui| {
                                ui.strong(format!(
                                    "Expected srgb color, but is {} instead",
                                    self.value.kind()
                                ));
                                if ui.small_button("Reset to black").clicked() {
                                    *self.value = Value::Color(ColorValue::Srgba(
                                        palette::Srgba::new(0.0, 0.0, 0.0, 1.0),
                                    ));
                                    changed = true;
                                }
                                ui.strong("?");
                            })
                            .response;

                        if changed {
                            r.mark_changed();
                        }

                        r
                    }
                },
                ColorModel::Hsv => match *self.value {
                    Value::Color(ColorValue::Hsv(ref mut c)) => {
                        let mut hsva =
                            egui::ecolor::Hsva::new(c.hue.into_inner(), c.saturation, c.value, 1.0);
                        let r = egui::color_picker::color_edit_button_hsva(
                            ui,
                            &mut hsva,
                            egui::color_picker::Alpha::Opaque,
                        );
                        if r.changed() {
                            *c = palette::Hsv::new(hsva.h, hsva.s, hsva.v);
                        }
                        r
                    }
                    Value::Color(color) => {
                        let mut changed = false;
                        let mut r = ui
                            .horizontal(|ui| {
                                ui.strong(format!(
                                    "Expected hsv color, but is {} instead",
                                    color.kind()
                                ));
                                if ui.small_button("Reset to hsv").clicked() {
                                    *self.value = Value::Color(ColorValue::Hsv(color.into_hsv()));
                                    changed = true;
                                }
                                ui.strong("?");
                            })
                            .response;

                        if changed {
                            r.mark_changed();
                        }

                        r
                    }
                    _ => {
                        let mut changed = false;
                        let mut r = ui
                            .horizontal(|ui| {
                                ui.strong(format!(
                                    "Expected hsv color, but is {} instead",
                                    self.value.kind()
                                ));
                                if ui.small_button("Reset to black").clicked() {
                                    *self.value = Value::Color(ColorValue::Hsv(palette::Hsv::new(
                                        0.0, 0.0, 0.0,
                                    )));
                                    changed = true;
                                }
                                ui.strong("?");
                            })
                            .response;

                        if changed {
                            r.mark_changed();
                        }

                        r
                    }
                },
                ColorModel::Hsva => match *self.value {
                    Value::Color(ColorValue::Hsva(ref mut c)) => {
                        let mut hsva =
                            egui::ecolor::Hsva::new(c.hue.into_inner(), c.saturation, c.value, 1.0);
                        let r = egui::color_picker::color_edit_button_hsva(
                            ui,
                            &mut hsva,
                            egui::color_picker::Alpha::Opaque,
                        );
                        if r.changed() {
                            *c = palette::Hsva::new(hsva.h, hsva.s, hsva.v, hsva.a);
                        }
                        r
                    }
                    Value::Color(color) => {
                        let mut changed = false;
                        let mut r = ui
                            .horizontal(|ui| {
                                ui.strong(format!(
                                    "Expected hsva color, but is {} instead",
                                    color.kind()
                                ));
                                if ui.small_button("Reset to hsva").clicked() {
                                    *self.value = Value::Color(ColorValue::Hsva(color.into_hsva()));
                                    changed = true;
                                }
                                ui.strong("?");
                            })
                            .response;

                        if changed {
                            r.mark_changed();
                        }

                        r
                    }
                    _ => {
                        let mut changed = false;
                        let mut r = ui
                            .horizontal(|ui| {
                                ui.strong(format!(
                                    "Expected hsva color, but is {} instead",
                                    self.value.kind()
                                ));
                                if ui.small_button("Reset to black").clicked() {
                                    *self.value = Value::Color(ColorValue::Hsva(
                                        palette::Hsva::new(0.0, 0.0, 0.0, 1.0),
                                    ));
                                    changed = true;
                                }
                                ui.strong("?");
                            })
                            .response;

                        if changed {
                            r.mark_changed();
                        }

                        r
                    }
                },
            },
            Some(&Model::Option(ref model)) => match self.value {
                Value::Option(value) => egui_probe::option_probe_with(
                    value,
                    ui,
                    style,
                    || Box::new(default_value(model.as_deref())),
                    |value, ui, style| {
                        ValueProbe::new(model.as_deref(), value, self.local_id.with("some"))
                            .probe(ui, style)
                    },
                ),
                _ => {
                    let mut changed = false;
                    let mut r = ui
                        .horizontal(|ui| {
                            ui.strong(format!(
                                "Expected option, but is {} instead",
                                self.value.kind()
                            ));
                            if ui.small_button("Reset to some").clicked() {
                                *self.value =
                                    Value::Option(Some(Box::new(std::mem::take(&mut self.value))));
                                changed = true;
                            }
                            ui.strong("?");
                        })
                        .response;

                    if changed {
                        r.mark_changed();
                    }

                    r
                }
            },
            Some(&Model::Array { ref elem, len }) => match self.value {
                Value::Array(values) => {
                    let mut changed = false;

                    if let Some(len) = len {
                        if values.len() != len {
                            values.resize_with(len, || default_value(elem.as_deref()));
                            changed = true;
                        }
                    }

                    let mut r = ui
                        .horizontal(|ui| {
                            self.local_id = ui.make_persistent_id(self.id_source);

                            let local_elem;
                            let elem = match elem {
                                Some(elem) => &*elem,
                                None => {
                                    let (r, m) = local_model(ui, self.local_id, style);
                                    changed |= r.changed();
                                    local_elem = m;
                                    &local_elem
                                }
                            };

                            if len.is_none() {
                                let r = ui.small_button(style.add_button_text());
                                if r.clicked() {
                                    let value = elem.default_value();

                                    values.push(value);
                                    changed = true;
                                }
                            }
                        })
                        .response;

                    if changed {
                        r.mark_changed();
                    }

                    r
                }
                _ => {
                    let mut changed = false;

                    let mut r = ui
                        .horizontal(|ui| {
                            ui.strong(format!(
                                "Expected array, but is {} instead",
                                self.value.kind()
                            ));
                            if ui.small_button("Reset to default array").clicked() {
                                *self.value = Value::Array(
                                    (0..len.unwrap_or(0))
                                        .map(|_| default_value(elem.as_deref()))
                                        .collect(),
                                );
                                changed = true;
                            }
                            ui.strong("?");
                        })
                        .response;

                    if changed {
                        r.mark_changed();
                    }

                    r
                }
            },
            Some(&Model::Map(ref elem)) => match self.value {
                Value::Map(values) => {
                    #[derive(Clone)]
                    struct NewKey(String);

                    let mut changed: bool = false;

                    let mut r = ui
                        .horizontal(|ui| {
                            self.local_id = ui.make_persistent_id(self.id_source);

                            let local_elem;
                            let elem = match elem {
                                Some(elem) => &*elem,
                                None => {
                                    let (r, m) = local_model(ui, self.local_id, style);
                                    changed |= r.changed();
                                    local_elem = m;
                                    &local_elem
                                }
                            };

                            let mut new_key = ui
                                .ctx()
                                .data(|d| d.get_temp::<NewKey>(self.local_id))
                                .unwrap_or(NewKey(String::new()));

                            ui.text_edit_singleline(&mut new_key.0);

                            let r = ui.small_button(style.add_button_text());
                            if r.clicked() {
                                let value = elem.default_value();

                                values.insert(std::mem::take(&mut new_key.0), value);
                                changed = true;
                            }

                            ui.ctx().data_mut(|d| d.insert_temp(self.local_id, new_key));
                        })
                        .response;

                    if changed {
                        r.mark_changed();
                    }

                    r
                }
                _ => {
                    let mut changed = false;
                    let mut r = ui
                        .horizontal(|ui| {
                            ui.strong(format!(
                                "Expected list, but is {} instead",
                                self.value.kind()
                            ));
                            if ui.small_button("Reset to empty map").clicked() {
                                *self.value = Value::Map(HashMap::new());
                                changed = true;
                            }
                            ui.strong("?");
                        })
                        .response;

                    if changed {
                        r.mark_changed();
                    }

                    r
                }
            },
            Some(&Model::Enum(ref variants)) => match self.value {
                Value::Unit if variants.is_empty() => ui.weak("No variants"),
                Value::Enum(name, value) if !variants.is_empty() => {
                    self.local_id = ui.make_persistent_id(self.id_source);
                    let mut changed = false;

                    let mut r = ui
                        .horizontal(|ui| {
                            egui::ComboBox::from_id_salt(self.local_id)
                                .selected_text(name.as_str())
                                .show_ui(ui, |ui| {
                                    for &(vname, ref vmodel) in variants {
                                        let r = ui.selectable_label(vname == *name, vname.as_str());
                                        if r.clicked() && vname != *name {
                                            *name = vname;
                                            **value = default_value(vmodel.as_ref());
                                            changed = true;
                                        }
                                    }
                                });

                            match variants.iter().find(|v| v.0 == *name) {
                                None => {
                                    ui.strong(format!("Unknown variant: {}", name));
                                    if ui.small_button("Reset to first variant").clicked() {
                                        let v = &variants[0];
                                        *name = v.0;
                                        **value = default_value(v.1.as_ref());
                                        changed = true;
                                    }
                                    ui.strong("?");
                                }
                                Some((_, model)) => {
                                    let local;
                                    let model = match model {
                                        Some(model) => &*model,
                                        None => {
                                            let (r, m) = local_model(ui, self.local_id, style);
                                            changed |= r.changed();
                                            local = m;
                                            &local
                                        }
                                    };

                                    if !matches!(model, Model::Unit) {
                                        let mut probe =
                                            ValueProbe::new(Some(model), value, self.local_id);
                                        changed |= probe.probe(ui, style).changed();
                                    }
                                }
                            }
                        })
                        .response;

                    if changed {
                        r.mark_changed();
                    }

                    r
                }
                _ if variants.is_empty() => {
                    let mut changed = false;
                    let mut r = ui
                        .horizontal(|ui| {
                            ui.strong(format!(
                                "Expected void enum, but is {} instead",
                                self.value.kind()
                            ));
                            if ui.small_button("Reset to unit").clicked() {
                                *self.value = Value::Unit;
                                changed = true;
                            }
                            ui.strong("?");
                        })
                        .response;

                    if changed {
                        r.mark_changed();
                    }

                    r
                }
                _ => {
                    let mut changed = false;
                    let mut r = ui
                        .horizontal(|ui| {
                            ui.strong(format!(
                                "Expected enum, but is {} instead",
                                self.value.kind()
                            ));
                            if ui.small_button("Reset to first variant").clicked() {
                                let v = &variants[0];
                                *self.value =
                                    Value::Enum(v.0, Box::new(default_value(v.1.as_ref())));
                                changed = true;
                            }
                            ui.strong("?");
                        })
                        .response;

                    if changed {
                        r.mark_changed();
                    }

                    r
                }
            },

            _ => todo!(),
        }
    }

    fn iterate_inner(&mut self, ui: &mut Ui, f: &mut dyn FnMut(&str, &mut Ui, &mut dyn EguiProbe)) {
        match self.model {
            None => {
                let local_elem = local_model_inner(ui, self.local_id, f);
                let mut probe = ValueProbe::new(Some(&local_elem), self.value, self.local_id);
                f("value", ui, &mut probe);
            }
            Some(Model::Unit) => {}
            Some(Model::Bool) => {}
            Some(Model::Int { .. }) => {}
            Some(Model::Float { .. }) => {}
            Some(Model::String { .. }) => {}
            Some(Model::Color(_)) => {}
            Some(Model::Array { elem, len }) => {
                let local_elem;
                let elem = match elem {
                    None => {
                        local_elem = local_model_inner(ui, self.local_id, f);
                        &local_elem
                    }
                    Some(elem) => &**elem,
                };

                match self.value {
                    Value::Array(elems) => {
                        if len.is_none() {
                            let mut idx = 0;
                            elems.retain_mut(|value| {
                                let mut probe =
                                    ValueProbe::new(Some(elem), value, self.local_id.with(idx));
                                let mut item = DeleteMe {
                                    value: &mut probe,
                                    delete: false,
                                };
                                f(&format!("[{idx}]"), ui, &mut item);
                                idx += 1;
                                !item.delete
                            });
                        } else {
                            for (idx, value) in elems.iter_mut().enumerate() {
                                let mut probe =
                                    ValueProbe::new(Some(elem), value, self.local_id.with(idx));
                                f(&format!("[{idx}]"), ui, &mut probe);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Some(Model::Map(elem)) => {
                let local_elem;
                let elem = match elem {
                    None => {
                        local_elem = local_model_inner(ui, self.local_id, f);
                        &local_elem
                    }
                    Some(value) => &**value,
                };

                match self.value {
                    Value::Map(values) => {
                        let mut idx = 0;
                        values.retain(|key, value| {
                            let mut probe =
                                ValueProbe::new(Some(elem), value, self.local_id.with(idx));
                            let mut item = DeleteMe {
                                value: &mut probe,
                                delete: false,
                            };
                            f(key, ui, &mut item);
                            idx += 1;
                            !item.delete
                        });
                    }
                    _ => {}
                }
            }
            Some(Model::Enum(ref variants)) => match *self.value {
                Value::Enum(name, ref mut value) => {
                    let local;
                    let model = match variants.iter().find(|v| v.0 == name) {
                        None => return,
                        Some((_, None)) => {
                            local = local_model_inner(ui, self.local_id, f);
                            &local
                        }
                        Some((_, Some(Model::Unit))) => return,
                        Some((_, Some(model))) => model,
                    };

                    let mut probe = ValueProbe::new(Some(model), value, self.local_id);
                    f("value", ui, &mut probe);
                }
                _ => {}
            },
            _ => todo!(),
        }
    }
}

fn convert_to_string<T: ToString>(
    ui: &mut Ui,
    value: &T,
    kind: &str,
) -> (Response, Option<String>) {
    let mut convert = false;
    let s = value.to_string();

    let r = ui
        .horizontal(|ui| {
            ui.strong(format!("Expected string, but is {} instead", kind));
            if ui.small_button(format!("Convert to {s:?}")).clicked() {
                convert = true;
            }
            ui.strong("?");
        })
        .response;

    (r, if convert { Some(s) } else { None })
}

fn local_model(ui: &mut Ui, local_id: Id, style: &Style) -> (Response, Model) {
    let mut local_model = ui
        .ctx()
        .data(|d| d.get_temp::<Model>(local_id))
        .unwrap_or_default();

    let r = ModelProbe {
        model: &mut local_model,
        id_source: local_id,
        local_id: Id::NULL,
    }
    .probe(ui, style);

    ui.ctx()
        .data_mut(|d| d.insert_temp(local_id, local_model.clone()));
    (r, local_model)
}

fn local_model_inner(
    ui: &mut Ui,
    local_id: Id,
    f: &mut dyn FnMut(&str, &mut Ui, &mut dyn EguiProbe),
) -> Model {
    let mut local_model = ui
        .ctx()
        .data(|d| d.get_temp::<Model>(local_id))
        .unwrap_or_default();

    let mut probe = ModelProbe {
        model: &mut local_model,
        id_source: local_id,
        local_id: Id::NULL,
    };

    probe.iterate_inner(ui, f);

    ui.ctx()
        .data_mut(|d| d.insert_temp(local_id, local_model.clone()));

    local_model
}
