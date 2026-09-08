use std::collections::BTreeMap;

use arcana::{gametime::ClockStep, mev};
use serde_json::Value;
use winit::{event::WindowEvent, window::WindowId};

use crate::{
    error::ModalErrors,
    filters::{FilterManager, FiltersTemplate},
    ide::Ide,
    plugins::{PluginManager, PluginsTemplate},
    project::{Project, ProjectData},
    render::{RenderManager, RenderTemplate},
    scene_editor::{SceneEditor, SceneEditorTemplate},
    systems::{SystemManager, SystemsTemplate},
    toaster::Toaster,
    ui::UserTextures,
};

pub(crate) type ToolId = u64;

/// A view is a unique attachment to a materialized tool.
pub(crate) struct View {
    tool: ToolId,
}

impl View {
    pub fn tool(&self) -> ToolId {
        self.tool
    }
}

pub(crate) trait Tool {
    fn title(&self) -> &str;

    fn ui(&mut self, ui: &mut egui::Ui, window: WindowId, context: &mut ToolContext<'_>);

    fn update(&mut self, _step: ClockStep, _context: &mut ToolContext<'_>) {}

    fn handle_event(
        &mut self,
        _window: WindowId,
        _event: &WindowEvent,
        _context: &mut ToolContext<'_>,
    ) -> bool {
        false
    }

    fn close(&mut self, _textures: &mut UserTextures<'_>) {}

    /// Session-only tools need not provide a snapshot.
    fn save(&self) -> Option<Value> {
        None
    }
}

pub(crate) trait ToolTemplate {
    fn key(&self) -> &str;
    fn title(&self) -> &str;
    fn create(&self, state: Option<Value>) -> Result<Box<dyn Tool>, serde_json::Error>;
}

pub(crate) struct Toolbox {
    templates: Vec<Box<dyn ToolTemplate>>,
}

impl Toolbox {
    pub fn new() -> Self {
        let mut toolbox = Self {
            templates: Vec::new(),
        };
        toolbox.register(Box::new(PluginsTemplate));
        toolbox.register(Box::new(SystemsTemplate));
        toolbox.register(Box::new(FiltersTemplate));
        toolbox.register(Box::new(RenderTemplate));
        toolbox.register(Box::new(SceneEditorTemplate));
        toolbox
    }

    pub fn register(&mut self, template: Box<dyn ToolTemplate>) {
        assert!(
            !self.templates.iter().any(|t| t.key() == template.key()),
            "duplicate tool template"
        );
        self.templates.push(template);
    }

    pub fn templates(&self) -> impl Iterator<Item = &dyn ToolTemplate> {
        self.templates.iter().map(Box::as_ref)
    }

    pub fn materialize(&self, key: &str, state: Option<Value>) -> Option<Box<dyn Tool>> {
        let template = self.templates().find(|t| t.key() == key)?;
        match template.create(state) {
            Ok(tool) => Some(tool),
            Err(error) => {
                tracing::warn!("Cannot restore tool {key}: {error}");
                None
            }
        }
    }
}

pub(crate) enum ToolCommand {
    Create {
        tool: Box<dyn Tool>,
        template: Option<String>,
        open: bool,
    },
    Open(ToolId),
    Detach(ToolId),
    Close(ToolId),
}

/// Shared services and deferred child-tool requests, supplied afresh for every call.
pub(crate) struct ToolContext<'a> {
    pub scene_editor: &'a mut SceneEditor,
    pub plugins: &'a mut PluginManager,
    pub systems: &'a mut SystemManager,
    pub filters: &'a mut FilterManager,
    pub renders: &'a mut RenderManager,

    pub queue: &'a mut mev::Queue,
    pub cvt: &'a mut mev::kernels::Cvt,
    pub textures: UserTextures<'a>,

    pub errors: &'a mut ModalErrors,
    pub toaster: &'a mut Toaster,

    pub ide: Option<&'a dyn Ide>,
    pub toolbox: &'a Toolbox,
    pub commands: &'a mut Vec<ToolCommand>,

    pub project: &'a Project,
    pub data: &'a ProjectData,
}

impl ToolContext<'_> {
    /// A child without a template is retained for this session only.
    pub fn create_child(&mut self, tool: Box<dyn Tool>, open: bool) {
        self.commands.push(ToolCommand::Create {
            tool,
            template: None,
            open,
        });
    }

    pub fn open(&mut self, tool: ToolId) {
        self.commands.push(ToolCommand::Open(tool));
    }
}

pub(crate) struct MaterializedTool {
    pub tool: Box<dyn Tool>,
    template: Option<String>,
    attached: bool,
    visible_window: Option<WindowId>,
}

#[derive(Default)]
pub(crate) struct Tools {
    next_id: ToolId,
    entries: BTreeMap<ToolId, MaterializedTool>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct ToolState {
    pub template: String,
    pub state: Value,
}

impl Tools {
    pub fn insert(&mut self, tool: Box<dyn Tool>, template: Option<String>) -> ToolId {
        let id = self.next_id;
        self.next_id = id.checked_add(1).expect("tool IDs exhausted");
        self.entries.insert(
            id,
            MaterializedTool {
                tool,
                template,
                attached: false,
                visible_window: None,
            },
        );
        id
    }

