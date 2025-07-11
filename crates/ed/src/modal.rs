//! This module implements the modal UI system for the editor.

use egui::{Ui, WidgetText};

/// Container for modal windows.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct ModalStack {
    /// Stack of modal open windows.
    ///
    /// Only top-most window is interactive.
    /// Other are rendered with UI interactions disabled.
    array: Vec<egui::Id>,
}

pub enum ModalStatus {
    Open,
    Closed,
}

impl ModalStack {
    pub fn new() -> Self {
        ModalStack { array: Vec::new() }
    }

    pub fn push(&mut self, id: egui::Id) {
        self.array.push(id);
    }

    pub fn is_empty(&self) -> bool {
        self.array.is_empty()
    }

    pub fn contains(&self, id: egui::Id) -> bool {
        self.array.contains(&id)
    }

    pub fn show<R>(
        &mut self,
        ctx: &egui::Context,
        modal: Modal,
        add_contents: impl FnOnce(&mut Ui) -> ModalStatus,
    ) -> ModalStatus {
        if !self.contains(modal.id) {
            return ModalStatus::Closed;
        }

        let is_top = self.array.last() == Some(&modal.id);
        let window = modal.window.enabled(is_top);
        let mut is_open = true;
        let r = window
            .open(&mut is_open)
            .show(ctx, add_contents)
            .expect("Window should open");

        match (r.inner, is_open) {
            (None | Some(ModalStatus::Open), true) => ModalStatus::Open,
            (_, false) | (Some(ModalStatus::Closed), true) => {
                debug_assert!(is_top, "Only top-most modal can be closed");
                self.array.retain(|&id| id != modal.id);
                ModalStatus::Closed
            }
        }
    }
}

#[must_use = "You should call `ModalStack::show` to display the modal window."]
pub struct Modal {
    id: egui::Id,
    window: egui::Window<'static>,
}

impl Modal {
    pub fn new(title: egui::WidgetText) -> Self {
        let id = egui::Id::new(&title);
        let window = egui::Window::new(title)
            .id(id)
            .resizable(false)
            .collapsible(false);
        Modal { id, window }
    }

    pub fn id(&self) -> egui::Id {
        self.id
    }

    pub fn set_id(&mut self, id: egui::Id) {
        self.id = id;
        self.window = self.window.id(id);
    }

    pub fn with_id(mut self, id: egui::Id) -> Self {
        self.id = id;
        self.window = self.window.id(id);
        self
    }
}
