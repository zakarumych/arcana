use std::collections::BTreeMap;

use arcana::{gametime::ClockStep, mev};
use serde_json::Value;
use winit::window::WindowId;

use crate::{
    error::ModalErrors,
    filters::FilterManager,
    ide::Ide,
    plugins::{PluginManager, PluginsTemplate},
    project::{Project, ProjectData},
    render::{RenderManager, RenderTemplate},
    simulation::{Simulation, SimulationTemplate},
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
        toolbox.register(Box::new(RenderTemplate));
        toolbox.register(Box::new(SimulationTemplate));
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
    pub simulation: &'a mut Simulation,
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
}

#[derive(Default)]
pub(crate) struct Tools {
    next_id: ToolId,
    entries: BTreeMap<ToolId, MaterializedTool>,
}

#[derive(serde::Serialize, serde::Deserialize)]
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
        }
    }

    pub fn remove(&mut self, id: ToolId) {
        self.entries.remove(&id);
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

    pub fn restore(&mut self, toolbox: &Toolbox, states: Vec<ToolState>) -> Vec<Option<ToolId>> {
        self.entries.clear();
        states
            .into_iter()
            .map(|state| {
                let tool = toolbox.materialize(&state.template, Some(state.state))?;
                Some(self.insert(tool, Some(state.template)))
            })
            .collect()
    }
}
