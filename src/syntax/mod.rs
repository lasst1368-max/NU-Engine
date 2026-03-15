use ash::vk;
use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ClearFlags: u32 {
        const COLOR = 1 << 0;
        const DEPTH = 1 << 1;
        const STENCIL = 1 << 2;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct RenderStateFlags: u32 {
        const DEPTH_TEST = 1 << 0;
        const BLEND = 1 << 1;
        const CULL_FACE = 1 << 2;
    }
}

pub const CLEAR_COLOR: ClearFlags = ClearFlags::COLOR;
pub const CLEAR_DEPTH: ClearFlags = ClearFlags::DEPTH;
pub const CLEAR_STENCIL: ClearFlags = ClearFlags::STENCIL;
pub const GL_COLOR_BUFFER_BIT: ClearFlags = ClearFlags::COLOR;
pub const GL_DEPTH_BUFFER_BIT: ClearFlags = ClearFlags::DEPTH;
pub const GL_STENCIL_BUFFER_BIT: ClearFlags = ClearFlags::STENCIL;

pub const DEPTH_TEST: RenderStateFlags = RenderStateFlags::DEPTH_TEST;
pub const BLEND: RenderStateFlags = RenderStateFlags::BLEND;
pub const CULL_FACE: RenderStateFlags = RenderStateFlags::CULL_FACE;

pub const GL_TRIANGLES: vk::PrimitiveTopology = vk::PrimitiveTopology::TRIANGLE_LIST;
pub const GL_LINES: vk::PrimitiveTopology = vk::PrimitiveTopology::LINE_LIST;
pub const GL_POINTS: vk::PrimitiveTopology = vk::PrimitiveTopology::POINT_LIST;
pub const GL_TEXTURE_2D: u32 = 0;
pub const GL_FRAMEBUFFER: u32 = 0;
pub const GL_RENDERBUFFER: u32 = 0;
pub const GL_ARRAY_BUFFER: BufferTarget = BufferTarget::Array;
pub const GL_ELEMENT_ARRAY_BUFFER: BufferTarget = BufferTarget::ElementArray;
pub const GL_UNIFORM_BUFFER: BufferTarget = BufferTarget::Uniform;
pub const GL_STATIC_DRAW: BufferUsage = BufferUsage::StaticDraw;
pub const GL_DYNAMIC_DRAW: BufferUsage = BufferUsage::DynamicDraw;
pub const GL_STREAM_DRAW: BufferUsage = BufferUsage::StreamDraw;
pub const GL_FLOAT: VertexAttribType = VertexAttribType::Float32;
pub const GL_TRUE: bool = true;
pub const GL_FALSE: bool = false;
pub const GL_TEXTURE0: u32 = 0;
pub const GL_COLOR_ATTACHMENT0: FramebufferAttachment = FramebufferAttachment::Color(0);
pub const GL_DEPTH_ATTACHMENT: FramebufferAttachment = FramebufferAttachment::Depth;
pub const GL_DEPTH_STENCIL_ATTACHMENT: FramebufferAttachment = FramebufferAttachment::DepthStencil;

pub const VK_TRIANGLE_LIST: vk::PrimitiveTopology = vk::PrimitiveTopology::TRIANGLE_LIST;
pub const VK_LINE_LIST: vk::PrimitiveTopology = vk::PrimitiveTopology::LINE_LIST;
pub const VK_POINT_LIST: vk::PrimitiveTopology = vk::PrimitiveTopology::POINT_LIST;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexType {
    U16,
    U32,
}

