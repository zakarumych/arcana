use std::sync::Arc;

use arcana::error::Error;
use chrono::{DateTime, Duration, Local};
use egui::Vec2;
use parking_lot::Mutex;

pub struct ToasterLayer {
    toasts: Arc<Mutex<Vec<Toast>>>,
}

impl<S> tracing_subscriber::Layer<S> for ToasterLayer
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(&self, event: &tracing::Event, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let level = match *event.metadata().level() {
            tracing::Level::ERROR => Level::Error,
            tracing::Level::WARN => Level::Warning,
            _ => return,
        };

        let mut message = None;

        event.record(
            &mut |field: &tracing::field::Field, value: &dyn core::fmt::Debug| {
                if field.name() == "message" {
                    message = Some(format!("{value:?}"));
                }
            },
        );

        if let Some(message) = message {
            let mut toasts = self.toasts.lock();
            toasts.push(Toast {
                idx: 0,
                level,
                message,
                expanded: None,
                timestamp: Local::now(),
            });
        }
    }
}

struct Toast {
    idx: u64,
    level: Level,
    message: String,
    expanded: Option<String>,
    timestamp: DateTime<Local>,
}

impl Toast {
    fn time_left(&self, now: DateTime<Local>) -> Duration {
        let elapsed = now - self.timestamp;
        let max_duration = match self.level {
            Level::Error => DEFAULT_ERROR_DURATION,
            Level::Warning => DEFAULT_WARN_DURATION,
            _ => DEFAULT_INFO_DURATION,
        };

        (max_duration - elapsed).max(Duration::zero())
    }

    fn time_fraction_left(&self, now: DateTime<Local>) -> f32 {
        let elapsed = now - self.timestamp;
        let max_duration = match self.level {
            Level::Error => DEFAULT_ERROR_DURATION,
            Level::Warning => DEFAULT_WARN_DURATION,
            _ => DEFAULT_INFO_DURATION,
        };

        (max_duration - elapsed).num_milliseconds() as f32 / max_duration.num_milliseconds() as f32
    }

    fn color(&self, theme: egui::Theme) -> egui::Color32 {
        match (self.level, theme) {
            (Level::Error, egui::Theme::Dark) => egui::Color32::RED,
            (Level::Error, egui::Theme::Light) => egui::Color32::RED,
            (Level::Warning, egui::Theme::Dark) => egui::Color32::YELLOW,
            (Level::Warning, egui::Theme::Light) => egui::Color32::BROWN,
            (Level::Info, egui::Theme::Dark) => egui::Color32::WHITE,
            (Level::Info, egui::Theme::Light) => egui::Color32::BLACK,
        }
    }

    fn animated(&self) -> bool {
        matches!(self.level, Level::Info | Level::Warning)
    }
}

#[derive(Clone, Copy)]
enum Level {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Default)]
struct ToasterWidgetState {
    toasts: Vec<ToastState>,
}

#[derive(Copy, Clone)]
struct ToastState {
    idx: u64,
    size: egui::Vec2,
}

pub struct Toaster {
    next_idx: u64,
    toasts: Vec<Toast>,
    remote: Arc<Mutex<Vec<Toast>>>,
}

