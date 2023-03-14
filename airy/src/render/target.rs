use std::sync::Arc;

use edict::{Component, Relation, World};

pub enum RenderAccess {
    Read,
    Write,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct TargetId(u64);

/// Render target component.
#[derive(Component)]
pub struct RenderTarget {
    id: u64,
    name: Arc<str>,
    version: u32,
    access: Option<RenderAccess>,
}

impl RenderTarget {
    pub fn new(name: Arc<str>, world: &World) -> Self {
        let id = world.expect_resource_mut::<RenderTargetCounter>().next();
        RenderTarget {
            id,
            name,
            version: 1,
            access: None,
        }
    }

    pub fn id(&self) -> TargetId {
        TargetId(self.id)
    }

    pub fn read(&mut self) -> bool {
        match self.access {
            Some(RenderAccess::Write) => false,
            _ => {
                self.access = Some(RenderAccess::Read);
                true
            }
        }
    }

    pub fn write(&mut self) -> Option<RenderTarget> {
        match self.access {
            Some(_) => None,
            None => {
                self.access = Some(RenderAccess::Write);
                Some(RenderTarget {
                    id: self.id,
                    name: self.name.clone(),
                    version: self.version + 1,
                    access: None,
                })
            }
        }
    }
}

/// Component that indicates that the render target needs to be updated.
/// Indication is done by touching this component mutably.
#[derive(Component)]
pub struct RenderTargetUpdate;

/// Component that indicates that the render target needs to be updated
/// every frame.
#[derive(Component)]
pub struct RenderTargetAlwaysUpdate;

/// RenderTarget -> RenderNode relation.
#[derive(Clone, Copy)]
pub struct TargetFor;

impl Relation for TargetFor {
    const EXCLUSIVE: bool = true;
    const SYMMETRIC: bool = false;
    const OWNED: bool = false;
}

#[derive(Default)]
pub(crate) struct RenderTargetCounter(u64);

impl RenderTargetCounter {
    pub fn new() -> Self {
        RenderTargetCounter(0)
    }

    pub fn next(&mut self) -> u64 {
        self.0 += 1;
        self.0
    }
}
