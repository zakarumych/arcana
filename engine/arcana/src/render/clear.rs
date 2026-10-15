use arcana_ecs::world::World;
use arcana_model::{ColorModel, Model, Value};
use arcana_work_graph::{
    Exec, Image2D, Image2DReq, Job, JobDesc, JobId, Planner, Req, TargetCreateDesc,
};

/// Creates a color image and clears every pixel each frame.
/// `Color` is converted from sRGBA to linear premultiplied RGBA.
/// Consumer requirements select extent and format, defaulting to 1x1 RGBA8.
pub struct NewColorImage;

impl NewColorImage {
    pub fn id() -> JobId {
        arcana_id::hash_id!("arcana::render::NewColorImage" => JobId)
    }
}

impl Job for NewColorImage {
    fn name() -> crate::Name {
        crate::Name::from_name_static("New color image")
    }

    fn desc() -> JobDesc {
        JobDesc {
            params: vec![(crate::name!(Color), Model::Color(ColorModel::Srgba))],
            creates: vec![TargetCreateDesc::new::<Image2D>(crate::name!(Image))],
            ..JobDesc::default()
        }
    }

    fn new() -> Self {
        Self
    }

    fn plan(&mut self, mut planner: Planner<'_>, _: &mut World) {
        planner.create_with_req::<Image2D>(Image2DReq {
            extent: Req::Suggest(mev::Extent2::new(1, 1)),
            format: Req::Suggest(mev::PixelFormat::Rgba8Unorm),
            usage: mev::ImageUsage::TARGET,
        });
    }

    fn exec(&mut self, exec: Exec<'_>, _: &mut World) {
        let Some(target) = exec.create::<Image2D>() else {
            return;
        };
        if !target.format().is_color() {
            tracing::error!("New color image requires a color format");
            return;
        }
        let Value::Color(color) = exec.param("Color") else {
            tracing::error!("Invalid clear color");
            return;
        };
        let color = color.into_srgba().into_linear();
        let clear = mev::ClearColor(
            color.red * color.alpha,
            color.green * color.alpha,
            color.blue * color.alpha,
            color.alpha,
        );
        let encoder = exec.new_encoder();
        encoder.init_image(
            mev::PipelineStages::all(),
            mev::PipelineStages::COLOR_OUTPUT,
            &target.0,
        );
        encoder.render(mev::RenderPassDesc {
            name: "New color image",
            color_attachments: &[mev::AttachmentDesc::new(&target.0).clear(clear)],
            depth_stencil_attachment: None,
        });
    }
}

/// Creates a depth image and clears every depth value each frame.
/// `Depth` defaults to zero, is clamped to [0, 1], and treats nonfinite values as zero.
/// Consumer requirements select extent and format, defaulting to 1x1 D32Float.
/// Stencil, when present, is cleared to zero.
pub struct NewDepthImage;

impl NewDepthImage {
    pub fn id() -> JobId {
        arcana_id::hash_id!("arcana::render::NewDepthImage" => JobId)
    }
}

impl Job for NewDepthImage {
    fn name() -> crate::Name {
        crate::Name::from_name_static("New depth image")
    }

    fn desc() -> JobDesc {
        JobDesc {
            params: vec![(crate::name!(Depth), Model::Float)],
            creates: vec![TargetCreateDesc::new::<Image2D>(crate::name!(Image))],
            ..JobDesc::default()
        }
    }

    fn new() -> Self {
        Self
    }

    fn plan(&mut self, mut planner: Planner<'_>, _: &mut World) {
        planner.create_with_req::<Image2D>(Image2DReq {
            extent: Req::Suggest(mev::Extent2::new(1, 1)),
            format: Req::Suggest(mev::PixelFormat::D32Float),
            usage: mev::ImageUsage::TARGET,
        });
    }

