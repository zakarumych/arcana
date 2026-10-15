use arcana::{
    Ident,
    hash::HashMap,
    model::{ColorModel, ColorValue, Model, Value},
    smol_str::SmolStr,
};
use egui::{Id, Response, Ui, Widget};
use egui_probe::{DeleteMe, EguiProbe, Probe, Style};

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
                let r = ui.selectable_label(matches!(self.model, Model::Unit), "Unit");
                if r.clicked() && !matches!(self.model, Model::Unit) {
                    *self.model = Model::Unit;
                    changed = true;
                }
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
                    ui.selectable_label(matches!(self.model.as_deref(), Some(Model::Unit)), "Unit");
                if r.clicked() {
                    *self.model = Some(Box::new(Model::Unit));
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

/// Available references in a single ID domain, supplied by the owning editor.
pub struct ReferenceChoices {
    pub domain: Ident,
    pub values: Vec<(u64, String)>,
}

pub struct ValueProbe<'a> {
    model: Option<&'a Model>,
    local_id: Id,
    value: &'a mut Value,
    id_source: Id,
    references: &'a [ReferenceChoices],
}

impl<'a> ValueProbe<'a> {
    pub fn new(model: Option<&'a Model>, value: &'a mut Value, id_source: impl egui::AsId) -> Self {
        ValueProbe {
            model,
            local_id: Id::NULL,
            value,
            id_source: Id::new(id_source),
            references: &[],
        }
    }

    pub fn with_references(mut self, references: &'a [ReferenceChoices]) -> Self {
        self.references = references;
        self
    }

    pub fn show(mut self, ui: &mut Ui) -> Response {
        Probe::new(&mut self).show(ui)
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
            Some(Model::Record(fields)) => {
                self.local_id = ui.make_persistent_id(self.id_source);
                ui.weak(format!("{} fields", fields.len()))
            }
            Some(model @ (Model::Vec2 | Model::Vec3)) => {
                let mut changed = false;
                let mut response = ui
                    .horizontal(|ui| {
                        let mut axis = |label, value: &mut f64| {
                            changed |= ui
                                .add(egui::DragValue::new(value).prefix(label).speed(0.1))
                                .changed();
                        };
                        match self.value {
                            Value::Vec2(v) if matches!(model, Model::Vec2) => {
                                axis("X ", &mut v.x);
                                axis("Y ", &mut v.y);
                            }
                            Value::Vec3(v) if matches!(model, Model::Vec3) => {
                                axis("X ", &mut v.x);
                                axis("Y ", &mut v.y);
                                axis("Z ", &mut v.z);
                            }
                            _ => {
                                if ui.small_button("Reset vector").clicked() {
                                    *self.value = model.default_value();
                                    changed = true;
                                }
                            }
                        }
                    })
                    .response;
                if changed {
                    response.mark_changed();
                }
                response
            }
            Some(&Model::Reference(domain)) => {
                reference_probe(ui, self.id_source, domain, self.value, self.references)
            }
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
                Value::String(value) => {
                    let mut s = String::from(value.as_str());
                    let r = s.probe(ui, style);
                    *value = s.into();
                    r
                }
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
                                *self.value = Value::String(SmolStr::default());
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
                    || Box::new(model.as_ref().map_or(Value::Unit, |m| m.default_value())),
                    |value, ui, style| {
                        ValueProbe::new(model.as_deref(), value, self.local_id.with("some"))
                            .with_references(self.references)
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
                            values.resize_with(len, || {
                                elem.as_ref().map_or(Value::Unit, |e| e.default_value())
                            });
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
                                        .map(|_| {
                                            elem.as_ref().map_or(Value::Unit, |e| e.default_value())
                                        })
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

                                values.insert(std::mem::take(&mut new_key.0).into(), value);
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
                                *self.value = Value::Map(HashMap::default());
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
                                            **value = vmodel
                                                .as_ref()
                                                .map_or(Value::Unit, |m| m.default_value());
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
                                        **value =
                                            v.1.as_ref().map_or(Value::Unit, |m| m.default_value());
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
                                            ValueProbe::new(Some(model), value, self.local_id)
                                                .with_references(self.references);
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
                                *self.value = Value::Enum(
                                    v.0,
                                    Box::new(
                                        v.1.as_ref().map_or(Value::Unit, |m| m.default_value()),
                                    ),
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

            _ => todo!(),
        }
    }

    fn iterate_inner(&mut self, ui: &mut Ui, f: &mut dyn FnMut(&str, &mut Ui, &mut dyn EguiProbe)) {
        match self.model {
            None => {
                let local_elem = local_model_inner(ui, self.local_id, f);
                let mut probe = ValueProbe::new(Some(&local_elem), self.value, self.local_id)
                    .with_references(self.references);
                f("value", ui, &mut probe);
            }
            Some(Model::Unit) => {}
            Some(Model::Bool) => {}
            Some(Model::Int { .. }) => {}
            Some(Model::Float { .. }) => {}
            Some(Model::String { .. }) => {}
            Some(Model::Color(_)) => {}
            Some(Model::Reference(_)) => {}
            Some(Model::Vec2 | Model::Vec3) => {}
            Some(Model::Record(fields)) => {
                if let Value::Map(values) = self.value {
                    for (name, model) in fields {
                        if let Some(value) = values.get_mut(name.as_str()) {
                            let mut probe = ValueProbe::new(
                                model.as_ref(),
                                value,
                                self.id_source.with(name.as_str()),
                            )
                            .with_references(self.references);
                            f(name.as_str(), ui, &mut probe);
                        }
                    }
                }
            }
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
                                    ValueProbe::new(Some(elem), value, self.local_id.with(idx))
                                        .with_references(self.references);
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
                                    ValueProbe::new(Some(elem), value, self.local_id.with(idx))
                                        .with_references(self.references);
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
                                ValueProbe::new(Some(elem), value, self.local_id.with(idx))
                                    .with_references(self.references);
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
            Some(Model::Enum(variants)) => match *self.value {
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

                    let mut probe = ValueProbe::new(Some(model), value, self.local_id)
                        .with_references(self.references);
                    f("value", ui, &mut probe);
                }
                _ => {}
            },
            _ => todo!(),
        }
    }
}