    pub fn attach(&mut self, id: ToolId) -> Option<View> {
        let entry = self.entries.get_mut(&id)?;
        if entry.attached {
            return None;
        }
        entry.attached = true;
        Some(View { tool: id })
    }

    pub fn detach(&mut self, id: ToolId) {
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.attached = false;
            entry.visible_window = None;
        }
    }

    pub fn remove(&mut self, id: ToolId, textures: &mut UserTextures<'_>) {
        if let Some(mut entry) = self.entries.remove(&id) {
            entry.tool.close(textures);
        }
    }

    pub fn begin_ui(&mut self, window: WindowId) {
        for entry in self.entries.values_mut() {
            if entry.visible_window == Some(window) {
                entry.visible_window = None;
            }
        }
    }

    pub fn mark_visible(&mut self, id: ToolId, window: WindowId) {
        if let Some(entry) = self.entries.get_mut(&id) {
            if entry.attached {
                entry.visible_window = Some(window);
            }
        }
    }

    pub fn clear_visibility(&mut self) {
        for entry in self.entries.values_mut() {
            entry.visible_window = None;
        }
    }

    fn dispatch_event(
        &mut self,
        window: WindowId,
        mut handle: impl FnMut(&mut dyn Tool) -> bool,
    ) -> bool {
        self.entries.values_mut().any(|entry| {
            entry.attached && entry.visible_window == Some(window) && handle(&mut *entry.tool)
        })
    }

    pub fn handle_event(
        &mut self,
        window: WindowId,
        event: &WindowEvent,
        context: &mut ToolContext<'_>,
    ) -> bool {
        self.dispatch_event(window, |tool| tool.handle_event(window, event, context))
    }

    pub fn get_mut(&mut self, id: ToolId) -> Option<&mut MaterializedTool> {
        self.entries.get_mut(&id)
    }

    pub fn title(&self, id: ToolId) -> &str {
        self.entries
            .get(&id)
            .map_or("Missing tool", |e| e.tool.title())
    }

    pub fn iter(&self) -> impl Iterator<Item = (ToolId, &MaterializedTool)> {
        self.entries.iter().map(|(&id, entry)| (id, entry))
    }

    pub fn update(&mut self, step: ClockStep, context: &mut ToolContext<'_>) {
        for entry in self.entries.values_mut() {
            entry.tool.update(step, context);
        }
    }

    pub fn snapshot(&self) -> (BTreeMap<ToolId, usize>, Vec<ToolState>) {
        let mut ids = BTreeMap::new();
        let mut states = Vec::new();
        for (id, entry) in self.iter() {
            if let (Some(template), Some(state)) = (&entry.template, entry.tool.save()) {
                ids.insert(id, states.len());
                states.push(ToolState {
                    template: template.clone(),
                    state,
                });
            }
        }
        (ids, states)
    }

