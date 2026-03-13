use ash::vk;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorResourceKind {
    UniformBuffer,
    StorageBuffer,
    CombinedImageSampler,
    StorageImage,
}

#[derive(Debug, Clone)]
pub struct DescriptorBindingTemplate {
    pub binding: u32,
    pub kind: DescriptorResourceKind,
    pub descriptor_count: u32,
    pub stage_flags: vk::ShaderStageFlags,
}

#[derive(Debug, Clone, Default)]
pub struct DescriptorSetTemplate {
    pub name: String,
    pub bindings: Vec<DescriptorBindingTemplate>,
}
