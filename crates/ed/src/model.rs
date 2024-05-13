use std::hash::Hash;

pub use ::arcana::model::{Model, Value};
use arcana::model::{default_value, ColorModel, ColorValue};
use egui::{Id, Response, Ui, Widget};
use egui_probe::{DeleteMe, EguiProbe, Style};
use hashbrown::HashMap;

pub struct ModelProbe<'a> {
    model: &'a mut Model,
    id_source: Id,
}

impl EguiProbe for ModelProbe<'_> {
    fn probe(&mut self, ui: &mut Ui, _style: &Style) -> Response {
        egui::ComboBox::from_id_source(self.id_source)
            .show_ui(ui, |ui| {
                let r = ui.selectable_label(matches!(self.model, Model::Bool), "Bool");
                if r.clicked() {
                    *self.model = Model::Bool;
                }
                let r = ui.selectable_label(matches!(self.model, Model::Int), "Int");
                if r.clicked() {
                    *self.model = Model::Int;
                }
                let r = ui.selectable_label(matches!(self.model, Model::Float), "Float");
                if r.clicked() {
                    *self.model = Model::Float;
                }
                let r = ui.selectable_label(matches!(self.model, Model::String), "String");
                if r.clicked() {
                    *self.model = Model::String;
                }
                let r = ui.selectable_label(matches!(self.model, Model::Color(_)), "Color");
                if r.clicked() && !matches!(self.model, Model::Color(_)) {
                    *self.model = Model::Color(ColorModel::Srgb);
                }
                let r = ui.selectable_label(matches!(self.model, Model::Option(_)), "Option");
                if r.clicked() && !matches!(self.model, Model::Option(_)) {
                    *self.model = Model::Option(None);
                }
                let r = ui.selectable_label(matches!(self.model, Model::Array { .. }), "Array");
                if r.clicked() && !matches!(self.model, Model::Array { .. }) {
                    *self.model = Model::Array {
                        elem: None,
                        len: None,
                    };
                }
                let r = ui.selectable_label(matches!(self.model, Model::Map(_)), "Map");
                if r.clicked() && !matches!(self.model, Model::Map(_)) {
                    *self.model = Model::Map(None);
                }
                let r = ui.selectable_label(matches!(self.model, Model::Tuple { .. }), "Tuple");
                if r.clicked() && !matches!(self.model, Model::Tuple { .. }) {
                    *self.model = Model::Tuple(Vec::new());
                }
                let r = ui.selectable_label(matches!(self.model, Model::Record { .. }), "Record");
                if r.clicked() && !matches!(self.model, Model::Record { .. }) {
                    *self.model = Model::Record(Vec::new());
                }
            })
            .response
    }

    fn has_inner(&mut self) -> bool {
        match self.model {
            Model::Option(_) => true,
            Model::Array { .. } => true,
            Model::Map(_) => true,
            _ => false,
        }
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
                    id_source: self.id_source.with("Model"),
                };
                f("model", ui, &mut probe);
            }
            Model::Array { ref mut elem, .. } => {
                let mut probe = MaybeModelProbe {
                    model: elem,
                    id_source: self.id_source.with("Model"),
                };
                f("elem", ui, &mut probe);
            }
            Model::Map(ref mut model) => {
                let mut probe = MaybeModelProbe {
                    model: model,
                    id_source: self.id_source.with("Model"),
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
}

impl EguiProbe for MaybeModelProbe<'_> {
    fn probe(&mut self, ui: &mut Ui, _style: &Style) -> Response {
        egui::ComboBox::from_id_source(self.id_source)
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

    fn has_inner(&mut self) -> bool {
        match self.model.as_deref() {
            Some(Model::Option(_)) => true,
            Some(Model::Array { .. }) => true,
            Some(Model::Map(_)) => true,
            _ => false,
        }
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
                    id_source: self.id_source.with("Model"),
                };
                f("model", ui, &mut probe);
            }
            Some(Model::Array { elem, .. }) => {
                let mut probe = MaybeModelProbe {
                    model: elem,
                    id_source: self.id_source.with("Model"),
                };
                f("elem", ui, &mut probe);
            }
            Some(Model::Map(elem)) => {
                let mut probe = MaybeModelProbe {
                    model: elem,
                    id_source: self.id_source.with("Model"),
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
                let mut local_elem = ui
                    .ctx()
                    .data(|d| d.get_temp::<Model>(self.local_id))
                    .unwrap_or_default();

                let r = ModelProbe {
                    model: &mut local_elem,
                    id_source: self.id_source.with("none"),
                }
                .probe(ui, style);

                ui.ctx()
                    .data_mut(|d| d.insert_temp(self.local_id, local_elem.clone()));
                r
            }
            Some(&Model::Bool) => match self.value {
                Value::Bool(value) => value.probe(ui, style),
                _ => {
                    ui.horizontal(|ui| {
                        ui.strong(format!(
                            "Expected boolean, but is {} instead",
                            self.value.kind()
                        ));
                        if ui.small_button("Reset to false").clicked() {
                            *self.value = Value::Bool(false);
                        }
                        ui.strong("?");
                    })
                    .response
                }
            },
            Some(&Model::Int) => match self.value {
                Value::Int(value) => value.probe(ui, style),
                Value::Float(value) => {
                    let f = *value as i64;

                    ui.horizontal(|ui| {
                        ui.strong(format!(
                            "Expected integer, but is {} instead",
                            self.value.kind()
                        ));

                        if ui.small_button(format!("Convert to {f}")).clicked() {
                            *self.value = Value::Int(f);
                        }

                        ui.strong("?");
                    })
                    .response
                }
                _ => {
                    ui.horizontal(|ui| {
                        ui.strong(format!(
                            "Expected integer, but is {} instead",
                            self.value.kind()
                        ));
                        if ui.small_button(format!("Reset to 0")).clicked() {
                            *self.value = Value::Int(0);
                        }
                        ui.strong("?");
                    })
                    .response
                }
            },
            Some(&Model::Float) => match self.value {
                Value::Float(value) => value.probe(ui, style),
                Value::Int(value) => {
                    let f = *value as f64;

                    ui.horizontal(|ui| {
                        ui.strong(format!(
                            "Expected integer, but is {} instead",
                            self.value.kind()
                        ));

                        if ui.small_button(format!("Convert to {f:0.1}")).clicked() {
                            *self.value = Value::Float(f);
                        }

                        ui.strong("?");
                    })
                    .response
                }
                _ => {
                    ui.horizontal(|ui| {
                        ui.strong(format!(
                            "Expected integer, but is {} instead",
                            self.value.kind()
                        ));
                        if ui.small_button(format!("Reset to 0.0")).clicked() {
                            *self.value = Value::Float(0.0);
                        }
                        ui.strong("?");
                    })
                    .response
                }
            },
            Some(&Model::String) => match self.value {
                Value::String(value) => value.probe(ui, style),
                Value::Bool(value) => {
                    let (r, s) = convert_to_string(ui, value, "bool");
                    if let Some(s) = s {
                        *self.value = Value::String(s);
                    }
                    r
                }
                Value::Int(value) => {
                    let (r, s) = convert_to_string(ui, value, "int");
                    if let Some(s) = s {
                        *self.value = Value::String(s);
                    }
                    r
                }
                Value::Float(value) => {
                    let (r, s) = convert_to_string(ui, value, "float");
                    if let Some(s) = s {
                        *self.value = Value::String(s);
                    }
                    r
                }
                _ => {
                    ui.horizontal(|ui| {
                        ui.strong(format!(
                            "Expected string, but is {} instead",
                            self.value.kind()
                        ));
                        if ui.small_button("Reset to empty string").clicked() {
                            *self.value = Value::String(String::new());
                        }
                        ui.strong("?");
                    })
                    .response
                }
            },
            Some(&Model::Color(model)) => match model {
                ColorModel::Luma => match *self.value {
                    Value::Color(ColorValue::Luma(ref mut luma)) => {
                        egui::DragValue::new(&mut luma.luma)
                            .clamp_range(0.0..=1.0)
                            .max_decimals(3)
                            .ui(ui)
                    }
                    Value::Color(color) => {
                        ui.horizontal(|ui| {
                            ui.strong(format!(
                                "Expected luma color, but is {} instead",
                                color.kind()
                            ));
                            if ui.small_button("Reset to luma").clicked() {
                                *self.value = Value::Color(ColorValue::Luma(color.into_luma()));
                            }
                            ui.strong("?");
                        })
                        .response
                    }
                    _ => {
                        ui.horizontal(|ui| {
                            ui.strong(format!(
                                "Expected luma color, but is {} instead",
                                self.value.kind()
                            ));
                            if ui.small_button("Reset to black").clicked() {
                                *self.value =
                                    Value::Color(ColorValue::Luma(palette::LinLuma::new(0.0)));
                            }
                            ui.strong("?");
                        })
                        .response
                    }
                },
                ColorModel::Lumaa => match *self.value {
                    Value::Color(ColorValue::Lumaa(ref mut lumaa)) => {
                        ui.vertical(|ui| {
                            egui::DragValue::new(&mut lumaa.luma)
                                .clamp_range(0.0..=1.0)
                                .max_decimals(3)
                                .ui(ui);
                            egui::DragValue::new(&mut lumaa.alpha)
                                .clamp_range(0.0..=1.0)
                                .max_decimals(3)
                                .ui(ui);
                        })
                        .response
                    }
                    Value::Color(color) => {
                        ui.horizontal(|ui| {
                            ui.strong(format!(
                                "Expected luma color, but is {} instead",
                                color.kind()
                            ));
                            if ui.small_button("Reset to luma").clicked() {
                                *self.value = Value::Color(ColorValue::Lumaa(color.into_lumaa()));
                            }
                            ui.strong("?");
                        })
                        .response
                    }
                    _ => {
                        ui.horizontal(|ui| {
                            ui.strong(format!(
                                "Expected lumaa color, but is {} instead",
                                self.value.kind()
                            ));
                            if ui.small_button("Reset to black").clicked() {
                                *self.value = Value::Color(ColorValue::Lumaa(
                                    palette::LinLumaa::new(0.0, 1.0),
                                ));
                            }
                            ui.strong("?");
                        })
                        .response
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
                        ui.horizontal(|ui| {
                            ui.strong(format!(
                                "Expected srgb color, but is {} instead",
                                color.kind()
                            ));
                            if ui.small_button("Reset to srgb").clicked() {
                                *self.value = Value::Color(ColorValue::Srgb(color.into_srgb()));
                            }
                            ui.strong("?");
                        })
                        .response
                    }
                    _ => {
                        ui.horizontal(|ui| {
                            ui.strong(format!(
                                "Expected srgb color, but is {} instead",
                                self.value.kind()
                            ));
                            if ui.small_button("Reset to black").clicked() {
                                *self.value = Value::Color(ColorValue::Srgb(palette::Srgb::new(
                                    0.0, 0.0, 0.0,
                                )));
                            }
                            ui.strong("?");
                        })
                        .response
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
                        ui.horizontal(|ui| {
                            ui.strong(format!(
                                "Expected srgb color, but is {} instead",
                                color.kind()
                            ));
                            if ui.small_button("Reset to srgb").clicked() {
                                *self.value = Value::Color(ColorValue::Srgba(color.into_srgba()));
                            }
                            ui.strong("?");
                        })
                        .response
                    }
                    _ => {
                        ui.horizontal(|ui| {
                            ui.strong(format!(
                                "Expected srgb color, but is {} instead",
                                self.value.kind()
                            ));
                            if ui.small_button("Reset to black").clicked() {
                                *self.value = Value::Color(ColorValue::Srgba(palette::Srgba::new(
                                    0.0, 0.0, 0.0, 1.0,
                                )));
                            }
                            ui.strong("?");
                        })
                        .response
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
                        ui.horizontal(|ui| {
                            ui.strong(format!(
                                "Expected hsv color, but is {} instead",
                                color.kind()
                            ));
                            if ui.small_button("Reset to hsv").clicked() {
                                *self.value = Value::Color(ColorValue::Hsv(color.into_hsv()));
                            }
                            ui.strong("?");
                        })
                        .response
                    }
                    _ => {
                        ui.horizontal(|ui| {
                            ui.strong(format!(
                                "Expected hsv color, but is {} instead",
                                self.value.kind()
                            ));
                            if ui.small_button("Reset to black").clicked() {
                                *self.value =
                                    Value::Color(ColorValue::Hsv(palette::Hsv::new(0.0, 0.0, 0.0)));
                            }
                            ui.strong("?");
                        })
                        .response
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
                        ui.horizontal(|ui| {
                            ui.strong(format!(
                                "Expected hsva color, but is {} instead",
                                color.kind()
                            ));
                            if ui.small_button("Reset to hsva").clicked() {
                                *self.value = Value::Color(ColorValue::Hsva(color.into_hsva()));
                            }
                            ui.strong("?");
                        })
                        .response
                    }
                    _ => {
                        ui.horizontal(|ui| {
                            ui.strong(format!(
                                "Expected hsva color, but is {} instead",
                                self.value.kind()
                            ));
                            if ui.small_button("Reset to black").clicked() {
                                *self.value = Value::Color(ColorValue::Hsva(palette::Hsva::new(
                                    0.0, 0.0, 0.0, 1.0,
                                )));
                            }
                            ui.strong("?");
                        })
                        .response
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
                        ValueProbe::new(model.as_deref(), value, self.id_source.with("some"))
                            .probe(ui, style);
                    },
                ),
                _ => {
                    ui.horizontal(|ui| {
                        ui.strong(format!(
                            "Expected option, but is {} instead",
                            self.value.kind()
                        ));
                        if ui.small_button("Reset to some").clicked() {
                            *self.value = Value::Option(Some(Box::new(self.value.take())));
                        }
                        ui.strong("?");
                    })
                    .response
                }
            },
            Some(&Model::Array { ref elem, len }) => match self.value {
                Value::Array(values) => {
                    if let Some(len) = len {
                        if values.len() != len {
                            values.resize_with(len, || default_value(elem.as_deref()));
                        }
                    }

                    ui.horizontal(|ui| {
                        self.local_id = ui.make_persistent_id(self.id_source.with("List"));

                        let mut local_elem = elem.is_none().then(|| {
                            ui.ctx()
                                .data(|d| d.get_temp::<Model>(self.local_id))
                                .unwrap_or_default()
                        });

                        if let Some(local_elem) = &mut local_elem {
                            ModelProbe {
                                model: local_elem,
                                id_source: self.local_id,
                            }
                            .probe(ui, style);
                        }

                        if len.is_none() {
                            let r = ui.small_button(style.add_button_text());
                            if r.clicked() {
                                let value = elem
                                    .as_deref()
                                    .or(local_elem.as_ref())
                                    .unwrap()
                                    .default_value();

                                values.push(value);
                            }
                        }

                        if let Some(local_elem) = local_elem {
                            ui.ctx()
                                .data_mut(|d| d.insert_temp(self.local_id, local_elem));
                        }
                    })
                    .response
                }
                _ => {
                    ui.horizontal(|ui| {
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
                        }
                        ui.strong("?");
                    })
                    .response
                }
            },
            Some(&Model::Map(ref elem)) => match self.value {
                Value::Map(values) => {
                    #[derive(Clone)]
                    struct NewKey(String);

                    ui.horizontal(|ui| {
                        self.local_id = ui.make_persistent_id(self.id_source.with("Map"));

                        let mut new_key = ui
                            .ctx()
                            .data(|d| d.get_temp::<NewKey>(self.local_id))
                            .unwrap_or(NewKey(String::new()));

                        let mut local_elem = elem.is_none().then(|| {
                            ui.ctx()
                                .data(|d| d.get_temp::<Model>(self.local_id))
                                .unwrap_or_default()
                        });

                        if let Some(local_elem) = &mut local_elem {
                            ModelProbe {
                                model: local_elem,
                                id_source: self.local_id,
                            }
                            .probe(ui, style);
                        }

                        ui.text_edit_singleline(&mut new_key.0);

                        let r = ui.small_button(style.add_button_text());
                        if r.clicked() {
                            let value = elem
                                .as_deref()
                                .or(local_elem.as_ref())
                                .unwrap()
                                .default_value();

                            values.insert(std::mem::take(&mut new_key.0), value);
                        }

                        if let Some(local_elem) = local_elem {
                            ui.ctx()
                                .data_mut(|d| d.insert_temp(self.local_id, local_elem));
                        }

                        ui.ctx().data_mut(|d| d.insert_temp(self.local_id, new_key));
                    })
                    .response
                }
                _ => {
                    ui.horizontal(|ui| {
                        ui.strong(format!(
                            "Expected list, but is {} instead",
                            self.value.kind()
                        ));
                        if ui.small_button("Reset to empty map").clicked() {
                            *self.value = Value::Map(HashMap::new());
                        }
                        ui.strong("?");
                    })
                    .response
                }
            },
            _ => todo!(),
        }
    }

    fn has_inner(&mut self) -> bool {
        match self.model {
            None => true,
            Some(Model::Bool) => false,
            Some(Model::Int { .. }) => false,
            Some(Model::Float { .. }) => false,
            Some(Model::String { .. }) => false,
            Some(Model::Array { elem, .. }) => elem.is_none() || value_has_inner(self.value),
            Some(Model::Map(model)) => model.is_none() || value_has_inner(self.value),
            _ => todo!(),
        }
    }

    fn iterate_inner(&mut self, ui: &mut Ui, f: &mut dyn FnMut(&str, &mut Ui, &mut dyn EguiProbe)) {
        match self.model {
            None => {
                let local_elem = ui
                    .ctx()
                    .data(|d| d.get_temp::<Model>(self.local_id))
                    .unwrap_or_default();
                let mut probe = ValueProbe::new(Some(&local_elem), self.value, self.id_source);
                f("value", ui, &mut probe);
            }
            Some(Model::Bool) => {}
            Some(Model::Int { .. }) => {}
            Some(Model::Float { .. }) => {}
            Some(Model::String { .. }) => {}
            Some(Model::Array { elem, len }) => {
                let mut local_elem;
                let elem = match elem {
                    None => {
                        local_elem = ui
                            .ctx()
                            .data(|d| d.get_temp::<Model>(self.local_id))
                            .unwrap_or_default();
                        let mut probe = ModelProbe {
                            model: &mut local_elem,
                            id_source: self.local_id,
                        };
                        if probe.has_inner() {
                            probe.iterate_inner(ui, f);
                        }
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(self.local_id, local_elem.clone()));

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
            Some(Model::Map(ref elem)) => {
                let mut local_elem;
                let elem = match elem {
                    None => {
                        local_elem = ui
                            .ctx()
                            .data(|d| d.get_temp::<Model>(self.local_id))
                            .unwrap_or_default();
                        let mut probe = ModelProbe {
                            model: &mut local_elem,
                            id_source: self.local_id,
                        };
                        if probe.has_inner() {
                            probe.iterate_inner(ui, f);
                        }
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(self.local_id, local_elem.clone()));

                        &local_elem
                    }
                    Some(value) => &**value,
                };

                match self.value {
                    Value::Map(values) => {
                        let id: Id = self.id_source.with("List");

                        let mut idx = 0;
                        values.retain(|key, value| {
                            let mut probe = ValueProbe::new(Some(elem), value, id.with(idx));
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

fn value_has_inner(value: &Value) -> bool {
    match value {
        Value::Array(values) => !values.is_empty(),
        Value::Map(values) => !values.is_empty(),
        _ => false,
    }
}
