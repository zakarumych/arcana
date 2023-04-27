use crate::backend::{Buffer, CommandEncoder, Device, Image, Sampler};

use super::ShaderStages;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArgumentKind {
    Constant,
    UniformBuffer,
    StorageBuffer,
    SampledImage,
    StorageImage,
    Sampler,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Argument {
    pub kind: ArgumentKind,
    pub size: u32,
    pub stages: ShaderStages,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArgumentGroup<'a> {
    pub arguments: &'a [Argument],
}

#[derive(Clone, Copy)]
pub enum WriteArgument<'a> {
    Const(&'a [u8]),
    Buffer(&'a [Buffer]),
    Image(&'a [Image]),
    Sampler(&'a [Sampler]),
}

pub trait Arguments {
    type Cache;

    fn bind(&self, cache: &mut Self::Cache, encoder: &mut CommandEncoder);
}