fn parse_reference_id(text: &str) -> Result<u64, &'static str> {
    let id = text
        .trim()
        .parse::<u64>()
        .map_err(|_| "Enter an unsigned 64-bit decimal ID")?;
    if id == 0 {
        return Err("Reference IDs must be nonzero");
    }
    Ok(id)
}

fn reference_probe(
    ui: &mut Ui,
    id_source: Id,
    domain: Ident,
    value: &mut Value,
    references: &[ReferenceChoices],
) -> Response {
    let current = match *value {
        Value::Reference(actual, bits) if actual == domain => Some(bits),
        _ => None,
    };
    let mut changed = false;
    let mut response = ui
        .vertical(|ui| {
            match *value {
                Value::Reference(actual, _) if actual != domain => {
                    ui.colored_label(
                        egui::Color32::RED,
                        format!("Expected {domain} reference; found {actual}"),
                    );
                }
                Value::Reference(_, 0) => {
                    ui.colored_label(egui::Color32::RED, "Reference ID is zero");
                }
                Value::Reference(_, _) => {}
                _ => {
                    ui.colored_label(
                        egui::Color32::RED,
                        format!("Expected {domain} reference; found {}", value.kind()),
                    );
                }
            }

            if let Some(choices) = references.iter().find(|choices| choices.domain == domain) {
                let selected = current.and_then(|bits| {
                    choices
                        .values
                        .iter()
                        .find(|(candidate, _)| *candidate == bits)
                });
                let label = selected.map_or_else(
                    || match current {
                        Some(bits) => format!("Unresolved {domain}: {bits}"),
                        None => format!("Select {domain}"),
                    },
                    |(_, label)| label.clone(),
                );
                egui::ComboBox::from_id_salt(id_source.with("reference-choice"))
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        for (bits, label) in &choices.values {
                            if ui.selectable_label(current == Some(*bits), label).clicked() {
                                let replacement = Value::Reference(domain, *bits);
                                changed |= *value != replacement;
                                *value = replacement;
                            }
                        }
                    });
                if choices.values.is_empty() {
                    ui.weak(format!("No {domain} references available"));
                } else if current.is_some() && selected.is_none() {
                    ui.colored_label(egui::Color32::RED, "The referenced item is unavailable");
                }
            } else {
                let id = ui.make_persistent_id(id_source.with("reference-id"));
                let mut text = if ui.memory(|memory| memory.has_focus(id)) {
                    ui.data(|data| data.get_temp::<String>(id))
                } else {
                    None
                }
                .unwrap_or_else(|| current.map_or_else(String::new, |bits| bits.to_string()));
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut text)
                        .id(id)
                        .hint_text(format!("{domain} ID"))
                        .desired_width(190.0),
                );
                match parse_reference_id(&text) {
                    Ok(bits) if edit.changed() => {
                        let replacement = Value::Reference(domain, bits);
                        changed |= *value != replacement;
                        *value = replacement;
                    }
                    Err(error) if edit.has_focus() || !text.is_empty() => {
                        ui.colored_label(egui::Color32::RED, error);
                    }
                    _ => {}
                }
                if edit.has_focus() {
                    ui.data_mut(|data| data.insert_temp(id, text));
                } else {
                    ui.data_mut(|data| data.remove::<String>(id));
                }
            }
        })
        .response;
    if changed {
        response.mark_changed();
    }
    response
}

