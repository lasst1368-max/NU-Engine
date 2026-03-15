use crate::backend::{GraphicsBackendKind, VULKAN_BACKEND_INFO};
use crate::core::{ApiConfig, ApiError, VulkanContext, VulkanContextBuilder};
use ash::vk;
use ash::{Device, Instance};

use super::{
    BufferDesc, BufferInfo, DeviceRequest, Driver, DriverBuffer, DriverCommandRecorder,
    DriverDescriptor, DriverDevice, DriverError, DriverGraphicsPipeline, DriverSurface,
    DriverTexture, GraphicsPipelineDesc, GraphicsPipelineInfo, SurfaceConfig, TextureDesc,
    TextureInfo,
};

#[derive(Debug, Clone)]
pub struct VulkanRhiDriver {
    config: ApiConfig,
}

impl VulkanRhiDriver {
    pub fn new(config: ApiConfig) -> Self {
        Self { config }
    }
}

pub struct VulkanRhiDevice {
    context: VulkanContext,
    label: String,
}

impl VulkanRhiDevice {
    pub fn context(&self) -> &VulkanContext {
        &self.context
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VulkanRhiSurface {
    config: SurfaceConfig,
}

pub struct VulkanRhiBuffer {
    info: BufferInfo,
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    device: Device,
}

pub struct VulkanRhiTexture {
    info: TextureInfo,
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
    device: Device,
}

pub struct VulkanRhiGraphicsPipeline {
    info: GraphicsPipelineInfo,
    render_pass: vk::RenderPass,
    layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    device: Device,
}

#[derive(Default)]
pub struct VulkanRhiCommandRecorder {
    frame_open: bool,
    command_count: u64,
}

impl Driver for VulkanRhiDriver {
    fn descriptor(&self) -> DriverDescriptor {
        DriverDescriptor::from_backend(VULKAN_BACKEND_INFO)
    }

    fn create_device(&self, request: &DeviceRequest) -> Result<Box<dyn DriverDevice>, DriverError> {
        if request.backend != GraphicsBackendKind::Vulkan {
            return Err(DriverError {
                reason: format!(
                    "VulkanRhiDriver cannot create backend {:?}",
                    request.backend
                ),
            });
        }

        let mut config = self.config.clone();
        config.enable_validation = request.enable_validation;
        let context = VulkanContextBuilder::new(config.clone())
            .build_headless()
            .map_err(api_error_to_driver_error)?;
        Ok(Box::new(VulkanRhiDevice {
            context,
            label: config.application_name,
        }))
    }
}

impl DriverDevice for VulkanRhiDevice {
    fn backend(&self) -> GraphicsBackendKind {
        GraphicsBackendKind::Vulkan
    }

    fn label(&self) -> &str {
        &self.label
    }

    fn create_surface(&self, config: SurfaceConfig) -> Result<Box<dyn DriverSurface>, DriverError> {
        Ok(Box::new(VulkanRhiSurface { config }))
    }

    fn create_command_recorder(&self) -> Box<dyn DriverCommandRecorder> {
        Box::new(VulkanRhiCommandRecorder::default())
    }

    fn create_buffer(&self, desc: BufferDesc) -> Result<Box<dyn DriverBuffer>, DriverError> {
        let instance = self
            .context
            .handles()
            .instance
            .as_ref()
            .ok_or_else(|| DriverError {
                reason: "Vulkan headless instance is not initialized".to_string(),
            })?;
        let device = self
            .context
            .handles()
            .device
            .as_ref()
            .ok_or_else(|| DriverError {
                reason: "Vulkan logical device is not initialized".to_string(),
            })?;
        let physical_device =
            self.context
                .handles()
                .physical_device
                .ok_or_else(|| DriverError {
                    reason: "Vulkan physical device is not initialized".to_string(),
                })?;
        let (buffer, memory) = create_vulkan_buffer(
            instance,
            device,
            physical_device,
            desc.size_bytes.max(1),
            map_buffer_usage(desc.usage),
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;
        Ok(Box::new(VulkanRhiBuffer {
            info: BufferInfo {
                size_bytes: desc.size_bytes,
                usage: desc.usage,
            },
            buffer,
            memory,
            device: device.clone(),
        }))
    }

    fn create_texture(&self, desc: TextureDesc) -> Result<Box<dyn DriverTexture>, DriverError> {
        let instance = self
            .context
            .handles()
            .instance
            .as_ref()
            .ok_or_else(|| DriverError {
                reason: "Vulkan headless instance is not initialized".to_string(),
            })?;
        let device = self
            .context
            .handles()
            .device
            .as_ref()
            .ok_or_else(|| DriverError {
                reason: "Vulkan logical device is not initialized".to_string(),
            })?;
        let physical_device =
            self.context
                .handles()
                .physical_device
                .ok_or_else(|| DriverError {
                    reason: "Vulkan physical device is not initialized".to_string(),
                })?;
        let (image, memory, view) = create_vulkan_texture(instance, device, physical_device, desc)?;
        Ok(Box::new(VulkanRhiTexture {
            info: TextureInfo {
                width: desc.width,
                height: desc.height,
                format: desc.format,
            },
            image,
            memory,
            view,
            device: device.clone(),
        }))
    }

    fn create_graphics_pipeline(
        &self,
        desc: GraphicsPipelineDesc,
    ) -> Result<Box<dyn DriverGraphicsPipeline>, DriverError> {
        let device = self
            .context
            .handles()
            .device
            .as_ref()
            .ok_or_else(|| DriverError {
                reason: "Vulkan logical device is not initialized".to_string(),
            })?;
        let render_pass = create_rhi_render_pass(&device.clone(), &desc)?;
        let layout_info = vk::PipelineLayoutCreateInfo::default();
        let layout = match vk_result(
            unsafe { device.create_pipeline_layout(&layout_info, None) },
            "create_pipeline_layout(rhi)",
        ) {
            Ok(layout) => layout,
            Err(err) => {
                unsafe {
                    device.destroy_render_pass(render_pass, None);
                }
                return Err(err);
            }
        };
        let pipeline =
            match create_rhi_graphics_pipeline(&device.clone(), render_pass, layout, &desc) {
                Ok(pipeline) => pipeline,
                Err(err) => {
                    unsafe {
                        device.destroy_pipeline_layout(layout, None);
                        device.destroy_render_pass(render_pass, None);
                    }
                    return Err(err);
                }
            };
        Ok(Box::new(VulkanRhiGraphicsPipeline {
            info: GraphicsPipelineInfo {
                color_format: desc.color_format,
                depth_format: desc.depth_format,
                topology: desc.topology,
            },
            render_pass,
            layout,
            pipeline,
            device: device.clone(),
        }))
    }
}

impl DriverSurface for VulkanRhiSurface {
    fn config(&self) -> SurfaceConfig {
        self.config
    }
}

impl DriverBuffer for VulkanRhiBuffer {
    fn backend(&self) -> GraphicsBackendKind {
        GraphicsBackendKind::Vulkan
    }

    fn info(&self) -> BufferInfo {
        self.info
    }
}

impl Drop for VulkanRhiBuffer {
    fn drop(&mut self) {
        unsafe {
            if self.buffer != vk::Buffer::null() {
                self.device.destroy_buffer(self.buffer, None);
            }
            if self.memory != vk::DeviceMemory::null() {
                self.device.free_memory(self.memory, None);
            }
        }
    }
}

impl DriverTexture for VulkanRhiTexture {
    fn backend(&self) -> GraphicsBackendKind {
        GraphicsBackendKind::Vulkan
    }

    fn info(&self) -> TextureInfo {
        self.info
    }
}

impl Drop for VulkanRhiTexture {
    fn drop(&mut self) {
        unsafe {
            if self.view != vk::ImageView::null() {
                self.device.destroy_image_view(self.view, None);
            }
            if self.image != vk::Image::null() {
                self.device.destroy_image(self.image, None);
            }
            if self.memory != vk::DeviceMemory::null() {
                self.device.free_memory(self.memory, None);
            }
        }
    }
}

impl DriverGraphicsPipeline for VulkanRhiGraphicsPipeline {
    fn backend(&self) -> GraphicsBackendKind {
        GraphicsBackendKind::Vulkan
    }

    fn info(&self) -> GraphicsPipelineInfo {
        self.info
    }
}

impl Drop for VulkanRhiGraphicsPipeline {
    fn drop(&mut self) {
        unsafe {
            if self.pipeline != vk::Pipeline::null() {
                self.device.destroy_pipeline(self.pipeline, None);
            }
            if self.layout != vk::PipelineLayout::null() {
                self.device.destroy_pipeline_layout(self.layout, None);
            }
            if self.render_pass != vk::RenderPass::null() {
                self.device.destroy_render_pass(self.render_pass, None);
            }
        }
    }
}

impl DriverCommandRecorder for VulkanRhiCommandRecorder {
    fn begin_frame(&mut self) {
        self.frame_open = true;
        self.command_count = self.command_count.saturating_add(1);
    }

    fn end_frame(&mut self) {
        self.frame_open = false;
    }

    fn command_count(&self) -> u64 {
        self.command_count
    }
}

fn api_error_to_driver_error(err: ApiError) -> DriverError {
    DriverError {
        reason: err.to_string(),
    }
}

fn map_buffer_usage(usage: crate::rhi::BufferUsage) -> vk::BufferUsageFlags {
    match usage {
        crate::rhi::BufferUsage::Vertex => vk::BufferUsageFlags::VERTEX_BUFFER,
        crate::rhi::BufferUsage::Index => vk::BufferUsageFlags::INDEX_BUFFER,
        crate::rhi::BufferUsage::Uniform => vk::BufferUsageFlags::UNIFORM_BUFFER,
        crate::rhi::BufferUsage::Storage => vk::BufferUsageFlags::STORAGE_BUFFER,
        crate::rhi::BufferUsage::TransferSrc => vk::BufferUsageFlags::TRANSFER_SRC,
        crate::rhi::BufferUsage::TransferDst => vk::BufferUsageFlags::TRANSFER_DST,
    }
}

fn map_texture_format(format: crate::rhi::TextureFormat) -> vk::Format {
    match format {
        crate::rhi::TextureFormat::Rgba8Unorm => vk::Format::R8G8B8A8_UNORM,
        crate::rhi::TextureFormat::Bgra8Unorm => vk::Format::B8G8R8A8_UNORM,
        crate::rhi::TextureFormat::D32Float => vk::Format::D32_SFLOAT,
        crate::rhi::TextureFormat::R32Uint => vk::Format::R32_UINT,
        crate::rhi::TextureFormat::Rgba16Float => vk::Format::R16G16B16A16_SFLOAT,
        crate::rhi::TextureFormat::R8Unorm => vk::Format::R8_UNORM,
    }
}

fn map_topology(topology: crate::rhi::PrimitiveTopology) -> vk::PrimitiveTopology {
    match topology {
        crate::rhi::PrimitiveTopology::TriangleList => vk::PrimitiveTopology::TRIANGLE_LIST,
        crate::rhi::PrimitiveTopology::LineList => vk::PrimitiveTopology::LINE_LIST,
        crate::rhi::PrimitiveTopology::PointList => vk::PrimitiveTopology::POINT_LIST,
    }
}

fn map_vertex_format(format: crate::rhi::VertexFormat) -> vk::Format {
    match format {
        crate::rhi::VertexFormat::Float32x2 => vk::Format::R32G32_SFLOAT,
        crate::rhi::VertexFormat::Float32x3 => vk::Format::R32G32B32_SFLOAT,
        crate::rhi::VertexFormat::Float32x4 => vk::Format::R32G32B32A32_SFLOAT,
        crate::rhi::VertexFormat::Uint32 => vk::Format::R32_UINT,
    }
}

fn map_vertex_input_rate(rate: crate::rhi::VertexInputRate) -> vk::VertexInputRate {
    match rate {
        crate::rhi::VertexInputRate::Vertex => vk::VertexInputRate::VERTEX,
        crate::rhi::VertexInputRate::Instance => vk::VertexInputRate::INSTANCE,
    }
}

fn create_rhi_render_pass(
    device: &Device,
    desc: &GraphicsPipelineDesc,
) -> Result<vk::RenderPass, DriverError> {
    let color_attachment = vk::AttachmentDescription::default()
        .format(map_texture_format(desc.color_format))
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
    let color_ref = [vk::AttachmentReference::default()
        .attachment(0)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];

    let mut attachments = vec![color_attachment];
    let depth_ref = desc.depth_format.map(|format| {
        attachments.push(
            vk::AttachmentDescription::default()
                .format(map_texture_format(format))
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL),
        );
        vk::AttachmentReference::default()
            .attachment(1)
            .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
    });

    let mut subpass = vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_ref);
    if let Some(ref depth_ref) = depth_ref {
        subpass = subpass.depth_stencil_attachment(depth_ref);
    }
    let subpasses = [subpass];
    let render_pass_info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments)
        .subpasses(&subpasses);
    vk_result(
        unsafe { device.create_render_pass(&render_pass_info, None) },
        "create_render_pass(rhi)",
    )
}

