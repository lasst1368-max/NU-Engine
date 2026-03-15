pub mod app;
pub mod backend;
pub mod core;
pub mod demo;
pub mod editor;
pub mod engine;
pub mod event;
pub mod executor;
pub mod ffi;
pub mod graphics;
pub mod lighting;
pub mod physics;
pub mod pipeline;
pub mod renderer;
pub mod resource;
pub mod rhi;
pub mod runtime;
pub mod scene;
pub mod script;
pub mod syntax;

pub use api_macros::use_backend;
use ash::vk;
use core::{VulkanContext, VulkanContextBuilder};
use pipeline::PipelineLibrary;
use renderer::{FrameContext, FramePacket, Renderer2D, Renderer3D};
use resource::ResourceRegistry;

pub use app::{WindowApp, WindowConfig, run_window_app};
pub use backend::{
    ALL_BACKEND_INFO, BackendInfo, DX12_BACKEND_INFO, GraphicsBackendKind, METAL_BACKEND_INFO,
    VULKAN_BACKEND_INFO, backend_count, backend_info,
};
pub use core::{ApiConfig, ApiError};
pub use demo::{run_physics_demo, run_spinning_block_demo, run_square_demo};
pub use editor::{
    SceneEditor,
    ui::{EditorUiError, run_basic_scene_editor},
};
pub use engine::world::{
    CameraComponent, EntityId, LightComponent, MeshRendererComponent, NuSceneWorld,
    PhysicsBodyComponent, SceneEntity, TransformComponent,
};
pub use engine::{
    EXPLICIT_NUSCENE_TEMPLATE, EngineError, HotReloadManager, LightKind, NU_SCENE_EXTENSION,
    NU_SCENE_FORMAT_HEADER, NuCameraSection, NuEnvironmentSection, NuLightSection,
    NuMaterialSection, NuMeshScriptSection, NuMeshSection, NuPhysicsBodyKind,
    NuPhysicsColliderKind, NuPhysicsSection, NuSceneDocument, NuSceneMetadata,
    NuScenePhysicsBinding, NuScenePhysicsRuntime, NuSceneSection, NuTransform, ReloadBatch,
    ReloadedShader, ReloadedTexture, SceneAssetBindings, SceneAssetReferences, SceneBackend,
    SceneSyntax, ShaderProgramAssetHandles, ShaderProgramPaths, ShaderStage,
    build_scene_physics_runtime, build_scene_physics_runtime_with_config, build_scene_world,
    load_obj_mesh_asset, load_scene_file, parse_scene_str, publish_reload_batch_events,
    register_scene_assets,
};
pub use event::{
    EngineEvent, EventBus, EventCategoryMask, EventDeliveryMode, EventListenerHandle,
    EventSubscription, ResourceEventKind,
};
pub use executor::{
    BufferBinding, DeferredAction, DescriptorSetBinding, DescriptorSetLayoutBindings,
    DescriptorWritePlan, ExecutionReport, ExecutorError, ExecutorResources,
    FramebufferAttachmentSource, FramebufferBinding, GraphicsPipelineCache,
    GraphicsPipelineCompiler, GraphicsPipelineDescriptor, MeshBinding, PipelineBinding,
    PipelineProgramContext, ShaderBinding, ShaderProgramDefinition, TextureBinding,
    UniformValueKind, VertexAttributeBinding, VulkanExecutor, VulkanGraphicsPipelineCompiler,
    apply_descriptor_writes, resolve_descriptor_writes_for_bindings,
};
pub use lighting::{
    DirectionalLight, LightingConfig, LiveShadowConfig, MAX_POINT_LIGHTS, MAX_SPOT_LIGHTS,
    PointLight, ShadowConfig, ShadowMode, SpotLight,
};
pub use physics::{
    BodyHandle, BodyType, ColliderShape, CollisionContact, PhysicsConfig, PhysicsMaterial,
    PhysicsWorld, RigidBody, detect_collision,
};
pub use resource::{
    AssetHandle, AssetKind, AssetManager, AssetRecord, AssetState, BufferDesc, BufferHandle,
    BufferUsage, ImageDesc, ImageHandle, ImageUsage,
};
pub use rhi::vulkan::{
    VulkanRhiBuffer, VulkanRhiCommandRecorder, VulkanRhiDevice, VulkanRhiDriver,
    VulkanRhiGraphicsPipeline, VulkanRhiSurface, VulkanRhiTexture,
};
pub use rhi::{
    AdapterPreference, BufferDesc as RhiBufferDesc, BufferInfo as RhiBufferInfo,
    BufferUsage as RhiBufferUsage, DRIVER_CATALOG, DeviceRequest, Driver, DriverApi, DriverBuffer,
    DriverCommandRecorder, DriverDescriptor, DriverDevice, DriverError, DriverGraphicsPipeline,
    DriverSurface, DriverTexture, GraphicsPipelineDesc,
    GraphicsPipelineInfo as RhiGraphicsPipelineInfo, PresentMode, PrimitiveTopology,
    ShaderStage as RhiShaderStage, SurfaceConfig, TextureDesc as RhiTextureDesc, TextureFormat,
    TextureInfo as RhiTextureInfo, VertexAttributeDesc as RhiVertexAttributeDesc,
    VertexBindingDesc as RhiVertexBindingDesc, VertexFormat as RhiVertexFormat,
    VertexInputRate as RhiVertexInputRate, driver_catalog,
};
pub use runtime::{
    SampledTextureDescriptorResources, create_sampled_texture_descriptor_resources,
    create_sampled_texture_descriptor_set_layout, register_sampled_texture_descriptor,
    resolve_sampled_texture_descriptor_writes, run_scene, update_sampled_texture_descriptor,
};
pub use scene::{
    Camera2D, Camera3D, Canvas2D, CircleDraw, CubeDraw3D, DrawSpace, Frustum, LineDraw, Mesh3D,
    MeshAsset3D, MeshDraw3D, MeshMaterial3D, MeshVertex3D, PrimitiveDraw, QuadDraw, RectDraw,
    Scene, SceneConfig, SceneFrame, ScreenshotResolution, ShapeStyle, SphereDraw3D, SquareDraw,
    TextAnchor, TextDraw,
};
pub use scene::primitives::{
    generate_capsule, generate_cone, generate_cylinder, generate_icosphere, generate_torus,
};
pub use scene::sculpt::{BrushFalloff, BrushMode, SculptBrush, SculptMesh};
pub use script::{NaMoveBinding, NaMoveDirection, NaScriptProgram, parse_na_script};
pub mod prelude {
    pub use crate::syntax::opengl::*;
}