fn convert_to_string<T: ToString>(
    ui: &mut Ui,
    value: &T,
    kind: &str,
) -> (Response, Option<SmolStr>) {
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

    (r, if convert { Some(s.into()) } else { None })
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

#[cfg(test)]
mod reference_tests {
    use super::parse_reference_id;

    #[test]
    fn reference_ids_keep_all_64_bits() {
        assert_eq!(parse_reference_id("18446744073709551615"), Ok(u64::MAX));
        assert_eq!(parse_reference_id("9007199254740993"), Ok((1 << 53) + 1));
        assert_eq!(parse_reference_id("1"), Ok(1));
        for text in ["0", "-1", "18446744073709551616", "1.5", ""] {
            assert!(parse_reference_id(text).is_err());
        }
    }
}

#[cfg(test)]
mod spatial_tests {
    use super::*;
    use arcana::{athena::Vector2, model::Reflect, scene::dim2};

    #[test]
    fn record_vector_field_can_be_dragged() {
        let model = dim2::Rect::model();
        let mut value = dim2::Rect {
            min: Vector2::new(2.0, 3.0),
            max: Vector2::new(10.0, 20.0),
        }
        .to_value();
        let context = egui::Context::default();
        let mut pointer = egui::Pos2::ZERO;
        let mut changed = false;
        for frame in 0..5 {
            let mut input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(500.0, 300.0),
                )),
                ..Default::default()
            };
            if frame > 0 {
                input.events.push(egui::Event::PointerMoved(pointer));
            }
            if frame == 2 || frame == 4 {
                input.events.push(egui::Event::PointerButton {
                    pos: pointer,
                    button: egui::PointerButton::Primary,
                    pressed: frame == 2,
                    modifiers: Default::default(),
                });
            }
            let mut output = context.run_ui(input, |ui| {
                let mut probe = ValueProbe::new(Some(&model), &mut value, "bounds-test");
                probe.probe(ui, &Style::default());
                probe.iterate_inner(ui, &mut |name, ui, field| {
                    let response = field.probe(ui, &Style::default());
                    changed |= response.changed();
                    if name == "min" && frame == 0 {
                        pointer = egui::pos2(response.rect.left() + 15.0, response.rect.center().y);
                    }
                    field.iterate_inner(ui, &mut |_, _, _| panic!("vector has no children"));
                });
            });
            output.textures_delta.clear();
            if frame == 2 {
                pointer.x += 30.0;
            }
        }
        let edited = dim2::Rect::from_value(value).expect("valid edited bounds");
        assert!(changed);
        assert_ne!(edited.min.x, 2.0);
        assert_eq!(edited.min.y, 3.0);
        assert_eq!(edited.max, Vector2::new(10.0, 20.0));
    }
}
