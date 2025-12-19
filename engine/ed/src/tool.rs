use arcana::{
    Ident, Name, id::{TimeUidGen, make_uid}, model::Value
};
use egui::Ui;
use hashbrown::HashMap;

use crate::{ide::Ide, instance::Instance, plugins::Plugins, project::Project};

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

    /// Updates the tool state with new [`Plugins`] instance.
    fn update_plugins(&mut self, project: &mut Project, plugins: &Plugins) {
        let _ = (project, plugins);
    }
}

struct BoxedTool {
    plugin: Name,
    name: Name,

    last_state: Value,
    tool: Option<Box<dyn Tool>>,
}

impl BoxedTool {
    fn new(plugin: Name, name: Name, tool: Option<Box<dyn Tool>>) -> Self {
        BoxedTool {
            plugin,
            name,
            last_state: Value::Unit,
            tool,
        }
    }

    fn update_plugins(&mut self, project: &mut Project, plugins: &Plugins) {
        // This is a tool from plugin. Reload it.
        self.save();
        self.tool = None;

        if let Some(plugin) = plugins.get(self.plugin) {
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
    plugins: Plugins,
}

impl Toolbox {
    pub fn new() -> Self {
        Toolbox {
            tools: HashMap::new(),
            idgen: TimeUidGen::random(),
            plugins: Plugins::default(),
        }
    }

    pub fn update_plugins(&mut self, project: &mut Project, plugins: &Plugins) {
        self.plugins = plugins.clone();
        for tool in self.tools.values_mut() {
            tool.update_plugins(project, plugins);
        }
    }

    pub fn tick(&mut self, project: &mut Project, instance: &mut Instance) {
        for tool in self.tools.values_mut() {
            tool.tick(project, instance);
        }
    }

    pub fn enumerate(&self) -> impl Iterator<Item = (Name, Name)> + '_ {
        [].into_iter()
    }

    pub fn add(&mut self, plugin: Name, name: Name) -> ToolId {
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