fn create_rhi_graphics_pipeline(
    device: &Device,
    render_pass: vk::RenderPass,
    layout: vk::PipelineLayout,
    desc: &GraphicsPipelineDesc,
) -> Result<vk::Pipeline, DriverError> {
    if desc.vertex_spirv.is_empty() || desc.fragment_spirv.is_empty() {
        return Err(DriverError {
            reason: "graphics pipeline creation requires vertex and fragment SPIR-V".to_string(),
        });
    }
    let vert_module = create_shader_module(device, &desc.vertex_spirv)?;
    let frag_module = create_shader_module(device, &desc.fragment_spirv)?;
    let main = c"main";
    let shader_stages = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vert_module)
            .name(main),
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(frag_module)
            .name(main),
    ];
    let vertex_bindings: Vec<_> = desc
        .vertex_bindings
        .iter()
        .map(|binding| {
            vk::VertexInputBindingDescription::default()
                .binding(binding.binding)
                .stride(binding.stride)
                .input_rate(map_vertex_input_rate(binding.input_rate))
        })
        .collect();
    let vertex_attributes: Vec<_> = desc
        .vertex_attributes
        .iter()
        .map(|attribute| {
            vk::VertexInputAttributeDescription::default()
                .location(attribute.location)
                .binding(attribute.binding)
                .format(map_vertex_format(attribute.format))
                .offset(attribute.offset_bytes)
        })
        .collect();
    let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&vertex_bindings)
        .vertex_attribute_descriptions(&vertex_attributes);
    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(map_topology(desc.topology))
        .primitive_restart_enable(false);
    let viewport_state = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);
    let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
        .depth_clamp_enable(false)
        .rasterizer_discard_enable(false)
        .polygon_mode(vk::PolygonMode::FILL)
        .cull_mode(vk::CullModeFlags::BACK)
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .line_width(1.0);
    let multisample = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1)
        .sample_shading_enable(false);
    let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(desc.depth_format.is_some())
        .depth_write_enable(desc.depth_format.is_some())
        .depth_compare_op(vk::CompareOp::LESS)
        .stencil_test_enable(false);
    let color_blend_attachment = [vk::PipelineColorBlendAttachmentState::default()
        .blend_enable(false)
        .color_write_mask(
            vk::ColorComponentFlags::R
                | vk::ColorComponentFlags::G
                | vk::ColorComponentFlags::B
                | vk::ColorComponentFlags::A,
        )];
    let color_blend =
        vk::PipelineColorBlendStateCreateInfo::default().attachments(&color_blend_attachment);
    let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic_state =
        vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
    let pipeline_info = [vk::GraphicsPipelineCreateInfo::default()
        .stages(&shader_stages)
        .vertex_input_state(&vertex_input)
        .input_assembly_state(&input_assembly)
        .viewport_state(&viewport_state)
        .rasterization_state(&rasterizer)
        .multisample_state(&multisample)
        .depth_stencil_state(&depth_stencil)
        .color_blend_state(&color_blend)
        .dynamic_state(&dynamic_state)
        .layout(layout)
        .render_pass(render_pass)
        .subpass(0)];
    let pipeline = match unsafe {
        device.create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_info, None)
    } {
        Ok(mut pipelines) => pipelines.remove(0),
        Err((_, result)) => {
            unsafe {
                device.destroy_shader_module(vert_module, None);
                device.destroy_shader_module(frag_module, None);
            }
            return Err(DriverError {
                reason: format!("vulkan error during create_graphics_pipelines(rhi): {result:?}"),
            });
        }
    };
    unsafe {
        device.destroy_shader_module(vert_module, None);
        device.destroy_shader_module(frag_module, None);
    }
    Ok(pipeline)
}

