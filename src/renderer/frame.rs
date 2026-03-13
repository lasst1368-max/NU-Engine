use ash::vk;

use crate::renderer::{MeshDraw, SpriteDraw};

#[derive(Debug, Clone, Copy)]
pub struct FrameContext {
    pub frame_index: u64,
    pub image_index: u32,
    pub viewport: vk::Extent2D,
    pub delta_time_seconds: f32,
}

#[derive(Debug, Clone)]
pub struct FramePacket {
    pub frame_index: u64,
    pub viewport: vk::Extent2D,
    pub sprite_draws: Vec<SpriteDraw>,
    pub mesh_draws: Vec<MeshDraw>,
    pub estimated_gpu_cost: u64,
}
