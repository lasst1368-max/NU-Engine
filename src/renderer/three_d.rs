use ash::vk;

use crate::resource::{BufferHandle, ImageHandle};

#[derive(Debug, Clone)]
pub struct MeshDraw {
    pub vertex_buffer: Option<BufferHandle>,
    pub index_buffer: Option<BufferHandle>,
    pub material_name: String,
    pub albedo_texture: Option<ImageHandle>,
    pub index_count: u32,
    pub instance_count: u32,
    pub model_matrix: [[f32; 4]; 4],
    pub cull_mode: vk::CullModeFlags,
}

impl Default for MeshDraw {
    fn default() -> Self {
        Self {
            vertex_buffer: None,
            index_buffer: None,
            material_name: "default_lit".to_string(),
            albedo_texture: None,
            index_count: 36,
            instance_count: 1,
            model_matrix: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            cull_mode: vk::CullModeFlags::BACK,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Renderer3D {
    pub pass_name: String,
    pending: Vec<MeshDraw>,
}

impl Default for Renderer3D {
    fn default() -> Self {
        Self {
            pass_name: "main_3d_pass".to_string(),
            pending: Vec::new(),
        }
    }
}

impl Renderer3D {
    pub fn queue_mesh(&mut self, draw: MeshDraw) {
        self.pending.push(draw);
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn clear(&mut self) {
        self.pending.clear();
    }

    pub fn drain(&mut self) -> Vec<MeshDraw> {
        std::mem::take(&mut self.pending)
    }

    pub fn estimate_gpu_cost(&self, draw_count: u32) -> u64 {
        u64::from(draw_count) * 110
    }
}
