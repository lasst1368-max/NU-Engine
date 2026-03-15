pub mod vulkan;

use crate::backend::{ALL_BACKEND_INFO, BackendInfo, GraphicsBackendKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum DriverApi {
    Vulkan = GraphicsBackendKind::Vulkan as u32,
    Dx12 = GraphicsBackendKind::Dx12 as u32,
    Metal = GraphicsBackendKind::Metal as u32,
    Software = 0x100,
}

impl DriverApi {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vulkan => "vulkan",
            Self::Dx12 => "dx12",
            Self::Metal => "metal",
            Self::Software => "software",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AdapterPreference {
    Default = 0,
    HighPerformance = 1,
    LowPower = 2,
    Software = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PresentMode {
    Immediate = 1,
    Fifo = 2,
    Mailbox = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TextureFormat {
    Rgba8Unorm = 1,
    Bgra8Unorm = 2,
    D32Float = 3,
    R32Uint = 4,
    /// 16-bit float per channel — use for HDR render targets and intermediate buffers.
    Rgba16Float = 5,
    /// Single-channel 8-bit — depth/roughness/AO mask textures.
    R8Unorm = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum BufferUsage {
    Vertex = 1,
    Index = 2,
    Uniform = 3,
    Storage = 4,
    TransferSrc = 5,
    TransferDst = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ShaderStage {
    Vertex = 1,
    Fragment = 2,
    Compute = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PrimitiveTopology {
    TriangleList = 1,
    LineList = 2,
    PointList = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VertexFormat {
    Float32x2 = 1,
    Float32x3 = 2,
    Float32x4 = 3,
    Uint32 = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VertexInputRate {
    Vertex = 1,
    Instance = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriverDescriptor {
    pub backend: GraphicsBackendKind,
    pub api: DriverApi,
    pub display_name: &'static str,
    pub dll_name: &'static str,
}

impl DriverDescriptor {
    pub const fn from_backend(info: BackendInfo) -> Self {
        Self {
            backend: info.kind,
            api: match info.kind {
                GraphicsBackendKind::Vulkan => DriverApi::Vulkan,
                GraphicsBackendKind::Dx12 => DriverApi::Dx12,
                GraphicsBackendKind::Metal => DriverApi::Metal,
            },
            display_name: info.display_name,
            dll_name: info.dll_name,
        }
    }
}

pub const DRIVER_CATALOG: [DriverDescriptor; 3] = [
    DriverDescriptor::from_backend(ALL_BACKEND_INFO[0]),
    DriverDescriptor::from_backend(ALL_BACKEND_INFO[1]),
    DriverDescriptor::from_backend(ALL_BACKEND_INFO[2]),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceConfig {
    pub width: u32,
    pub height: u32,
    pub present_mode: PresentMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceRequest {
    pub backend: GraphicsBackendKind,
    pub adapter_preference: AdapterPreference,
    pub enable_validation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferDesc {
    pub size_bytes: u64,
    pub usage: BufferUsage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureDesc {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VertexBindingDesc {
    pub binding: u32,
    pub stride: u32,
    pub input_rate: VertexInputRate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VertexAttributeDesc {
    pub location: u32,
    pub binding: u32,
    pub format: VertexFormat,
    pub offset_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicsPipelineDesc {
    pub color_format: TextureFormat,
    pub depth_format: Option<TextureFormat>,
    pub topology: PrimitiveTopology,
    pub vertex_spirv: Vec<u32>,
    pub fragment_spirv: Vec<u32>,
    pub vertex_bindings: Vec<VertexBindingDesc>,
    pub vertex_attributes: Vec<VertexAttributeDesc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferInfo {
    pub size_bytes: u64,
    pub usage: BufferUsage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureInfo {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsPipelineInfo {
    pub color_format: TextureFormat,
    pub depth_format: Option<TextureFormat>,
    pub topology: PrimitiveTopology,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverError {
    pub reason: String,
}

pub trait Driver {
    fn descriptor(&self) -> DriverDescriptor;
    fn create_device(&self, request: &DeviceRequest) -> Result<Box<dyn DriverDevice>, DriverError>;
}

pub trait DriverDevice {
    fn backend(&self) -> GraphicsBackendKind;
    fn label(&self) -> &str;
    fn create_surface(&self, config: SurfaceConfig) -> Result<Box<dyn DriverSurface>, DriverError>;
    fn create_command_recorder(&self) -> Box<dyn DriverCommandRecorder>;
    fn create_buffer(&self, desc: BufferDesc) -> Result<Box<dyn DriverBuffer>, DriverError>;
    fn create_texture(&self, desc: TextureDesc) -> Result<Box<dyn DriverTexture>, DriverError>;
    fn create_graphics_pipeline(
        &self,
        desc: GraphicsPipelineDesc,
    ) -> Result<Box<dyn DriverGraphicsPipeline>, DriverError>;
}

pub trait DriverSurface {
    fn config(&self) -> SurfaceConfig;
}

pub trait DriverBuffer {
    fn backend(&self) -> GraphicsBackendKind;
    fn info(&self) -> BufferInfo;
}

pub trait DriverTexture {
    fn backend(&self) -> GraphicsBackendKind;
    fn info(&self) -> TextureInfo;
}

pub trait DriverGraphicsPipeline {
    fn backend(&self) -> GraphicsBackendKind;
    fn info(&self) -> GraphicsPipelineInfo;
}

pub trait DriverCommandRecorder {
    fn begin_frame(&mut self);
    fn end_frame(&mut self);
    fn command_count(&self) -> u64;
}

pub const fn driver_catalog() -> &'static [DriverDescriptor] {
    &DRIVER_CATALOG
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_catalog_covers_all_known_backends() {
        assert_eq!(driver_catalog().len(), 3);
        assert_eq!(driver_catalog()[0].backend, GraphicsBackendKind::Vulkan);
        assert_eq!(driver_catalog()[1].backend, GraphicsBackendKind::Dx12);
        assert_eq!(driver_catalog()[2].backend, GraphicsBackendKind::Metal);
    }
}
