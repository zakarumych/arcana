pub struct Sampler {
    sampler: metal::SamplerState,
}

impl Sampler {
    pub(super) fn new(sampler: metal::SamplerState) -> Self {
        Self { sampler }
    }
}
