//! Defines rendering for the Airy engine.

mod build;
mod target;

use std::fmt;

use blink_alloc::BlinkAlloc;
use edict::{
    epoch::EpochId,
    query::{Or2, With},
    Component, Entities, EntityId, Modified, State, World,
};
use hashbrown::{hash_map::DefaultHashBuilder, HashMap, HashSet};

pub(crate) use self::target::RenderTargetCounter;
use self::target::TargetId;
pub use self::{
    build::{Render, RenderBuilderContext},
    target::{RenderTarget, RenderTargetAlwaysUpdate, RenderTargetUpdate, TargetFor},
};

#[derive(Debug)]
pub enum RenderError {
    OutOfMemory(nix::OutOfMemory),
}

impl From<nix::OutOfMemory> for RenderError {
    #[inline(always)]
    fn from(err: nix::OutOfMemory) -> Self {
        RenderError::OutOfMemory(err)
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::OutOfMemory(err) => fmt::Display::fmt(err, f),
        }
    }
}

pub struct RenderContext<'a, 'b> {
    device: &'a nix::Device,
    queue: &'a mut nix::Queue,
    images: &'a mut HashMap<TargetId, nix::Image, DefaultHashBuilder, &'b BlinkAlloc>,
    cbufs: &'a mut Vec<nix::CommandBuffer, &'b BlinkAlloc>,
    world: &'a World,
}

impl<'a> RenderContext<'a, '_> {
    pub fn device(&self) -> &nix::Device {
        self.device
    }

    pub fn new_command_encoder(&mut self) -> Result<nix::CommandEncoder, RenderError> {
        self.queue
            .new_command_encoder()
            .map_err(RenderError::OutOfMemory)
    }

    pub fn commit(&mut self, cbuf: nix::CommandBuffer) {
        self.cbufs.push(cbuf);
    }

    pub fn target(&self, id: EntityId) -> &nix::Image {
        let id = self
            .world
            .for_one::<&RenderTarget, _, _>(id, |target| target.id())
            .unwrap();
        self.images.get(&id).unwrap()
    }
}

#[derive(Component)]
struct RenderComponent {
    name: Box<str>,
    render: Box<dyn Render>,
    depends_on: HashSet<EntityId>,
}

impl RenderComponent {
    fn name(&self) -> &str {
        &self.name
    }

    fn run<'a, 'b>(
        &mut self,
        device: &'a nix::Device,
        queue: &'a mut nix::Queue,
        world: &'a World,
        images: &'a mut HashMap<TargetId, nix::Image, DefaultHashBuilder, &'b BlinkAlloc>,
        cbufs: &'a mut Vec<nix::CommandBuffer, &'b BlinkAlloc>,
        blink: &'b BlinkAlloc,
    ) -> Result<(), RenderError> {
        self.render.render(
            RenderContext {
                device,
                queue,
                images,
                cbufs,
                world,
            },
            world,
            blink,
        )
    }

    fn depends_on(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.depends_on.iter().copied()
    }
}

#[derive(Default)]
pub struct RenderState {
    last_epoch: EpochId,
    blink: BlinkAlloc,
}

enum RenderQueueItem<'b> {
    Render(EntityId),
    Commands(Vec<nix::CommandBuffer, &'b BlinkAlloc>),
}

/// Render system.
/// Traverses render-targets that needs to be updated and collects all
/// render nodes that needs to run.
pub fn render_system(world: &World, mut state: State<RenderState>) {
    let state = &mut *state;

    let device = world.expect_resource_mut::<nix::Device>();
    let mut queue = world.expect_resource_mut::<nix::Queue>();
    let mut images = HashMap::new_in(&state.blink);
    let mut drop_surfaces = Vec::new_in(&state.blink);
    let mut surface_images = Vec::new_in(&state.blink);

    let mut render_targets_to_update = Vec::new_in(&state.blink);
    world
        .query::<(Entities, &mut RenderTarget, Option<&mut nix::Surface>)>()
        .filter(Or2::new(
            <With<RenderTargetAlwaysUpdate>>::query(),
            <Modified<With<RenderTargetUpdate>>>::new(state.last_epoch),
        ))
        .for_each(|(eid, target, surface_opt)| {
            if let Some(surface) = surface_opt {
                match surface.next_frame(&mut *queue) {
                    Err(err) => {
                        tracing::error!(err = ?err);
                        drop_surfaces.push(eid);
                        return;
                    }
                    Ok(surface_image) => {
                        images.insert(target.id(), surface_image.image().clone());
                        surface_images.push((eid, surface_image));
                    }
                }
            }
            render_targets_to_update.push(eid);
        });
    state.last_epoch = world.epoch();

    let mut render_queue = Vec::new_in(&state.blink);
    let mut visited = HashSet::new_in(&state.blink);

    let mut target_for_query = world.new_query().relates_exclusive::<&TargetFor>();
    let mut render_query = world.query::<&mut RenderComponent>();

    for target in render_targets_to_update {
        if let Ok((TargetFor, render)) = target_for_query.get_one(target) {
            render_queue.push(RenderQueueItem::Render(render));
        }
    }

    while let Some(item) = render_queue.pop() {
        match item {
            RenderQueueItem::Render(render) => {
                if !visited.insert(render) {
                    continue;
                }

                let render = render_query.get_one(render).unwrap();
                let mut cbufs = Vec::new_in(&state.blink);
                let result = render.run(
                    &device,
                    &mut queue,
                    world,
                    &mut images,
                    &mut cbufs,
                    &state.blink,
                );

                match result {
                    Ok(()) => {
                        render_queue.push(RenderQueueItem::Commands(cbufs));
                        for target in render.depends_on() {
                            if let Ok((TargetFor, render)) = target_for_query.get_one(target) {
                                render_queue.push(RenderQueueItem::Render(render));
                            }
                        }
                    }
                    Err(err) => {
                        tracing::event!(tracing::Level::ERROR, err = ?err);
                    }
                }
            }
            RenderQueueItem::Commands(cbufs) => {
                queue.submit(cbufs).unwrap();
            }
        }
    }

    let mut encoder = queue.new_command_encoder().unwrap();

    for surface_image in surface_images {
        encoder.present(surface_image.1);
    }

    queue.submit(Some(encoder.finish().unwrap())).unwrap();
    queue.check_point();
}
