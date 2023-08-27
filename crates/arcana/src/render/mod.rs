//! Defines rendering for the Airy engine.

mod render;
mod target;

use std::{
    collections::VecDeque, fmt, marker::PhantomData, num::NonZeroU64, ops::Range, sync::Arc,
};

use blink_alloc::BlinkAlloc;
use edict::{State, World};
use hashbrown::{
    hash_map::{DefaultHashBuilder, Entry},
    HashMap, HashSet,
};
use mev::PipelineStages;
use parking_lot::Mutex;
use winit::window::{Window, WindowId};

use self::{render::RenderId, target::RenderTarget};

pub use self::render::{Render, RenderBuilderContext, TargetId};

pub trait RenderTargetType: 'static {
    fn add_target(
        graph: &mut RenderGraph,
        name: Box<str>,
        target_for: RenderId,
        stages: mev::PipelineStages,
    ) -> TargetId<Self>
    where
        Self: Sized;

    fn get_target(graph: &RenderGraph, id: NonZeroU64) -> &RenderTarget<Self>;
    fn get_target_mut(graph: &mut RenderGraph, id: NonZeroU64) -> &mut RenderTarget<Self>;

    fn write_target<'a>(
        cx: &'a mut RenderContext<'_, '_>,
        id: TargetId<Self>,
        encoder: &mut mev::CommandEncoder,
    ) -> &'a Self;

    fn read_target<'a>(
        cx: &'a mut RenderContext<'_, '_>,
        id: TargetId<Self>,
        encoder: &mut mev::CommandEncoder,
    ) -> &'a Self;

    fn add_dependency(edges: &mut RenderNodeEdges, id: TargetId<Self>);
    fn add_renders_to(edges: &mut RenderNodeEdges, id: TargetId<Self>);

    fn depends_on(edges: &RenderNodeEdges) -> hashbrown::hash_set::Iter<'_, TargetId<Self>>;
    fn renders_to(edges: &RenderNodeEdges) -> hashbrown::hash_set::Iter<'_, TargetId<Self>>;
}

impl RenderTargetType for mev::Image {
    fn add_target(
        graph: &mut RenderGraph,
        name: Box<str>,
        target_for: RenderId,
        stages: mev::PipelineStages,
    ) -> TargetId<mev::Image> {
        let id = graph.new_id();
        let tid = TargetId(id, 0, PhantomData);
        graph
            .image_targets
            .insert(id, RenderTarget::new(name, target_for, stages));
        tid
    }

    fn get_target(graph: &RenderGraph, id: NonZeroU64) -> &RenderTarget<mev::Image> {
        graph.image_targets.get(&id).expect("Invalid target id")
    }

    fn get_target_mut(graph: &mut RenderGraph, id: NonZeroU64) -> &mut RenderTarget<mev::Image> {
        graph.image_targets.get_mut(&id).expect("Invalid target id")
    }

    fn write_target<'a>(
        cx: &'a mut RenderContext<'_, '_>,
        id: TargetId<mev::Image>,
        encoder: &mut mev::CommandEncoder,
    ) -> &'a mev::Image {
        let image = cx.images.get(&id.0).expect("Invalid target id");
        let barrier = cx.write_image_barriers.remove(&id);
        if let Some(barrier) = barrier {
            if cx.init_images.remove(&id.0) {
                encoder.init_image(barrier.start, barrier.end, image);
            } else {
                encoder.barrier(barrier.start, barrier.end);
            }
        }
        image
    }

    fn read_target<'a>(
        cx: &'a mut RenderContext<'_, '_>,
        id: TargetId<mev::Image>,
        encoder: &mut mev::CommandEncoder,
    ) -> &'a mev::Image {
        let image = cx.images.get(&id.0).expect("Invalid target id");
        let barrier = cx.read_image_barriers.remove(&id);
        if let Some(barrier) = barrier {
            encoder.barrier(barrier.start, barrier.end);
        }
        image
    }

    fn add_dependency(edges: &mut RenderNodeEdges, id: TargetId<mev::Image>) {
        edges.depends_on_images.insert(id);
    }

    fn add_renders_to(edges: &mut RenderNodeEdges, id: TargetId<mev::Image>) {
        edges.renders_to_images.insert(id);
    }

    fn depends_on(edges: &RenderNodeEdges) -> hashbrown::hash_set::Iter<'_, TargetId<mev::Image>> {
        edges.depends_on_images.iter()
    }

    fn renders_to(edges: &RenderNodeEdges) -> hashbrown::hash_set::Iter<'_, TargetId<mev::Image>> {
        edges.renders_to_images.iter()
    }
}

