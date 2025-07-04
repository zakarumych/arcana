use arcana::{
    id::{make_uid, TimeUidGen},
    model::Value,
    Ident,
};
use egui::Ui;
use hashbrown::HashMap;

use crate::{container::Container, ide::Ide, instance::Instance, project::Project};

make_uid! {
    /// ID of opened tool.
    pub ToolId;
}

pub trait Tool {
    /// Called regularly to update tool state.
    ///
    /// Tool is allowed to modify `project`, `data` and `instance` as it needs.
    fn tick(&mut self, project: &mut Project, instance: &mut Instance) {
        let _ = (project, instance);
    }

    /// Shows the tool UI.
    ///
    /// This method is called when tool is visible and should render its UI.
    fn show(
        &mut self,
        project: &mut Project,
        ide: Option<&dyn Ide>,
        instance: &mut Instance,
        ui: &mut Ui,
    );

    /// Serializes the tool state to a value to allow loading it later
    /// with updated tool instance.
    fn save(&self) -> Value {
        Value::Unit
    }

    /// Loads the tool state from a value.
    fn load(&mut self, state: &Value) {
        let _ = state;
    }

    /// Updates the tool state with new container instance.
    fn update_plugins(&mut self, project: &mut Project, container: &Container) {
        let _ = (project, container);
    }
}

struct BoxedTool {
    plugin: Ident,
    name: Ident,

    last_state: Value,
    tool: Option<Box<dyn Tool>>,
}

impl BoxedTool {
    fn new(plugin: Ident, name: Ident, tool: Option<Box<dyn Tool>>) -> Self {
        BoxedTool {
            plugin,
            name,
            last_state: Value::Unit,
            tool,
        }
    }

    fn update_container(&mut self, project: &mut Project, container: &Container) {
        // This is a tool from plugin. Reload it.
        self.save();
        self.tool = None;

        if let Some(plugin) = container.get_plugin(self.plugin) {
            let _ = (project, plugin);
            todo!()
        }
    }

    fn tick(&mut self, project: &mut Project, instance: &mut Instance) {
        if let Some(tool) = &mut self.tool {
            tool.tick(project, instance);
        }
    }

    fn show(
        &mut self,
        project: &mut Project,
        ide: Option<&dyn Ide>,
        instance: &mut Instance,
        ui: &mut Ui,
    ) {
        if let Some(tool) = &mut self.tool {
            tool.show(project, ide, instance, ui);
        } else {
            ui.horizontal_centered(|ui| {
                ui.label(format!("Tool {} is not loaded", self.name));
            });
        }
    }

    fn title(&self) -> String {
        format!("{} @ {}", self.name, self.plugin)
    }

    fn save(&mut self) {
        if let Some(tool) = &self.tool {
            self.last_state = tool.save();
        }
    }
}

pub struct Toolbox {
    tools: HashMap<ToolId, BoxedTool>,
    idgen: TimeUidGen,
    container: Container,
}

impl Toolbox {
    pub fn new() -> Self {
        Toolbox {
            tools: HashMap::new(),
            idgen: TimeUidGen::random(),
            container: Container::default(),
        }
    }

    pub fn update_container(&mut self, project: &mut Project, container: &Container) {
        self.container = container.clone();
        for tool in self.tools.values_mut() {
            tool.update_container(project, container);
        }
    }

    pub fn tick(&mut self, project: &mut Project, instance: &mut Instance) {
        for tool in self.tools.values_mut() {
            tool.tick(project, instance);
        }
    }

    pub fn enumerate(&self) -> impl Iterator<Item = (Ident, Ident)> + '_ {
        [].into_iter()
    }

    pub fn add(&mut self, plugin: Ident, name: Ident) -> ToolId {
        let id = ToolId::generate(&mut self.idgen);
        let tool = BoxedTool::new(plugin, name, None);
        self.tools.insert(id, tool);
        id
    }

    pub fn remove(&mut self, id: ToolId) {
        self.tools.remove(&id);
    }

    pub fn show(
        &mut self,
        id: ToolId,
        project: &mut Project,
        ide: Option<&dyn Ide>,
        instance: &mut Instance,
        ui: &mut Ui,
    ) {
        if let Some(tool) = self.tools.get_mut(&id) {
            if let Some(tool) = &mut tool.tool {
                tool.show(project, ide, instance, ui);
            } else {
                ui.horizontal_centered(|ui| {
                    ui.label(format!("Tool {} is not loaded", tool.name));
                });
            }
        } else {
            ui.horizontal_centered(|ui| {
                ui.label(format!("Tool {} is not found", id));
            });
        }
    }

    pub fn title(&self, id: ToolId) -> String {
        if let Some(tool) = self.tools.get(&id) {
            tool.title()
        } else {
            format!("Tool {}", id)
        }
    }
}
