//! This module provides a way to build renders using closures.
//! Without knowing details of rendering system implementation.

use std::{
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    num::NonZeroU64,
};

use blink_alloc::BlinkAlloc;
use edict::World;

use super::{
    RenderContext, RenderError, RenderGraph, RenderNode, RenderNodeEdges, RenderTargetType,
};

pub struct TargetId<T: ?Sized>(
    pub(super) NonZeroU64,
    pub(super) usize,
    pub(super) PhantomData<fn() -> T>,
);

impl<T: ?Sized> Clone for TargetId<T> {
    #[cfg_attr(inline_more, inline(always))]
    fn clone(&self) -> Self {
        *self
    }

    #[cfg_attr(inline_more, inline(always))]
    fn clone_from(&mut self, source: &Self) {
        *self = *source;
    }
}

impl<T: ?Sized> Copy for TargetId<T> {}

impl fmt::Debug for TargetId<mev::Image> {
    #[cfg_attr(inline_more, inline(always))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TargetId<mev::Image>")
            .field(&self.0)
            .field(&self.1)
            .finish()
    }
}

impl fmt::Debug for TargetId<mev::Buffer> {
    #[cfg_attr(inline_more, inline(always))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TargetId<mev::Buffer>")
            .field(&self.0)
            .field(&self.1)
            .finish()
    }
}

impl<T: ?Sized> PartialEq for TargetId<T> {
    #[cfg_attr(inline_more, inline(always))]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1
    }

    #[cfg_attr(inline_more, inline(always))]
    fn ne(&self, other: &Self) -> bool {
        self.0 != other.0 || self.1 != other.1
    }
}

impl<T: ?Sized> Eq for TargetId<T> {}

impl<T: ?Sized> Hash for TargetId<T> {
    #[cfg_attr(inline_more, inline(always))]
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.get());
        state.write_usize(self.1);
    }

    #[cfg_attr(inline_more, inline(always))]
    fn hash_slice<H: Hasher>(data: &[Self], state: &mut H) {
        for item in data {
            state.write_u64(item.0.get());
            state.write_usize(item.1);
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct RenderId(NonZeroU64);

pub trait Render: Send + 'static {
    fn render(&mut self, world: &World, ctx: RenderContext<'_, '_>) -> Result<(), RenderError>;
}

impl<F> Render for F
where
    F: for<'a, 'b> FnMut(&World, RenderContext<'a, 'b>) -> Result<(), RenderError> + Send + 'static,
{
    fn render(&mut self, world: &World, ctx: RenderContext<'_, '_>) -> Result<(), RenderError> {
        (self)(world, ctx)
    }
}

/// Render building context provided to rendering closures.
#[must_use]
pub struct RenderBuilderContext<'a> {
    name: Box<str>,
    id: NonZeroU64,
    edges: RenderNodeEdges,
    graph: &'a mut RenderGraph,
}

impl<'a> RenderBuilderContext<'a> {
    /// Creates new render building context.
    /// Use it to build new render node and add it to the graph.
    #[must_use]
    pub fn new(name: impl Into<String>, graph: &'a mut RenderGraph) -> Self {
        let id = graph.new_id();
        RenderBuilderContext {
            name: name.into().into_boxed_str(),
            id,
            edges: RenderNodeEdges::new(),
            graph,
        }
    }

    /// Builds render node and adds it to the graph.
    pub fn build<R>(self, render: R)
    where
        R: Render,
    {
        let Self {
            id,
            name,
            edges,
            graph,
        } = self;

        graph.renders.insert(
            RenderId(id),
            RenderNode {
                name,
                render: Box::new(render),
                edges,
            },
        );
    }

    /// Creates new render target in the graph.
    /// Specifies at which stages render will write to the target.
    pub fn create_target<T>(&mut self, name: &str, stages: mev::PipelineStages) -> TargetId<T>
    where
        T: RenderTargetType,
    {
        let target = self
            .graph
            .add_target::<T>(name.into(), RenderId(self.id), stages);
        self.edges.add_renders_to(target);
        target
    }

    /// Updates existing render target in the graph.
    /// Creates new version of the target and returns its id.
    /// Specifies at which stages render will write to the target.
    pub fn write_target<T>(
        &mut self,
        target: TargetId<T>,
        stages: mev::PipelineStages,
    ) -> TargetId<T>
    where
        T: RenderTargetType,
    {
        self.graph
            .get_target_mut::<T>(target.0)
            .write(target.1, RenderId(self.id), stages);
        self.edges.add_dependency(target);
        let new_target = TargetId(target.0, target.1 + 1, PhantomData);
        self.edges.add_renders_to(new_target);
        new_target
    }

    /// Updates existing render target in the graph.
    /// Specifies at which stages render will read from the target.
    pub fn read_target<T>(&mut self, target: TargetId<T>, stages: mev::PipelineStages)
    where
        T: RenderTargetType,
    {
        self.graph
            .get_target_mut::<T>(target.0)
            .read(target.1, stages);
        self.edges.add_dependency(target);
    }
}