impl RenderTargetType for mev::Buffer {
    fn add_target(
        graph: &mut RenderGraph,
        name: Box<str>,
        target_for: RenderId,
        stages: mev::PipelineStages,
    ) -> TargetId<mev::Buffer> {
        let id = graph.new_id();
        let tid = TargetId(id, 0, PhantomData);
        graph
            .buffer_targets
            .insert(id, RenderTarget::new(name, target_for, stages));
        tid
    }

    fn get_target(graph: &RenderGraph, id: NonZeroU64) -> &RenderTarget<mev::Buffer> {
        graph.buffer_targets.get(&id).expect("Invalid target id")
    }

    fn get_target_mut(graph: &mut RenderGraph, id: NonZeroU64) -> &mut RenderTarget<mev::Buffer> {
        graph
            .buffer_targets
            .get_mut(&id)
            .expect("Invalid target id")
    }

    fn write_target<'a>(
        cx: &'a mut RenderContext<'_, '_>,
        id: TargetId<mev::Buffer>,
        encoder: &mut mev::CommandEncoder,
    ) -> &'a mev::Buffer {
        let buffer = cx.buffers.get(&id.0).expect("Invalid target id");
        let barrier = cx.write_buffer_barriers.remove(&id);
        if let Some(barrier) = barrier {
            encoder.barrier(barrier.start, barrier.end);
        }
        buffer
    }

    fn read_target<'a>(
        cx: &'a mut RenderContext<'_, '_>,
        id: TargetId<mev::Buffer>,
        encoder: &mut mev::CommandEncoder,
    ) -> &'a mev::Buffer {
        let buffer = cx.buffers.get(&id.0).expect("Invalid target id");
        let barrier = cx.read_buffer_barriers.remove(&id);
        if let Some(barrier) = barrier {
            encoder.barrier(barrier.start, barrier.end);
        }
        buffer
    }

    fn add_dependency(edges: &mut RenderNodeEdges, id: TargetId<mev::Buffer>) {
        edges.depends_on_buffers.insert(id);
    }

    fn add_renders_to(edges: &mut RenderNodeEdges, id: TargetId<mev::Buffer>) {
        edges.renders_to_buffers.insert(id);
    }

    fn depends_on(edges: &RenderNodeEdges) -> hashbrown::hash_set::Iter<'_, TargetId<mev::Buffer>> {
        edges.depends_on_buffers.iter()
    }

    fn renders_to(edges: &RenderNodeEdges) -> hashbrown::hash_set::Iter<'_, TargetId<mev::Buffer>> {
        edges.renders_to_buffers.iter()
    }
}

pub struct RenderGraph {
    renders: HashMap<RenderId, RenderNode>,
    image_targets: HashMap<NonZeroU64, RenderTarget<mev::Image>>,
    buffer_targets: HashMap<NonZeroU64, RenderTarget<mev::Buffer>>,
    presents: HashMap<WindowId, TargetId<mev::Image>>,
    next_id: u64,
}

impl RenderGraph {
    pub fn new() -> Self {
        RenderGraph {
            renders: HashMap::new(),
            image_targets: HashMap::new(),
            buffer_targets: HashMap::new(),
            presents: HashMap::new(),
            next_id: 1,
        }
    }

    fn new_id(&mut self) -> NonZeroU64 {
        let id = self.next_id;
        self.next_id += 1;
        NonZeroU64::new(id).unwrap()
    }

    fn add_target<T>(
        &mut self,
        name: Box<str>,
        target_for: RenderId,
        stages: mev::PipelineStages,
    ) -> TargetId<T>
    where
        T: RenderTargetType,
    {
        T::add_target(self, name, target_for, stages)
    }

    fn get_target<T>(&self, id: NonZeroU64) -> &RenderTarget<T>
    where
        T: RenderTargetType,
    {
        T::get_target(self, id)
    }

    fn get_target_mut<T>(&mut self, id: NonZeroU64) -> &mut RenderTarget<T>
    where
        T: RenderTargetType,
    {
        T::get_target_mut(self, id)
    }

