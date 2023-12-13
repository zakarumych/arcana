mod graph;
mod job;
mod target;

pub use self::{
    graph::WorkGraph,
    job::{Access, Job, JobId, JobNode, PlanJob, Planner, RunJob, Runner, Setup},
    target::{Target, TargetInfoMerge},
};

/// Generic 2d image target.
/// It does not hold particular meaning behind pixel values.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Image2D(pub mev::Image);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Image2DInfo {
    pub extent: mev::Extent2,
    pub format: mev::PixelFormat,
    pub usage: mev::ImageUsage,
}

impl Target for Image2D {
    type Info = Image2DInfo;

    fn name() -> &'static str {
        "Image2D"
    }

    fn allocate(device: &mev::Device, name: &str, info: &Image2DInfo) -> Self {
        let image = device
            .new_image(mev::ImageDesc {
                dimensions: info.extent.into(),
                format: info.format,
                usage: info.usage,
                layers: 1,
                levels: 1,
                name,
            })
            .unwrap();

        Image2D(image)
    }
}

/// Generic 2d image target.
/// It does not hold particular meaning behind pixel values.
/// Consumers are going to sample it,
/// so its actual extent and format is irrelevant,
/// but consumers may still provide desired extent and usage.
///
/// Largest required extent is used and usage is merged.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SampledImage2D(pub mev::Image);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SampledImage2DInfo {
    pub extent: mev::Extent2,
    pub usage: mev::ImageUsage,
}

impl Target for SampledImage2D {
    type Info = SampledImage2DInfo;

    fn name() -> &'static str {
        "SampledImage2D"
    }

    fn allocate(device: &mev::Device, name: &str, info: &SampledImage2DInfo) -> Self {
        let image = device
            .new_image(mev::ImageDesc {
                dimensions: info.extent.into(),
                format: todo!(),
                usage: info.usage,
                layers: 1,
                levels: 1,
                name,
            })
            .unwrap();

        SampledImage2D(image)
    }
}

impl TargetInfoMerge for SampledImage2D {
    fn merge_info(info: &mut SampledImage2DInfo, other: &SampledImage2DInfo) {
        info.extent = mev::Extent2::new(
            info.extent.width().max(other.extent.width()),
            info.extent.height().max(other.extent.height()),
        );
        info.usage |= other.usage;
    }
}

struct DummyJob;

impl job::Job for DummyJob {
    fn setup(self, mut setup: job::Setup<'_>) -> job::JobNode {
        let image = setup.produce::<Image2D>();

        setup.build(
            "Dummy".to_owned(),
            move |mut planner: job::Planner| {
                planner.output(image, "dummy");
            },
            move |mut runner: job::Runner| {
                let image = runner.output(image, mev::PipelineStages::all());
            },
        )
    }
}
