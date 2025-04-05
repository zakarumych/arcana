use arcana_intern::Ident;
use edict::world::World;
use hashbrown::HashMap;

use crate::{id::SeqIdGen, plugin};

use super::{container::Container, project::ProjectData};

pub trait Tool {
    fn id(&self) -> ToolId;

    fn show(&mut self, data: &mut ProjectData, world: &mut World) -> bool;

    fn save(&self) -> serde_json::Value;

    fn load(&mut self, state: &serde_json::Value);
}

arcana::make_id! {
    /// ID of opened tool.
    pub ToolId;
}

struct BoxedTool {
    plugin: Option<Ident>,
    name: Ident,

    last_state: serde_json::Value,
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

        if let Some(plugin) = container.get_plugin(plugin_name) {
            plugin.tools
        }
    }

    fn save(&self) {
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
    }
}
