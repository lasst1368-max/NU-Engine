use ash::vk;
use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct GpuFeatureFlags: u64 {
        const SAMPLER_ANISOTROPY = 1 << 0;
        const DESCRIPTOR_INDEXING = 1 << 1;
        const DYNAMIC_RENDERING = 1 << 2;
        const MULTI_DRAW_INDIRECT = 1 << 3;
        const MESH_SHADING = 1 << 4;
        const RAY_QUERY = 1 << 5;
        const TIMESTAMP_QUERY = 1 << 6;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineMode {
    TwoD,
    ThreeD,
    Hybrid,
}

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub application_name: String,
    pub engine_name: String,
    pub mode: EngineMode,
    pub frames_in_flight: u32,
    pub preferred_color_format: vk::Format,
    pub preferred_depth_format: vk::Format,
    pub preferred_present_mode: vk::PresentModeKHR,
    pub required_features: GpuFeatureFlags,
    pub enable_validation: bool,
    pub enable_gpu_timestamps: bool,
    pub max_bindless_textures: u32,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            application_name: "nu App".to_string(),
            engine_name: "nu".to_string(),
            mode: EngineMode::Hybrid,
            frames_in_flight: 2,
            preferred_color_format: vk::Format::B8G8R8A8_UNORM,
            preferred_depth_format: vk::Format::D32_SFLOAT,
            preferred_present_mode: vk::PresentModeKHR::MAILBOX,
            required_features: GpuFeatureFlags::SAMPLER_ANISOTROPY
                | GpuFeatureFlags::DYNAMIC_RENDERING
                | GpuFeatureFlags::MULTI_DRAW_INDIRECT,
            enable_validation: true,
            enable_gpu_timestamps: true,
            max_bindless_textures: 4096,
        }
    }
}