    pub fn restore(
        &mut self,
        toolbox: &Toolbox,
        states: Vec<ToolState>,
        textures: &mut UserTextures<'_>,
    ) -> Vec<Option<ToolId>> {
        for (_, mut entry) in std::mem::take(&mut self.entries) {
            entry.tool.close(textures);
        }
        states
            .into_iter()
            .map(|state| {
                let tool = toolbox.materialize(&state.template, Some(state.state))?;
                Some(self.insert(tool, Some(state.template)))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;

    struct FakeTool {
        name: &'static str,
        lifecycle: Rc<RefCell<Vec<String>>>,
    }

    impl Tool for FakeTool {
        fn title(&self) -> &str {
            self.name
        }

        fn ui(&mut self, _: &mut egui::Ui, _: WindowId, _: &mut ToolContext<'_>) {}

        fn close(&mut self, _: &mut UserTextures<'_>) {
            self.lifecycle
                .borrow_mut()
                .push(format!("close {}", self.name));
        }
    }

    impl Drop for FakeTool {
        fn drop(&mut self) {
            self.lifecycle
                .borrow_mut()
                .push(format!("drop {}", self.name));
        }
    }

    fn insert(
        tools: &mut Tools,
        name: &'static str,
        lifecycle: &Rc<RefCell<Vec<String>>>,
    ) -> ToolId {
        tools.insert(
            Box::new(FakeTool {
                name,
                lifecycle: lifecycle.clone(),
            }),
            None,
        )
    }

    #[test]
    fn input_routes_only_to_visible_attached_tools_and_stops_at_first_consumer() {
        let mut tools = Tools::default();
        let lifecycle = Rc::new(RefCell::new(Vec::new()));
        let first = insert(&mut tools, "first", &lifecycle);
        let second = insert(&mut tools, "second", &lifecycle);
        let other = insert(&mut tools, "other", &lifecycle);
        let window = WindowId::from(1);
        let other_window = WindowId::from(2);
        for id in [first, second, other] {
            assert!(tools.attach(id).is_some());
        }
        tools.mark_visible(first, window);
        tools.mark_visible(second, window);
        tools.mark_visible(other, other_window);
        let mut routed = Vec::new();
        assert!(tools.dispatch_event(window, |tool| {
            routed.push(tool.title().to_owned());
            true
        }));
        assert_eq!(routed, ["first"]);

        tools.begin_ui(window);
        tools.mark_visible(second, window);
        routed.clear();
        assert!(!tools.dispatch_event(window, |tool| {
            routed.push(tool.title().to_owned());
            false
        }));
        assert_eq!(routed, ["second"]);
        assert!(tools.dispatch_event(other_window, |_| true));

        tools.detach(second);
        assert!(!tools.dispatch_event(window, |_| true));
        assert!(tools.get_mut(second).is_some());
        assert!(lifecycle.borrow().is_empty());
        assert!(tools.attach(second).is_some());
        assert!(tools.attach(second).is_none());
        assert!(!tools.dispatch_event(window, |_| true));

        tools.clear_visibility();
        assert!(!tools.dispatch_event(other_window, |_| true));
    }

    #[test]
    fn remove_and_restore_close_tools_before_dropping_them() {
        let mut tools = Tools::default();
        let lifecycle = Rc::new(RefCell::new(Vec::new()));
        let removed = insert(&mut tools, "removed", &lifecycle);
        let retained = insert(&mut tools, "retained", &lifecycle);
        let mut ui = crate::ui::Ui::new();
        tools.remove(removed, &mut ui.textures());
        tools.remove(removed, &mut ui.textures());
        assert_eq!(*lifecycle.borrow(), ["close removed", "drop removed"]);
        assert!(tools.get_mut(retained).is_some());

        let restored = tools.restore(&Toolbox::new(), Vec::new(), &mut ui.textures());
        assert!(restored.is_empty());
        assert_eq!(
            *lifecycle.borrow(),
            [
                "close removed",
                "drop removed",
                "close retained",
                "drop retained"
            ]
        );
    }
}
