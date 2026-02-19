use egui::{Id, Sense};

/// Monitor widget shows values as a graph.
pub struct Monitor<T> {
    iter: T,
    id_source: Id,
}

impl<T> Monitor<T> {
    pub fn new(iter: T) -> Self {
        Monitor {
            iter,
            id_source: Id::new("monitor"),
        }
    }

    pub fn with_id_source(mut self, id_source: Id) -> Self {
        self.id_source = id_source;
        self
    }
}

mod state {
    use egui::{Context, Id};

    #[derive(Clone, Copy)]
    struct MonitorStateData {
        target_min: f32,
        target_max: f32,
    }

    pub struct MonitorState {
        target_min: f32,
        target_max: f32,
        new_target_min: f32,
        new_target_max: f32,
        min: f32,
        max: f32,
        id: Id,
    }

    impl MonitorState {
        fn initial(id: Id) -> Self {
            MonitorState {
                min: 0.0,
                max: 1.0,
                target_min: 0.0,
                target_max: 1.0,
                new_target_max: f32::NEG_INFINITY,
                new_target_min: f32::INFINITY,
                id,
            }
        }

        pub fn load(cx: &Context, id: Id) -> Self {
            match cx.data(|d| d.get_temp::<MonitorStateData>(id)) {
                Some(data) => {
                    let animation_time = cx.style().animation_time;

                    MonitorState {
                        target_min: data.target_min,
                        target_max: data.target_max,
                        new_target_min: f32::INFINITY,
                        new_target_max: f32::NEG_INFINITY,
                        min: cx.animate_value_with_time(id, data.target_min, animation_time),
                        max: cx.animate_value_with_time(id, data.target_max, animation_time),
                        id,
                    }
                }
                None => MonitorState::initial(id),
            }
        }

        pub fn store(&self, cx: &Context) {
            if self.new_target_min != self.target_min || self.new_target_max != self.target_max {
                cx.data_mut(|d| {
                    d.insert_temp(
                        self.id,
                        MonitorStateData {
                            target_min: self.new_target_min,
                            target_max: self.new_target_max,
                        },
                    );
                });
            }
        }

        pub fn min(&self) -> f32 {
            if self.min < self.max {
                self.min
            } else {
                self.max.next_down()
            }
        }

        pub fn max(&self) -> f32 {
            if self.min < self.max {
                self.max
            } else {
                self.min.next_up()
            }
        }

        pub fn bump_range(&mut self, value: f32) {
            if value < self.new_target_min {
                self.new_target_min = value;
            }

            if value > self.new_target_max {
                self.new_target_max = value;
            }
        }
    }
}

impl<T> Monitor<T>
where
    T: ExactSizeIterator<Item = f32>,
{
    pub fn show(self, ui: &mut egui::Ui) {
        let frame = egui::Frame::canvas(&ui.style());
        let iter = self.iter;
        let len = iter.len();

        let id = ui.id().with(self.id_source);
        let mut state = state::MonitorState::load(ui.ctx(), id);

        frame.show(ui, |ui: &mut egui::Ui| {
            let r = ui.allocate_response(
                egui::vec2(
                    (ui.spacing().interact_size.x * 3.0).min(ui.available_width()),
                    ui.spacing().interact_size.y.min(ui.available_height()),
                ),
                Sense::hover(),
            );

            if len >= 2 {
                ui.painter().add(egui::Shape::Path(egui::epaint::PathShape {
                    points: iter
                        .enumerate()
                        .map(|(idx, value)| {
                            state.bump_range(value);

                            let x = egui::emath::remap(
                                idx as f32,
                                0.0..=(len - 1) as f32,
                                r.rect.left()..=r.rect.right(),
                            );

                            let y = egui::emath::remap(
                                value,
                                state.min()..=state.max(),
                                r.rect.bottom()..=r.rect.top(),
                            );

                            egui::pos2(x, y)
                        })
                        .collect(),

                    closed: false,
                    fill: egui::Color32::TRANSPARENT,
                    stroke: ui.visuals().widgets.noninteractive.fg_stroke,
                }));
            }
        });

        state.store(ui.ctx());
    }
}