fn create_shader_module(
    device: &Device,
    spirv_words: &[u32],
) -> Result<vk::ShaderModule, DriverError> {
    let info = vk::ShaderModuleCreateInfo::default().code(spirv_words);
    vk_result(
        unsafe { device.create_shader_module(&info, None) },
        "create_shader_module(rhi)",
    )
}

fn create_vulkan_buffer(
    instance: &Instance,
    device: &Device,
    physical_device: vk::PhysicalDevice,
    size: u64,
    usage: vk::BufferUsageFlags,
    memory_properties: vk::MemoryPropertyFlags,
) -> Result<(vk::Buffer, vk::DeviceMemory), DriverError> {
    let buffer_info = vk::BufferCreateInfo::default()
        .size(size)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    let buffer = vk_result(
        unsafe { device.create_buffer(&buffer_info, None) },
        "create_buffer(rhi)",
    )?;
    let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
    let memory_type_index = find_memory_type(
        instance,
        physical_device,
        requirements.memory_type_bits,
        memory_properties,
    )?;
    let allocation = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(memory_type_index);
    let memory = match vk_result(
        unsafe { device.allocate_memory(&allocation, None) },
        "allocate_memory(buffer_rhi)",
    ) {
        Ok(memory) => memory,
        Err(err) => {
            unsafe {
                device.destroy_buffer(buffer, None);
            }
            return Err(err);
        }
    };
    if let Err(err) = vk_result(
        unsafe { device.bind_buffer_memory(buffer, memory, 0) },
        "bind_buffer_memory(rhi)",
    ) {
        unsafe {
            device.free_memory(memory, None);
            device.destroy_buffer(buffer, None);
        }
        return Err(err);
    }
    Ok((buffer, memory))
}

