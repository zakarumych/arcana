use edict::{entity::EntityId, flow::FlowWorld, query::Entities};
use hashbrown::HashMap;
use smallvec::SmallVec;

use crate::{
    codex::{Codex, CodexId, CodexValues, InputId},
    component::CodexComponent,
    execute::execute,
};

pub struct CodexContext {
    /// Map of ID to Codex.
    map: HashMap<CodexId, Codex>,

    /// List of executions to perform.
    executions: SmallVec<[(EntityId, CodexId, InputId, CodexValues); 32]>,
}

impl CodexContext {
    pub fn new() -> Self {
        CodexContext {
            executions: SmallVec::new(),
            map: HashMap::new(),
        }
    }

    /// Adds a new codex to the context and returns its ID.
    pub fn add_codex(&mut self, id: CodexId, codex: Codex) -> CodexId {
        self.map.insert(id, codex);
        id
    }

    /// Adds a new codex to the context with a specific ID.
    pub fn get_codex(&self, id: &CodexId) -> Option<&Codex> {
        self.map.get(id)
    }

    pub fn remove_codex(&mut self, id: &CodexId) -> Option<Codex> {
        self.map.remove(id)
    }

    pub fn run(&mut self, world: FlowWorld) {
        world.map(|world| {
            for (entity, codex_component) in world.view::<(Entities, &mut CodexComponent)>() {
                let Some(codex) = self.map.get(&codex_component.id()) else {
                    tracing::error!(
                        "Entity {} is associated with an unknown codex {}",
                        entity.id(),
                        codex_component.id()
                    );
                    continue;
                };
                codex_component.drain_executions(
                    entity.id(),
                    codex_component.id(),
                    codex,
                    &mut self.executions,
                );
            }
        });

        for (entity, id, input, values) in self.executions.drain(..) {
            let Ok(entity) = world.entity(entity) else {
                continue;
            };

            let Some(codex) = self.map.get(&id) else {
                continue;
            };

            execute(codex, input, entity, values);
        }
    }
}
