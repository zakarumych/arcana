pub struct Sampler {
    sampler: metal::SamplerState,
}

#[repr(transparent)]
pub struct SamplerId(u64);

impl crate::private::Sealed for SamplerId {}
impl crate::traits::Argument for SamplerId {
    const KIND: crate::generic::ArgumentKind = crate::generic::ArgumentKind::Sampler;
}

impl<const N: usize> crate::private::Sealed for [SamplerId; N] {}
impl<const N: usize> crate::traits::Argument for [SamplerId; N] {
    const KIND: crate::generic::ArgumentKind = crate::generic::ArgumentKind::Sampler;
}

impl crate::private::Sealed for [SamplerId] {}
impl crate::traits::Argument for [SamplerId] {
    const KIND: crate::generic::ArgumentKind = crate::generic::ArgumentKind::Sampler;
}
