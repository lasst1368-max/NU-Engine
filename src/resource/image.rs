use ash::vk;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageUsage {
    Texture2D,
    TextureCube,
    RenderTarget,
    DepthStencil,
    Storage,
}

#[derive(Debug, Clone)]
pub struct ImageDesc {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub mip_levels: u32,
    pub format: vk::Format,
    pub usage: ImageUsage,
}
