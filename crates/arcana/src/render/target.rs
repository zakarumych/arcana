use std::marker::PhantomData;

use super::render::RenderId;

struct RenderTargetVersion {
    target_for: RenderId,
    waits: mev::PipelineStages,
    writes: mev::PipelineStages,
    reads: mev::PipelineStages,
}

pub struct RenderTarget<T: ?Sized> {
    name: Box<str>,
    versions: Vec<RenderTargetVersion>,
    marker: PhantomData<fn() -> T>,
}

impl<T> RenderTarget<T> {
    pub fn new(name: Box<str>, target_for: RenderId, stages: mev::PipelineStages) -> Self {
        RenderTarget {
            name,
            versions: vec![RenderTargetVersion {
                waits: mev::PipelineStages::empty(),
                writes: stages,
                reads: mev::PipelineStages::empty(),
                target_for,
            }],
            marker: PhantomData,
        }
    }

    pub fn versions(&self) -> usize {
        self.versions.len()
    }

    pub fn read(&mut self, version: usize, stages: mev::PipelineStages) {
        assert_eq!(self.versions.len(), version + 1);
        let last = self.versions.last_mut().unwrap();
        last.reads |= stages;
    }

    pub fn write(&mut self, version: usize, target_for: RenderId, stages: mev::PipelineStages) {
        assert_eq!(self.versions.len(), version + 1);
        assert!(self.versions.last().unwrap().reads.is_empty());
        self.versions.push(RenderTargetVersion {
            waits: mev::PipelineStages::empty(),
            writes: stages,
            reads: mev::PipelineStages::empty(),
            target_for,
        });
    }

    pub fn waits(&self, version: usize) -> mev::PipelineStages {
        self.versions[version].waits
    }

    pub fn writes(&self, version: usize) -> mev::PipelineStages {
        self.versions[version].writes
    }

    pub fn reads(&self, version: usize) -> mev::PipelineStages {
        self.versions[version].reads
    }

    pub fn target_for(&self, version: usize) -> RenderId {
        self.versions[version].target_for
    }
}