    fn exec(&mut self, exec: Exec<'_>, _: &mut World) {
        let Some(target) = exec.create::<Image2D>() else {
            return;
        };
        if !target.format().is_depth() {
            tracing::error!("New depth image requires a depth format");
            return;
        }
        let Value::Float(depth) = exec.param("Depth") else {
            tracing::error!("Invalid clear depth");
            return;
        };
        let depth = if depth.is_finite() {
            depth.clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let encoder = exec.new_encoder();
        encoder.init_image(
            mev::PipelineStages::all(),
            mev::PipelineStages::EARLY_FRAGMENT_TEST | mev::PipelineStages::LATE_FRAGMENT_TEST,
            &target.0,
        );
        encoder.render(mev::RenderPassDesc {
            name: "New depth image",
            color_attachments: &[],
            depth_stencil_attachment: Some(
                mev::AttachmentDesc::new(&target.0)
                    .clear(mev::ClearDepthStencil { depth, stencil: 0 }),
            ),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcana_work_graph::{JobIdx, PinId, WorkGraph};

    #[test]
    fn builtin_clear_jobs_have_creates_and_retain_instances() {
        let mut hub = arcana_plugin::PluginsHub::new();
        crate::render::register_builtin_jobs(&mut hub);
        let color = &*hub.jobs[&NewColorImage::id()] as *const dyn Job;
        let depth = &*hub.jobs[&NewDepthImage::id()] as *const dyn Job;
        crate::render::register_builtin_jobs(&mut hub);
        assert!(std::ptr::eq(color, &*hub.jobs[&NewColorImage::id()]));
        assert!(std::ptr::eq(depth, &*hub.jobs[&NewDepthImage::id()]));
        for id in [NewColorImage::id(), NewDepthImage::id()] {
            let info = crate::render::builtin_jobs()
                .into_iter()
                .find(|info| info.id == id)
                .unwrap();
            assert!(info.desc.reads.is_empty());
            assert!(info.desc.updates.is_empty());
            assert_eq!(info.desc.creates.len(), 1);
            assert_eq!(info.desc.params.len(), 1);
            assert_eq!(
                info.desc.creates[0],
                TargetCreateDesc::new::<Image2D>(crate::name!(Image))
            );
        }
    }

    #[test]
    #[ignore = "requires a native GPU device"]
    fn gpu_clear_color_and_depth_pixels() {
        let instance = mev::Instance::load().unwrap();
        let (idx, family) = instance
            .capabilities()
            .devices
            .iter()
            .enumerate()
            .find_map(|(idx, device)| {
                if !device.features.contains(mev::Features::BUFFER_MAP) {
                    return None;
                }
                device
                    .families
                    .iter()
                    .position(|family| {
                        family
                            .queue_flags
                            .contains(mev::QueueFlags::GRAPHICS | mev::QueueFlags::TRANSFER)
                    })
                    .map(|family| (idx, family as u32))
            })
            .expect("graphics/transfer queue with host-readable buffers");
        let (device, mut queues) = instance
            .new_device(mev::DeviceDesc {
                idx,
                queues: &[family],
                features: mev::Features::BUFFER_MAP,
            })
            .unwrap();
        let mut queue = queues.pop().unwrap();
        let mut world = World::new();
        let mut hub = arcana_plugin::PluginsHub::new();
        crate::render::register_builtin_jobs(&mut hub);
        let Value::Color(default_color) = Model::Color(ColorModel::Srgba).default_value() else {
            unreachable!();
        };
        let mut color = default_color.into_srgba();
        color.red = 1.0;
        color.green = 0.5;
        color.alpha = 0.5;
        let color_value = Value::Color(arcana_model::ColorValue::Srgba(color));
        for (id, desc, value, format, expected) in [
            (
                NewColorImage::id(),
                NewColorImage::desc(),
                color_value,
                mev::PixelFormat::Rgba8Unorm,
                [128, 27, 0, 128],
            ),
            (
                NewDepthImage::id(),
                NewDepthImage::desc(),
                Value::Float(0.375),
                mev::PixelFormat::D32Float,
                0.375f32.to_ne_bytes(),
            ),
            (
                NewDepthImage::id(),
                NewDepthImage::desc(),
                Value::Float(2.0),
                mev::PixelFormat::D32Float,
                1.0f32.to_ne_bytes(),
            ),
            (
                NewDepthImage::id(),
                NewDepthImage::desc(),
                Value::Float(f64::NAN),
                mev::PixelFormat::D32Float,
                0.0f32.to_ne_bytes(),
            ),
        ] {
            let params = [(desc.params[0].0, value)].into_iter().collect();
            let mut graph = WorkGraph::new(
                [(JobIdx(0), (id, desc, params))].into_iter().collect(),
                Default::default(),
            )
            .unwrap();
            // No sink means no commands or resources are demanded.
            graph.run(&mut queue, &mut world, &mut hub.jobs).unwrap();
            let extent = mev::Extent2::new(7, 3);
            let image = device.new_image(mev::ImageDesc {
                dims: extent,
                format,
                usage: mev::ImageUsage::TARGET | mev::ImageUsage::TRANSFER_SRC,
                levels: 1,
                name: "Clear image test",
            });
            graph.set_sink(
                PinId {
                    job: JobIdx(0),
                    pin: 0,
                },
                Image2D(image.clone()),
                Image2DReq::from_image(&image),
            );
            let mut readback = device.new_buffer(mev::BufferDesc {
                size: 7 * 3 * 4,
                usage: mev::BufferUsage::TRANSFER_DST | mev::BufferUsage::HOST_READ,
                name: "Clear image readback",
            });
            for _ in 0..2 {
                graph.run(&mut queue, &mut world, &mut hub.jobs).unwrap();
                let mut encoder = queue.new_command_encoder();
                encoder.barrier(mev::PipelineStages::all(), mev::PipelineStages::TRANSFER);
                encoder.copy().copy_image_to_buffer(
                    &image,
                    &readback,
                    mev::TexelShape::new(format, extent),
                );
                queue.submit_checkpoint([encoder.finish()]).unwrap();
                queue.wait_idle().unwrap();
                let mut bytes = [0; 7 * 3 * 4];
                readback.read(0, &mut bytes).unwrap();
                for pixel in bytes.chunks_exact(4) {
                    assert_eq!(pixel, expected, "Unexpected {format:?} clear pixel");
                }
            }
        }
    }
}
