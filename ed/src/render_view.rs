//! Rendering an ECS world into an editor image.

use arcana::{
    ecs::{entity::EntityId, query::Cpy, world::World},
    id::Stid,
    mev,
    plugin::PluginsHub,
    render::{CurrentRenderer, RenderGraphId, Renderer},
    work_graph::{Image2D, Image2DInfo, PinId, WorkGraph},
};

use crate::{
    plugins::Plugins,
    render::{RenderGraphNode, RenderManager},
    ui::{Sampler, UserTextures},
};

struct CachedGraph {
    renderer: EntityId,
    graph: RenderGraphId,
    modification: u64,
    present: PinId,
    work: WorkGraph,
}

pub(crate) struct RenderView {
    cache: Option<CachedGraph>,
    image: Option<mev::Image2D>,
    extent: mev::Extent2,
    texture_id: Option<egui::TextureId>,
    focused: bool,

    /// Keeps callbacks and cached targets valid until they have been dropped.
    plugins: Option<Plugins>,
}

impl RenderView {
    pub(crate) fn new() -> Self {
        RenderView {
            cache: None,
            image: None,
            extent: mev::Extent2::ZERO,
            texture_id: None,
            focused: false,
            plugins: None,
        }
    }

    pub(crate) fn show(
        &mut self,
        textures: &mut UserTextures<'_>,
        ui: &mut egui::Ui,
    ) -> egui::Response {
        egui::Frame::NONE
            .corner_radius(egui::CornerRadius::same(5))
            .stroke(if self.focused {
                egui::Stroke::new(2.0, egui::Color32::ORANGE)
            } else {
                egui::Stroke::new(1.0, egui::Color32::LIGHT_GRAY)
            })
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                let size = ui.available_size().max(egui::Vec2::ZERO);
                let scale = ui.ctx().pixels_per_point();
                self.extent = mev::Extent2::new((size.x * scale) as u32, (size.y * scale) as u32);
                let id = *self.texture_id.get_or_insert_with(|| textures.new_id());
                let response = ui.add(
                    egui::Image::new(egui::load::SizedTexture { id, size })
                        .sense(egui::Sense::click()),
                );
                if response.clicked() {
                    response.request_focus();
                }
                if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                    response.surrender_focus();
                }
                self.focused = response.has_focus();
                response
            })
            .inner
    }

    pub(crate) fn render(
        &mut self,
        world: &mut World,
        hub: &mut PluginsHub,
        plugins: Option<&Plugins>,
        renderer: Option<EntityId>,
        queue: &mut mev::Queue,
        renders: &RenderManager,
        textures: &mut UserTextures<'_>,
    ) -> Result<(), mev::SurfaceError> {
        if self.plugins.as_ref() != plugins {
            self.invalidate(textures);
            self.plugins = plugins.cloned();
        }

        if self.extent.width() == 0 || self.extent.height() == 0 {
            self.invalidate(textures);
            return Ok(());
        }
        let Some(entity) = renderer else {
            self.invalidate(textures);
            return Ok(());
        };
        let Ok(renderer) = world.get::<Cpy<Renderer>>(entity) else {
            self.invalidate(textures);
            return Ok(());
        };
        let Some(graph) = renders.get_render_graph(renderer.graph) else {
            self.invalidate(textures);
            return Ok(());
        };
        let Some(present) = graph.get_present() else {
            self.invalidate(textures);
            return Ok(());
        };
        let valid_present = match graph.snarl.get_node(egui_snarl::NodeId(present.job.0)) {
            Some(RenderGraphNode::Job { desc, .. }) => {
                present.pin < desc.output_count()
                    && desc.output_type(present.pin) == Stid::of::<Image2D>()
            }
            _ => false,
        };
        if !valid_present {
            self.invalidate(textures);
            tracing::error!("Render graph present must be connected to an Image2D output");
            return Ok(());
        }

        if self.cache.as_ref().is_none_or(|cache| {
            cache.renderer != entity
                || cache.graph != renderer.graph
                || cache.modification != graph.modification
        }) {
            self.invalidate(textures);
            let work = match graph.make_work_graph() {
                Ok(work) => work,
                Err(error) => {
                    tracing::error!("Failed to make render work graph: {error:?}");
                    return Ok(());
                }
            };
            self.cache = Some(CachedGraph {
                renderer: entity,
                graph: renderer.graph,
                modification: graph.modification,
                present,
                work,
            });
        }

        if self
            .image
            .as_ref()
            .is_none_or(|image| image.extent() != self.extent)
        {
            self.image = Some(queue.new_image(mev::ImageDesc {
                dims: self.extent,
                format: mev::PixelFormat::Rgba8Srgb,
                usage: mev::ImageUsage::TARGET
                    | mev::ImageUsage::SAMPLED
                    | mev::ImageUsage::STORAGE,
                levels: 1,
                name: "Editor render view",
            }));
        }

        let image = self
            .image
            .as_ref()
            .expect("Render output has been allocated")
            .clone();
        let cache = self.cache.as_mut().expect("Render graph has been prepared");
        cache.work.set_sink(
            cache.present,
            Image2D(image.clone()),
            Image2DInfo::from_image(&image),
        );
        world.insert_resource(CurrentRenderer { entity });
        if let Err(error) = cache.work.run(queue, world, &mut hub.jobs) {
            self.invalidate(textures);
            tracing::error!("Failed to render editor view: {error:?}");
            return Ok(());
        }
        if let Some(id) = self.texture_id {
            textures.set(id, image, Sampler::NearestNearest);
        }
        Ok(())
    }

    fn invalidate(&mut self, textures: &mut UserTextures<'_>) {
        if let Some(id) = self.texture_id {
            textures.remove(id);
        }
        self.cache = None;
        self.image = None;
    }

    /// Drops plugin-owned caches before releasing their library guard.
    pub(crate) fn reset(&mut self) {
        self.cache = None;
        self.image = None;
        self.plugins = None;
        self.focused = false;
    }

    pub(crate) fn clear(&mut self, textures: &mut UserTextures<'_>) {
        self.invalidate(textures);
        self.texture_id = None;
        self.plugins = None;
        self.focused = false;
    }

    pub(crate) fn work_graph(&self) -> Option<&WorkGraph> {
        self.cache.as_ref().map(|cache| &cache.work)
    }

    pub(crate) fn work_graph_mut(&mut self) -> Option<&mut WorkGraph> {
        self.cache.as_mut().map(|cache| &mut cache.work)
    }

    #[cfg(test)]
    pub(crate) fn texture_id(&self) -> Option<egui::TextureId> {
        self.texture_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcana::{
        ecs::component::Component,
        hash::HashMap,
        work_graph::{Exec, Job, JobDesc, JobId, Planner, TargetCreateDesc},
    };
    use egui_snarl::{InPinId, OutPinId};

    use crate::{
        entity_ids::EntityIds,
        render::{RenderGraph, RenderGraphNode},
    };

    #[derive(Clone, Copy, Component)]
    struct OwnerData(u32);

    #[derive(Default)]
    struct Observations(Vec<(EntityId, u32, &'static str)>);

    struct OwnerJob;

    impl Job for OwnerJob {
        fn name() -> arcana::Name {
            arcana::name!(OwnerJob)
        }

        fn desc() -> JobDesc {
            JobDesc {
                creates: vec![TargetCreateDesc::new::<Image2D>(arcana::name!(color))],
                ..JobDesc::default()
            }
        }

        fn new() -> Self {
            Self
        }

        fn plan(&mut self, mut planner: Planner<'_>, world: &mut World) {
            let entity = world
                .get_resource::<CurrentRenderer>()
                .expect("Renderer resource")
                .entity;
            let value = world
                .get::<Cpy<OwnerData>>(entity)
                .expect("Auxiliary owner data")
                .0;
            world
                .get_resource_mut::<Observations>()
                .expect("Observation resource")
                .0
                .push((entity, value, "plan"));
            assert!(planner.create::<Image2D>().is_some());
        }

        fn exec(&mut self, exec: Exec<'_>, world: &mut World) {
            let entity = world
                .get_resource::<CurrentRenderer>()
                .expect("Renderer resource")
                .entity;
            let value = world
                .get::<Cpy<OwnerData>>(entity)
                .expect("Auxiliary owner data")
                .0;
            world
                .get_resource_mut::<Observations>()
                .expect("Observation resource")
                .0
                .push((entity, value, "exec"));
            let output = exec.create::<Image2D>().expect("Present target");
            let encoder = exec.new_encoder();
            encoder.init_image(
                mev::PipelineStages::all(),
                mev::PipelineStages::COLOR_OUTPUT,
                output.as_image(),
            );
            let color = if value < 20 {
                mev::ClearColor::RED
            } else {
                mev::ClearColor::BLUE
            };
            drop(encoder.render(mev::RenderPassDesc {
                name: "Renderer owner test",
                color_attachments: &[mev::AttachmentDesc::new(output.as_image()).clear(color)],
                depth_stencil_attachment: None,
            }));
        }
    }

    fn test_graph(job: JobId) -> RenderGraph {
        let mut graph = RenderGraph::new(arcana::name!(OwnerGraph));
        let present = graph
            .snarl
            .node_ids()
            .find_map(|(id, node)| matches!(node, RenderGraphNode::MainPresent).then_some(id))
            .expect("Present node");
        let owner = graph.snarl.insert_node(
            egui::Pos2::ZERO,
            RenderGraphNode::Job {
                job,
                name: OwnerJob::name(),
                plugin: arcana::name!(test),
                desc: OwnerJob::desc(),
                params: HashMap::default(),
                location: None,
                active: true,
            },
        );
        graph.snarl.connect(
            OutPinId {
                node: owner,
                output: 0,
            },
            InPinId {
                node: present,
                input: 0,
            },
        );
        graph
    }

    #[test]
    #[ignore = "Requires a native graphics device; executes and submits a render graph"]
    fn native_render_graph_reads_owner_in_both_phases_and_invalidates_output() {
        let instance = mev::Instance::load().expect("Native graphics instance");
        let (device_index, family_index) = instance
            .capabilities()
            .devices
            .iter()
            .enumerate()
            .find_map(|(device, caps)| {
                caps.families
                    .iter()
                    .position(|family| family.queue_flags.contains(mev::QueueFlags::GRAPHICS))
                    .map(|family| (device, family as u32))
            })
            .expect("Graphics queue family");
        let (_device, mut queues) = instance
            .new_device(mev::DeviceDesc {
                idx: device_index,
                queues: &[family_index],
                features: mev::Features::empty(),
            })
            .expect("Graphics device");
        let mut queue = queues.pop().expect("Requested queue");
        let ids = EntityIds::new();
        let mut world = ids.new_world();
        world.insert_resource(Observations::default());
        let graph_id = arcana::id::static_id!(1 as RenderGraphId);
        let job_id = arcana::id::static_id!(1 as JobId);
        let first = world
            .spawn((Renderer { graph: graph_id }, OwnerData(10)))
            .id();
        let second = world
            .spawn((Renderer { graph: graph_id }, OwnerData(20)))
            .id();
        let mut hub = PluginsHub::new();
        hub.jobs.insert(job_id, Box::new(OwnerJob));
        let mut renders = RenderManager::new();
        renders.insert_test_graph(graph_id, test_graph(job_id));
        let mut ui = crate::ui::Ui::new();
        let mut view = RenderView::new();
        view.extent = mev::Extent2::new(32, 32);
        view.texture_id = Some(ui.textures().new_id());

        view.render(
            &mut world,
            &mut hub,
            None,
            Some(first),
            &mut queue,
            &renders,
            &mut ui.textures(),
        )
        .expect("First render");
        let first_image = view.image.clone().expect("First output allocated");
        world.get::<&mut OwnerData>(first).expect("First owner").0 = 11;
        view.render(
            &mut world,
            &mut hub,
            None,
            Some(first),
            &mut queue,
            &renders,
            &mut ui.textures(),
        )
        .expect("Repeated render");
        assert_eq!(view.image.as_ref(), Some(&first_image));

        view.render(
            &mut world,
            &mut hub,
            None,
            Some(second),
            &mut queue,
            &renders,
            &mut ui.textures(),
        )
        .expect("Second owner render");
        assert_ne!(view.image.as_ref(), Some(&first_image));
        assert_eq!(
            view.cache.as_ref().expect("Second graph cache").renderer,
            second
        );
        assert_eq!(
            world
                .get_resource::<Observations>()
                .expect("Observation resource")
                .0,
            vec![
                (first, 10, "plan"),
                (first, 10, "exec"),
                (first, 11, "plan"),
                (first, 11, "exec"),
                (second, 20, "plan"),
                (second, 20, "exec")
            ]
        );

        let mut independent = ids.new_world();
        independent.spawn_at(first, (Renderer { graph: graph_id }, OwnerData(30)));
        independent.insert_resource(Observations::default());
        let mut independent_view = RenderView::new();
        independent_view.extent = view.extent;
        independent_view
            .render(
                &mut independent,
                &mut hub,
                None,
                Some(first),
                &mut queue,
                &renders,
                &mut ui.textures(),
            )
            .expect("Independent world render");
        assert_eq!(
            independent
                .get_resource::<Observations>()
                .expect("Independent observations")
                .0,
            vec![(first, 30, "plan"), (first, 30, "exec")]
        );

        world.drop::<Renderer>(second).expect("Second entity");
        view.render(
            &mut world,
            &mut hub,
            None,
            Some(second),
            &mut queue,
            &renders,
            &mut ui.textures(),
        )
        .expect("Missing owner clears output");
        assert!(view.image.is_none());
        assert!(view.cache.is_none());
        view.render(
            &mut world,
            &mut hub,
            None,
            Some(first),
            &mut queue,
            &renders,
            &mut ui.textures(),
        )
        .expect("Restored output");
        renders.insert_test_graph(graph_id, RenderGraph::new(arcana::name!(NoPresent)));
        view.render(
            &mut world,
            &mut hub,
            None,
            Some(first),
            &mut queue,
            &renders,
            &mut ui.textures(),
        )
        .expect("Disconnected present clears output");
        assert!(view.image.is_none());
        assert!(view.cache.is_none());
        view.clear(&mut ui.textures());
        assert!(view.texture_id.is_none());
        queue.wait_idle().expect("Submitted renders finish");
    }
}