    /// Sets up render graph to present target to the window.
    /// Render system will look for this window and present the target to it if found.
    pub fn present(&mut self, target: TargetId<mev::Image>, window: WindowId) {
        let rt = self
            .image_targets
            .get_mut(&target.0)
            .expect("Invalid target id");

        rt.read(rt.versions() - 1, PipelineStages::empty());
        self.presents.insert(window, target);
    }
}

pub struct UpdateTargets {
    update_images: HashSet<TargetId<mev::Image>>,
    update_buffers: HashSet<TargetId<mev::Buffer>>,
}

impl UpdateTargets {
    pub fn new() -> Self {
        UpdateTargets {
            update_images: HashSet::new(),
            update_buffers: HashSet::new(),
        }
    }
}

type BlinkHashMap<'a, K, V> = HashMap<K, V, DefaultHashBuilder, &'a BlinkAlloc>;
type BlinkHashSet<'a, T> = HashSet<T, DefaultHashBuilder, &'a BlinkAlloc>;

#[derive(Debug)]
pub enum RenderError {
    OutOfMemory(mev::OutOfMemory),
}

impl From<mev::OutOfMemory> for RenderError {
    #[inline(always)]
    fn from(err: mev::OutOfMemory) -> Self {
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

type TargetMap<'a, T> = BlinkHashMap<'a, NonZeroU64, T>;
type BarrierMap<'a, T> = BlinkHashMap<'a, TargetId<T>, Range<mev::PipelineStages>>;
type ImageInitSet<'a> = BlinkHashSet<'a, NonZeroU64>;

pub struct RenderContext<'a, 'b> {
    device: &'a mev::Device,
    queue: &'a mut mev::Queue,

    // Maps render target ids to image resources.
    images: &'a mut TargetMap<'b, mev::Image>,

    // Maps render target to pipeline stages that need to be waited for.
    init_images: &'a mut ImageInitSet<'b>,
    write_image_barriers: &'a mut BarrierMap<'b, mev::Image>,
    read_image_barriers: &'a mut BarrierMap<'b, mev::Image>,

    // Maps render target ids to buffer resources.
    buffers: &'a mut TargetMap<'b, mev::Buffer>,

    // Maps render target to pipeline stages that need to be waited for.
    write_buffer_barriers: &'a mut BarrierMap<'b, mev::Buffer>,
    read_buffer_barriers: &'a mut BarrierMap<'b, mev::Buffer>,

    cbufs: &'a mut Vec<mev::CommandBuffer, &'b BlinkAlloc>,
    world: &'a World,
}

impl<'a> RenderContext<'a, '_> {
    pub fn device(&self) -> &mev::Device {
        self.device
    }

    pub fn new_command_encoder(&mut self) -> Result<mev::CommandEncoder, RenderError> {
        self.queue
            .new_command_encoder()
            .map_err(RenderError::OutOfMemory)
    }

    pub fn commit(&mut self, cbuf: mev::CommandBuffer) {
        self.cbufs.push(cbuf);
    }

    pub fn write_target<T>(&mut self, id: TargetId<T>, encoder: &mut mev::CommandEncoder) -> &T
    where
        T: RenderTargetType,
    {
        T::write_target(self, id, encoder)
    }

    pub fn read_target<T>(&mut self, id: TargetId<T>, encoder: &mut mev::CommandEncoder) -> &T
    where
        T: RenderTargetType,
    {
        T::read_target(self, id, encoder)
    }
}

pub struct RenderNodeEdges {
    depends_on_images: HashSet<TargetId<mev::Image>>,
    renders_to_images: HashSet<TargetId<mev::Image>>,
    depends_on_buffers: HashSet<TargetId<mev::Buffer>>,
    renders_to_buffers: HashSet<TargetId<mev::Buffer>>,
}

impl RenderNodeEdges {
    fn new() -> Self {
        RenderNodeEdges {
            depends_on_images: HashSet::new(),
            renders_to_images: HashSet::new(),
            depends_on_buffers: HashSet::new(),
            renders_to_buffers: HashSet::new(),
        }
    }

    fn add_dependency<T>(&mut self, id: TargetId<T>)
    where
        T: RenderTargetType,
    {
        T::add_dependency(self, id)
    }

    fn add_renders_to<T>(&mut self, id: TargetId<T>)
    where
        T: RenderTargetType,
    {
        T::add_renders_to(self, id)
    }

    fn depends_on<T>(&self) -> impl Iterator<Item = TargetId<T>> + '_
    where
        T: RenderTargetType,
    {
        T::depends_on(self).copied()
    }

