//! This module provides a way to build renders using closures.
//! Without knowing details of rendering system implementation.

use std::sync::Arc;

use blink_alloc::BlinkAlloc;
use edict::{ActionEncoder, EntityId, QueryOneError, World};
use hashbrown::HashSet;

use super::{RenderComponent, RenderContext, RenderError, RenderTarget, TargetFor};

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
    id: EntityId,
    actions: WorldOrEncoder<'a>,
    depends_on: HashSet<EntityId>,
}

impl<'a> RenderBuilderContext<'a> {
    #[must_use]
    pub fn new(name: impl Into<String>, world: &'a mut World) -> Self {
        let id = world.allocate();
        RenderBuilderContext {
            name: name.into().into_boxed_str(),
            id,
            actions: WorldOrEncoder::World(world),
            depends_on: HashSet::new(),
        }
    }

    #[must_use]
    pub fn with_encoder(name: impl Into<String>, encoder: &'a mut ActionEncoder<'a>) -> Self {
        let id = encoder.allocate();
        RenderBuilderContext {
            name: name.into().into_boxed_str(),
            id,
            actions: WorldOrEncoder::Encoder(encoder),
            depends_on: HashSet::new(),
        }
    }

    pub fn build<R>(self, render: R)
    where
        R: Render,
    {
        let Self {
            id,
            name,
            mut actions,
            depends_on,
        } = self;
        actions.closure(move |world| {
            let _ = world.insert(
                id,
                RenderComponent {
                    name,
                    render: Box::new(render),
                    depends_on,
                },
            );
        });
    }

    /// Creates new render target.
    /// Returns entity id of created target.
    ///
    /// Built render will be target's producer,
    /// whenever target is to be updated, the render will be invoked.
    pub fn create_target(&mut self, name: &str) -> EntityId {
        let target = self.actions.allocate();
        let name = Arc::<str>::from(name);
        let id = self.id;

        self.actions.closure(move |world: &mut World| {
            let render_target = RenderTarget::new(name, world);
            world.insert(target, render_target).unwrap();

            // Connect target to render
            world.add_relation(target, TargetFor, id).unwrap();
        });

        target
    }

    /// Writes to render target and produces new render target.
    /// Returns entity id of updated target.
    /// Old target become inaccessible.
    /// Old target must be not accessed by any other render.
    ///
    /// Built render will statically depend on old target,
    /// always invoking target's producer before itself.
    /// Built render will be new target's producer,
    /// whenever new target is to be updated, the render will be invoked.
    pub fn update_target(&mut self, target: EntityId) -> EntityId {
        self.depends_on.insert(target);
        let new_target = self.actions.allocate();
        let id = self.id;

        self.actions.closure(move |world: &mut World| {
            match world.query_one_mut::<&mut RenderTarget>(target) {
                Ok(render_target) => match render_target.write() {
                    None => {
                        tracing::error!("Render target {target} is already accessed");
                        let _ = world.despawn(new_target);
                        let _ = world.despawn(id);
                    }
                    Some(updated) => {
                        world.insert(new_target, updated).unwrap();
                        world.add_relation(new_target, TargetFor, id).unwrap();
                    }
                },
                Err(QueryOneError::NotSatisfied) => {
                    tracing::error!("{target} is not a render target");
                    let _ = world.despawn(id);
                }
                Err(QueryOneError::NoSuchEntity) => {
                    tracing::error!("Render target {target} was removed");
                    let _ = world.despawn(id);
                }
            }
        });

        new_target
    }

    /// Declares that render reads from the target.
    ///
    /// Built render will statically depend on target,
    /// always invoking target's producer before itself.
    pub fn read_target<T>(&mut self, target: EntityId) {
        self.depends_on.insert(target);
        let id = self.id;
        self.actions.closure(move |world: &mut World| {
            match world.query_one_mut::<&mut RenderTarget>(target) {
                Ok(render_target) => {
                    if !render_target.read() {
                        tracing::error!("Render target {target} is already written");
                        let _ = world.despawn(id);
                    }
                }
                Err(QueryOneError::NotSatisfied) => {
                    tracing::error!("{target} is not a render target");
                    let _ = world.despawn(id);
                }
                Err(QueryOneError::NoSuchEntity) => {
                    tracing::error!("Render target {target} was removed");
                    let _ = world.despawn(id);
                }
            }
        });
    }
}

enum WorldOrEncoder<'a> {
    World(&'a mut World),
    Encoder(&'a mut ActionEncoder<'a>),
}

impl WorldOrEncoder<'_> {
    fn closure(&mut self, closure: impl FnOnce(&mut World) + Send + 'static) {
        match self {
            WorldOrEncoder::World(world) => closure(world),
            WorldOrEncoder::Encoder(encoder) => encoder.closure(closure),
        }
    }

    fn allocate(&mut self) -> EntityId {
        match self {
            WorldOrEncoder::World(world) => world.allocate(),
            WorldOrEncoder::Encoder(encoder) => encoder.allocate(),
        }
    }
}
