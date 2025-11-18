//! Contains error types used in the ed app.

use std::path::PathBuf;

use arcana::error::Error;
use smallvec::SmallVec;
use thiserror::Error;

/// Error type for file open errors.
#[derive(Debug, Error)]
#[error("Failed to open file at {path}")]
pub struct FileOpenError {
    pub path: PathBuf,

    #[source]
    pub source: std::io::Error,
}

/// Error type for file read errors.
#[derive(Debug, Error)]
#[error("Failed to read file at {path}")]
pub struct FileReadError {
    pub path: PathBuf,

    #[source]
    pub source: std::io::Error,
}

/// Error type for file copy errors.
#[derive(Debug, Error)]
#[error("Failed to copy file from {from} to {to}")]
pub struct FileCopyError {
    pub from: PathBuf,
    pub to: PathBuf,

    #[source]
    pub source: std::io::Error,
}

/// Error widget that displays an error message in the UI.
pub struct ErrorWidget {
    title: egui::RichText,
    error: Option<Error>,
}

impl ErrorWidget {
    pub fn show(&self, ui: &mut egui::Ui) {
        ui.heading(self.title.clone());

        if let Some(error) = &self.error {
            ui.label(error.to_string());

            ui.collapsing("Details", |ui| {
                let mut text = format!("{}", error.fmt_chain());
                ui.text_edit_multiline(&mut text);
            });

            if error.backtrace().status() == std::backtrace::BacktraceStatus::Captured {
                ui.collapsing("Backtrace", |ui| {
                    let mut text = format!("{:#?}", error.backtrace());
                    ui.text_edit_multiline(&mut text);
                });
            }
        }
    }
}

/// Array of errors that are displayed in modal dialogs.
///
/// When error occurs in response to user action it can be pushed to this dialog.
///
/// Usually there can't be more than one error at a time, since when dialog has one error it
/// prevents interaction with the rest of the UI.
pub struct Errors {
    widgets: SmallVec<[(ErrorWidget, egui::ViewportId); 16]>,
}

impl Errors {
    pub fn new() -> Self {
        Errors {
            widgets: SmallVec::new(),
        }
    }

    pub fn push_error(
        &mut self,
        viewport: egui::ViewportId,
        title: impl Into<egui::RichText>,
        error: impl Into<Error>,
    ) {
        self.widgets.push((
            ErrorWidget {
                title: title.into(),
                error: Some(error.into()),
            },
            viewport,
        ));
    }

    pub fn show(&mut self, cx: &egui::Context) {
        if self.widgets.is_empty() {
            return;
        }

        let root_id = egui::Id::new("arcana-ed-error-dialog-root");

        let mut close = SmallVec::<[_; 2]>::new();
        for (idx, (error, viewport)) in self.widgets.iter().enumerate() {
            if *viewport == cx.viewport_id() {
                let id = root_id.with(idx);
                egui::Modal::new(id).show(cx, |ui| {
                    error.show(ui);
                    if ui.button("Oh well").clicked() {
                        close.push(idx);
                    }
                });
            }
        }

        // Suboptimal in general, but in practice it's at most one index which is also the last one.
        for &idx in close.iter().rev() {
            self.widgets.remove(idx);
        }
    }
}

/// Small icon that signals an error.
///
/// When asynchronous error occurs and it is related to some UI elements,
/// placing this icon next to the element is convenient for user to see what went wrong.
///
/// On hover it shows a popup with the error details.
pub struct ErrorIcon {
    widget: ErrorWidget,
}

impl ErrorIcon {
    pub fn new(title: impl Into<egui::RichText>, error: impl Into<Error>) -> Self {
        ErrorIcon {
            widget: ErrorWidget {
                title: title.into(),
                error: Some(error.into()),
            },
        }
    }

    pub fn show_with_popup(&self, ui: &mut egui::Ui) {
        let r = ui.label(
            egui::RichText::new(egui_phosphor::regular::WARNING)
                .color(egui::Color32::RED)
                .strong(),
        );

        r.on_hover_ui(|ui| {
            self.widget.show(ui);
        });
    }
}

macro_rules! try_sink_error {
    ($e:expr) => {
        $crate::errors::try_sink_error!($e => errors)
    };
    ($e:expr) => {
        $crate::errors::try_sink_error!($e => errors)
    };
}