    fn renders_to<T>(&self) -> impl Iterator<Item = TargetId<T>> + '_
    where
        T: RenderTargetType,
    {
        T::renders_to(self).copied()
    }
}

struct RenderNode {
    name: Box<str>,
    render: Box<dyn Render>,
    edges: RenderNodeEdges,
}

impl RenderNode {
    fn name(&self) -> &str {
        &self.name
    }

    fn run<'a, 'b>(
        &mut self,
        device: &'a mev::Device,
        queue: &'a mut mev::Queue,
        world: &'a World,
        images: &'a mut TargetMap<'b, mev::Image>,
        init_images: &'a mut ImageInitSet<'b>,
        write_image_barriers: &'a mut BarrierMap<'b, mev::Image>,
        read_image_barriers: &'a mut BarrierMap<'b, mev::Image>,
        buffers: &'a mut TargetMap<'b, mev::Buffer>,
        write_buffer_barriers: &'a mut BarrierMap<'b, mev::Buffer>,
        read_buffer_barriers: &'a mut BarrierMap<'b, mev::Buffer>,
        cbufs: &'a mut Vec<mev::CommandBuffer, &'b BlinkAlloc>,
        blink: &'b BlinkAlloc,
    ) -> Result<(), RenderError> {
        self.render.render(
            RenderContext::<'a, 'b> {
                device,
                queue,
                images,
                init_images,
                write_image_barriers,
                read_image_barriers,
                buffers,
                write_buffer_barriers,
                read_buffer_barriers,
                cbufs,
                world,
            },
            world,
            blink,
        )
    }
}

#[derive(Default)]
pub struct RenderResources {
    surfaces: HashMap<WindowId, mev::Surface>,
    images: HashMap<NonZeroU64, mev::Image>,
    buffers: HashMap<NonZeroU64, mev::Buffer>,
}

#[derive(Default)]
pub struct RenderState {
    blink: BlinkAlloc,
    resources: RenderResources,
}

/// Render system for the game.
///
/// Can be added to scheduler to render the frame.
pub fn render_system(world: &mut World, mut state: State<RenderState>) {
    let state = &mut *state;
    // let world = world.local();

    let device = world.expect_resource::<mev::Device>();

    let mut owned_queue = world.get_resource_mut::<mev::Queue>();
    let mut shard_queue = world.get_resource_mut::<Arc<Mutex<mev::Queue>>>();
    let mut queue_lock;

    let queue = match (&mut owned_queue, &mut shard_queue) {
        (Some(queue), _) => &mut **queue,
        (None, Some(queue)) => {
            queue_lock = queue.lock();
            &mut *queue_lock
        }
        (None, None) => {
            panic!("No mev::Queue found")
        }
    };

    let mut graph = world.expect_resource_mut::<RenderGraph>();
    let mut update_targets = world.get_resource_mut::<UpdateTargets>();
    let window = world.expect_resource_mut::<Window>();

    render(
        &mut *graph,
        &*device,
        queue,
        &state.blink,
        update_targets.as_deref_mut(),
        Some(&*window),
        &*world,
        &mut state.resources,
    );
}

/// Rendering function.
pub fn render<'a>(
    graph: &mut RenderGraph,
    device: &mev::Device,
    queue: &mut mev::Queue,
    blink: &BlinkAlloc,
    update_targets: Option<&mut UpdateTargets>,
    windows: impl IntoIterator<Item = &'a Window>,
    world: &World,
    resources: &mut RenderResources,
) {
    // Collect all targets that needs to be updated.
    // If target is bound to surface, fetch next frame.
    let mut image_targets_to_update = Vec::new_in(blink);
    let mut buffer_targets_to_update = Vec::new_in(blink);

    if let Some(update_targets) = update_targets {
        image_targets_to_update.extend(update_targets.update_images.drain());
        buffer_targets_to_update.extend(update_targets.update_buffers.drain());
    }

    // Maps render target ids to image resources.
    let mut images = HashMap::new_in(blink);
    let mut buffers = HashMap::new_in(blink);

    // Maps render target entity to access stages.
    let mut init_images = HashSet::new_in(blink);
    let mut write_image_barriers = HashMap::new_in(blink);
    let mut read_image_barriers = HashMap::new_in(blink);
    let mut write_buffer_barriers = HashMap::new_in(blink);
    let mut read_buffer_barriers = HashMap::new_in(blink);

    let mut drop_surfaces = Vec::new_in(blink);
    let mut frames = Vec::new_in(blink);

    for window in windows {
        let wid = window.id();
        let Some(&tid) = graph.presents.get(&wid) else {
            // This graph does not presenting to this window.
            continue;
        };

        // Target is guaranteed to exist.
        let rt = &graph.image_targets[&tid.0];

        let surface = match resources.surfaces.entry(wid) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let surface = device.new_surface(&window, &window).unwrap();
                entry.insert(surface)
            }
        };

        match surface.next_frame(&mut *queue, rt.writes(0)) {
            Err(err) => {
                tracing::error!(err = ?err);
                drop_surfaces.push(wid);
                continue;
            }
            Ok(frame) => {
                let image = frame.image();
                init_images.insert(tid.0);
                images.insert(tid.0, image.clone());
                frames.push((frame, rt.writes(tid.1) | rt.reads(tid.1)));
            }
        }

        image_targets_to_update.push(tid);
    }

    // Find all renders to activate.
    let mut activate_renders = HashSet::new_in(blink);
    let mut render_queue = VecDeque::new_in(blink);

    // For all targets that needs to be updated.
    while let Some(tid) = image_targets_to_update.pop() {
        let rt = graph.get_target::<mev::Image>(tid.0);

        write_image_barriers.insert(tid, rt.waits(tid.1)..rt.writes(tid.1));
        if !rt.reads(tid.1).is_empty() {
            read_image_barriers.insert(tid, rt.writes(tid.1)..rt.reads(tid.1));
        }

        // Activate render node attached to the target.
        let rid = rt.target_for(tid.1);

        // Mark as activated.
        if activate_renders.insert(rid) {
            // Push to queue.
            render_queue.push_back(rid);

            // Update dependencies.
            let render = &graph.renders[&rid];
            image_targets_to_update.extend(render.edges.depends_on());
        }
    }

    // For all targets that needs to be updated.
    while let Some(tid) = buffer_targets_to_update.pop() {
        let rt = graph.get_target::<mev::Buffer>(tid.0);

        write_buffer_barriers.insert(tid, rt.waits(tid.1)..rt.writes(tid.1));
        if !rt.reads(tid.1).is_empty() {
            read_buffer_barriers.insert(tid, rt.writes(tid.1)..rt.reads(tid.1));
        }

        // Activate render node attached to the target.
        let rid = rt.target_for(tid.1);

        // Mark as activated.
        if activate_renders.insert(rid) {
            // Push to queue.
            render_queue.push_back(rid);

            // Update dependencies.
            let render = &graph.renders[&rid];
            buffer_targets_to_update.extend(render.edges.depends_on());
        }
    }

    // Build render schedule from roots to leaves.
    let mut render_schedule = Vec::new_in(blink);
    let mut image_scheduled = HashSet::new_in(blink);
    let mut buffer_scheduled = HashSet::new_in(blink);

    // Quadratic algorithm, but it's ok for now.
    while let Some(rid) = render_queue.pop_front() {
        let render = &graph.renders[&rid];

        let mut ready = render
            .edges
            .depends_on::<mev::Image>()
            .all(|tid| image_scheduled.contains(&tid));

        ready &= render
            .edges
            .depends_on::<mev::Buffer>()
            .all(|tid| buffer_scheduled.contains(&tid));

        if ready {
            // Scheduled the render.
            debug_assert!(!render_schedule.contains(&rid), "Render already scheduled");
            render_schedule.push(rid);

            for tid in render.edges.renders_to::<mev::Image>() {
                let inserted = image_scheduled.insert(tid);
                debug_assert!(inserted, "Target already scheduled");
            }

            for tid in render.edges.renders_to::<mev::Buffer>() {
                let inserted = buffer_scheduled.insert(tid);
                debug_assert!(inserted, "Target already scheduled");
            }
        } else {
            // Push back to queue.
            render_queue.push_back(rid);
        }
    }

    let mut cbufs = Vec::new_in(blink);

    // Walk render schedule and run renders in opposite order.
    while let Some(rid) = render_schedule.pop() {
        let render = graph.renders.get_mut(&rid).unwrap();

        let cbufs_pre = cbufs.len();
        let result = render.run(
            device,
            queue,
            &*world,
            &mut images,
            &mut init_images,
            &mut write_image_barriers,
            &mut read_image_barriers,
            &mut buffers,
            &mut write_buffer_barriers,
            &mut read_buffer_barriers,
            &mut cbufs,
            blink,
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