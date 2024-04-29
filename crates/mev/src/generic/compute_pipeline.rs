use crate::generic::Shader;

pub struct ComputePipelineDesc<'a> {
    pub name: &'a str,
    pub shader: Shader<'a>,
    pub work_group_size: [u32; 3],
}
