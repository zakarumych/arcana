mod graph;
mod job;
mod target;

use crate::with_stid;

pub use self::{
    graph::WorkGraph,
    job::{
        Exec, Job, JobCreateDesc, JobCreateTarget, JobDesc, JobReadDesc, JobReadTarget,
        JobUpdateDesc, JobUpdateTarget, Planner,
    },
    target::{Target, TargetHub, TargetId},
};

/// Generic 2d image target.
/// It does not hold particular meaning behind pixel values.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Image2D(pub mev::Image);

with_stid!(Image2D = 0x9010634f06624678);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Image2DInfo {
    pub extent: mev::Extent2,
    pub format: mev::PixelFormat,
    pub usage: mev::ImageUsage,
}

impl target::Target for Image2D {
    type Info = Image2DInfo;

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

with_stid!(SampledImage2D = 0x9010634f06624679);

impl target::Target for SampledImage2D {
    type Info = SampledImage2DInfo;

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

    fn merge_info(info: &mut SampledImage2DInfo, other: &SampledImage2DInfo) {
        info.extent = mev::Extent2::new(
            info.extent.width().max(other.extent.width()),
            info.extent.height().max(other.extent.height()),
        );
        info.usage |= other.usage;
    }
}