pub struct HighPowerVulkanApi {
    pub context: VulkanContext,
    pub pipelines: PipelineLibrary,
    pub resources: ResourceRegistry,
    pub renderer_2d: Renderer2D,
    pub renderer_3d: Renderer3D,
    frame_index: u64,
}

impl HighPowerVulkanApi {
    pub fn builder() -> VulkanContextBuilder {
        VulkanContextBuilder::new(ApiConfig::default())
    }

    pub fn bootstrap(config: ApiConfig) -> Result<Self, ApiError> {
        let context = VulkanContextBuilder::new(config).build_stub()?;
        Ok(Self::from_context(context))
    }

    pub fn from_context(context: VulkanContext) -> Self {
        Self {
            context,
            pipelines: PipelineLibrary::default(),
            resources: ResourceRegistry::default(),
            renderer_2d: Renderer2D::default(),
            renderer_3d: Renderer3D::default(),
            frame_index: 0,
        }
    }

    pub fn begin_frame(&mut self, extent: vk::Extent2D, delta_time_seconds: f32) -> FrameContext {
        self.frame_index += 1;
        FrameContext {
            frame_index: self.frame_index,
            image_index: 0,
            viewport: extent,
            delta_time_seconds,
        }
    }

    pub fn end_frame(&mut self, frame: FrameContext) -> Result<FramePacket, ApiError> {
        if frame.frame_index != self.frame_index {
            return Err(ApiError::InvalidFrameState {
                reason: "frame index does not match active frame".to_string(),
            });
        }

        let sprite_draws = self.renderer_2d.drain();
        let mesh_draws = self.renderer_3d.drain();
        let estimated_gpu_cost = self
            .renderer_2d
            .estimate_gpu_cost(sprite_draws.len() as u32)
            + self.renderer_3d.estimate_gpu_cost(mesh_draws.len() as u32);

        Ok(FramePacket {
            frame_index: frame.frame_index,
            viewport: frame.viewport,
            sprite_draws,
            mesh_draws,
            estimated_gpu_cost,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::EngineMode;
    use crate::renderer::{MeshDraw, SpriteDraw};
    use crate::syntax::opengl::{
        BufferHandle, FramebufferHandle, GL_ARRAY_BUFFER, GL_COLOR_ATTACHMENT0,
        GL_DEPTH_ATTACHMENT, GL_FALSE, GL_FLOAT, GL_STATIC_DRAW, GL_TEXTURE0, GL_UNIFORM_BUFFER,
        GfxCommand, GlHandleKind, MeshHandle, RenderbufferHandle, TextureHandle,
    };

    #[test]
    fn boilerplate_collects_2d_and_3d_work() {
        let mut config = ApiConfig::default();
        config.mode = EngineMode::Hybrid;

        let mut api = HighPowerVulkanApi::bootstrap(config).expect("bootstrap should succeed");
        let frame = api.begin_frame(
            vk::Extent2D {
                width: 1920,
                height: 1080,
            },
            0.016,
        );

        api.renderer_2d.queue_sprite(SpriteDraw::default());
        api.renderer_3d.queue_mesh(MeshDraw::default());

        let packet = api.end_frame(frame).expect("end_frame should succeed");
        assert_eq!(packet.sprite_draws.len(), 1);
        assert_eq!(packet.mesh_draws.len(), 1);
        assert!(packet.estimated_gpu_cost > 0);
    }

    #[use_backend(opengl)]
    fn opengl_compatibility_macro_records_commands() -> usize {
        gl.clear_color(0.0, 0.0, 0.0, 1.0);
        gl.clear(CLEAR_COLOR | CLEAR_DEPTH);
        gl.use_program(ShaderHandle(4));
        gl.bind_mesh(MeshHandle(2));
        gl.draw_elements(GL_TRIANGLES, 36);
        ctx.commands().len()
    }

    #[test]
    fn compatibility_macro_supports_opengl_profile() {
        assert_eq!(opengl_compatibility_macro_records_commands(), 5);
    }

    #[use_backend(opengl)]
    fn opengl_receiver_style_buffer_commands() -> usize {
        gl.bind_buffer(GL_ARRAY_BUFFER, BufferHandle(3));
        gl.buffer_data(GL_ARRAY_BUFFER, 48, std::ptr::null::<u8>(), GL_DYNAMIC_DRAW);
        gl.vertex_attrib_pointer(0, 3, GL_FLOAT, GL_FALSE, 24, std::ptr::null::<u8>());
        gl.enable_vertex_attrib_array(0);
        gl.active_texture(GL_TEXTURE0 + 2);
        gl.bind_texture_2d(TextureHandle(5));
        gl.framebuffer_texture_2d(GL_COLOR_ATTACHMENT0, TextureHandle(6), 0);
        ctx.commands().len()
    }

    #[test]
    fn compatibility_macro_supports_receiver_style_gl_buffer_calls() {
        assert_eq!(opengl_receiver_style_buffer_commands(), 7);
    }

    #[use_backend]
    fn opengl_free_function_macro_records_commands() -> usize {
        glClearColor(0.0, 0.0, 0.0, 1.0);
        glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
        glUseProgram(ShaderHandle(4));
        glBindVertexArray(MeshHandle(2));
        glBindTexture(GL_TEXTURE_2D, TextureHandle(6));
        glBindFramebuffer(GL_FRAMEBUFFER, FramebufferHandle(7));
        glUniformMatrix4fv(
            "u_mvp",
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        );
        glUniform3fv("u_light", [1.0, 2.0, 3.0]);
        glEnable(DEPTH_TEST);
        glViewport(0, 0, 1280, 720);
        glDrawElements(GL_TRIANGLES, 36, GL_UNSIGNED_INT, 0);
        ctx.commands().len()
    }

    #[test]
    fn compatibility_macro_supports_gl_free_function_rewrite() {
        assert_eq!(opengl_free_function_macro_records_commands(), 11);
    }

    #[use_backend]
    fn opengl_buffer_and_framebuffer_rewrite_commands() -> Vec<GfxCommand> {
        glBindBuffer(GL_ARRAY_BUFFER, BufferHandle(11));
        glBufferData(GL_ARRAY_BUFFER, 96, std::ptr::null::<u8>(), GL_STATIC_DRAW);
        glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, 24, 0);
        glEnableVertexAttribArray(0);
        glDisableVertexAttribArray(1);
        glActiveTexture(GL_TEXTURE0 + 1);
        glBindTexture(GL_TEXTURE_2D, TextureHandle(12));
        glFramebufferTexture2D(
            GL_FRAMEBUFFER,
            GL_COLOR_ATTACHMENT0,
            GL_TEXTURE_2D,
            TextureHandle(13),
            0,
        );
        ctx.commands().to_vec()
    }

    #[test]
    fn compatibility_macro_supports_gl_buffer_and_framebuffer_rewrite() {
        let commands = opengl_buffer_and_framebuffer_rewrite_commands();
        assert_eq!(
            commands,
            vec![
                GfxCommand::BindBuffer {
                    target: GL_ARRAY_BUFFER,
                    buffer: BufferHandle(11),
                },
                GfxCommand::UploadBufferData {
                    target: GL_ARRAY_BUFFER,
                    size_bytes: 96,
                    usage: GL_STATIC_DRAW,
                },
                GfxCommand::DefineVertexAttribute {
                    index: 0,
                    size: 3,
                    attrib_type: GL_FLOAT,
                    normalized: GL_FALSE,
                    stride: 24,
                    offset_bytes: 0,
                },
                GfxCommand::SetVertexAttributeEnabled {
                    index: 0,
                    enabled: true,
                },
                GfxCommand::SetVertexAttributeEnabled {
                    index: 1,
                    enabled: false,
                },
                GfxCommand::SetActiveTextureUnit(GL_TEXTURE0 + 1),
                GfxCommand::BindTexture {
                    slot: GL_TEXTURE0 + 1,
                    texture: TextureHandle(12),
                },
                GfxCommand::AttachFramebufferTexture {
                    attachment: GL_COLOR_ATTACHMENT0,
                    texture: TextureHandle(13),
                    level: 0,
                },
            ]
        );
    }

    #[use_backend]
    fn opengl_resource_lifecycle_commands() -> Vec<GfxCommand> {
        let mut buffers = [0_u32; 2];
        let mut vertex_arrays = [0_u32; 1];
        let mut framebuffers = [0_u32; 1];
        let mut renderbuffers = [0_u32; 1];

        glGenBuffers(2, &mut buffers);
        glBindBuffer(GL_UNIFORM_BUFFER, buffers[0]);
        glBindBufferBase(GL_UNIFORM_BUFFER, 0, buffers[1]);
        glBufferSubData(GL_UNIFORM_BUFFER, 16, 32, std::ptr::null::<u8>());
        glGenVertexArrays(1, &mut vertex_arrays);
        glBindVertexArray(vertex_arrays[0]);
        glVertexAttribDivisor(0, 1);
        glGenFramebuffers(1, &mut framebuffers);
        glBindFramebuffer(GL_FRAMEBUFFER, framebuffers[0]);
        glGenRenderbuffers(1, &mut renderbuffers);
        glFramebufferRenderbuffer(
            GL_FRAMEBUFFER,
            GL_DEPTH_ATTACHMENT,
            GL_RENDERBUFFER,
            renderbuffers[0],
        );
        glDeleteRenderbuffers(1, &renderbuffers);
        glDeleteFramebuffers(1, &framebuffers);
        glDeleteVertexArrays(1, &vertex_arrays);
        glDeleteBuffers(2, &buffers);
        ctx.commands().to_vec()
    }

    #[test]
    fn compatibility_macro_supports_gl_resource_lifecycle_calls() {
        let commands = opengl_resource_lifecycle_commands();
        assert_eq!(
            commands,
            vec![
                GfxCommand::GenerateHandles {
                    kind: GlHandleKind::Buffer,
                    ids: vec![1, 2],
                },
                GfxCommand::BindBuffer {
                    target: GL_UNIFORM_BUFFER,
                    buffer: BufferHandle(1),
                },
                GfxCommand::BindBufferBase {
                    target: GL_UNIFORM_BUFFER,
                    index: 0,
                    buffer: BufferHandle(2),
                },
                GfxCommand::UploadBufferSubData {
                    target: GL_UNIFORM_BUFFER,
                    offset_bytes: 16,
                    size_bytes: 32,
                },
                GfxCommand::GenerateHandles {
                    kind: GlHandleKind::VertexArray,
                    ids: vec![3],
                },
                GfxCommand::BindMesh(MeshHandle(3)),
                GfxCommand::SetVertexAttributeDivisor {
                    index: 0,
                    divisor: 1,
                },
                GfxCommand::GenerateHandles {
                    kind: GlHandleKind::Framebuffer,
                    ids: vec![4],
                },
                GfxCommand::BindFramebuffer(FramebufferHandle(4)),
                GfxCommand::GenerateHandles {
                    kind: GlHandleKind::Renderbuffer,
                    ids: vec![5],
                },
                GfxCommand::AttachFramebufferRenderbuffer {
                    attachment: GL_DEPTH_ATTACHMENT,
                    renderbuffer: RenderbufferHandle(5),
                },
                GfxCommand::DeleteHandles {
                    kind: GlHandleKind::Renderbuffer,
                    ids: vec![5],
                },
                GfxCommand::DeleteHandles {
                    kind: GlHandleKind::Framebuffer,
                    ids: vec![4],
                },
                GfxCommand::DeleteHandles {
                    kind: GlHandleKind::VertexArray,
                    ids: vec![3],
                },
                GfxCommand::DeleteHandles {
                    kind: GlHandleKind::Buffer,
                    ids: vec![1, 2],
                },
            ]
        );
    }

    #[use_backend(vulkan)]
    fn vulkan_compatibility_macro_records_commands() -> usize {
        ctx.begin_render_pass(&RenderPassHandle(1));
        ctx.bind_pipeline(&PipelineHandle(2));
        ctx.bind_mesh(MeshHandle(3));
        ctx.draw(36, 1, 0, 0);
        ctx.commands().len()
    }

    #[test]
    fn compatibility_macro_supports_vulkan_profile() {
        assert_eq!(vulkan_compatibility_macro_records_commands(), 4);
    }

    #[use_backend(raw)]
    fn raw_compatibility_macro_records_commands() -> usize {
        let begin_info = RawRenderPassBeginInfo {
            render_pass: RenderPassHandle(7),
            framebuffer: FramebufferHandle(9),
            clear_flags: CLEAR_COLOR | CLEAR_DEPTH,
        };
        ctx.cmd_begin_render_pass(&begin_info, SubpassContents::INLINE);
        ctx.cmd_bind_pipeline(PipelineBindPoint::GRAPHICS, PipelineHandle(8));
        ctx.cmd_draw(3, 1, 0, 0);
        ctx.commands().len()
    }

    #[test]
    fn compatibility_macro_supports_raw_profile() {
        assert_eq!(raw_compatibility_macro_records_commands(), 3);
    }
}
