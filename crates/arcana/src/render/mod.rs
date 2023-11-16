//! Defines rendering for the Airy engine.

mod render;
mod target;

use std::{
    collections::VecDeque, fmt, marker::PhantomData, num::NonZeroU64, ops::Range, sync::Arc,
};

use base64::engine::general_purpose::NO_PAD;
use blink_alloc::BlinkAlloc;
use edict::{EntityId, State, World};
use hashbrown::{
    hash_map::{DefaultHashBuilder, Entry},
    HashMap, HashSet,
};
use mev::PipelineStages;
use parking_lot::Mutex;
use winit::window::{Window, WindowId};

// use crate::window::Windows;

use crate::viewport::{self, ViewportTexture};

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
        let image = cx.values.images.get(&id.0).expect("Invalid target id");
        let barrier = cx.values.write_image_barriers.remove(&id);
        if let Some(barrier) = barrier {
            if cx.values.init_images.remove(&id.0) {
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
        let image = cx.values.images.get(&id.0).expect("Invalid target id");
        let barrier = cx.values.read_image_barriers.remove(&id);
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
        let buffer = cx.values.buffers.get(&id.0).expect("Invalid target id");
        let barrier = cx.values.write_buffer_barriers.remove(&id);
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
        let buffer = cx.values.buffers.get(&id.0).expect("Invalid target id");
        let barrier = cx.values.read_buffer_barriers.remove(&id);
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
    presents: HashMap<EntityId, TargetId<mev::Image>>,
    main_present: Option<TargetId<mev::Image>>,
    next_id: u64,
}

impl RenderGraph {
    pub fn new() -> Self {
        RenderGraph {
            renders: HashMap::new(),
            image_targets: HashMap::new(),
            buffer_targets: HashMap::new(),
            presents: HashMap::new(),
            main_present: None,
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

    /// Sets up render graph to present target to the main viewport.
    /// Render system will look for main viewport and present the target to it if found.
    pub fn present(&mut self, target: TargetId<mev::Image>) {
        self.main_present = Some(target);
    }

    /// Sets up render graph to present target to the viewport.
    /// Render system will look for this viewport and present the target to it if found.
    pub fn present_to(&mut self, target: TargetId<mev::Image>, viewport: EntityId) {
        self.presents.insert(viewport, target);
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

struct RenderContextValues<'a> {
    // Maps render target ids to image resources.
    images: TargetMap<'a, mev::Image>,

    // Maps render target to pipeline stages that need to be waited for.
    init_images: ImageInitSet<'a>,
    write_image_barriers: BarrierMap<'a, mev::Image>,
    read_image_barriers: BarrierMap<'a, mev::Image>,

    // Maps render target ids to buffer resources.
    buffers: TargetMap<'a, mev::Buffer>,

    // Maps render target to pipeline stages that need to be waited for.
    write_buffer_barriers: BarrierMap<'a, mev::Buffer>,
    read_buffer_barriers: BarrierMap<'a, mev::Buffer>,

    cbufs: Vec<mev::CommandBuffer, &'a BlinkAlloc>,
}

pub struct RenderContext<'a, 'b> {
    device: &'a mev::Device,
    queue: &'a mut mev::Queue,
    values: &'a mut RenderContextValues<'b>,
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
        self.values.cbufs.push(cbuf);
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

    fn render<'a, 'b>(
        &mut self,
        world: &mut World,
        ctx: RenderContext<'a, 'b>,
    ) -> Result<(), RenderError> {
        self.render.render(world, ctx)
    }
}

#[derive(Default)]
pub struct RenderResources {
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

    let mut graph = world.remove_resource::<RenderGraph>().unwrap();
    let mut update_targets = world.remove_resource::<UpdateTargets>();
    let device = world.remove_resource::<mev::Device>().unwrap();
    let mut owned_queue = world.remove_resource::<mev::Queue>();
    let mut shared_queue = world.remove_resource::<Arc<Mutex<mev::Queue>>>();

    {
        let mut queue_lock;

        let queue = match (&mut owned_queue, &mut shared_queue) {
            (Some(queue), _) => queue,
            (None, Some(queue)) => {
                queue_lock = queue.lock();
                &mut *queue_lock
            }
            (None, None) => {
                panic!("No mev::Queue found")
            }
        };

        render(
            &mut graph,
            &device,
            queue,
            &state.blink,
            update_targets.as_mut(),
            world,
            &mut state.resources,
        );
    }

    world.insert_resource(graph);
    world.insert_resource(update_targets);
    world.insert_resource(device);
    if let Some(owned_queue) = owned_queue {
        world.insert_resource(owned_queue);
    }
    if let Some(shared_queue) = shared_queue {
        world.insert_resource(shared_queue);
    }
}

/// Rendering function.
pub fn render<'a>(
    graph: &mut RenderGraph,
    device: &mev::Device,
    queue: &mut mev::Queue,
    blink: &BlinkAlloc,
    update_targets: Option<&mut UpdateTargets>,
    world: &mut World,
    resources: &mut RenderResources,
) {
    let mut ctx = RenderContextValues {
        // Maps render target ids to image resources.
        images: HashMap::new_in(blink),
        buffers: HashMap::new_in(blink),

        // Maps render target entity to access stages.
        init_images: HashSet::new_in(blink),
        write_image_barriers: HashMap::new_in(blink),
        read_image_barriers: HashMap::new_in(blink),
        write_buffer_barriers: HashMap::new_in(blink),
        read_buffer_barriers: HashMap::new_in(blink),

        // Submitted command buffers.
        cbufs: Vec::new_in(blink),
    };

    // Collect all targets that needs to be updated.
    // If target is bound to surface, fetch next frame.
    let mut image_targets_to_update = Vec::new_in(blink);
    let mut buffer_targets_to_update = Vec::new_in(blink);

    if let Some(update_targets) = update_targets {
        image_targets_to_update.extend(update_targets.update_images.drain());
        buffer_targets_to_update.extend(update_targets.update_buffers.drain());
    }

    let mut frames = Vec::new_in(blink);

    let mut insert_surfaces = Vec::new_in(blink);
    let mut drop_surfaces = Vec::new_in(blink);

    for (&viewport, &tid) in graph.presents.iter() {
        // Target is guaranteed to exist.
        let rt = &graph.image_targets[&tid.0];

        let Ok(tripple) = world.get::<(
            Option<&Window>,
            Option<&mut mev::Surface>,
            Option<&ViewportTexture>,
        )>(viewport) else {
            continue;
        };

        match tripple {
            (None, surface, None) => {
                if surface.is_some() {
                    drop_surfaces.push(viewport);
                }
            }
            (None, surface, Some(texture)) => {
                if surface.is_some() {
                    drop_surfaces.push(viewport);
                }

                ctx.init_images.insert(tid.0);
                ctx.images.insert(tid.0, texture.image.clone());
            }
            (Some(window), surface, _) => {
                let window_id = window.id();
                let mut new_surface = None;

                let surface = match surface {
                    None => new_surface.get_or_insert(device.new_surface(window, window).unwrap()),
                    Some(surface) => surface,
                };
                match surface.next_frame(&mut *queue, rt.writes(0)) {
                    Err(err) => {
                        tracing::error!(err = ?err);
                        if new_surface.is_none() {
                            drop_surfaces.push(viewport);
                        }
                        new_surface = None;
                        continue;
                    }
                    Ok(frame) => {
                        let image = frame.image();
                        ctx.init_images.insert(tid.0);
                        ctx.images.insert(tid.0, image.clone());
                        frames.push((frame, rt.writes(tid.1) | rt.reads(tid.1)));
                    }
                }
                if let Some(surface) = new_surface {
                    insert_surfaces.push((viewport, surface));
                }
            }
        }

        image_targets_to_update.push(tid);
    }

    for (viewport, surface) in insert_surfaces {
        world.insert_external(viewport, surface);
    }
    for viewport in drop_surfaces {
        world.remove::<mev::Surface>(viewport);
    }

    let mut new_main_surface = None;
    let mut remove_main_surface = false;
    if let Some(tid) = graph.main_present {
        // Target is guaranteed to exist.
        let rt = &graph.image_targets[&tid.0];

        let window = world.get_resource::<Window>();
        let mut surface = world.get_resource_mut::<mev::Surface>();
        let texture = world.get_resource::<ViewportTexture>();

        match (
            window.as_deref(),
            surface.as_deref_mut(),
            texture.as_deref(),
        ) {
            (None, surface, None) => {
                remove_main_surface = surface.is_some();
            }
            (None, surface, Some(texture)) => {
                remove_main_surface = surface.is_some();

                ctx.init_images.insert(tid.0);
                ctx.images.insert(tid.0, texture.image.clone());
            }
            (Some(window), surface, _) => {
                let window_id = window.id();

                let surface = match surface {
                    None => {
                        new_main_surface.get_or_insert(device.new_surface(window, window).unwrap())
                    }
                    Some(surface) => surface,
                };
                match surface.next_frame(&mut *queue, rt.writes(0)) {
                    Err(err) => {
                        tracing::error!(err = ?err);
                        remove_main_surface = new_main_surface.is_none();
                        new_main_surface = None;
                    }
                    Ok(frame) => {
                        let image = frame.image();
                        ctx.init_images.insert(tid.0);
                        ctx.images.insert(tid.0, image.clone());
                        frames.push((frame, rt.writes(tid.1) | rt.reads(tid.1)));
                    }
                }
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

        ctx.write_image_barriers
            .insert(tid, rt.waits(tid.1)..rt.writes(tid.1));
        if !rt.reads(tid.1).is_empty() {
            ctx.read_image_barriers
                .insert(tid, rt.writes(tid.1)..rt.reads(tid.1));
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

        ctx.write_buffer_barriers
            .insert(tid, rt.waits(tid.1)..rt.writes(tid.1));
        if !rt.reads(tid.1).is_empty() {
            ctx.read_buffer_barriers
                .insert(tid, rt.writes(tid.1)..rt.reads(tid.1));
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

    // Walk render schedule and run renders in opposite order.
    while let Some(rid) = render_schedule.pop() {
        let render = graph.renders.get_mut(&rid).unwrap();

        let cbufs_pre = ctx.cbufs.len();
        let result = render.render(
            &mut *world,
            RenderContext {
                device,
                queue,
                values: &mut ctx,
            },
        );

        match result {
            Ok(()) => {
                let cbufs_post = ctx.cbufs.len();
                ctx.cbufs[cbufs_pre..cbufs_post].reverse();
            }
            Err(err) => {
                tracing::event!(tracing::Level::ERROR, err = ?err);
            }
        }
    }

    ctx.cbufs.reverse();
    queue.submit(ctx.cbufs, false).unwrap();

    let mut encoder = queue.new_command_encoder().unwrap();

    for (frame, after) in frames {
        encoder.present(frame, after);
    }

    queue.submit(Some(encoder.finish().unwrap()), true).unwrap();
}
