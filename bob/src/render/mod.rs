//! Defines rendering for the Airy engine.

mod render;
mod target;

use std::{collections::VecDeque, fmt, num::NonZeroU64, ops::Range};

use blink_alloc::BlinkAlloc;
use edict::{State, World};
use hashbrown::{hash_map::DefaultHashBuilder, HashMap, HashSet};

use crate::window::Windows;

use self::{render::RenderId, target::RenderTarget};

pub use self::render::{Render, RenderBuilderContext, TargetId};

pub struct RenderGraph {
    renders: HashMap<RenderId, RenderNode>,
    targets: HashMap<NonZeroU64, RenderTarget>,

    next_id: u64,
}

impl RenderGraph {
    pub fn new() -> Self {
        RenderGraph {
            renders: HashMap::new(),
            targets: HashMap::new(),
            next_id: 1,
        }
    }

    fn new_id(&mut self) -> NonZeroU64 {
        let id = self.next_id;
        self.next_id += 1;
        NonZeroU64::new(id).unwrap()
    }
}

pub struct UpdateTargets {
    update: HashSet<TargetId>,
}

impl UpdateTargets {
    pub fn new() -> Self {
        UpdateTargets {
            update: HashSet::new(),
        }
    }
}

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
    images: &'a mut BlinkHashMap<'b, NonZeroU64, nix::Image>,

    // Maps render target to pipeline stages that need to be waited for.
    write_barriers: &'a mut BlinkHashMap<'b, TargetId, Range<nix::PipelineStages>>,
    read_barriers: &'a mut BlinkHashMap<'b, TargetId, Range<nix::PipelineStages>>,

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
        id: TargetId,
    ) -> (&nix::Image, Option<Range<nix::PipelineStages>>) {
        let image = &self.images[&id.0];
        let barrier = self.write_barriers.remove(&id);
        (image, barrier)
    }

    /// Returns render target image and pipeline stages that need to be waited for.
    pub fn read_target_sync(
        &mut self,
        id: TargetId,
    ) -> (&nix::Image, Option<Range<nix::PipelineStages>>) {
        let image = &self.images[&id.0];
        let barrier = self.read_barriers.remove(&id);
        (image, barrier)
    }

    /// Returns render target image
    /// and inserts pipeline barrier if needed.
    pub fn write_target(&mut self, id: TargetId, encoder: &mut nix::CommandEncoder) -> &nix::Image {
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
    pub fn read_target(&mut self, id: TargetId, encoder: &mut nix::CommandEncoder) -> &nix::Image {
        let (image, barrier) = self.read_target_sync(id);
        if let Some(barrier) = barrier {
            encoder.barrier(barrier.start, barrier.end);
        }
        image
    }
}

struct RenderNode {
    name: Box<str>,
    render: Box<dyn Render>,
    depends_on: HashSet<TargetId>,
    renders_to: HashSet<TargetId>,
}

impl RenderNode {
    fn name(&self) -> &str {
        &self.name
    }

    fn run<'a, 'b>(
        &mut self,
        device: &'a nix::Device,
        queue: &'a mut nix::Queue,
        world: &'a World,
        images: &'a mut BlinkHashMap<'b, NonZeroU64, nix::Image>,
        write_barriers: &'a mut BlinkHashMap<'b, TargetId, Range<nix::PipelineStages>>,
        read_barriers: &'a mut BlinkHashMap<'b, TargetId, Range<nix::PipelineStages>>,
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

    fn depends_on(&self) -> impl Iterator<Item = TargetId> + '_ {
        self.depends_on.iter().copied()
    }
}

#[derive(Default)]
pub struct RenderState {
    blink: BlinkAlloc,
}

/// Render system.
/// Traverses render-targets that needs to be updated and collects all
/// render nodes that needs to run.
pub fn render_system(world: &mut World, mut state: State<RenderState>) {
    let state = &mut *state;
    let world = world.local();

    let device = world.expect_resource::<nix::Device>();
    let mut queue = world.expect_resource_mut::<nix::Queue>();

    let mut graph = world.expect_resource_mut::<RenderGraph>();
    let mut update_targets = world.get_resource_mut::<UpdateTargets>();
    let mut windows = world.expect_resource_mut::<Windows>();

    // Collect all targets that needs to be updated.
    // If target is bound to surface, fetch next frame.
    let mut render_targets_to_update = Vec::new_in(&state.blink);

    if let Some(update_targets) = update_targets.as_deref_mut() {
        render_targets_to_update.extend(update_targets.update.drain());
    }

    // Maps render target ids to image resources.
    let mut images = HashMap::new_in(&state.blink);

    // Maps render target entity to access stages.
    let mut write_barriers = HashMap::new_in(&state.blink);
    let mut read_barriers = HashMap::new_in(&state.blink);

    let mut drop_surfaces = Vec::new_in(&state.blink);
    let mut frames = Vec::new_in(&state.blink);

    for window in windows.windows.iter_mut() {
        let tid = window.target();
        let rt = &graph.targets[&tid.0];

        match window.surface_mut().next_frame(&mut *queue, rt.writes(0)) {
            Err(err) => {
                tracing::error!(err = ?err);
                drop_surfaces.push(tid);
                return;
            }
            Ok(frame) => {
                let image = frame.image();
                images.insert(tid.0, image.clone());
                frames.push((frame, rt.writes(tid.1) | rt.reads(tid.1)));
            }
        }

        render_targets_to_update.push(tid);
    }
    drop(windows);

    // Find all renders to activate.
    let mut activate_renders = HashSet::new_in(&state.blink);
    let mut render_queue = VecDeque::new_in(&state.blink);

    // For all targets that needs to be updated.
    while let Some(tid) = render_targets_to_update.pop() {
        let rt = &graph.targets[&tid.0];

        write_barriers.insert(tid, rt.waits(tid.1)..rt.writes(tid.1));
        if !rt.reads(tid.1).is_empty() {
            read_barriers.insert(tid, rt.writes(tid.1)..rt.reads(tid.1));
        }

        // Activate render node attached to the target.
        let rid = rt.target_for(tid.1);

        // Mark as activated.
        if activate_renders.insert(rid) {
            // Push to queue.
            render_queue.push_back(rid);

            // Update dependencies.
            let render = &graph.renders[&rid];
            render_targets_to_update.extend(render.depends_on());
        }
    }

    // Build render schedule from roots to leaves.
    let mut render_schedule = Vec::new_in(&state.blink);
    let mut target_scheduled = HashSet::new_in(&state.blink);

    // Quadratic algorithm, but it's ok for now.
    while let Some(rid) = render_queue.pop_front() {
        let render = &graph.renders[&rid];

        let ready = render
            .depends_on()
            .all(|tid| target_scheduled.contains(&tid));

        if ready {
            // Scheduled the render.
            debug_assert!(!render_schedule.contains(&rid), "Render already scheduled");
            render_schedule.push(rid);

            for &tid in &render.renders_to {
                let inserted = target_scheduled.insert(tid);
                debug_assert!(inserted, "Target already scheduled");
            }
        } else {
            // Push back to queue.
            render_queue.push_back(rid);
        }
    }

    let mut cbufs = Vec::new_in(&state.blink);

    // Walk render schedule and run renders in opposite order.
    while let Some(rid) = render_schedule.pop() {
        let render = graph.renders.get_mut(&rid).unwrap();

        let cbufs_pre = cbufs.len();
        let result = render.run(
            &device,
            &mut queue,
            &*world,
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