pub const GL_UNSIGNED_SHORT: IndexType = IndexType::U16;
pub const GL_UNSIGNED_INT: IndexType = IndexType::U32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferTarget {
    Array,
    ElementArray,
    Uniform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferUsage {
    StaticDraw,
    DynamicDraw,
    StreamDraw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VertexAttribType {
    Float32,
    UnsignedShort,
    UnsignedInt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FramebufferAttachment {
    Color(u32),
    Depth,
    DepthStencil,
}

pub trait IntoByteOffset {
    fn into_byte_offset(self) -> u64;
}

pub trait IntoShaderHandle {
    fn into_shader_handle(self) -> ShaderHandle;
}

pub trait IntoMeshHandle {
    fn into_mesh_handle(self) -> MeshHandle;
}

pub trait IntoTextureHandle {
    fn into_texture_handle(self) -> TextureHandle;
}

pub trait IntoFramebufferHandle {
    fn into_framebuffer_handle(self) -> FramebufferHandle;
}

pub trait IntoBufferHandle {
    fn into_buffer_handle(self) -> BufferHandle;
}

pub trait IntoRenderbufferHandle {
    fn into_renderbuffer_handle(self) -> RenderbufferHandle;
}

impl IntoByteOffset for u64 {
    fn into_byte_offset(self) -> u64 {
        self
    }
}

impl IntoByteOffset for usize {
    fn into_byte_offset(self) -> u64 {
        self as u64
    }
}

impl IntoByteOffset for i32 {
    fn into_byte_offset(self) -> u64 {
        self as u64
    }
}

impl<T> IntoByteOffset for *const T {
    fn into_byte_offset(self) -> u64 {
        self as usize as u64
    }
}

impl<T> IntoByteOffset for *mut T {
    fn into_byte_offset(self) -> u64 {
        self as usize as u64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderPassHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FramebufferHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderbufferHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DescriptorSetHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlHandleKind {
    Buffer,
    Texture,
    VertexArray,
    Framebuffer,
    Renderbuffer,
}

impl IntoShaderHandle for ShaderHandle {
    fn into_shader_handle(self) -> ShaderHandle {
        self
    }
}

impl IntoShaderHandle for u32 {
    fn into_shader_handle(self) -> ShaderHandle {
        ShaderHandle(self)
    }
}

impl IntoMeshHandle for MeshHandle {
    fn into_mesh_handle(self) -> MeshHandle {
        self
    }
}

impl IntoMeshHandle for u32 {
    fn into_mesh_handle(self) -> MeshHandle {
        MeshHandle(self)
    }
}

impl IntoTextureHandle for TextureHandle {
    fn into_texture_handle(self) -> TextureHandle {
        self
    }
}

impl IntoTextureHandle for u32 {
    fn into_texture_handle(self) -> TextureHandle {
        TextureHandle(self)
    }
}

impl IntoFramebufferHandle for FramebufferHandle {
    fn into_framebuffer_handle(self) -> FramebufferHandle {
        self
    }
}

impl IntoFramebufferHandle for u32 {
    fn into_framebuffer_handle(self) -> FramebufferHandle {
        FramebufferHandle(self)
    }
}

impl IntoBufferHandle for BufferHandle {
    fn into_buffer_handle(self) -> BufferHandle {
        self
    }
}

impl IntoBufferHandle for u32 {
    fn into_buffer_handle(self) -> BufferHandle {
        BufferHandle(self)
    }
}

impl IntoRenderbufferHandle for RenderbufferHandle {
    fn into_renderbuffer_handle(self) -> RenderbufferHandle {
        self
    }
}

impl IntoRenderbufferHandle for u32 {
    fn into_renderbuffer_handle(self) -> RenderbufferHandle {
        RenderbufferHandle(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendProfile {
    OpenGl,
    Vulkan,
    Raw,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawRenderPassBeginInfo {
    pub render_pass: RenderPassHandle,
    pub framebuffer: FramebufferHandle,
    pub clear_flags: ClearFlags,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GfxCommand {
    SetClearColor([f32; 4]),
    Clear(ClearFlags),
    SetViewport(ViewportState),
    SetRenderState {
        flag: RenderStateFlags,
        enabled: bool,
    },
    UseProgram(ShaderHandle),
    BindPipeline(PipelineHandle),
    BindMesh(MeshHandle),
    BindFramebuffer(FramebufferHandle),
    BindBuffer {
        target: BufferTarget,
        buffer: BufferHandle,
    },
    BindBufferBase {
        target: BufferTarget,
        index: u32,
        buffer: BufferHandle,
    },
    BindTexture {
        slot: u32,
        texture: TextureHandle,
    },
    UploadBufferData {
        target: BufferTarget,
        size_bytes: u64,
        usage: BufferUsage,
    },
    UploadBufferSubData {
        target: BufferTarget,
        offset_bytes: u64,
        size_bytes: u64,
    },
    DefineVertexAttribute {
        index: u32,
        size: i32,
        attrib_type: VertexAttribType,
        normalized: bool,
        stride: i32,
        offset_bytes: u64,
    },
    SetVertexAttributeEnabled {
        index: u32,
        enabled: bool,
    },
    SetVertexAttributeDivisor {
        index: u32,
        divisor: u32,
    },
    SetActiveTextureUnit(u32),
    AttachFramebufferTexture {
        attachment: FramebufferAttachment,
        texture: TextureHandle,
        level: i32,
    },
    AttachFramebufferRenderbuffer {
        attachment: FramebufferAttachment,
        renderbuffer: RenderbufferHandle,
    },
    GenerateHandles {
        kind: GlHandleKind,
        ids: Vec<u32>,
    },
    DeleteHandles {
        kind: GlHandleKind,
        ids: Vec<u32>,
    },
    SetUniformMat4 {
        name: String,
        value: [[f32; 4]; 4],
    },
    SetUniformVec3 {
        name: String,
        value: [f32; 3],
    },
    BeginRenderPass(RenderPassHandle),
    EndRenderPass,
    BindDescriptorSet {
        set: u32,
        descriptor_set: DescriptorSetHandle,
    },
    Draw {
        topology: vk::PrimitiveTopology,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    },
    DrawIndexed {
        topology: vk::PrimitiveTopology,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
        index_type: IndexType,
        index_offset_bytes: u64,
    },
    RawBeginRenderPass {
        begin_info: RawRenderPassBeginInfo,
        contents: vk::SubpassContents,
    },
    RawBindPipeline {
        bind_point: vk::PipelineBindPoint,
        pipeline: PipelineHandle,
    },
    RawDraw {
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    },
    RawDrawIndexed {
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
    },
}

pub trait CommonGfxContext {
    fn clear_color(&mut self, r: f32, g: f32, b: f32, a: f32);
    fn clear(&mut self, flags: ClearFlags);
    fn viewport(&mut self, x: i32, y: i32, width: u32, height: u32);
    fn bind_mesh(&mut self, mesh: MeshHandle);
    fn bind_framebuffer(&mut self, framebuffer: FramebufferHandle);
    fn bind_buffer(&mut self, target: BufferTarget, buffer: BufferHandle);
    fn bind_texture(&mut self, slot: u32, texture: TextureHandle);
    fn enable(&mut self, flag: RenderStateFlags);
    fn disable(&mut self, flag: RenderStateFlags);
    fn commands(&self) -> &[GfxCommand];
}

#[derive(Debug, Clone)]
struct CommandRecorder {
    profile: BackendProfile,
    commands: Vec<GfxCommand>,
}

impl CommandRecorder {
    fn new(profile: BackendProfile) -> Self {
        Self {
            profile,
            commands: Vec::new(),
        }
    }

    fn profile(&self) -> BackendProfile {
        self.profile
    }

    fn push(&mut self, command: GfxCommand) {
        self.commands.push(command);
    }

    fn clear_color(&mut self, rgba: [f32; 4]) {
        self.push(GfxCommand::SetClearColor(rgba));
    }

    fn clear(&mut self, flags: ClearFlags) {
        self.push(GfxCommand::Clear(flags));
    }

    fn viewport(&mut self, x: i32, y: i32, width: u32, height: u32) {
        self.push(GfxCommand::SetViewport(ViewportState {
            x,
            y,
            width,
            height,
        }));
    }

    fn bind_mesh(&mut self, mesh: MeshHandle) {
        self.push(GfxCommand::BindMesh(mesh));
    }

    fn bind_framebuffer(&mut self, framebuffer: FramebufferHandle) {
        self.push(GfxCommand::BindFramebuffer(framebuffer));
    }

    fn bind_buffer(&mut self, target: BufferTarget, buffer: BufferHandle) {
        self.push(GfxCommand::BindBuffer { target, buffer });
    }

    fn bind_buffer_base(&mut self, target: BufferTarget, index: u32, buffer: BufferHandle) {
        self.push(GfxCommand::BindBufferBase {
            target,
            index,
            buffer,
        });
    }

    fn bind_texture(&mut self, slot: u32, texture: TextureHandle) {
        self.push(GfxCommand::BindTexture { slot, texture });
    }

    fn upload_buffer_data(&mut self, target: BufferTarget, size_bytes: u64, usage: BufferUsage) {
        self.push(GfxCommand::UploadBufferData {
            target,
            size_bytes,
            usage,
        });
    }

    fn upload_buffer_sub_data(&mut self, target: BufferTarget, offset_bytes: u64, size_bytes: u64) {
        self.push(GfxCommand::UploadBufferSubData {
            target,
            offset_bytes,
            size_bytes,
        });
    }

    fn define_vertex_attribute(
        &mut self,
        index: u32,
        size: i32,
        attrib_type: VertexAttribType,
        normalized: bool,
        stride: i32,
        offset_bytes: u64,
    ) {
        self.push(GfxCommand::DefineVertexAttribute {
            index,
            size,
            attrib_type,
            normalized,
            stride,
            offset_bytes,
        });
    }

    fn set_vertex_attribute_enabled(&mut self, index: u32, enabled: bool) {
        self.push(GfxCommand::SetVertexAttributeEnabled { index, enabled });
    }

    fn set_vertex_attribute_divisor(&mut self, index: u32, divisor: u32) {
        self.push(GfxCommand::SetVertexAttributeDivisor { index, divisor });
    }

    fn set_active_texture_unit(&mut self, slot: u32) {
        self.push(GfxCommand::SetActiveTextureUnit(slot));
    }

    fn attach_framebuffer_texture(
        &mut self,
        attachment: FramebufferAttachment,
        texture: TextureHandle,
        level: i32,
    ) {
        self.push(GfxCommand::AttachFramebufferTexture {
            attachment,
            texture,
            level,
        });
    }

    fn attach_framebuffer_renderbuffer(
        &mut self,
        attachment: FramebufferAttachment,
        renderbuffer: RenderbufferHandle,
    ) {
        self.push(GfxCommand::AttachFramebufferRenderbuffer {
            attachment,
            renderbuffer,
        });
    }

    fn generate_handles(&mut self, kind: GlHandleKind, ids: Vec<u32>) {
        self.push(GfxCommand::GenerateHandles { kind, ids });
    }

    fn delete_handles(&mut self, kind: GlHandleKind, ids: Vec<u32>) {
        self.push(GfxCommand::DeleteHandles { kind, ids });
    }

    fn set_state(&mut self, flag: RenderStateFlags, enabled: bool) {
        self.push(GfxCommand::SetRenderState { flag, enabled });
    }

    fn use_program(&mut self, shader: ShaderHandle) {
        self.push(GfxCommand::UseProgram(shader));
    }

    fn set_uniform_mat4(&mut self, name: &str, value: [[f32; 4]; 4]) {
        self.push(GfxCommand::SetUniformMat4 {
            name: name.to_string(),
            value,
        });
    }

    fn set_uniform_vec3(&mut self, name: &str, value: [f32; 3]) {
        self.push(GfxCommand::SetUniformVec3 {
            name: name.to_string(),
            value,
        });
    }

    fn begin_render_pass(&mut self, render_pass: RenderPassHandle) {
        self.push(GfxCommand::BeginRenderPass(render_pass));
    }

    fn end_render_pass(&mut self) {
        self.push(GfxCommand::EndRenderPass);
    }

    fn bind_pipeline(&mut self, pipeline: PipelineHandle) {
        self.push(GfxCommand::BindPipeline(pipeline));
    }

    fn bind_descriptor_set(&mut self, set: u32, descriptor_set: DescriptorSetHandle) {
        self.push(GfxCommand::BindDescriptorSet {
            set,
            descriptor_set,
        });
    }

    fn draw(
        &mut self,
        topology: vk::PrimitiveTopology,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) {
        self.push(GfxCommand::Draw {
            topology,
            vertex_count,
            instance_count,
            first_vertex,
            first_instance,
        });
    }

    fn draw_indexed(
        &mut self,
        topology: vk::PrimitiveTopology,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
        index_type: IndexType,
        index_offset_bytes: u64,
    ) {
        self.push(GfxCommand::DrawIndexed {
            topology,
            index_count,
            instance_count,
            first_index,
            vertex_offset,
            first_instance,
            index_type,
            index_offset_bytes,
        });
    }

    fn raw_begin_render_pass(
        &mut self,
        begin_info: RawRenderPassBeginInfo,
        contents: vk::SubpassContents,
    ) {
        self.push(GfxCommand::RawBeginRenderPass {
            begin_info,
            contents,
        });
    }

    fn raw_bind_pipeline(&mut self, bind_point: vk::PipelineBindPoint, pipeline: PipelineHandle) {
        self.push(GfxCommand::RawBindPipeline {
            bind_point,
            pipeline,
        });
    }

    fn raw_draw(
        &mut self,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) {
        self.push(GfxCommand::RawDraw {
            vertex_count,
            instance_count,
            first_vertex,
            first_instance,
        });
    }

    fn raw_draw_indexed(
        &mut self,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
    ) {
        self.push(GfxCommand::RawDrawIndexed {
            index_count,
            instance_count,
            first_index,
            vertex_offset,
            first_instance,
        });
    }

    fn commands(&self) -> &[GfxCommand] {
        &self.commands
    }
}

pub mod opengl {
    use super::*;
    pub use super::{
        BLEND, BufferHandle, BufferTarget, BufferUsage, CLEAR_COLOR, CLEAR_DEPTH, CLEAR_STENCIL,
        CULL_FACE, ClearFlags, CommonGfxContext, DEPTH_TEST, DescriptorSetHandle,
        FramebufferAttachment, FramebufferHandle, GL_ARRAY_BUFFER, GL_COLOR_ATTACHMENT0,
        GL_COLOR_BUFFER_BIT, GL_DEPTH_ATTACHMENT, GL_DEPTH_BUFFER_BIT, GL_DEPTH_STENCIL_ATTACHMENT,
        GL_DYNAMIC_DRAW, GL_ELEMENT_ARRAY_BUFFER, GL_FALSE, GL_FLOAT, GL_FRAMEBUFFER, GL_LINES,
        GL_POINTS, GL_RENDERBUFFER, GL_STATIC_DRAW, GL_STENCIL_BUFFER_BIT, GL_STREAM_DRAW,
        GL_TEXTURE_2D, GL_TEXTURE0, GL_TRIANGLES, GL_TRUE, GL_UNIFORM_BUFFER, GL_UNSIGNED_INT,
        GL_UNSIGNED_SHORT, GfxCommand, GlHandleKind, IndexType, IntoBufferHandle, IntoByteOffset,
        IntoFramebufferHandle, IntoMeshHandle, IntoRenderbufferHandle, IntoShaderHandle,
        IntoTextureHandle, MeshHandle, PipelineHandle, RawRenderPassBeginInfo, RenderPassHandle,
        RenderStateFlags, RenderbufferHandle, ShaderHandle, TextureHandle, VertexAttribType,
        ViewportState,
    };

    #[derive(Debug, Clone)]
    pub struct BackendContext {
        recorder: CommandRecorder,
        active_texture_slot: u32,
        next_generated_id: u32,
    }

    impl Default for BackendContext {
        fn default() -> Self {
            Self::new()
        }
    }

    impl BackendContext {
        pub fn new() -> Self {
            Self {
                recorder: CommandRecorder::new(BackendProfile::OpenGl),
                active_texture_slot: 0,
                next_generated_id: 1,
            }
        }

        pub fn reset_commands(&mut self) {
            self.recorder.commands.clear();
        }

        pub fn command_count(&self) -> usize {
            self.recorder.commands().len()
        }

        pub fn profile(&self) -> BackendProfile {
            self.recorder.profile()
        }

        pub fn use_program<T: IntoShaderHandle>(&mut self, shader: T) {
            self.recorder.use_program(shader.into_shader_handle());
        }

        pub fn use_shader<T: IntoShaderHandle>(&mut self, shader: T) {
            self.use_program(shader);
        }

        pub fn set_uniform_mat4(&mut self, name: &str, value: [[f32; 4]; 4]) {
            self.recorder.set_uniform_mat4(name, value);
        }

        pub fn uniform_mat4(&mut self, name: &str, value: [[f32; 4]; 4]) {
            self.set_uniform_mat4(name, value);
        }

        pub fn set_uniform_vec3(&mut self, name: &str, value: [f32; 3]) {
            self.recorder.set_uniform_vec3(name, value);
        }

        pub fn uniform_vec3(&mut self, name: &str, value: [f32; 3]) {
            self.set_uniform_vec3(name, value);
        }

        pub fn bind_texture_2d<T: IntoTextureHandle>(&mut self, texture: T) {
            self.recorder
                .bind_texture(self.active_texture_slot, texture.into_texture_handle());
        }

        pub fn bind_vertex_array<T: IntoMeshHandle>(&mut self, mesh: T) {
            self.recorder.bind_mesh(mesh.into_mesh_handle());
        }

        pub fn bind_buffer<T: IntoBufferHandle>(&mut self, target: BufferTarget, buffer: T) {
            self.recorder
                .bind_buffer(target, buffer.into_buffer_handle());
        }

        pub fn bind_buffer_base<T: IntoBufferHandle>(
            &mut self,
            target: BufferTarget,
            index: u32,
            buffer: T,
        ) {
            self.recorder
                .bind_buffer_base(target, index, buffer.into_buffer_handle());
        }

        pub fn buffer_data<T>(
            &mut self,
            target: BufferTarget,
            size_bytes: u64,
            _data: T,
            usage: BufferUsage,
        ) {
            self.recorder.upload_buffer_data(target, size_bytes, usage);
        }

        pub fn buffer_sub_data<T: IntoByteOffset, U>(
            &mut self,
            target: BufferTarget,
            offset_bytes: T,
            size_bytes: u64,
            _data: U,
        ) {
            self.recorder.upload_buffer_sub_data(
                target,
                offset_bytes.into_byte_offset(),
                size_bytes,
            );
        }

        pub fn vertex_attrib_pointer<T: IntoByteOffset>(
            &mut self,
            index: u32,
            size: i32,
            attrib_type: VertexAttribType,
            normalized: bool,
            stride: i32,
            offset_bytes: T,
        ) {
            self.recorder.define_vertex_attribute(
                index,
                size,
                attrib_type,
                normalized,
                stride,
                offset_bytes.into_byte_offset(),
            );
        }

        pub fn enable_vertex_attrib_array(&mut self, index: u32) {
            self.recorder.set_vertex_attribute_enabled(index, true);
        }

        pub fn disable_vertex_attrib_array(&mut self, index: u32) {
            self.recorder.set_vertex_attribute_enabled(index, false);
        }

        pub fn vertex_attrib_divisor(&mut self, index: u32, divisor: u32) {
            self.recorder.set_vertex_attribute_divisor(index, divisor);
        }

        pub fn active_texture(&mut self, slot: u32) {
            self.active_texture_slot = slot;
            self.recorder.set_active_texture_unit(slot);
        }

        pub fn framebuffer_texture_2d<T: IntoTextureHandle>(
            &mut self,
            attachment: FramebufferAttachment,
            texture: T,
            level: i32,
        ) {
            self.recorder.attach_framebuffer_texture(
                attachment,
                texture.into_texture_handle(),
                level,
            );
        }

        pub fn framebuffer_renderbuffer<T: IntoRenderbufferHandle>(
            &mut self,
            attachment: FramebufferAttachment,
            renderbuffer: T,
        ) {
            self.recorder.attach_framebuffer_renderbuffer(
                attachment,
                renderbuffer.into_renderbuffer_handle(),
            );
        }

        pub fn bind_framebuffer<T: IntoFramebufferHandle>(&mut self, framebuffer: T) {
            self.recorder
                .bind_framebuffer(framebuffer.into_framebuffer_handle());
        }

        pub fn draw_elements(&mut self, mode: vk::PrimitiveTopology, count: u32) {
            self.recorder
                .draw_indexed(mode, count, 1, 0, 0, 0, GL_UNSIGNED_INT, 0);
        }

        pub fn draw_elements_typed<T: IntoByteOffset>(
            &mut self,
            mode: vk::PrimitiveTopology,
            count: u32,
            index_type: IndexType,
            offset_bytes: T,
        ) {
            self.recorder.draw_indexed(
                mode,
                count,
                1,
                0,
                0,
                0,
                index_type,
                offset_bytes.into_byte_offset(),
            );
        }

        pub fn draw_arrays(&mut self, mode: vk::PrimitiveTopology, first: u32, count: u32) {
            self.recorder.draw(mode, count, 1, first, 0);
        }

        pub fn gen_buffers(&mut self, count: u32, ids: &mut [u32]) {
            self.generate_handles(GlHandleKind::Buffer, count, ids);
        }

        pub fn delete_buffers(&mut self, _count: u32, ids: &[u32]) {
            self.recorder
                .delete_handles(GlHandleKind::Buffer, ids.to_vec());
        }

        pub fn gen_textures(&mut self, count: u32, ids: &mut [u32]) {
            self.generate_handles(GlHandleKind::Texture, count, ids);
        }

        pub fn delete_textures(&mut self, _count: u32, ids: &[u32]) {
            self.recorder
                .delete_handles(GlHandleKind::Texture, ids.to_vec());
        }

        pub fn gen_vertex_arrays(&mut self, count: u32, ids: &mut [u32]) {
            self.generate_handles(GlHandleKind::VertexArray, count, ids);
        }

        pub fn delete_vertex_arrays(&mut self, _count: u32, ids: &[u32]) {
            self.recorder
                .delete_handles(GlHandleKind::VertexArray, ids.to_vec());
        }

        pub fn gen_framebuffers(&mut self, count: u32, ids: &mut [u32]) {
            self.generate_handles(GlHandleKind::Framebuffer, count, ids);
        }

        pub fn delete_framebuffers(&mut self, _count: u32, ids: &[u32]) {
            self.recorder
                .delete_handles(GlHandleKind::Framebuffer, ids.to_vec());
        }

        pub fn gen_renderbuffers(&mut self, count: u32, ids: &mut [u32]) {
            self.generate_handles(GlHandleKind::Renderbuffer, count, ids);
        }

        pub fn delete_renderbuffers(&mut self, _count: u32, ids: &[u32]) {
            self.recorder
                .delete_handles(GlHandleKind::Renderbuffer, ids.to_vec());
        }

        fn generate_handles(&mut self, kind: GlHandleKind, count: u32, ids: &mut [u32]) {
            let count = count.min(ids.len() as u32);
            let generated = (0..count)
                .map(|_| {
                    let id = self.next_generated_id;
                    self.next_generated_id += 1;
                    id
                })
                .collect::<Vec<_>>();
            for (slot, id) in ids.iter_mut().zip(generated.iter().copied()) {
                *slot = id;
            }
            self.recorder.generate_handles(kind, generated);
        }
    }

    impl CommonGfxContext for BackendContext {
        fn clear_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
            self.recorder.clear_color([r, g, b, a]);
        }

        fn clear(&mut self, flags: ClearFlags) {
            self.recorder.clear(flags);
        }

        fn viewport(&mut self, x: i32, y: i32, width: u32, height: u32) {
            self.recorder.viewport(x, y, width, height);
        }

        fn bind_mesh(&mut self, mesh: MeshHandle) {
            self.recorder.bind_mesh(mesh);
        }

        fn bind_framebuffer(&mut self, framebuffer: FramebufferHandle) {
            self.recorder.bind_framebuffer(framebuffer);
        }

        fn bind_buffer(&mut self, target: BufferTarget, buffer: BufferHandle) {
            self.recorder.bind_buffer(target, buffer);
        }

        fn bind_texture(&mut self, slot: u32, texture: TextureHandle) {
            self.recorder.bind_texture(slot, texture);
        }

        fn enable(&mut self, flag: RenderStateFlags) {
            self.recorder.set_state(flag, true);
        }

        fn disable(&mut self, flag: RenderStateFlags) {
            self.recorder.set_state(flag, false);
        }

        fn commands(&self) -> &[GfxCommand] {
            self.recorder.commands()
        }
    }
}

pub mod vulkan {
    use super::*;
    pub use super::{
        BLEND, BufferHandle, BufferTarget, CLEAR_COLOR, CLEAR_DEPTH, CLEAR_STENCIL, CULL_FACE,
        ClearFlags, CommonGfxContext, DEPTH_TEST, DescriptorSetHandle, FramebufferHandle,
        GfxCommand, MeshHandle, PipelineHandle, RawRenderPassBeginInfo, RenderPassHandle,
        RenderStateFlags, ShaderHandle, TextureHandle, VK_LINE_LIST, VK_POINT_LIST,
        VK_TRIANGLE_LIST, ViewportState,
    };

    #[derive(Debug, Clone)]
    pub struct BackendContext {
        recorder: CommandRecorder,
    }

    impl Default for BackendContext {
        fn default() -> Self {
            Self::new()
        }
    }

    impl BackendContext {
        pub fn new() -> Self {
            Self {
                recorder: CommandRecorder::new(BackendProfile::Vulkan),
            }
        }

        pub fn profile(&self) -> BackendProfile {
            self.recorder.profile()
        }

        pub fn begin_render_pass(&mut self, render_pass: &RenderPassHandle) {
            self.recorder.begin_render_pass(*render_pass);
        }

        pub fn end_render_pass(&mut self) {
            self.recorder.end_render_pass();
        }

        pub fn bind_pipeline(&mut self, pipeline: &PipelineHandle) {
            self.recorder.bind_pipeline(*pipeline);
        }

        pub fn bind_descriptor_set(&mut self, set: u32, descriptor_set: &DescriptorSetHandle) {
            self.recorder.bind_descriptor_set(set, *descriptor_set);
        }

        pub fn draw(
            &mut self,
            vertex_count: u32,
            instance_count: u32,
            first_vertex: u32,
            first_instance: u32,
        ) {
            self.recorder.draw(
                vk::PrimitiveTopology::TRIANGLE_LIST,
                vertex_count,
                instance_count,
                first_vertex,
                first_instance,
            );
        }

        pub fn draw_indexed(
            &mut self,
            index_count: u32,
            instance_count: u32,
            first_index: u32,
            vertex_offset: i32,
            first_instance: u32,
        ) {
            self.recorder.draw_indexed(
                vk::PrimitiveTopology::TRIANGLE_LIST,
                index_count,
                instance_count,
                first_index,
                vertex_offset,
                first_instance,
                GL_UNSIGNED_INT,
                0,
            );
        }
    }

    impl CommonGfxContext for BackendContext {
        fn clear_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
            self.recorder.clear_color([r, g, b, a]);
        }

        fn clear(&mut self, flags: ClearFlags) {
            self.recorder.clear(flags);
        }

        fn viewport(&mut self, x: i32, y: i32, width: u32, height: u32) {
            self.recorder.viewport(x, y, width, height);
        }

        fn bind_mesh(&mut self, mesh: MeshHandle) {
            self.recorder.bind_mesh(mesh);
        }

        fn bind_framebuffer(&mut self, framebuffer: FramebufferHandle) {
            self.recorder.bind_framebuffer(framebuffer);
        }

        fn bind_buffer(&mut self, target: BufferTarget, buffer: BufferHandle) {
            self.recorder.bind_buffer(target, buffer);
        }

        fn bind_texture(&mut self, slot: u32, texture: TextureHandle) {
            self.recorder.bind_texture(slot, texture);
        }

        fn enable(&mut self, flag: RenderStateFlags) {
            self.recorder.set_state(flag, true);
        }

        fn disable(&mut self, flag: RenderStateFlags) {
            self.recorder.set_state(flag, false);
        }

        fn commands(&self) -> &[GfxCommand] {
            self.recorder.commands()
        }
    }
}

pub mod raw {
    use super::*;

    pub use super::{
        BLEND, BufferHandle, BufferTarget, CLEAR_COLOR, CLEAR_DEPTH, CLEAR_STENCIL, CULL_FACE,
        ClearFlags, CommonGfxContext, DEPTH_TEST, DescriptorSetHandle, FramebufferHandle,
        GfxCommand, MeshHandle, PipelineHandle, RawRenderPassBeginInfo, RenderPassHandle,
        RenderStateFlags, ShaderHandle, TextureHandle, ViewportState,
    };
    pub use ash::vk::{PipelineBindPoint, SubpassContents};

    #[derive(Debug, Clone)]
    pub struct BackendContext {
        recorder: CommandRecorder,
    }

    impl Default for BackendContext {
        fn default() -> Self {
            Self::new()
        }
    }

    impl BackendContext {
        pub fn new() -> Self {
            Self {
                recorder: CommandRecorder::new(BackendProfile::Raw),
            }
        }

        pub fn profile(&self) -> BackendProfile {
            self.recorder.profile()
        }

        pub fn cmd_begin_render_pass(
            &mut self,
            begin_info: &RawRenderPassBeginInfo,
            contents: vk::SubpassContents,
        ) {
            self.recorder
                .raw_begin_render_pass(begin_info.clone(), contents);
        }

        pub fn cmd_bind_pipeline(
            &mut self,
            bind_point: vk::PipelineBindPoint,
            pipeline: PipelineHandle,
        ) {
            self.recorder.raw_bind_pipeline(bind_point, pipeline);
        }

        pub fn cmd_bind_mesh(&mut self, mesh: MeshHandle) {
            self.recorder.bind_mesh(mesh);
        }

        pub fn cmd_draw(
            &mut self,
            vertex_count: u32,
            instance_count: u32,
            first_vertex: u32,
            first_instance: u32,
        ) {
            self.recorder
                .raw_draw(vertex_count, instance_count, first_vertex, first_instance);
        }

        pub fn cmd_draw_indexed(
            &mut self,
            index_count: u32,
            instance_count: u32,
            first_index: u32,
            vertex_offset: i32,
            first_instance: u32,
        ) {
            self.recorder.raw_draw_indexed(
                index_count,
                instance_count,
                first_index,
                vertex_offset,
                first_instance,
            );
        }
    }

    impl CommonGfxContext for BackendContext {
        fn clear_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
            self.recorder.clear_color([r, g, b, a]);
        }

        fn clear(&mut self, flags: ClearFlags) {
            self.recorder.clear(flags);
        }

        fn viewport(&mut self, x: i32, y: i32, width: u32, height: u32) {
            self.recorder.viewport(x, y, width, height);
        }

        fn bind_mesh(&mut self, mesh: MeshHandle) {
            self.recorder.bind_mesh(mesh);
        }

        fn bind_framebuffer(&mut self, framebuffer: FramebufferHandle) {
            self.recorder.bind_framebuffer(framebuffer);
        }

        fn bind_buffer(&mut self, target: BufferTarget, buffer: BufferHandle) {
            self.recorder.bind_buffer(target, buffer);
        }

        fn bind_texture(&mut self, slot: u32, texture: TextureHandle) {
            self.recorder.bind_texture(slot, texture);
        }

        fn enable(&mut self, flag: RenderStateFlags) {
            self.recorder.set_state(flag, true);
        }

        fn disable(&mut self, flag: RenderStateFlags) {
            self.recorder.set_state(flag, false);
        }

        fn commands(&self) -> &[GfxCommand] {
            self.recorder.commands()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opengl_profile_records_gl_style_commands() {
        let mut ctx = opengl::BackendContext::new();
        ctx.clear_color(0.0, 0.0, 0.0, 1.0);
        ctx.clear(CLEAR_COLOR | CLEAR_DEPTH);
        ctx.use_program(ShaderHandle(7));
        ctx.bind_mesh(MeshHandle(3));
        ctx.draw_elements(GL_TRIANGLES, 36);

        assert_eq!(ctx.profile(), BackendProfile::OpenGl);
        assert_eq!(ctx.commands().len(), 5);
    }

    #[test]
    fn vulkan_profile_records_vk_style_commands() {
        let mut ctx = vulkan::BackendContext::new();
        ctx.begin_render_pass(&RenderPassHandle(1));
        ctx.bind_pipeline(&PipelineHandle(2));
        ctx.bind_mesh(MeshHandle(3));
        ctx.draw(36, 1, 0, 0);

        assert_eq!(ctx.profile(), BackendProfile::Vulkan);
        assert_eq!(ctx.commands().len(), 4);
    }

    #[test]
    fn raw_profile_records_cmd_style_commands() {
        let mut ctx = raw::BackendContext::new();
        let begin_info = RawRenderPassBeginInfo {
            render_pass: RenderPassHandle(1),
            framebuffer: FramebufferHandle(9),
            clear_flags: CLEAR_COLOR | CLEAR_DEPTH,
        };
        ctx.cmd_begin_render_pass(&begin_info, vk::SubpassContents::INLINE);
        ctx.cmd_bind_pipeline(vk::PipelineBindPoint::GRAPHICS, PipelineHandle(2));
        ctx.cmd_draw(36, 1, 0, 0);

        assert_eq!(ctx.profile(), BackendProfile::Raw);
        assert_eq!(ctx.commands().len(), 3);
    }
}
