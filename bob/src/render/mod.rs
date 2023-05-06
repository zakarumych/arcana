//! Defines rendering for the Airy engine.

mod build;
mod target;

use std::{collections::VecDeque, fmt, ops::Range};

use blink_alloc::BlinkAlloc;
use edict::{
    epoch::EpochId,
    query::{Or2, With},
    Component, Entities, EntityId, Modified, State, World,
};
use hashbrown::{hash_map::DefaultHashBuilder, HashMap, HashSet};

use self::target::TargetId;

pub(crate) use self::target::RenderTargetCounter;

pub use self::{
    build::{Render, RenderBuilderContext},
    target::{
        NextVersionOf, RenderTarget, RenderTargetAlwaysUpdate, RenderTargetUpdate, TargetFor,
    },
};

type BlinkHashMap<'a, K, V> = HashMap<K, V, DefaultHashBuilder, &'a BlinkAlloc>;

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

    // Maps render target ids to image resources.
    images: &'a mut BlinkHashMap<'b, TargetId, nix::Image>,

    // Maps render target to pipeline stages that need to be waited for.
    write_barriers: &'a mut BlinkHashMap<'b, EntityId, Range<nix::PipelineStages>>,
    read_barriers: &'a mut BlinkHashMap<'b, EntityId, Range<nix::PipelineStages>>,

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

    /// Returns render target image and pipeline stages that need to be waited for.
    pub fn write_target_sync(
        &mut self,
        id: EntityId,
    ) -> (&nix::Image, Option<Range<nix::PipelineStages>>) {
        let tid = self
            .world
            .for_one::<&RenderTarget, _, _>(id, |target| target.id())
            .unwrap();

        let image = &self.images[&tid];
        let barrier = self.write_barriers.remove(&id);
        (image, barrier)
    }

    /// Returns render target image and pipeline stages that need to be waited for.
    pub fn read_target_sync(
        &mut self,
        id: EntityId,
    ) -> (&nix::Image, Option<Range<nix::PipelineStages>>) {
        let tid = self
            .world
            .for_one::<&RenderTarget, _, _>(id, |target| target.id())
            .unwrap();

        let image = &self.images[&tid];
        let barrier = self.read_barriers.remove(&id);
        (image, barrier)
    }

    /// Returns render target image
    /// and inserts pipeline barrier if needed.
    pub fn write_target(&mut self, id: EntityId, encoder: &mut nix::CommandEncoder) -> &nix::Image {
        let (image, barrier) = self.write_target_sync(id);
        if let Some(barrier) = barrier {
            if barrier.start.is_empty() {
                encoder.init_image(barrier.start, barrier.end, image);
            } else {
                encoder.barrier(barrier.start, barrier.end);
            }
        }
        image
    }

    /// Returns render target image
    /// and inserts pipeline barrier if needed.
    pub fn read_target(&mut self, id: EntityId, encoder: &mut nix::CommandEncoder) -> &nix::Image {
        let (image, barrier) = self.read_target_sync(id);
        if let Some(barrier) = barrier {
            encoder.barrier(barrier.start, barrier.end);
        }
        image
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
        images: &'a mut BlinkHashMap<'b, TargetId, nix::Image>,
        write_barriers: &'a mut BlinkHashMap<'b, EntityId, Range<nix::PipelineStages>>,
        read_barriers: &'a mut BlinkHashMap<'b, EntityId, Range<nix::PipelineStages>>,
        cbufs: &'a mut Vec<nix::CommandBuffer, &'b BlinkAlloc>,
        blink: &'b BlinkAlloc,
    ) -> Result<(), RenderError> {
        self.render.render(
            RenderContext {
                device,
                queue,
                images,
                write_barriers,
                read_barriers,
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

/// Render system.
/// Traverses render-targets that needs to be updated and collects all
/// render nodes that needs to run.
pub fn render_system(world: &World, mut state: State<RenderState>) {
    let state = &mut *state;

    let device = world.expect_resource_mut::<nix::Device>();
    let mut queue = world.expect_resource_mut::<nix::Queue>();

    // Maps render target ids to image resources.
    let mut images = HashMap::new_in(&state.blink);

    // Maps render target entity to access stages.
    let mut write_barriers = HashMap::new_in(&state.blink);
    let mut read_barriers = HashMap::new_in(&state.blink);

    let mut drop_surfaces = Vec::new_in(&state.blink);
    let mut frames = Vec::new_in(&state.blink);

    // Collect all targets that needs to be updated.
    // If target is bound to surface, fetch next frame.
    let mut render_targets_to_update = Vec::new_in(&state.blink);
    world
        .query::<(Entities, &RenderTarget, Option<&mut nix::Surface>)>()
        .filter(Or2::new(
            <With<RenderTargetAlwaysUpdate>>::query(),
            <Modified<With<RenderTargetUpdate>>>::new(state.last_epoch),
        ))
        .for_each(|(tid, rt, surface_opt)| {
            if let Some(surface) = surface_opt {
                let mut prev_tid = tid;
                while let Ok((NextVersionOf, parent)) = world
                    .new_query()
                    .relates_exclusive::<&NextVersionOf>()
                    .get_one(prev_tid)
                {
                    prev_tid = parent;
                }

                let writes = world
                    .query::<&RenderTarget>()
                    .get_one(prev_tid)
                    .unwrap()
                    .writes();

                match surface.next_frame(&mut *queue, writes) {
                    Err(err) => {
                        tracing::error!(err = ?err);
                        drop_surfaces.push(tid);
                        return;
                    }
                    Ok(frame) => {
                        let image = frame.image();
                        images.insert(rt.id(), image.clone());
                        frames.push((frame, rt.writes() | rt.reads()));
                    }
                }
            }

            // All ancestors of the render target needs to be updated.
            render_targets_to_update.push(tid);
        });

    // Save the epoch.
    state.last_epoch = world.epoch();

    // Find all renders to activate.
    let mut target_for_query = world.new_query().relates_exclusive::<&TargetFor>();
    let mut activate_renders = HashSet::new_in(&state.blink);
    let mut render_queue = VecDeque::new_in(&state.blink);
    let mut render_query = world.query::<&mut RenderComponent>();
    let mut next_version_query = world.new_query().relates_exclusive::<&NextVersionOf>();
    let mut render_target_query = world.query::<&RenderTarget>();

    // For all targets that needs to be updated.
    while let Some(tid) = render_targets_to_update.pop() {
        let rt = render_target_query.get_one(tid).unwrap();

        write_barriers.insert(tid, rt.waits()..rt.writes());
        if !rt.reads().is_empty() {
            read_barriers.insert(tid, rt.writes()..rt.reads());
        }

        // Activate render node attached to the target.
        if let Ok((TargetFor, rid)) = target_for_query.get_one(tid) {
            // Mark as activated.
            if activate_renders.insert(rid) {
                // Push to queue.
                render_queue.push_back(rid);

                // Update dependencies.
                let render = render_query.get_one(rid).unwrap();
                render_targets_to_update.extend(render.depends_on());
            }
        }
    }

    // Build render schedule from roots to leaves.
    let mut render_schedule = Vec::new_in(&state.blink);
    let mut target_scheduled = HashSet::new_in(&state.blink);

    let mut targets_of_query = world.new_query().related::<TargetFor>();

    // Quadratic algorithm, but it's ok for now.
    while let Some(rid) = render_queue.pop_front() {
        let render = render_query.get_one(rid).unwrap();

        let ready = render
            .depends_on()
            .all(|tid| target_scheduled.contains(&tid));

        if ready {
            // Scheduled the render.
            debug_assert!(!render_schedule.contains(&rid), "Render already scheduled");
            render_schedule.push(rid);

            if let Ok(targets) = targets_of_query.get_one(rid) {
                for &tid in targets {
                    let inserted = target_scheduled.insert(tid);
                    debug_assert!(inserted, "Target already scheduled");
                }
            }
        } else {
            // Push back to queue.
            render_queue.push_back(rid);
        }
    }

    let mut cbufs = Vec::new_in(&state.blink);

    // Walk render schedule and run renders in opposite order.
    while let Some(rid) = render_schedule.pop() {
        let render = render_query.get_one(rid).unwrap();

        let cbufs_pre = cbufs.len();
        let result = render.run(
            &device,
            &mut queue,
            world,
            &mut images,
            &mut write_barriers,
            &mut read_barriers,
            &mut cbufs,
            &state.blink,
        );

        match result {
            Ok(()) => {
                let cbufs_post = cbufs.len();
                cbufs[cbufs_pre..cbufs_post].reverse();
            }
            Err(err) => {
                tracing::event!(tracing::Level::ERROR, err = ?err);
            }
        }
    }

    cbufs.reverse();
    queue.submit(cbufs, false).unwrap();

    let mut encoder = queue.new_command_encoder().unwrap();

    for (frame, after) in frames {
        encoder.present(frame, after);
    }

    queue.submit(Some(encoder.finish().unwrap()), true).unwrap();
}