fn create_vulkan_texture(
    instance: &Instance,
    device: &Device,
    physical_device: vk::PhysicalDevice,
    desc: TextureDesc,
) -> Result<(vk::Image, vk::DeviceMemory, vk::ImageView), DriverError> {
    let format = map_texture_format(desc.format);
    let is_depth = desc.format == crate::rhi::TextureFormat::D32Float;
    let is_hdr_rt = desc.format == crate::rhi::TextureFormat::Rgba16Float;
    let aspect_mask = if is_depth {
        vk::ImageAspectFlags::DEPTH
    } else {
        vk::ImageAspectFlags::COLOR
    };
    let usage = if is_depth {
        vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED
    } else if is_hdr_rt {
        // HDR render targets need to be written as color attachments and sampled.
        vk::ImageUsageFlags::COLOR_ATTACHMENT
            | vk::ImageUsageFlags::SAMPLED
            | vk::ImageUsageFlags::TRANSFER_DST
    } else {
        vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST
    };
    let image_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(format)
        .extent(vk::Extent3D {
            width: desc.width.max(1),
            height: desc.height.max(1),
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED);
    let image = vk_result(
        unsafe { device.create_image(&image_info, None) },
        "create_image(rhi)",
    )?;
    let requirements = unsafe { device.get_image_memory_requirements(image) };
    let memory_type_index = find_memory_type(
        instance,
        physical_device,
        requirements.memory_type_bits,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )?;
    let allocation = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(memory_type_index);
    let memory = match vk_result(
        unsafe { device.allocate_memory(&allocation, None) },
        "allocate_memory(texture_rhi)",
    ) {
        Ok(memory) => memory,
        Err(err) => {
            unsafe {
                device.destroy_image(image, None);
            }
            return Err(err);
        }
    };
    if let Err(err) = vk_result(
        unsafe { device.bind_image_memory(image, memory, 0) },
        "bind_image_memory(rhi)",
    ) {
        unsafe {
            device.free_memory(memory, None);
            device.destroy_image(image, None);
        }
        return Err(err);
    }
    let view_info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(format)
        .subresource_range(
            vk::ImageSubresourceRange::default()
                .aspect_mask(aspect_mask)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1),
        );
    let view = match vk_result(
        unsafe { device.create_image_view(&view_info, None) },
        "create_image_view(rhi)",
    ) {
        Ok(view) => view,
        Err(err) => {
            unsafe {
                device.free_memory(memory, None);
                device.destroy_image(image, None);
            }
            return Err(err);
        }
    };
    Ok((image, memory, view))
}

