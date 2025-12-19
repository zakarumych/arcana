pub use ::edict::*;
use arcana_id::make_uid;
pub mod flow;

make_uid! {
    /// ID of the system
    pub SystemId;
}

pub mod component {
    pub use edict::component::*;

    pub use arcana_proc::{Component, Relation};
}
