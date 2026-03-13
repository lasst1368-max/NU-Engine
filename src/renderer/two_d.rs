use ash::vk;

use crate::resource::{BufferHandle, ImageHandle};

#[derive(Debug, Clone)]
pub struct SpriteDraw {
    pub vertex_buffer: Option<BufferHandle>,
    pub index_buffer: Option<BufferHandle>,
    pub texture: Option<ImageHandle>,
    pub index_count: u32,
    pub instance_count: u32,
    pub transform: [[f32; 4]; 4],
    pub tint_rgba: [f32; 4],
    pub scissor: Option<vk::Rect2D>,
}

impl Default for SpriteDraw {
    fn default() -> Self {
        Self {
            vertex_buffer: None,
            index_buffer: None,
            texture: None,
            index_count: 6,
            instance_count: 1,
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            tint_rgba: [1.0, 1.0, 1.0, 1.0],
            scissor: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Renderer2D {
    pub pass_name: String,
    pending: Vec<SpriteDraw>,
}

impl Default for Renderer2D {
    fn default() -> Self {
        Self {
            pass_name: "main_2d_pass".to_string(),
            pending: Vec::new(),
        }
    }
}

impl Renderer2D {
    pub fn queue_sprite(&mut self, draw: SpriteDraw) {
        self.pending.push(draw);
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn clear(&mut self) {
        self.pending.clear();
    }

    pub fn drain(&mut self) -> Vec<SpriteDraw> {
        std::mem::take(&mut self.pending)
    }

    pub fn estimate_gpu_cost(&self, draw_count: u32) -> u64 {
        u64::from(draw_count) * 40
    }
}