impl Toaster {
    pub fn new() -> Self {
        Toaster {
            next_idx: 0,
            toasts: Vec::new(),
            remote: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn tracing_layer(&self) -> ToasterLayer {
        ToasterLayer {
            toasts: self.remote.clone(),
        }
    }

    fn push(&mut self, level: Level, message: String) {
        self.toasts.push(Toast {
            level,
            message,
            expanded: None,
            timestamp: Local::now(),
            idx: self.next_idx,
        });
        self.next_idx += 1;
    }

    pub fn push_error(&mut self, message: String, error: &Error) {
        self.toasts.push(Toast {
            level: Level::Error,
            message: format!("{message}\n{error:?}"),
            expanded: Some(format!("{message}\n{error:#?}")),
            timestamp: Local::now(),
            idx: self.next_idx,
        });
        self.next_idx += 1;
    }

    pub fn push_info(&mut self, message: String) {
        self.push(Level::Info, message);
    }

    pub fn tick(&mut self) {
        let len = self.toasts.len();
        self.toasts.append(&mut self.remote.lock());
        for i in len..self.toasts.len() {
            self.toasts[i].idx = self.next_idx;
            self.next_idx += 1;
        }

        let now = Local::now();

        self.toasts.retain(|toast| !toast.time_left(now).is_zero());
    }

    pub fn show(&mut self, cx: &egui::Context) {
        let theme = cx.theme();
        let now = Local::now();
        let content_rect = cx.content_rect();
        let toaster_id = egui::Id::new("arcana-ed-toaster-area");

        let mut state = cx.data(|data| {
            data.get_temp::<ToasterWidgetState>(toaster_id)
                .unwrap_or_default()
        });

        // Remove toasts that are not in the toaster anymore.
        state
            .toasts
            .retain(|toast_state| self.toasts.iter().any(|toast| toast.idx == toast_state.idx));

        debug_assert_eq!(
            state
                .toasts
                .iter()
                .map(|toast| toast.idx)
                .collect::<Vec<_>>(),
            self.toasts[..state.toasts.len()]
                .iter()
                .map(|toast| toast.idx)
                .collect::<Vec<_>>()
        );

        // Initialize toasts that are in the toaster but not in the state yet.
        for i in state.toasts.len()..self.toasts.len() {
            state.toasts.push(ToastState {
                idx: self.toasts[i].idx,
                size: egui::Vec2::ZERO,
            });
        }

        if state.toasts.is_empty() {
            return;
        }

        let style = cx.style();
        let toast_spacing = style.spacing.item_spacing.y;
        let desired_margin = style.spacing.window_margin;

        // Calculate toasts size.
        let toasts_size = state
            .toasts
            .iter()
            .fold(egui::Vec2::new(0.0, -toast_spacing), |mut acc, toast| {
                acc.y += toast.size.y + toast_spacing;
                acc.x = acc.x.max(toast.size.x);
                acc
            })
            .max(egui::Vec2::ZERO);

        let cumulative_height = state
            .toasts
            .iter()
            .rev()
            .scan(0.0, |acc, toast| {
                let height = *acc;
                *acc += toast.size.y + toast_spacing;
                Some(height)
            })
            .collect::<Vec<_>>();

        // Show toasts.
        let area = egui::Area::new(toaster_id)
            .order(egui::Order::Foreground)
            .fixed_pos(content_rect.left_top())
            .movable(false)
            .kind(egui::UiKind::Popup)
            .sense(egui::Sense::HOVER);

        area.show(cx, |ui| {
            ui.set_max_size(content_rect.right_bottom() - ui.max_rect().left_top());
            // Position toasts at the bottom right of the screen.

            let max_rect = ui.max_rect();

            // ui.painter()
            //     .debug_rect(max_rect, egui::Color32::BLUE, "max_rect");
            let max_margin = (max_rect.size() - toasts_size).max(Vec2::ZERO) * 0.5;

            let toasts_max = egui::pos2(
                max_rect.right() - desired_margin.rightf().min(max_margin.x),
                max_rect.bottom() - desired_margin.bottomf().min(max_margin.y),
            );

            let toasts_min = egui::pos2(
                max_rect.left() + desired_margin.leftf().min(max_margin.x),
                max_rect.top() + desired_margin.topf().min(max_margin.y),
            );

            let toasts_rect = egui::Rect::from_min_max(toasts_min, toasts_max);
            // ui.painter()
            //     .debug_rect(toasts_rect, egui::Color32::GREEN, "toasts_rect");

            // Toats are positioned from the bottom to the top, eating space as they go up, overlapping each other if there's not enough vertical space.
            let mut platform = toasts_rect.bottom();

            let mut remove_idxs = Vec::new();

            for ((toast, state), &height_on_top) in self
                .toasts
                .iter()
                .zip(state.toasts.iter_mut())
                .zip(cumulative_height.iter().rev())
            {
                debug_assert_eq!(toast.idx, state.idx);

                // ui.painter().debug_rect(
                //     toasts_rect.with_min_y(platform).with_max_y(platform),
                //     egui::Color32::RED,
                //     format!("toast {} platform", toast.idx),
                // );

                let toast_id = ui.make_persistent_id(("arcana-toast", toast.idx));
                let height_available = platform - toasts_rect.top();
                let height_required = height_on_top + state.size.y;

                let mut bottom = ui
                    .ctx()
                    .animate_value_with_time(toast_id, platform, 1.0f32)
                    .max(toasts_rect.top() + height_required) // if floating too hight that it won't fit, snap down to minimum depth required to fit the toast.
                    .min(platform); // but don't go below the platform just yet.

                if height_required > 0.0 && height_available < height_required {
                    // Not enougth space to fit all remaining toasts without floating at all.
                    // Overlap with the previous toast

                    // How much linear overlap would do we need to fit all remaining toasts.
                    let factor = height_available / height_required;

                    // Apply non-linearity.
                    // This makes bottom toasts overlap more than top toasts.
                    let factor2 = factor * factor;
                    let overlap = (1.0 - factor2) * (state.size.y + toast_spacing);

                    platform = (platform + overlap).min(toasts_rect.bottom());
                    bottom = platform;
                }

                let toast_rect = egui::Rect::from_min_max(
                    egui::pos2(toasts_rect.right() - state.size.x, bottom - state.size.y),
                    egui::pos2(toasts_rect.right(), bottom),
                );

                {
                    let mut toast_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(toast_rect)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );

                    let frame = egui::Frame::window(ui.style());
                    let r = frame
                        .show(&mut toast_ui, |toast_ui| {
                            let toast_color = toast.color(theme);
                            let lr = toast_ui
                                .label(egui::RichText::new(&toast.message).color(toast_color));
                            let br = toast_ui.small_button("x");
                            if br.clicked() {
                                remove_idxs.push(toast.idx);
                            };

                            let rect = lr.rect.union(br.rect);

                            let progress_rect = egui::Rect::from_min_max(
                                egui::pos2(rect.left(), rect.bottom() + toast_spacing),
                                egui::pos2(rect.right(), rect.bottom() + toast_spacing * 2.0),
                            );

                            toast_ui.allocate_rect(progress_rect, egui::Sense::hover());

                            let progress_width = progress_rect.width();
                            let fill_width = toast.time_fraction_left(now);

                            let fill_rect = egui::Rect::from_min_max(
                                progress_rect.left_top(),
                                egui::pos2(
                                    progress_rect.left() + progress_width * fill_width,
                                    progress_rect.bottom(),
                                ),
                            );

                            ui.painter().rect_stroke(
                                progress_rect,
                                0.0,
                                egui::Stroke::new(1.0, toast_color),
                                egui::StrokeKind::Middle,
                            );
                            ui.painter().rect_filled(fill_rect, 0.0, toast_color);
                        })
                        .response;

                    if let Some(expanded) = &toast.expanded {
                        r.on_hover_ui(|ui| {
                            ui.label(expanded);
                        });
                    }

                    // Update toast size in the state.

                    assert!(!toast_ui.min_size().any_nan());
                    state.size = toast_ui.min_size();

                    platform = bottom - state.size.y - toast_spacing;
                }
            }

            for idx in remove_idxs {
                self.toasts.retain(|toast| toast.idx != idx);
                state.toasts.retain(|toast_state| toast_state.idx != idx);
            }

            if self.toasts.iter().any(|toast| toast.animated()) {
                ui.ctx().request_repaint();
            }

            cx.data_mut(|data| data.insert_temp(toaster_id, state));
        });
    }
}

const DEFAULT_INFO_DURATION: Duration = Duration::seconds(5);
const DEFAULT_WARN_DURATION: Duration = Duration::seconds(10);
const DEFAULT_ERROR_DURATION: Duration = Duration::MAX;
