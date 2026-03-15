mod asset;
mod buffer;
mod image;

use std::collections::HashMap;

pub use asset::{AssetHandle, AssetKind, AssetManager, AssetRecord, AssetState};
pub use buffer::{BufferDesc, BufferHandle, BufferUsage};
pub use image::{ImageDesc, ImageHandle, ImageUsage};

#[derive(Debug, Default)]
pub struct ResourceRegistry {
    next_buffer_id: u32,
    next_image_id: u32,
    buffers: HashMap<BufferHandle, BufferDesc>,
    images: HashMap<ImageHandle, ImageDesc>,
}

impl ResourceRegistry {
    pub fn create_buffer(&mut self, desc: BufferDesc) -> BufferHandle {
        self.next_buffer_id = self.next_buffer_id.wrapping_add(1);
        let handle = BufferHandle(self.next_buffer_id);
        self.buffers.insert(handle, desc);
        handle
    }

    pub fn create_image(&mut self, desc: ImageDesc) -> ImageHandle {
        self.next_image_id = self.next_image_id.wrapping_add(1);
        let handle = ImageHandle(self.next_image_id);
        self.images.insert(handle, desc);
        handle
    }

    pub fn get_buffer(&self, handle: BufferHandle) -> Option<&BufferDesc> {
        self.buffers.get(&handle)
    }

    pub fn get_image(&self, handle: ImageHandle) -> Option<&ImageDesc> {
        self.images.get(&handle)
    }

    pub fn destroy_buffer(&mut self, handle: BufferHandle) -> Option<BufferDesc> {
        self.buffers.remove(&handle)
    }

    pub fn destroy_image(&mut self, handle: ImageHandle) -> Option<ImageDesc> {
        self.images.remove(&handle)
    }
}
