mod descriptor;
mod material;

use std::collections::HashMap;

use ash::vk;

pub use descriptor::{DescriptorBindingTemplate, DescriptorResourceKind, DescriptorSetTemplate};
pub use material::MaterialTemplate;

#[derive(Debug, Clone)]
pub struct GraphicsPipelineTemplate {
    pub name: String,
    pub material_name: String,
    pub render_pass_name: String,
    pub topology: vk::PrimitiveTopology,
    pub color_format: vk::Format,
    pub depth_format: Option<vk::Format>,
    pub sample_count: vk::SampleCountFlags,
    pub use_dynamic_viewport: bool,
    pub use_dynamic_scissor: bool,
}

impl Default for GraphicsPipelineTemplate {
    fn default() -> Self {
        Self {
            name: "main_graphics".to_string(),
            material_name: "default_lit".to_string(),
            render_pass_name: "main_3d_pass".to_string(),
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            color_format: vk::Format::B8G8R8A8_UNORM,
            depth_format: Some(vk::Format::D32_SFLOAT),
            sample_count: vk::SampleCountFlags::TYPE_1,
            use_dynamic_viewport: true,
            use_dynamic_scissor: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct PipelineLibrary {
    graphics_templates: HashMap<String, GraphicsPipelineTemplate>,
    material_templates: HashMap<String, MaterialTemplate>,
    descriptor_templates: HashMap<String, DescriptorSetTemplate>,
}

impl PipelineLibrary {
    pub fn register_graphics_pipeline(&mut self, template: GraphicsPipelineTemplate) {
        self.graphics_templates
            .insert(template.name.clone(), template);
    }

    pub fn register_material(&mut self, template: MaterialTemplate) {
        self.material_templates
            .insert(template.name.clone(), template);
    }

    pub fn register_descriptor_set(&mut self, template: DescriptorSetTemplate) {
        self.descriptor_templates
            .insert(template.name.clone(), template);
    }

    pub fn graphics_pipeline(&self, name: &str) -> Option<&GraphicsPipelineTemplate> {
        self.graphics_templates.get(name)
    }

    pub fn material(&self, name: &str) -> Option<&MaterialTemplate> {
        self.material_templates.get(name)
    }

    pub fn descriptor_set(&self, name: &str) -> Option<&DescriptorSetTemplate> {
        self.descriptor_templates.get(name)
    }
}
