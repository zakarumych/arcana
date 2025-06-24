use edict::{entity::EntityId, flow::FlowWorld, query::Entities};
use hashbrown::HashMap;
use smallvec::SmallVec;

use crate::{
    codex::{Codex, CodexId, CodexValues, InputId},
    component::CodexComponent,
    execute::execute,
};

struct CodexContainer {
    executions: SmallVec<[(EntityId, CodexId, InputId, CodexValues); 32]>,
    codexes: HashMap<CodexId, Codex>,
}

impl CodexContainer {
    pub fn new() -> Self {
        CodexContainer {
            executions: SmallVec::new(),
            codexes: HashMap::new(),
        }
    }

    pub fn add_codex(&mut self, codex: Codex) -> CodexId {
        let id = codex.id();
        self.codexes.insert(id, codex);
        id
    }

    pub fn get_codex(&self, id: &CodexId) -> Option<&Codex> {
        self.codexes.get(id)
    }

    pub fn remove_codex(&mut self, id: &CodexId) -> Option<Codex> {
        self.codexes.remove(id)
    }

    pub fn execute(&mut self, world: FlowWorld) {
        world.map(|world| {
            for (entity, codex_component) in world.view::<(Entities, &mut CodexComponent)>() {
                let Some(codex) = self.codexes.get(&codex_component.id()) else {
                    tracing::error!(
                        "Entity {} is associated with an unknown codex {}",
                        entity.id(),
                        codex_component.id()
                    );
                    continue;
                };
                codex_component.drain_executions(entity.id(), codex, &mut self.executions);
            }
        });

        for (entity, id, input, values) in self.executions.drain(..) {
            let Ok(entity) = world.entity(entity) else {
                continue;
            };

            let Some(codex) = self.codexes.get(&id) else {
                continue;
            };

            execute(codex, input, entity, values);
        }
    }
}
