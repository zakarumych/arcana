use super::render::RenderId;

struct RenderTargetVersion {
    target_for: RenderId,
    waits: nix::PipelineStages,
    writes: nix::PipelineStages,
    reads: nix::PipelineStages,
}

pub struct RenderTarget {
    name: Box<str>,
    versions: Vec<RenderTargetVersion>,
}

impl RenderTarget {
    pub fn new(name: Box<str>, target_for: RenderId, stages: nix::PipelineStages) -> Self {
        RenderTarget {
            name,
            versions: vec![RenderTargetVersion {
                waits: nix::PipelineStages::empty(),
                writes: stages,
                reads: nix::PipelineStages::empty(),
                target_for,
            }],
        }
    }

    pub fn versions(&self) -> usize {
        self.versions.len()
    }

    pub fn read(&mut self, version: usize, stages: nix::PipelineStages) {
        assert_eq!(self.versions.len(), version + 1);
        let last = self.versions.last_mut().unwrap();
        last.reads |= stages;
    }

    pub fn write(&mut self, version: usize, target_for: RenderId, stages: nix::PipelineStages) {
        assert_eq!(self.versions.len(), version + 1);
        assert!(self.versions.last().unwrap().reads.is_empty());
        self.versions.push(RenderTargetVersion {
            waits: nix::PipelineStages::empty(),
            writes: stages,
            reads: nix::PipelineStages::empty(),
            target_for,
        });
    }

    pub fn waits(&self, version: usize) -> nix::PipelineStages {
        self.versions[version].waits
    }

    pub fn writes(&self, version: usize) -> nix::PipelineStages {
        self.versions[version].writes
    }

    pub fn reads(&self, version: usize) -> nix::PipelineStages {
        self.versions[version].reads
    }

    pub fn target_for(&self, version: usize) -> RenderId {
        self.versions[version].target_for
    }
}
