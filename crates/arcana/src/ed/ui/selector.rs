use std::hash::Hash;

enum OnAbsent<K> {
    Clear,
    PickFirst,
    Keep(Box<dyn Fn(&K) -> &str>),
}

/// Selector widget to pick element from available options.
pub struct Selector<K, E> {
    id: egui::Id,
    get_text: Box<dyn for<'a> Fn(&'a K, &'a E) -> &'a str>,
    on_absent: OnAbsent<K>,
}

impl<K, E> Selector<K, E> {
    pub fn new<F>(id_source: impl Hash, get_text: F) -> Self
    where
        F: for<'a> Fn(&'a K, &'a E) -> &'a str + 'static,
    {
        Selector {
            id: egui::Id::new(id_source),
            get_text: Box::new(get_text),
            on_absent: OnAbsent::Clear,
        }
    }

    pub fn keep_absent<F>(mut self, get_text: F) -> Self
    where
        F: Fn(&K) -> &str + 'static,
    {
        self.on_absent = OnAbsent::Keep(Box::new(get_text));
        self
    }

    pub fn pick_first(mut self) -> Self {
        self.on_absent = OnAbsent::PickFirst;
        self
    }

    pub fn show<'a>(
        self,
        current: &mut Option<K>,
        options: impl Iterator<Item = (&'a K, &'a E)> + Clone,
        ui: &mut egui::Ui,
    ) -> egui::Response
    where
        K: Clone + PartialEq + 'a,
        E: 'a,
    {
        let mut current_text = "";
        let current_entry = options.clone().find(|(k, _)| Some(*k) == current.as_ref());

        match current_entry {
            None => match self.on_absent {
                OnAbsent::Clear => {
                    *current = None;
                }
                OnAbsent::PickFirst => {
                    *current = options.clone().next().map(|(k, _)| k.clone());
                }
                OnAbsent::Keep(get_text) => match current {
                    None => {}
                    Some(k) => {
                        current_text = get_text(k);
                    }
                },
            },
            Some((k, e)) => {
                current_text = (self.get_text)(k, e);
            }
        }

        let ir = egui::ComboBox::from_id_source(self.id)
            .selected_text(current_text)
            .show_ui(ui, |ui| {
                for (k, e) in options {
                    let text = (self.get_text)(k, e);
                    if ui
                        .selectable_label(current.as_ref() == Some(k), text)
                        .clicked_by(egui::PointerButton::Primary)
                    {
                        *current = Some(k.clone());
                        ui.close_menu();
                    }
                }
            });

        if ir.response.clicked_by(egui::PointerButton::Secondary) {
            *current = None;
        }

        ir.response
    }
}
