use ash::vk::Handle;
use ash::{Device, vk};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::syntax::{
    BufferHandle, BufferTarget, ClearFlags, DescriptorSetHandle, FramebufferAttachment,
    FramebufferHandle, GfxCommand, GlHandleKind, MeshHandle, PipelineHandle, RenderPassHandle,
    RenderStateFlags, RenderbufferHandle, ShaderHandle, TextureHandle, VertexAttribType,
    ViewportState,
};

#[derive(Debug)]
pub enum ExecutorError {
    MissingResource {
        resource_type: &'static str,
        handle: u32,
    },
    InvalidState {
        reason: String,
    },
    Vulkan {
        context: &'static str,
        result: vk::Result,
    },
}

impl Display for ExecutorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingResource {
                resource_type,
                handle,
            } => {
                write!(
                    f,
                    "missing executor resource ({resource_type}) for handle {handle}"
                )
            }
            Self::InvalidState { reason } => write!(f, "invalid executor state: {reason}"),
            Self::Vulkan { context, result } => {
                write!(f, "vulkan execution failed during {context}: {result:?}")
            }
        }
    }
}

impl Error for ExecutorError {}

#[derive(Debug, Clone, PartialEq)]
pub enum DeferredAction {
    CompileGraphicsPipeline {
        shader: ShaderHandle,
        descriptor: GraphicsPipelineDescriptor,
    },
    ResolveUniformBinding {
        shader: Option<ShaderHandle>,
        name: String,
        kind: UniformValueKind,
    },
    ResolveTextureBinding {
        slot: u32,
        texture: TextureHandle,
    },
    ResolveBufferUpload {
        target: BufferTarget,
        buffer: Option<BufferHandle>,
        offset_bytes: u64,
        size_bytes: u64,
    },
    ResolveFramebuffer {
        framebuffer: FramebufferHandle,
        attachment: FramebufferAttachment,
        source: FramebufferAttachmentSource,
    },
    ResolveDescriptorBufferBinding {
        target: BufferTarget,
        index: u32,
        buffer: BufferHandle,
    },
    HandleLifecycle {
        kind: GlHandleKind,
        ids: Vec<u32>,
        created: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniformValueKind {
    Mat4,
    Vec3,
    BufferBlock,
    StorageBlock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramebufferAttachmentSource {
    Texture(TextureHandle),
    Renderbuffer(RenderbufferHandle),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GraphicsPipelineDescriptor {
    pub topology: vk::PrimitiveTopology,
    pub render_state: RenderStateFlags,
    pub vertex_attributes: Vec<VertexAttributeBinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VertexAttributeBinding {
    pub index: u32,
    pub binding: u32,
    pub size: i32,
    pub attrib_type: VertexAttribType,
    pub normalized: bool,
    pub stride: i32,
    pub offset_bytes: u64,
    pub enabled: bool,
    pub divisor: u32,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ExecutionReport {
    pub dispatched_commands: usize,
    pub descriptor_writes_applied: usize,
    pub deferred_actions: Vec<DeferredAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PipelineCacheKey {
    pub shader: ShaderHandle,
    pub program_context: PipelineProgramContext,
    pub descriptor: GraphicsPipelineDescriptor,
}

pub trait GraphicsPipelineCompiler {
    fn pipeline_cache_context(
        &self,
        shader: ShaderHandle,
    ) -> Result<PipelineProgramContext, ExecutorError>;

    fn compile_graphics_pipeline(
        &mut self,
        shader: ShaderHandle,
        descriptor: &GraphicsPipelineDescriptor,
    ) -> Result<PipelineBinding, ExecutorError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineProgramContext {
    pub layout_raw: u64,
    pub render_pass_raw: u64,
    pub subpass: u32,
    pub sample_count_raw: u32,
    pub polygon_mode_raw: i32,
    pub front_face_raw: i32,
}

#[derive(Debug, Default)]
pub struct GraphicsPipelineCache {
    next_handle: u32,
    entries: HashMap<PipelineCacheKey, PipelineHandle>,
    owned_handles: HashSet<PipelineHandle>,
}

impl GraphicsPipelineCache {
    pub fn get(&self, key: &PipelineCacheKey) -> Option<PipelineHandle> {
        self.entries.get(key).copied()
    }

    pub fn get_or_compile(
        &mut self,
        resources: &mut ExecutorResources,
        compiler: &mut dyn GraphicsPipelineCompiler,
        shader: ShaderHandle,
        descriptor: &GraphicsPipelineDescriptor,
    ) -> Result<(PipelineHandle, PipelineBinding, bool), ExecutorError> {
        let program_context = compiler.pipeline_cache_context(shader)?;
        let key = PipelineCacheKey {
            shader,
            program_context,
            descriptor: descriptor.clone(),
        };
        if let Some(handle) = self.entries.get(&key).copied() {
            let binding = resources.pipelines.get(&handle).copied().ok_or(
                ExecutorError::MissingResource {
                    resource_type: "cached pipeline",
                    handle: handle.0,
                },
            )?;
            return Ok((handle, binding, false));
        }

        let binding = compiler.compile_graphics_pipeline(shader, descriptor)?;
        let handle = PipelineHandle(self.next_handle.max(1));
        self.next_handle = handle.0 + 1;
        resources.pipelines.insert(handle, binding);
        self.entries.insert(key, handle);
        self.owned_handles.insert(handle);
        Ok((handle, binding, true))
    }

    pub fn cached_pipeline_count(&self) -> usize {
        self.entries.len()
    }

    pub fn release_cached_pipelines(
        &mut self,
        resources: &mut ExecutorResources,
    ) -> Vec<(PipelineHandle, PipelineBinding)> {
        let owned_handles = std::mem::take(&mut self.owned_handles);
        self.entries
            .retain(|_, handle| !owned_handles.contains(handle));
        owned_handles
            .into_iter()
            .filter_map(|handle| {
                resources
                    .pipelines
                    .remove(&handle)
                    .map(|binding| (handle, binding))
            })
            .collect()
    }

    pub fn destroy_cached_pipelines(&mut self, device: &Device, resources: &mut ExecutorResources) {
        for (_, binding) in self.release_cached_pipelines(resources) {
            if binding.pipeline != vk::Pipeline::null() {
                unsafe {
                    device.destroy_pipeline(binding.pipeline, None);
                }
            }
        }
    }

    pub fn destroy_shader_pipelines(
        &mut self,
        device: &Device,
        resources: &mut ExecutorResources,
        shader: ShaderHandle,
    ) {
        self.destroy_matching_pipelines(device, resources, |key| key.shader == shader);
    }

    pub fn destroy_program_pipelines(
        &mut self,
        device: &Device,
        resources: &mut ExecutorResources,
        shader: ShaderHandle,
        program_context: PipelineProgramContext,
    ) {
        self.destroy_matching_pipelines(device, resources, |key| {
            key.shader == shader && key.program_context == program_context
        });
    }

    fn destroy_matching_pipelines(
        &mut self,
        device: &Device,
        resources: &mut ExecutorResources,
        mut predicate: impl FnMut(&PipelineCacheKey) -> bool,
    ) {
        let owned_for_shader = self
            .entries
            .iter()
            .filter_map(|(key, handle)| {
                if predicate(key) && self.owned_handles.contains(handle) {
                    Some(*handle)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        self.entries.retain(|key, _| !predicate(key));
        for handle in owned_for_shader {
            self.owned_handles.remove(&handle);
            if let Some(binding) = resources.pipelines.remove(&handle) {
                if binding.pipeline != vk::Pipeline::null() {
                    unsafe {
                        device.destroy_pipeline(binding.pipeline, None);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ShaderProgramDefinition {
    pub vertex_spirv: Vec<u32>,
    pub fragment_spirv: Vec<u32>,
    pub layout: vk::PipelineLayout,
    pub render_pass: vk::RenderPass,
    pub subpass: u32,
}

pub struct VulkanGraphicsPipelineCompiler {
    device: Device,
    shader_programs: HashMap<ShaderHandle, ShaderProgramDefinition>,
    sample_count: vk::SampleCountFlags,
    polygon_mode: vk::PolygonMode,
    front_face: vk::FrontFace,
}

impl VulkanGraphicsPipelineCompiler {
    pub fn new(device: Device) -> Self {
        Self {
            device,
            shader_programs: HashMap::new(),
            sample_count: vk::SampleCountFlags::TYPE_1,
            polygon_mode: vk::PolygonMode::FILL,
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
        }
    }

    pub fn register_shader_program(
        &mut self,
        handle: ShaderHandle,
        definition: ShaderProgramDefinition,
    ) {
        self.shader_programs.insert(handle, definition);
    }

    pub fn set_sample_count(&mut self, sample_count: vk::SampleCountFlags) {
        self.sample_count = sample_count;
    }

    pub fn set_polygon_mode(&mut self, polygon_mode: vk::PolygonMode) {
        self.polygon_mode = polygon_mode;
    }

    pub fn set_front_face(&mut self, front_face: vk::FrontFace) {
        self.front_face = front_face;
    }
}

impl GraphicsPipelineCompiler for VulkanGraphicsPipelineCompiler {
    fn pipeline_cache_context(
        &self,
        shader: ShaderHandle,
    ) -> Result<PipelineProgramContext, ExecutorError> {
        let program = self
            .shader_programs
            .get(&shader)
            .ok_or(ExecutorError::MissingResource {
                resource_type: "shader program definition",
                handle: shader.0,
            })?;
        Ok(PipelineProgramContext {
            layout_raw: program.layout.as_raw(),
            render_pass_raw: program.render_pass.as_raw(),
            subpass: program.subpass,
            sample_count_raw: self.sample_count.as_raw(),
            polygon_mode_raw: self.polygon_mode.as_raw(),
            front_face_raw: self.front_face.as_raw(),
        })
    }

    fn compile_graphics_pipeline(
        &mut self,
        shader: ShaderHandle,
        descriptor: &GraphicsPipelineDescriptor,
    ) -> Result<PipelineBinding, ExecutorError> {
        let program = self
            .shader_programs
            .get(&shader)
            .ok_or(ExecutorError::MissingResource {
                resource_type: "shader program definition",
                handle: shader.0,
            })?;

        let vert_module = create_shader_module(&self.device, &program.vertex_spirv)?;
        let frag_module = create_shader_module(&self.device, &program.fragment_spirv)?;
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

        let bindings = descriptor
            .vertex_attributes
            .iter()
            .filter(|attribute| attribute.enabled)
            .fold(BTreeMap::<u32, (i32, u32)>::new(), |mut map, attribute| {
                map.entry(attribute.binding)
                    .or_insert((attribute.stride, attribute.divisor));
                map
            })
            .into_iter()
            .map(|(binding, (stride, divisor))| {
                vertex_binding_description(binding, stride, divisor)
            })
            .collect::<Vec<_>>();
        let attributes = descriptor
            .vertex_attributes
            .iter()
            .filter(|attribute| attribute.enabled)
            .map(vertex_attribute_description)
            .collect::<Result<Vec<_>, _>>()?;
        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&bindings)
            .vertex_attribute_descriptions(&attributes);
        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(descriptor.topology)
            .primitive_restart_enable(false);
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(self.polygon_mode)
            .cull_mode(
                if descriptor
                    .render_state
                    .contains(RenderStateFlags::CULL_FACE)
                {
                    vk::CullModeFlags::BACK
                } else {
                    vk::CullModeFlags::NONE
                },
            )
            .front_face(self.front_face)
            .line_width(1.0);
        let multisample = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(self.sample_count)
            .sample_shading_enable(false);
        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(
                descriptor
                    .render_state
                    .contains(RenderStateFlags::DEPTH_TEST),
            )
            .depth_write_enable(
                descriptor
                    .render_state
                    .contains(RenderStateFlags::DEPTH_TEST),
            )
            .depth_compare_op(vk::CompareOp::LESS)
            .stencil_test_enable(false);
        let color_blend_attachment = [vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(descriptor.render_state.contains(RenderStateFlags::BLEND))
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .alpha_blend_op(vk::BlendOp::ADD)
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
            .layout(program.layout)
            .render_pass(program.render_pass)
            .subpass(program.subpass)];

        let pipeline = match unsafe {
            self.device
                .create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_info, None)
        } {
            Ok(mut pipelines) => pipelines.remove(0),
            Err((_, result)) => {
                unsafe {
                    self.device.destroy_shader_module(vert_module, None);
                    self.device.destroy_shader_module(frag_module, None);
                }
                return Err(ExecutorError::Vulkan {
                    context: "create_graphics_pipelines(executor)",
                    result,
                });
            }
        };

        unsafe {
            self.device.destroy_shader_module(vert_module, None);
            self.device.destroy_shader_module(frag_module, None);
        }

        Ok(PipelineBinding {
            pipeline,
            layout: program.layout,
            bind_point: vk::PipelineBindPoint::GRAPHICS,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ShaderBinding {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub bind_point: vk::PipelineBindPoint,
}

#[derive(Debug, Clone, Copy)]
pub struct PipelineBinding {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub bind_point: vk::PipelineBindPoint,
}

#[derive(Debug, Clone)]
pub struct MeshBinding {
    pub vertex_buffers: Vec<vk::Buffer>,
    pub vertex_offsets: Vec<u64>,
    pub index_buffer: Option<vk::Buffer>,
    pub index_offset: u64,
    pub index_type: vk::IndexType,
}

#[derive(Debug, Clone, Copy)]
pub struct FramebufferBinding {
    pub framebuffer: vk::Framebuffer,
    pub extent: vk::Extent2D,
    pub offset: vk::Offset2D,
}

#[derive(Debug, Clone)]
pub struct DescriptorSetBinding {
    pub set: vk::DescriptorSet,
    pub layout: DescriptorSetLayoutBindings,
}

#[derive(Debug, Clone, Default, Copy)]
pub struct BufferBinding {
    pub buffer: vk::Buffer,
    pub offset: u64,
    pub range: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct TextureBinding {
    pub image_view: vk::ImageView,
    pub sampler: vk::Sampler,
    pub image_layout: vk::ImageLayout,
}

#[derive(Debug, Clone, Default)]
pub struct DescriptorSetLayoutBindings {
    pub uniform_buffers_by_name: HashMap<String, u32>,
    pub storage_buffers_by_name: HashMap<String, u32>,
    pub combined_image_samplers_by_slot: HashMap<u32, u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorWritePlan {
    UniformBuffer {
        descriptor_set: vk::DescriptorSet,
        binding: u32,
        buffer: vk::Buffer,
        offset: u64,
        range: u64,
    },
    StorageBuffer {
        descriptor_set: vk::DescriptorSet,
        binding: u32,
        buffer: vk::Buffer,
        offset: u64,
        range: u64,
    },
    CombinedImageSampler {
        descriptor_set: vk::DescriptorSet,
        binding: u32,
        image_view: vk::ImageView,
        sampler: vk::Sampler,
        image_layout: vk::ImageLayout,
    },
}

#[derive(Debug, Default)]
pub struct ExecutorResources {
    pub shaders: HashMap<ShaderHandle, ShaderBinding>,
    pub pipelines: HashMap<PipelineHandle, PipelineBinding>,
    pub meshes: HashMap<MeshHandle, MeshBinding>,
    pub render_passes: HashMap<RenderPassHandle, vk::RenderPass>,
    pub framebuffers: HashMap<FramebufferHandle, FramebufferBinding>,
    pub descriptor_sets: HashMap<DescriptorSetHandle, DescriptorSetBinding>,
    pub buffers: HashMap<BufferHandle, BufferBinding>,
    pub textures: HashMap<TextureHandle, TextureBinding>,
    pub named_uniform_buffers: HashMap<String, BufferHandle>,
    pub named_storage_buffers: HashMap<String, BufferHandle>,
}

#[derive(Debug)]
struct ExecutorState {
    clear_color: [f32; 4],
    viewport: Option<ViewportState>,
    current_shader: Option<ShaderHandle>,
    current_pipeline_layout: Option<vk::PipelineLayout>,
    current_framebuffer: Option<FramebufferHandle>,
    current_topology: vk::PrimitiveTopology,
    bound_descriptor_sets: HashMap<u32, DescriptorSetHandle>,
    bound_textures: HashMap<u32, TextureHandle>,
    requested_uniforms: HashMap<String, UniformValueKind>,
    bound_buffers: HashMap<BufferTarget, BufferHandle>,
    vertex_attributes: BTreeMap<u32, VertexAttributeBinding>,
    render_state: RenderStateFlags,
    pipeline_dirty: bool,
}

impl Default for ExecutorState {
    fn default() -> Self {
        Self {
            clear_color: [0.0, 0.0, 0.0, 1.0],
            viewport: None,
            current_shader: None,
            current_pipeline_layout: None,
            current_framebuffer: None,
            current_topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            bound_descriptor_sets: HashMap::new(),
            bound_textures: HashMap::new(),
            requested_uniforms: HashMap::new(),
            bound_buffers: HashMap::new(),
            vertex_attributes: BTreeMap::new(),
            render_state: RenderStateFlags::empty(),
            pipeline_dirty: false,
        }
    }
}

pub struct VulkanExecutor<'a> {
    sink: AshSink<'a>,
    state: ExecutorState,
}

impl<'a> VulkanExecutor<'a> {
    pub fn new(device: &'a Device, command_buffer: vk::CommandBuffer) -> Self {
        Self {
            sink: AshSink {
                device,
                command_buffer,
            },
            state: ExecutorState::default(),
        }
    }

    pub fn execute(
        &mut self,
        resources: &mut ExecutorResources,
        commands: &[GfxCommand],
    ) -> Result<ExecutionReport, ExecutorError> {
        run_commands(&mut self.sink, resources, &mut self.state, commands, None)
    }

    pub fn execute_with_pipeline_cache(
        &mut self,
        resources: &mut ExecutorResources,
        commands: &[GfxCommand],
        cache: &mut GraphicsPipelineCache,
        compiler: &mut dyn GraphicsPipelineCompiler,
    ) -> Result<ExecutionReport, ExecutorError> {
        run_commands(
            &mut self.sink,
            resources,
            &mut self.state,
            commands,
            Some(PipelineResolver { cache, compiler }),
        )
    }
}

trait CommandSink {
    fn bind_pipeline(&mut self, bind_point: vk::PipelineBindPoint, pipeline: vk::Pipeline);
    fn bind_vertex_buffers(&mut self, buffers: &[vk::Buffer], offsets: &[u64]);
    fn bind_index_buffer(&mut self, buffer: vk::Buffer, offset: u64, index_type: vk::IndexType);
    fn set_viewport(&mut self, viewport: &ViewportState);
    fn set_scissor(&mut self, viewport: &ViewportState);
    fn clear_attachments(&mut self, attachments: &[vk::ClearAttachment], rect: vk::ClearRect);
    fn begin_render_pass(
        &mut self,
        begin_info: &vk::RenderPassBeginInfo<'_>,
        contents: vk::SubpassContents,
    );
    fn end_render_pass(&mut self);
    fn bind_descriptor_sets(
        &mut self,
        layout: vk::PipelineLayout,
        first_set: u32,
        sets: &[vk::DescriptorSet],
    );
    fn update_descriptor_writes(&mut self, writes: &[DescriptorWritePlan]);
    fn draw(
        &mut self,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    );
    fn draw_indexed(
        &mut self,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
    );
}

struct AshSink<'a> {
    device: &'a Device,
    command_buffer: vk::CommandBuffer,
}

struct PipelineResolver<'a> {
    cache: &'a mut GraphicsPipelineCache,
    compiler: &'a mut dyn GraphicsPipelineCompiler,
}

impl CommandSink for AshSink<'_> {
    fn bind_pipeline(&mut self, bind_point: vk::PipelineBindPoint, pipeline: vk::Pipeline) {
        unsafe {
            self.device
                .cmd_bind_pipeline(self.command_buffer, bind_point, pipeline)
        }
    }
    fn bind_vertex_buffers(&mut self, buffers: &[vk::Buffer], offsets: &[u64]) {
        unsafe {
            self.device
                .cmd_bind_vertex_buffers(self.command_buffer, 0, buffers, offsets)
        }
    }
    fn bind_index_buffer(&mut self, buffer: vk::Buffer, offset: u64, index_type: vk::IndexType) {
        unsafe {
            self.device
                .cmd_bind_index_buffer(self.command_buffer, buffer, offset, index_type)
        }
    }
    fn set_viewport(&mut self, viewport: &ViewportState) {
        unsafe {
            self.device.cmd_set_viewport(
                self.command_buffer,
                0,
                &[vk::Viewport {
                    x: viewport.x as f32,
                    y: viewport.y as f32,
                    width: viewport.width as f32,
                    height: viewport.height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            )
        }
    }
    fn set_scissor(&mut self, viewport: &ViewportState) {
        unsafe {
            self.device.cmd_set_scissor(
                self.command_buffer,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D {
                        x: viewport.x,
                        y: viewport.y,
                    },
                    extent: vk::Extent2D {
                        width: viewport.width,
                        height: viewport.height,
                    },
                }],
            )
        }
    }
    fn clear_attachments(&mut self, attachments: &[vk::ClearAttachment], rect: vk::ClearRect) {
        unsafe {
            self.device
                .cmd_clear_attachments(self.command_buffer, attachments, &[rect])
        }
    }
    fn begin_render_pass(
        &mut self,
        begin_info: &vk::RenderPassBeginInfo<'_>,
        contents: vk::SubpassContents,
    ) {
        unsafe {
            self.device
                .cmd_begin_render_pass(self.command_buffer, begin_info, contents)
        }
    }
    fn end_render_pass(&mut self) {
        unsafe { self.device.cmd_end_render_pass(self.command_buffer) }
    }
    fn bind_descriptor_sets(
        &mut self,
        layout: vk::PipelineLayout,
        first_set: u32,
        sets: &[vk::DescriptorSet],
    ) {
        unsafe {
            self.device.cmd_bind_descriptor_sets(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                layout,
                first_set,
                sets,
                &[],
            )
        }
    }
    fn update_descriptor_writes(&mut self, writes: &[DescriptorWritePlan]) {
        apply_descriptor_writes(self.device, writes);
    }
    fn draw(
        &mut self,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) {
        unsafe {
            self.device.cmd_draw(
                self.command_buffer,
                vertex_count,
                instance_count,
                first_vertex,
                first_instance,
            )
        }
    }
    fn draw_indexed(
        &mut self,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
    ) {
        unsafe {
            self.device.cmd_draw_indexed(
                self.command_buffer,
                index_count,
                instance_count,
                first_index,
                vertex_offset,
                first_instance,
            )
        }
    }
}

fn run_commands<S: CommandSink>(
    sink: &mut S,
    resources: &mut ExecutorResources,
    state: &mut ExecutorState,
    commands: &[GfxCommand],
    mut pipeline_resolver: Option<PipelineResolver<'_>>,
) -> Result<ExecutionReport, ExecutorError> {
    let mut report = ExecutionReport::default();
    for command in commands {
        match command {
            GfxCommand::SetClearColor(color) => state.clear_color = *color,
            GfxCommand::Clear(flags) => {
                let rect = clear_rect(resources, state)?;
                let attachments = clear_attachments(*flags, state.clear_color);
                if !attachments.is_empty() {
                    sink.clear_attachments(&attachments, rect);
                    report.dispatched_commands += 1;
                }
            }
            GfxCommand::SetViewport(viewport) => {
                state.viewport = Some(*viewport);
                sink.set_viewport(viewport);
                sink.set_scissor(viewport);
                report.dispatched_commands += 2;
            }
            GfxCommand::SetRenderState { flag, enabled } => {
                if *enabled {
                    state.render_state.insert(*flag);
                } else {
                    state.render_state.remove(*flag);
                }
                state.pipeline_dirty = true;
            }
            GfxCommand::UseProgram(shader) => {
                state.current_shader = Some(*shader);
                let descriptor = pipeline_descriptor(state);
                let binding = if let Some(resolver) = pipeline_resolver.as_mut() {
                    let (_, binding, _) = resolver.cache.get_or_compile(
                        resources,
                        resolver.compiler,
                        *shader,
                        &descriptor,
                    )?;
                    state.pipeline_dirty = false;
                    binding
                } else {
                    if state.pipeline_dirty {
                        report
                            .deferred_actions
                            .push(DeferredAction::CompileGraphicsPipeline {
                                shader: *shader,
                                descriptor,
                            });
                        state.pipeline_dirty = false;
                    }
                    let binding = resources.shaders.get(shader).copied().ok_or(
                        ExecutorError::MissingResource {
                            resource_type: "shader pipeline",
                            handle: shader.0,
                        },
                    )?;
                    PipelineBinding {
                        pipeline: binding.pipeline,
                        layout: binding.layout,
                        bind_point: binding.bind_point,
                    }
                };
                state.current_pipeline_layout = Some(binding.layout);
                sink.bind_pipeline(binding.bind_point, binding.pipeline);
                report.dispatched_commands += 1;
            }
            GfxCommand::BindPipeline(handle)
            | GfxCommand::RawBindPipeline {
                pipeline: handle, ..
            } => {
                let binding =
                    resources
                        .pipelines
                        .get(handle)
                        .ok_or(ExecutorError::MissingResource {
                            resource_type: "pipeline",
                            handle: handle.0,
                        })?;
                state.current_pipeline_layout = Some(binding.layout);
                sink.bind_pipeline(binding.bind_point, binding.pipeline);
                report.dispatched_commands += 1;
            }
            GfxCommand::BindMesh(mesh) => {
                let binding = resources
                    .meshes
                    .get(mesh)
                    .ok_or(ExecutorError::MissingResource {
                        resource_type: "mesh",
                        handle: mesh.0,
                    })?;
                sink.bind_vertex_buffers(&binding.vertex_buffers, &binding.vertex_offsets);
                report.dispatched_commands += 1;
                if let Some(index_buffer) = binding.index_buffer {
                    sink.bind_index_buffer(index_buffer, binding.index_offset, binding.index_type);
                    report.dispatched_commands += 1;
                }
            }
            GfxCommand::BindFramebuffer(framebuffer) => {
                state.current_framebuffer = Some(*framebuffer)
            }
            GfxCommand::BindBuffer { target, buffer } => {
                state.bound_buffers.insert(*target, *buffer);
            }
            GfxCommand::BindBufferBase {
                target,
                index,
                buffer,
            } => report
                .deferred_actions
                .push(DeferredAction::ResolveDescriptorBufferBinding {
                    target: *target,
                    index: *index,
                    buffer: *buffer,
                }),
            GfxCommand::BindTexture { slot, texture } => {
                state.bound_textures.insert(*slot, *texture);
                if let Some(write) = resolve_texture_write(resources, state, *slot, *texture) {
                    sink.update_descriptor_writes(&[write]);
                    report.descriptor_writes_applied += 1;
                } else {
                    report
                        .deferred_actions
                        .push(DeferredAction::ResolveTextureBinding {
                            slot: *slot,
                            texture: *texture,
                        });
                }
            }
            GfxCommand::UploadBufferData {
                target, size_bytes, ..
            } => report
                .deferred_actions
                .push(DeferredAction::ResolveBufferUpload {
                    target: *target,
                    buffer: state.bound_buffers.get(target).copied(),
                    offset_bytes: 0,
                    size_bytes: *size_bytes,
                }),
            GfxCommand::UploadBufferSubData {
                target,
                offset_bytes,
                size_bytes,
            } => report
                .deferred_actions
                .push(DeferredAction::ResolveBufferUpload {
                    target: *target,
                    buffer: state.bound_buffers.get(target).copied(),
                    offset_bytes: *offset_bytes,
                    size_bytes: *size_bytes,
                }),
            GfxCommand::DefineVertexAttribute {
                index,
                size,
                attrib_type,
                normalized,
                stride,
                offset_bytes,
            } => {
                state.vertex_attributes.insert(
                    *index,
                    VertexAttributeBinding {
                        index: *index,
                        binding: *index,
                        size: *size,
                        attrib_type: *attrib_type,
                        normalized: *normalized,
                        stride: *stride,
                        offset_bytes: *offset_bytes,
                        enabled: true,
                        divisor: 0,
                    },
                );
                state.pipeline_dirty = true;
            }
            GfxCommand::SetVertexAttributeEnabled { index, enabled } => {
                state
                    .vertex_attributes
                    .entry(*index)
                    .or_insert(default_vertex_attribute(*index))
                    .enabled = *enabled;
                state.pipeline_dirty = true;
            }
            GfxCommand::SetVertexAttributeDivisor { index, divisor } => {
                state
                    .vertex_attributes
                    .entry(*index)
                    .or_insert(default_vertex_attribute(*index))
                    .divisor = *divisor;
                state.pipeline_dirty = true;
            }
            GfxCommand::SetActiveTextureUnit(_) => {}
            GfxCommand::AttachFramebufferTexture {
                attachment,
                texture,
                ..
            } => {
                let framebuffer = state
                    .current_framebuffer
                    .ok_or(ExecutorError::InvalidState {
                        reason: "framebuffer attachment command issued with no bound framebuffer"
                            .into(),
                    })?;
                report
                    .deferred_actions
                    .push(DeferredAction::ResolveFramebuffer {
                        framebuffer,
                        attachment: *attachment,
                        source: FramebufferAttachmentSource::Texture(*texture),
                    });
            }
            GfxCommand::AttachFramebufferRenderbuffer {
                attachment,
                renderbuffer,
            } => {
                let framebuffer = state
                    .current_framebuffer
                    .ok_or(ExecutorError::InvalidState {
                        reason: "framebuffer renderbuffer command issued with no bound framebuffer"
                            .into(),
                    })?;
                report
                    .deferred_actions
                    .push(DeferredAction::ResolveFramebuffer {
                        framebuffer,
                        attachment: *attachment,
                        source: FramebufferAttachmentSource::Renderbuffer(*renderbuffer),
                    });
            }
            GfxCommand::SetUniformMat4 { name, .. } => {
                state
                    .requested_uniforms
                    .insert(name.clone(), UniformValueKind::Mat4);
                if let Some(write) =
                    resolve_uniform_write(resources, state, name, UniformValueKind::Mat4)
                {
                    sink.update_descriptor_writes(&[write]);
                    report.descriptor_writes_applied += 1;
                } else {
                    report
                        .deferred_actions
                        .push(DeferredAction::ResolveUniformBinding {
                            shader: state.current_shader,
                            name: name.clone(),
                            kind: UniformValueKind::Mat4,
                        });
                }
            }
            GfxCommand::SetUniformVec3 { name, .. } => {
                state
                    .requested_uniforms
                    .insert(name.clone(), UniformValueKind::Vec3);
                if let Some(write) =
                    resolve_uniform_write(resources, state, name, UniformValueKind::Vec3)
                {
                    sink.update_descriptor_writes(&[write]);
                    report.descriptor_writes_applied += 1;
                } else {
                    report
                        .deferred_actions
                        .push(DeferredAction::ResolveUniformBinding {
                            shader: state.current_shader,
                            name: name.clone(),
                            kind: UniformValueKind::Vec3,
                        });
                }
            }
            GfxCommand::BeginRenderPass(render_pass) => {
                let begin = render_pass_begin_info(
                    resources,
                    state.current_framebuffer,
                    *render_pass,
                    &[],
                )?;
                sink.begin_render_pass(&begin, vk::SubpassContents::INLINE);
                report.dispatched_commands += 1;
            }
            GfxCommand::EndRenderPass => {
                sink.end_render_pass();
                report.dispatched_commands += 1;
            }
            GfxCommand::BindDescriptorSet {
                set,
                descriptor_set,
            } => {
                let layout = state
                    .current_pipeline_layout
                    .ok_or(ExecutorError::InvalidState {
                        reason:
                            "descriptor set bind requested before a pipeline or shader was bound"
                                .into(),
                    })?;
                let binding = resources.descriptor_sets.get(descriptor_set).ok_or(
                    ExecutorError::MissingResource {
                        resource_type: "descriptor set",
                        handle: descriptor_set.0,
                    },
                )?;
                state.bound_descriptor_sets.insert(*set, *descriptor_set);
                sink.bind_descriptor_sets(layout, *set, &[binding.set]);
                report.dispatched_commands += 1;
                let writes = collect_descriptor_writes(resources, state);
                if !writes.is_empty() {
                    sink.update_descriptor_writes(&writes);
                    report.descriptor_writes_applied += writes.len();
                }
            }
            GfxCommand::Draw {
                topology,
                vertex_count,
                instance_count,
                first_vertex,
                first_instance,
                ..
            } => {
                if state.current_topology != *topology {
                    state.current_topology = *topology;
                    state.pipeline_dirty = true;
                }
                maybe_resolve_pipeline(&mut report, resources, state, pipeline_resolver.as_mut())?;
                sink.draw(
                    *vertex_count,
                    *instance_count,
                    *first_vertex,
                    *first_instance,
                );
                report.dispatched_commands += 1;
            }
            GfxCommand::RawDraw {
                vertex_count,
                instance_count,
                first_vertex,
                first_instance,
            } => {
                maybe_resolve_pipeline(&mut report, resources, state, pipeline_resolver.as_mut())?;
                sink.draw(
                    *vertex_count,
                    *instance_count,
                    *first_vertex,
                    *first_instance,
                );
                report.dispatched_commands += 1;
            }
            GfxCommand::DrawIndexed {
                topology,
                index_count,
                instance_count,
                first_index,
                vertex_offset,
                first_instance,
                ..
            } => {
                if state.current_topology != *topology {
                    state.current_topology = *topology;
                    state.pipeline_dirty = true;
                }
                maybe_resolve_pipeline(&mut report, resources, state, pipeline_resolver.as_mut())?;
                sink.draw_indexed(
                    *index_count,
                    *instance_count,
                    *first_index,
                    *vertex_offset,
                    *first_instance,
                );
                report.dispatched_commands += 1;
            }
            GfxCommand::RawDrawIndexed {
                index_count,
                instance_count,
                first_index,
                vertex_offset,
                first_instance,
            } => {
                maybe_resolve_pipeline(&mut report, resources, state, pipeline_resolver.as_mut())?;
                sink.draw_indexed(
                    *index_count,
                    *instance_count,
                    *first_index,
                    *vertex_offset,
                    *first_instance,
                );
                report.dispatched_commands += 1;
            }
            GfxCommand::RawBeginRenderPass {
                begin_info,
                contents,
            } => {
                let clear_values = raw_clear_values(begin_info.clear_flags, state.clear_color);
                let begin = render_pass_begin_info(
                    resources,
                    Some(begin_info.framebuffer),
                    begin_info.render_pass,
                    clear_values.as_slice(),
                )?;
                sink.begin_render_pass(&begin, *contents);
                report.dispatched_commands += 1;
            }
            GfxCommand::GenerateHandles { kind, ids } => {
                report
                    .deferred_actions
                    .push(DeferredAction::HandleLifecycle {
                        kind: *kind,
                        ids: ids.clone(),
                        created: true,
                    })
            }
            GfxCommand::DeleteHandles { kind, ids } => {
                report
                    .deferred_actions
                    .push(DeferredAction::HandleLifecycle {
                        kind: *kind,
                        ids: ids.clone(),
                        created: false,
                    })
            }
        }
    }
    Ok(report)
}

fn maybe_resolve_pipeline(
    report: &mut ExecutionReport,
    resources: &mut ExecutorResources,
    state: &mut ExecutorState,
    pipeline_resolver: Option<&mut PipelineResolver<'_>>,
) -> Result<(), ExecutorError> {
    if !state.pipeline_dirty {
        return Ok(());
    }
    if let Some(shader) = state.current_shader {
        let descriptor = pipeline_descriptor(state);
        if let Some(resolver) = pipeline_resolver {
            let (_, binding, _) =
                resolver
                    .cache
                    .get_or_compile(resources, resolver.compiler, shader, &descriptor)?;
            state.current_pipeline_layout = Some(binding.layout);
        } else {
            report
                .deferred_actions
                .push(DeferredAction::CompileGraphicsPipeline { shader, descriptor });
        }
    }
    state.pipeline_dirty = false;
    Ok(())
}

fn pipeline_descriptor(state: &ExecutorState) -> GraphicsPipelineDescriptor {
    GraphicsPipelineDescriptor {
        topology: state.current_topology,
        render_state: state.render_state,
        vertex_attributes: state.vertex_attributes.values().copied().collect(),
    }
}

fn default_vertex_attribute(index: u32) -> VertexAttributeBinding {
    VertexAttributeBinding {
        index,
        binding: index,
        size: 4,
        attrib_type: VertexAttribType::Float32,
        normalized: false,
        stride: 0,
        offset_bytes: 0,
        enabled: true,
        divisor: 0,
    }
}

fn clear_rect(
    resources: &ExecutorResources,
    state: &ExecutorState,
) -> Result<vk::ClearRect, ExecutorError> {
    let (offset, extent) = if let Some(viewport) = state.viewport {
        (
            vk::Offset2D {
                x: viewport.x,
                y: viewport.y,
            },
            vk::Extent2D {
                width: viewport.width,
                height: viewport.height,
            },
        )
    } else if let Some(framebuffer) = state.current_framebuffer {
        let binding =
            resources
                .framebuffers
                .get(&framebuffer)
                .ok_or(ExecutorError::MissingResource {
                    resource_type: "framebuffer",
                    handle: framebuffer.0,
                })?;
        (binding.offset, binding.extent)
    } else {
        return Err(ExecutorError::InvalidState {
            reason: "clear command issued without a viewport or bound framebuffer".into(),
        });
    };
    Ok(vk::ClearRect {
        rect: vk::Rect2D { offset, extent },
        base_array_layer: 0,
        layer_count: 1,
    })
}

fn clear_attachments(flags: ClearFlags, color: [f32; 4]) -> Vec<vk::ClearAttachment> {
    let mut attachments = Vec::new();
    if flags.contains(crate::syntax::CLEAR_COLOR) {
        attachments.push(
            vk::ClearAttachment::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .color_attachment(0)
                .clear_value(vk::ClearValue {
                    color: vk::ClearColorValue { float32: color },
                }),
        );
    }
    if flags.contains(crate::syntax::CLEAR_DEPTH) || flags.contains(crate::syntax::CLEAR_STENCIL) {
        let mut aspect = vk::ImageAspectFlags::empty();
        if flags.contains(crate::syntax::CLEAR_DEPTH) {
            aspect |= vk::ImageAspectFlags::DEPTH;
        }
        if flags.contains(crate::syntax::CLEAR_STENCIL) {
            aspect |= vk::ImageAspectFlags::STENCIL;
        }
        attachments.push(
            vk::ClearAttachment::default()
                .aspect_mask(aspect)
                .clear_value(vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 1.0,
                        stencil: 0,
                    },
                }),
        );
    }
    attachments
}

fn render_pass_begin_info<'a>(
    resources: &'a ExecutorResources,
    framebuffer_handle: Option<FramebufferHandle>,
    render_pass_handle: RenderPassHandle,
    clear_values: &'a [vk::ClearValue],
) -> Result<vk::RenderPassBeginInfo<'a>, ExecutorError> {
    let render_pass =
        resources
            .render_passes
            .get(&render_pass_handle)
            .ok_or(ExecutorError::MissingResource {
                resource_type: "render pass",
                handle: render_pass_handle.0,
            })?;
    let framebuffer_handle = framebuffer_handle.ok_or(ExecutorError::InvalidState {
        reason: "render pass begin requested with no bound framebuffer".into(),
    })?;
    let framebuffer =
        resources
            .framebuffers
            .get(&framebuffer_handle)
            .ok_or(ExecutorError::MissingResource {
                resource_type: "framebuffer",
                handle: framebuffer_handle.0,
            })?;
    Ok(vk::RenderPassBeginInfo::default()
        .render_pass(*render_pass)
        .framebuffer(framebuffer.framebuffer)
        .render_area(vk::Rect2D {
            offset: framebuffer.offset,
            extent: framebuffer.extent,
        })
        .clear_values(clear_values))
}

fn raw_clear_values(flags: ClearFlags, color: [f32; 4]) -> Vec<vk::ClearValue> {
    let mut values = Vec::new();
    if flags.contains(crate::syntax::CLEAR_COLOR) {
        values.push(vk::ClearValue {
            color: vk::ClearColorValue { float32: color },
        });
    }
    if flags.contains(crate::syntax::CLEAR_DEPTH) || flags.contains(crate::syntax::CLEAR_STENCIL) {
        values.push(vk::ClearValue {
            depth_stencil: vk::ClearDepthStencilValue {
                depth: 1.0,
                stencil: 0,
            },
        });
    }
    values
}

pub fn apply_descriptor_writes(device: &Device, writes: &[DescriptorWritePlan]) {
    for write in writes {
        match write {
            DescriptorWritePlan::UniformBuffer {
                descriptor_set,
                binding,
                buffer,
                offset,
                range,
            } => {
                let buffer_info = [vk::DescriptorBufferInfo::default()
                    .buffer(*buffer)
                    .offset(*offset)
                    .range(*range)];
                let write_info = [vk::WriteDescriptorSet::default()
                    .dst_set(*descriptor_set)
                    .dst_binding(*binding)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .buffer_info(&buffer_info)];
                unsafe {
                    device.update_descriptor_sets(&write_info, &[]);
                }
            }
            DescriptorWritePlan::StorageBuffer {
                descriptor_set,
                binding,
                buffer,
                offset,
                range,
            } => {
                let buffer_info = [vk::DescriptorBufferInfo::default()
                    .buffer(*buffer)
                    .offset(*offset)
                    .range(*range)];
                let write_info = [vk::WriteDescriptorSet::default()
                    .dst_set(*descriptor_set)
                    .dst_binding(*binding)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(&buffer_info)];
                unsafe {
                    device.update_descriptor_sets(&write_info, &[]);
                }
            }
            DescriptorWritePlan::CombinedImageSampler {
                descriptor_set,
                binding,
                image_view,
                sampler,
                image_layout,
            } => {
                let image_info = [vk::DescriptorImageInfo::default()
                    .sampler(*sampler)
                    .image_view(*image_view)
                    .image_layout(*image_layout)];
                let write_info = [vk::WriteDescriptorSet::default()
                    .dst_set(*descriptor_set)
                    .dst_binding(*binding)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&image_info)];
                unsafe {
                    device.update_descriptor_sets(&write_info, &[]);
                }
            }
        }
    }
}

pub fn resolve_descriptor_writes_for_bindings(
    resources: &ExecutorResources,
    descriptor_sets: &[DescriptorSetHandle],
    buffers: &[(&str, UniformValueKind)],
    textures: &[(u32, TextureHandle)],
) -> Vec<DescriptorWritePlan> {
    let mut writes = Vec::new();

    for descriptor_handle in descriptor_sets {
        let Some(descriptor) = resources.descriptor_sets.get(descriptor_handle) else {
            continue;
        };

        for (name, kind) in buffers {
            let (buffer_handle, binding, write_kind) = match kind {
                UniformValueKind::Mat4 | UniformValueKind::Vec3 | UniformValueKind::BufferBlock => {
                    let Some(buffer_handle) = resources.named_uniform_buffers.get(*name) else {
                        continue;
                    };
                    let Some(binding) = descriptor.layout.uniform_buffers_by_name.get(*name) else {
                        continue;
                    };
                    (*buffer_handle, *binding, UniformValueKind::BufferBlock)
                }
                UniformValueKind::StorageBlock => {
                    let Some(buffer_handle) = resources.named_storage_buffers.get(*name) else {
                        continue;
                    };
                    let Some(binding) = descriptor.layout.storage_buffers_by_name.get(*name) else {
                        continue;
                    };
                    (*buffer_handle, *binding, UniformValueKind::StorageBlock)
                }
            };
            let Some(buffer) = resources.buffers.get(&buffer_handle) else {
                continue;
            };
            let write = match write_kind {
                UniformValueKind::BufferBlock | UniformValueKind::Mat4 | UniformValueKind::Vec3 => {
                    DescriptorWritePlan::UniformBuffer {
                        descriptor_set: descriptor.set,
                        binding,
                        buffer: buffer.buffer,
                        offset: buffer.offset,
                        range: buffer.range,
                    }
                }
                UniformValueKind::StorageBlock => DescriptorWritePlan::StorageBuffer {
                    descriptor_set: descriptor.set,
                    binding,
                    buffer: buffer.buffer,
                    offset: buffer.offset,
                    range: buffer.range,
                },
            };
            writes.push(write);
        }

        for (slot, texture_handle) in textures {
            let Some(texture) = resources.textures.get(texture_handle) else {
                continue;
            };
            let Some(binding) = descriptor.layout.combined_image_samplers_by_slot.get(slot) else {
                continue;
            };
            writes.push(DescriptorWritePlan::CombinedImageSampler {
                descriptor_set: descriptor.set,
                binding: *binding,
                image_view: texture.image_view,
                sampler: texture.sampler,
                image_layout: texture.image_layout,
            });
        }
    }

    writes
}

fn collect_descriptor_writes(
    resources: &ExecutorResources,
    state: &ExecutorState,
) -> Vec<DescriptorWritePlan> {
    let mut writes = Vec::new();
    for (name, kind) in &state.requested_uniforms {
        if let Some(write) = resolve_uniform_write(resources, state, name, *kind) {
            writes.push(write);
        }
    }
    for (slot, texture) in &state.bound_textures {
        if let Some(write) = resolve_texture_write(resources, state, *slot, *texture) {
            writes.push(write);
        }
    }
    writes
}

fn resolve_uniform_write(
    resources: &ExecutorResources,
    state: &ExecutorState,
    name: &str,
    kind: UniformValueKind,
) -> Option<DescriptorWritePlan> {
    let (buffer_handle, binding_lookup_is_storage) = match kind {
        UniformValueKind::Mat4 | UniformValueKind::Vec3 | UniformValueKind::BufferBlock => {
            (*resources.named_uniform_buffers.get(name)?, false)
        }
        UniformValueKind::StorageBlock => (*resources.named_storage_buffers.get(name)?, true),
    };
    let buffer = resources.buffers.get(&buffer_handle)?;
    state
        .bound_descriptor_sets
        .values()
        .find_map(|descriptor_handle| {
            let descriptor = resources.descriptor_sets.get(descriptor_handle)?;
            if binding_lookup_is_storage {
                let binding = *descriptor.layout.storage_buffers_by_name.get(name)?;
                Some(DescriptorWritePlan::StorageBuffer {
                    descriptor_set: descriptor.set,
                    binding,
                    buffer: buffer.buffer,
                    offset: buffer.offset,
                    range: buffer.range,
                })
            } else {
                let binding = *descriptor.layout.uniform_buffers_by_name.get(name)?;
                Some(DescriptorWritePlan::UniformBuffer {
                    descriptor_set: descriptor.set,
                    binding,
                    buffer: buffer.buffer,
                    offset: buffer.offset,
                    range: buffer.range,
                })
            }
        })
}

fn resolve_texture_write(
    resources: &ExecutorResources,
    state: &ExecutorState,
    slot: u32,
    texture_handle: TextureHandle,
) -> Option<DescriptorWritePlan> {
    let texture = resources.textures.get(&texture_handle)?;
    state
        .bound_descriptor_sets
        .values()
        .find_map(|descriptor_handle| {
            let descriptor = resources.descriptor_sets.get(descriptor_handle)?;
            let binding = *descriptor
                .layout
                .combined_image_samplers_by_slot
                .get(&slot)?;
            Some(DescriptorWritePlan::CombinedImageSampler {
                descriptor_set: descriptor.set,
                binding,
                image_view: texture.image_view,
                sampler: texture.sampler,
                image_layout: texture.image_layout,
            })
        })
}

fn create_shader_module(
    device: &Device,
    spirv_words: &[u32],
) -> Result<vk::ShaderModule, ExecutorError> {
    let info = vk::ShaderModuleCreateInfo::default().code(spirv_words);
    unsafe { device.create_shader_module(&info, None) }.map_err(|result| ExecutorError::Vulkan {
        context: "create_shader_module(executor)",
        result,
    })
}

fn vertex_binding_description(
    binding: u32,
    stride: i32,
    divisor: u32,
) -> vk::VertexInputBindingDescription {
    vk::VertexInputBindingDescription::default()
        .binding(binding)
        .stride(stride.max(0) as u32)
        .input_rate(if divisor == 0 {
            vk::VertexInputRate::VERTEX
        } else {
            vk::VertexInputRate::INSTANCE
        })
}

fn vertex_attribute_description(
    attribute: &VertexAttributeBinding,
) -> Result<vk::VertexInputAttributeDescription, ExecutorError> {
    if attribute.divisor > 1 {
        return Err(ExecutorError::InvalidState {
            reason: format!(
                "vertex attribute divisor {} on location {} requires a divisor-capable pipeline path",
                attribute.divisor, attribute.index
            ),
        });
    }

    Ok(vk::VertexInputAttributeDescription::default()
        .location(attribute.index)
        .binding(attribute.binding)
        .format(attribute_format(attribute.attrib_type, attribute.size)?)
        .offset(attribute.offset_bytes as u32))
}

fn attribute_format(attrib_type: VertexAttribType, size: i32) -> Result<vk::Format, ExecutorError> {
    let format = match (attrib_type, size) {
        (VertexAttribType::Float32, 1) => vk::Format::R32_SFLOAT,
        (VertexAttribType::Float32, 2) => vk::Format::R32G32_SFLOAT,
        (VertexAttribType::Float32, 3) => vk::Format::R32G32B32_SFLOAT,
        (VertexAttribType::Float32, 4) => vk::Format::R32G32B32A32_SFLOAT,
        (VertexAttribType::UnsignedInt, 1) => vk::Format::R32_UINT,
        (VertexAttribType::UnsignedInt, 2) => vk::Format::R32G32_UINT,
        (VertexAttribType::UnsignedInt, 3) => vk::Format::R32G32B32_UINT,
        (VertexAttribType::UnsignedInt, 4) => vk::Format::R32G32B32A32_UINT,
        (VertexAttribType::UnsignedShort, 1) => vk::Format::R16_UINT,
        (VertexAttribType::UnsignedShort, 2) => vk::Format::R16G16_UINT,
        (VertexAttribType::UnsignedShort, 3) => vk::Format::R16G16B16_UINT,
        (VertexAttribType::UnsignedShort, 4) => vk::Format::R16G16B16A16_UINT,
        _ => {
            return Err(ExecutorError::InvalidState {
                reason: format!(
                    "unsupported vertex attribute format {:?} with component count {}",
                    attrib_type, size
                ),
            });
        }
    };
    Ok(format)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestSink {
        events: Vec<&'static str>,
        descriptor_writes: Vec<DescriptorWritePlan>,
    }

    impl CommandSink for TestSink {
        fn bind_pipeline(&mut self, _: vk::PipelineBindPoint, _: vk::Pipeline) {
            self.events.push("bind_pipeline");
        }
        fn bind_vertex_buffers(&mut self, _: &[vk::Buffer], _: &[u64]) {
            self.events.push("bind_vertex_buffers");
        }
        fn bind_index_buffer(&mut self, _: vk::Buffer, _: u64, _: vk::IndexType) {
            self.events.push("bind_index_buffer");
        }
        fn set_viewport(&mut self, _: &ViewportState) {
            self.events.push("set_viewport");
        }
        fn set_scissor(&mut self, _: &ViewportState) {
            self.events.push("set_scissor");
        }
        fn clear_attachments(&mut self, _: &[vk::ClearAttachment], _: vk::ClearRect) {
            self.events.push("clear_attachments");
        }
        fn begin_render_pass(&mut self, _: &vk::RenderPassBeginInfo<'_>, _: vk::SubpassContents) {
            self.events.push("begin_render_pass");
        }
        fn end_render_pass(&mut self) {
            self.events.push("end_render_pass");
        }
        fn bind_descriptor_sets(&mut self, _: vk::PipelineLayout, _: u32, _: &[vk::DescriptorSet]) {
            self.events.push("bind_descriptor_sets");
        }
        fn update_descriptor_writes(&mut self, writes: &[DescriptorWritePlan]) {
            self.events.push("update_descriptor_sets");
            self.descriptor_writes.extend_from_slice(writes);
        }
        fn draw(&mut self, _: u32, _: u32, _: u32, _: u32) {
            self.events.push("draw");
        }
        fn draw_indexed(&mut self, _: u32, _: u32, _: u32, _: i32, _: u32) {
            self.events.push("draw_indexed");
        }
    }

    fn resources() -> ExecutorResources {
        let mut resources = ExecutorResources::default();
        resources.shaders.insert(
            ShaderHandle(7),
            ShaderBinding {
                pipeline: vk::Pipeline::null(),
                layout: vk::PipelineLayout::null(),
                bind_point: vk::PipelineBindPoint::GRAPHICS,
            },
        );
        resources.pipelines.insert(
            PipelineHandle(2),
            PipelineBinding {
                pipeline: vk::Pipeline::null(),
                layout: vk::PipelineLayout::null(),
                bind_point: vk::PipelineBindPoint::GRAPHICS,
            },
        );
        resources.meshes.insert(
            MeshHandle(3),
            MeshBinding {
                vertex_buffers: vec![vk::Buffer::null()],
                vertex_offsets: vec![0],
                index_buffer: Some(vk::Buffer::null()),
                index_offset: 0,
                index_type: vk::IndexType::UINT32,
            },
        );
        resources
            .render_passes
            .insert(RenderPassHandle(5), vk::RenderPass::null());
        resources.framebuffers.insert(
            FramebufferHandle(9),
            FramebufferBinding {
                framebuffer: vk::Framebuffer::null(),
                extent: vk::Extent2D {
                    width: 1280,
                    height: 720,
                },
                offset: vk::Offset2D { x: 0, y: 0 },
            },
        );
        resources.descriptor_sets.insert(
            DescriptorSetHandle(4),
            DescriptorSetBinding {
                set: vk::DescriptorSet::null(),
                layout: DescriptorSetLayoutBindings {
                    uniform_buffers_by_name: HashMap::from([(String::from("u_mvp"), 0_u32)]),
                    storage_buffers_by_name: HashMap::from([(String::from("u_scene"), 2_u32)]),
                    combined_image_samplers_by_slot: HashMap::from([(2_u32, 1_u32)]),
                },
            },
        );
        resources.buffers.insert(
            BufferHandle(12),
            BufferBinding {
                buffer: vk::Buffer::null(),
                offset: 0,
                range: 64,
            },
        );
        resources
            .named_uniform_buffers
            .insert(String::from("u_mvp"), BufferHandle(12));
        resources
            .named_storage_buffers
            .insert(String::from("u_scene"), BufferHandle(12));
        resources.textures.insert(
            TextureHandle(4),
            TextureBinding {
                image_view: vk::ImageView::null(),
                sampler: vk::Sampler::null(),
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            },
        );
        resources
    }

    #[test]
    fn executor_dispatches_immediate_vulkan_commands() {
        let mut sink = TestSink::default();
        let mut state = ExecutorState::default();
        let report = run_commands(
            &mut sink,
            &mut resources(),
            &mut state,
            &[
                GfxCommand::BindFramebuffer(FramebufferHandle(9)),
                GfxCommand::SetViewport(ViewportState {
                    x: 0,
                    y: 0,
                    width: 640,
                    height: 480,
                }),
                GfxCommand::SetClearColor([0.1, 0.2, 0.3, 1.0]),
                GfxCommand::BeginRenderPass(RenderPassHandle(5)),
                GfxCommand::Clear(crate::syntax::CLEAR_COLOR | crate::syntax::CLEAR_DEPTH),
                GfxCommand::UseProgram(ShaderHandle(7)),
                GfxCommand::BindMesh(MeshHandle(3)),
                GfxCommand::DrawIndexed {
                    topology: vk::PrimitiveTopology::TRIANGLE_LIST,
                    index_count: 36,
                    instance_count: 1,
                    first_index: 0,
                    vertex_offset: 0,
                    first_instance: 0,
                    index_type: crate::syntax::IndexType::U32,
                    index_offset_bytes: 0,
                },
                GfxCommand::EndRenderPass,
            ],
            None,
        )
        .unwrap();
        assert_eq!(report.dispatched_commands, 9);
        assert_eq!(
            sink.events,
            vec![
                "set_viewport",
                "set_scissor",
                "begin_render_pass",
                "clear_attachments",
                "bind_pipeline",
                "bind_vertex_buffers",
                "bind_index_buffer",
                "draw_indexed",
                "end_render_pass"
            ]
        );
    }

    #[test]
    fn executor_reports_deferred_pipeline_and_binding_work() {
        let mut sink = TestSink::default();
        let mut state = ExecutorState::default();
        let report = run_commands(
            &mut sink,
            &mut resources(),
            &mut state,
            &[
                GfxCommand::BindFramebuffer(FramebufferHandle(9)),
                GfxCommand::DefineVertexAttribute {
                    index: 0,
                    size: 3,
                    attrib_type: VertexAttribType::Float32,
                    normalized: false,
                    stride: 24,
                    offset_bytes: 0,
                },
                GfxCommand::SetVertexAttributeDivisor {
                    index: 0,
                    divisor: 1,
                },
                GfxCommand::BindBuffer {
                    target: BufferTarget::Uniform,
                    buffer: BufferHandle(12),
                },
                GfxCommand::BindBufferBase {
                    target: BufferTarget::Uniform,
                    index: 0,
                    buffer: BufferHandle(12),
                },
                GfxCommand::UploadBufferSubData {
                    target: BufferTarget::Uniform,
                    offset_bytes: 16,
                    size_bytes: 64,
                },
                GfxCommand::BindTexture {
                    slot: 2,
                    texture: TextureHandle(4),
                },
                GfxCommand::AttachFramebufferRenderbuffer {
                    attachment: FramebufferAttachment::Depth,
                    renderbuffer: RenderbufferHandle(8),
                },
                GfxCommand::SetUniformMat4 {
                    name: "u_mvp".to_string(),
                    value: [[1.0; 4]; 4],
                },
                GfxCommand::UseProgram(ShaderHandle(7)),
            ],
            None,
        )
        .unwrap();
        assert_eq!(report.dispatched_commands, 1);
        assert!(report.deferred_actions.contains(
            &DeferredAction::ResolveDescriptorBufferBinding {
                target: BufferTarget::Uniform,
                index: 0,
                buffer: BufferHandle(12)
            }
        ));
        assert!(
            report
                .deferred_actions
                .contains(&DeferredAction::ResolveTextureBinding {
                    slot: 2,
                    texture: TextureHandle(4)
                })
        );
        assert!(
            report
                .deferred_actions
                .contains(&DeferredAction::ResolveFramebuffer {
                    framebuffer: FramebufferHandle(9),
                    attachment: FramebufferAttachment::Depth,
                    source: FramebufferAttachmentSource::Renderbuffer(RenderbufferHandle(8))
                })
        );
        assert!(report.deferred_actions.iter().any(|action| matches!(action, DeferredAction::CompileGraphicsPipeline { shader, descriptor } if *shader == ShaderHandle(7) && descriptor.vertex_attributes.len() == 1 && descriptor.vertex_attributes[0].divisor == 1)));
    }

    struct MockCompiler {
        compiled: usize,
        context: PipelineProgramContext,
    }

    impl GraphicsPipelineCompiler for MockCompiler {
        fn pipeline_cache_context(
            &self,
            _shader: ShaderHandle,
        ) -> Result<PipelineProgramContext, ExecutorError> {
            Ok(self.context)
        }

        fn compile_graphics_pipeline(
            &mut self,
            _shader: ShaderHandle,
            _descriptor: &GraphicsPipelineDescriptor,
        ) -> Result<PipelineBinding, ExecutorError> {
            self.compiled += 1;
            Ok(PipelineBinding {
                pipeline: vk::Pipeline::null(),
                layout: vk::PipelineLayout::null(),
                bind_point: vk::PipelineBindPoint::GRAPHICS,
            })
        }
    }

    #[test]
    fn pipeline_cache_compiles_once_for_same_descriptor() {
        let mut sink = TestSink::default();
        let mut state = ExecutorState::default();
        let mut resources = resources();
        let mut cache = GraphicsPipelineCache::default();
        let mut compiler = MockCompiler {
            compiled: 0,
            context: PipelineProgramContext {
                layout_raw: 1,
                render_pass_raw: 2,
                subpass: 0,
                sample_count_raw: 1,
                polygon_mode_raw: 0,
                front_face_raw: 0,
            },
        };
        let commands = [
            GfxCommand::DefineVertexAttribute {
                index: 0,
                size: 3,
                attrib_type: VertexAttribType::Float32,
                normalized: false,
                stride: 24,
                offset_bytes: 0,
            },
            GfxCommand::UseProgram(ShaderHandle(7)),
        ];

        let report_a = run_commands(
            &mut sink,
            &mut resources,
            &mut state,
            &commands,
            Some(PipelineResolver {
                cache: &mut cache,
                compiler: &mut compiler,
            }),
        )
        .unwrap();
        let report_b = run_commands(
            &mut sink,
            &mut resources,
            &mut state,
            &commands,
            Some(PipelineResolver {
                cache: &mut cache,
                compiler: &mut compiler,
            }),
        )
        .unwrap();

        assert_eq!(compiler.compiled, 1);
        assert!(report_a.deferred_actions.is_empty());
        assert!(report_b.deferred_actions.is_empty());
        assert_eq!(cache.entries.len(), 1);
    }

    #[test]
    fn pipeline_cache_releases_only_owned_pipelines() {
        let mut sink = TestSink::default();
        let mut state = ExecutorState::default();
        let mut resources = resources();
        let mut cache = GraphicsPipelineCache::default();
        let mut compiler = MockCompiler {
            compiled: 0,
            context: PipelineProgramContext {
                layout_raw: 1,
                render_pass_raw: 2,
                subpass: 0,
                sample_count_raw: 1,
                polygon_mode_raw: 0,
                front_face_raw: 0,
            },
        };
        let external_handle = PipelineHandle(99);
        resources.pipelines.insert(
            external_handle,
            PipelineBinding {
                pipeline: vk::Pipeline::null(),
                layout: vk::PipelineLayout::null(),
                bind_point: vk::PipelineBindPoint::GRAPHICS,
            },
        );

        run_commands(
            &mut sink,
            &mut resources,
            &mut state,
            &[GfxCommand::UseProgram(ShaderHandle(7))],
            Some(PipelineResolver {
                cache: &mut cache,
                compiler: &mut compiler,
            }),
        )
        .unwrap();

        let released = cache.release_cached_pipelines(&mut resources);
        assert_eq!(released.len(), 1);
        assert_eq!(cache.cached_pipeline_count(), 0);
        assert!(resources.pipelines.contains_key(&external_handle));
        assert!(resources.pipelines.contains_key(&PipelineHandle(2)));
        assert_eq!(resources.pipelines.len(), 2);
    }

    #[test]
    fn executor_applies_descriptor_writes_when_layout_metadata_matches() {
        let mut sink = TestSink::default();
        let mut state = ExecutorState::default();
        let mut resources = resources();
        let report = run_commands(
            &mut sink,
            &mut resources,
            &mut state,
            &[
                GfxCommand::UseProgram(ShaderHandle(7)),
                GfxCommand::BindDescriptorSet {
                    set: 0,
                    descriptor_set: DescriptorSetHandle(4),
                },
                GfxCommand::SetUniformMat4 {
                    name: "u_mvp".to_string(),
                    value: [[1.0; 4]; 4],
                },
                GfxCommand::BindTexture {
                    slot: 2,
                    texture: TextureHandle(4),
                },
            ],
            None,
        )
        .unwrap();

        assert_eq!(report.descriptor_writes_applied, 2);
        assert!(report.deferred_actions.is_empty());
        assert_eq!(
            sink.events,
            vec![
                "bind_pipeline",
                "bind_descriptor_sets",
                "update_descriptor_sets",
                "update_descriptor_sets"
            ]
        );
        assert_eq!(
            sink.descriptor_writes,
            vec![
                DescriptorWritePlan::UniformBuffer {
                    descriptor_set: vk::DescriptorSet::null(),
                    binding: 0,
                    buffer: vk::Buffer::null(),
                    offset: 0,
                    range: 64,
                },
                DescriptorWritePlan::CombinedImageSampler {
                    descriptor_set: vk::DescriptorSet::null(),
                    binding: 1,
                    image_view: vk::ImageView::null(),
                    sampler: vk::Sampler::null(),
                    image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                },
            ]
        );
    }

    #[test]
    fn public_descriptor_write_resolution_matches_registered_resources() {
        let resources = resources();
        let writes = resolve_descriptor_writes_for_bindings(
            &resources,
            &[DescriptorSetHandle(4)],
            &[
                ("u_mvp", UniformValueKind::BufferBlock),
                ("u_scene", UniformValueKind::StorageBlock),
            ],
            &[(2, TextureHandle(4))],
        );

        assert_eq!(
            writes,
            vec![
                DescriptorWritePlan::UniformBuffer {
                    descriptor_set: vk::DescriptorSet::null(),
                    binding: 0,
                    buffer: vk::Buffer::null(),
                    offset: 0,
                    range: 64,
                },
                DescriptorWritePlan::StorageBuffer {
                    descriptor_set: vk::DescriptorSet::null(),
                    binding: 2,
                    buffer: vk::Buffer::null(),
                    offset: 0,
                    range: 64,
                },
                DescriptorWritePlan::CombinedImageSampler {
                    descriptor_set: vk::DescriptorSet::null(),
                    binding: 1,
                    image_view: vk::ImageView::null(),
                    sampler: vk::Sampler::null(),
                    image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                },
            ]
        );
    }

    #[test]
    fn pipeline_cache_distinguishes_program_contexts() {
        let mut resources = resources();
        let mut cache = GraphicsPipelineCache::default();
        let descriptor = GraphicsPipelineDescriptor {
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            render_state: RenderStateFlags::empty(),
            vertex_attributes: Vec::new(),
        };
        let mut compiler = MockCompiler {
            compiled: 0,
            context: PipelineProgramContext {
                layout_raw: 1,
                render_pass_raw: 2,
                subpass: 0,
                sample_count_raw: 1,
                polygon_mode_raw: 0,
                front_face_raw: 0,
            },
        };

        cache
            .get_or_compile(&mut resources, &mut compiler, ShaderHandle(7), &descriptor)
            .unwrap();
        compiler.context.render_pass_raw = 3;
        cache
            .get_or_compile(&mut resources, &mut compiler, ShaderHandle(7), &descriptor)
            .unwrap();

        assert_eq!(compiler.compiled, 2);
        assert_eq!(cache.cached_pipeline_count(), 2);
    }
}