fn find_memory_type(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    type_filter: u32,
    required_properties: vk::MemoryPropertyFlags,
) -> Result<u32, DriverError> {
    let memory_properties =
        unsafe { instance.get_physical_device_memory_properties(physical_device) };
    for index in 0..memory_properties.memory_type_count {
        let memory_type = memory_properties.memory_types[index as usize];
        let supported = (type_filter & (1 << index)) != 0;
        if supported && memory_type.property_flags.contains(required_properties) {
            return Ok(index);
        }
    }
    Err(DriverError {
        reason: format!(
            "no compatible Vulkan memory type found for properties {:?}",
            required_properties
        ),
    })
}

fn vk_result<T>(result: Result<T, vk::Result>, context: &'static str) -> Result<T, DriverError> {
    result.map_err(|result| DriverError {
        reason: format!("vulkan error during {context}: {result:?}"),
    })
}

#[cfg(test)]
mod tests {
    use crate::backend::GraphicsBackendKind;
    use crate::core::ApiConfig;

    use super::*;

    #[test]
    fn stub_vulkan_driver_creates_device_surface_and_recorder() {
        let driver = VulkanRhiDriver::new(ApiConfig::default());
        let request = DeviceRequest {
            backend: GraphicsBackendKind::Vulkan,
            adapter_preference: crate::rhi::AdapterPreference::Default,
            enable_validation: false,
        };
        let device = driver.create_device(&request).expect("vulkan device");
        assert_eq!(device.backend(), GraphicsBackendKind::Vulkan);
        let surface = device
            .create_surface(SurfaceConfig {
                width: 1280,
                height: 720,
                present_mode: crate::rhi::PresentMode::Fifo,
            })
            .expect("surface");
        assert_eq!(surface.config().width, 1280);
        let mut recorder = device.create_command_recorder();
        recorder.begin_frame();
        recorder.end_frame();
        assert_eq!(recorder.command_count(), 1);
        let buffer = device
            .create_buffer(BufferDesc {
                size_bytes: 4096,
                usage: crate::rhi::BufferUsage::Vertex,
            })
            .expect("buffer");
        assert_eq!(buffer.info().size_bytes, 4096);
        let texture = device
            .create_texture(TextureDesc {
                width: 512,
                height: 512,
                format: crate::rhi::TextureFormat::Rgba8Unorm,
            })
            .expect("texture");
        assert_eq!(texture.info().width, 512);
        let pipeline_error = device
            .create_graphics_pipeline(GraphicsPipelineDesc {
                color_format: crate::rhi::TextureFormat::Bgra8Unorm,
                depth_format: Some(crate::rhi::TextureFormat::D32Float),
                topology: crate::rhi::PrimitiveTopology::TriangleList,
                vertex_spirv: Vec::new(),
                fragment_spirv: Vec::new(),
                vertex_bindings: Vec::new(),
                vertex_attributes: Vec::new(),
            })
            .expect_err("pipeline should require shader bytecode");
        assert!(pipeline_error.reason.contains("SPIR-V"));
    }
}
