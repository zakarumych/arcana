//! This module provides a way to build renders using closures.
//! Without knowing details of rendering system implementation.

use std::num::NonZeroU64;

use blink_alloc::BlinkAlloc;
use edict::World;
use hashbrown::HashSet;

use super::{target::RenderTarget, RenderContext, RenderError, RenderGraph, RenderNode};

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct TargetId(pub NonZeroU64, pub usize);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct RenderId(NonZeroU64);

pub trait Render: Send + 'static {
    fn render(
        &mut self,
        ctx: RenderContext<'_, '_>,
        world: &World,
        blink: &BlinkAlloc,
    ) -> Result<(), RenderError>;
}

impl<F> Render for F
where
    F: for<'a, 'b> FnMut(
            RenderContext<'a, 'b>,
            &'a World,
            &'a BlinkAlloc,
        ) -> Result<(), RenderError>
        + Send
        + 'static,
{
    fn render(
        &mut self,
        ctx: RenderContext<'_, '_>,
        world: &World,
        blink: &BlinkAlloc,
    ) -> Result<(), RenderError> {
        self(ctx, world, blink)
    }
}

/// Render building context provided to rendering closures.
#[must_use]
pub struct RenderBuilderContext<'a> {
    name: Box<str>,
    id: NonZeroU64,
    depends_on: HashSet<TargetId>,
    renders_to: HashSet<TargetId>,
    graph: &'a mut RenderGraph,
}

impl<'a> RenderBuilderContext<'a> {
    #[must_use]
    pub fn new(name: impl Into<String>, graph: &'a mut RenderGraph) -> Self {
        let id = graph.new_id();
        RenderBuilderContext {
            name: name.into().into_boxed_str(),
            id,
            depends_on: HashSet::new(),
            renders_to: HashSet::new(),
            graph,
        }
    }

    pub fn build<R>(self, render: R)
    where
        R: Render,
    {
        let Self {
            id,
            name,
            depends_on,
            renders_to,
            graph,
        } = self;

        graph.renders.insert(
            RenderId(id),
            RenderNode {
                name,
                render: Box::new(render),
                depends_on,
                renders_to,
            },
        );
    }

    /// Creates new render target.
    /// Returns id of created target.
    ///
    /// Built render will be target's producer,
    /// whenever target is to be updated, the render will be invoked.
    pub fn create_target(&mut self, name: &str, stages: nix::PipelineStages) -> TargetId {
        let target = RenderTarget::new(name.into(), RenderId(self.id), stages);
        let target_id = TargetId(self.graph.new_id(), 0);
        self.renders_to.insert(target_id);
        self.graph.targets.insert(target_id.0, target);

        target_id
    }

    /// Declares that render writes to the target.
    ///
    /// Built render will statically depend on previous target version,
    /// always invoking target's render before itself.
    ///
    /// Built render will be new target's render,
    /// whenever new target is to be updated, the render will be invoked.
    pub fn write_target(&mut self, target: TargetId, stages: nix::PipelineStages) -> TargetId {
        self.graph
            .targets
            .get_mut(&target.0)
            .unwrap()
            .write(target.1, RenderId(self.id), stages);
        self.depends_on.insert(target);
        let new_target = TargetId(target.0, target.1 + 1);
        self.renders_to.insert(new_target);
        new_target
    }

    /// Declares that render reads from the target.
    ///
    /// Built render will statically depend on target,
    /// always invoking target's render before itself.
    pub fn read_target<T>(&mut self, target: TargetId, stages: nix::PipelineStages) {
        self.graph
            .targets
            .get_mut(&target.0)
            .unwrap()
            .read(target.1, stages);
        self.depends_on.insert(target);
    }
}
