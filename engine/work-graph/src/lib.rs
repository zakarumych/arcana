//! This module contains work-graph implementation.
//! Work graph consists of jobs that declare resources they work on and set of edges between them.
//! Jobs work in isolation except for shared resources they declared.

mod graph;
mod job;
mod target;

use std::ops::Deref;

use arcana_id::has_stid;
use hashbrown::HashMap;

pub use self::{
    graph::{CommandStream, Cycle, Edge, Exec, HookId, JobIdx, PinId, Planner, WorkGraph},
    job::{Job, JobDesc, JobId, TargetCreateDesc, TargetReadDesc, TargetUpdateDesc},
    target::{MergeError, Req, Target, TargetId},
};

/// Generic 2d image target.
/// It does not hold particular meaning behind pixel values.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Image2D(pub mev::Image2D);

has_stid!(Image2D @ arcana_id);

impl Deref for Image2D {
    type Target = mev::Image2D;

    #[inline]
    fn deref(&self) -> &mev::Image2D {
        &self.0
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Image2DReq {
    pub extent: Req<mev::Extent2>,
    pub format: Req<mev::PixelFormat>,
    pub usage: mev::ImageUsage,
}

impl Image2DReq {
    #[inline(always)]
    pub fn from_image(image: &mev::Image2D) -> Self {
        Image2DReq {
            extent: Req::Strict(image.extent()),
            format: Req::Strict(image.format()),
            usage: image.usage(),
        }
    }
}

impl target::Target for Image2D {
    type Req = Image2DReq;

    fn allocate(device: &mev::Device, name: &str, req: &Image2DReq) -> Self {
        let dims = req.extent.get();
        let format = req.format.get();

        let image = device.new_image(mev::ImageDesc {
            dims,
            format,
            usage: req.usage,
            levels: 1,
            name,
        });

        Image2D(image)
    }

    fn merge_req(info: &mut Image2DReq, other: &Image2DReq) -> Result<(), MergeError>
    where
        Self: Sized,
    {
        info.extent.merge(&other.extent)?;
        info.format.merge(&other.format)?;
        info.usage |= other.usage;
        Ok(())
    }
}

pub type JobsSet = HashMap<JobId, Box<dyn Job>>;

#[doc(hidden)]
pub mod for_macro {
    pub use arcana_intern::ident;
}
