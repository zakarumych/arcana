use arcana_project::IdentBuf;
use edict::System;
use hashbrown::HashMap;

pub struct Systems {
    /// Systems that are registered by plugins.
    pub systems: HashMap<(IdentBuf, IdentBuf), Box<dyn System>>,
}
