use arcana::{
    ecs::world::World,
    id::{make_id, SeqIdGen},
    model::Value,
    project::Project,
    Ident,
};
use egui::Ui;
use hashbrown::HashMap;

use crate::{ide::Ide, instance::Instance};

use super::{container::Container, project::ProjectData};

make_id! {
    /// ID of opened tool.
    pub ToolId;
}

pub enum ShowResult {
    DoNothing,
}

pub trait Tool {
    fn id(&self) -> ToolId;

    fn tick(&mut self, project: &mut Project, data: &mut ProjectData);

    fn show(
        &mut self,
        data: &mut ProjectData,
        ide: Option<&dyn Ide>,
        instance: &mut Instance,
        ui: &mut Ui,
    ) -> ShowResult;

    fn save(&self) -> Value;

    fn load(&mut self, state: &Value);

    // Only Plugins tool can create new containers.
    #[inline]
    #[doc(hidden)]
    fn new_container(&self) -> Option<Container> {
        None
    }
}

struct BoxedTool {
    plugin: Option<Ident>,
    name: Ident,

    last_state: Value,
    tool: Option<Box<dyn Tool>>,
}

impl BoxedTool {
    fn update_container(&mut self, container: &Container) {
        let Some(plugin_name) = self.plugin else {
            return;
        };

        // This is a tool from plugin. Reload it.
        self.save();
        self.tool = None;

        // if let Some(plugin) = container.get_plugin(plugin_name) {
        //     plugin.tools
        // }
    }

    fn save(&mut self) {
        if let Some(tool) = &self.tool {
            self.last_state = tool.save();
        }
    }
}

pub struct Toolbox {
    tools: HashMap<ToolId, BoxedTool>,
    idgen: SeqIdGen,
    container: Container,
}

impl Toolbox {
    pub fn new() -> Self {
        Toolbox {
            tools: HashMap::new(),
            idgen: SeqIdGen::new(),
            container: Container::default(),
        }
    }

    pub fn update_container(&mut self, container: Container) {
        self.container = container;

        for tool in self.tools.values_mut() {
            tool.update_container(&self.container);
        }
    }

    pub fn tick(&mut self, project: &mut Project, data: &mut ProjectData) {
        for tool in self.tools.values_mut() {
            if let Some(tool) = &mut tool.tool {
                tool.tick(project, data);
            }
        }
    }

    pub fn show(
        &mut self,
        id: ToolId,
        data: &mut ProjectData,
        ide: Option<&dyn Ide>,
        instance: &mut Instance,
        ui: &mut Ui,
    ) -> ShowResult {
        if let Some(tool) = self.tools.get_mut(&id) {
            if let Some(tool) = &mut tool.tool {
                return tool.show(data, ide, instance, ui);
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

        ShowResult::DoNothing
    }
}
