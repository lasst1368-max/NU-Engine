use ash::vk;

#[derive(Debug, Clone)]
pub struct MaterialTemplate {
    pub name: String,
    pub vertex_shader_path: String,
    pub fragment_shader_path: String,
    pub polygon_mode: vk::PolygonMode,
    pub cull_mode: vk::CullModeFlags,
    pub blend_enabled: bool,
    pub depth_test_enabled: bool,
    pub descriptor_sets: Vec<String>,
}

impl Default for MaterialTemplate {
    fn default() -> Self {
        Self {
            name: "default_lit".to_string(),
            vertex_shader_path: "shaders/default_lit.vert.spv".to_string(),
            fragment_shader_path: "shaders/default_lit.frag.spv".to_string(),
            polygon_mode: vk::PolygonMode::FILL,
            cull_mode: vk::CullModeFlags::BACK,
            blend_enabled: false,
            depth_test_enabled: true,
            descriptor_sets: vec!["frame_globals".to_string(), "material".to_string()],
        }
    }
}
