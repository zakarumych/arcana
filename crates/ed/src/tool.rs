use arcana::{Ident, World};
use hashbrown::HashMap;

use crate::{container::Container, data::ProjectData};

pub trait Tool {
    fn show(&mut self, data: &mut ProjectData, world: &mut World) -> bool;

    fn save(&self) -> serde_json::Value;

    fn load(&mut self, state: serde_json::Value);
}

arcana::make_id! {
    /// ID of registered tool.
    pub ToolId;
}

struct BoxedTool {
    plugin: Ident,
    name: Ident,
    tool: Box<dyn Tool>,
}

pub struct Toolbox {
    tools: HashMap<ToolId, BoxedTool>,
    container: Option<Container>,
}

impl Toolbox {
    pub fn new() -> Self {
        Toolbox {
            tools: HashMap::new(),
            container: None,
        }
    }

    pub fn set_container(&mut self, container: Container) {
        self.tools.clear();
        self.container = Some(container);
    }
}
