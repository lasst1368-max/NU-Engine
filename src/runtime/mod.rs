use ash::khr::{surface, swapchain};
use ash::{Device, Entry, Instance, vk};
use fontdue::{Font, FontSettings, Metrics};
use image::{ColorType, ImageFormat};
use std::collections::HashMap;
use std::ffi::CString;
use std::io::Cursor;
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Instant;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{CursorGrabMode, Window};

use crate::app::{WindowApp, WindowConfig, run_window_app};
use crate::core::{ApiConfig, ApiError};
use crate::executor::{
    BufferBinding, DescriptorSetBinding, DescriptorSetLayoutBindings, ExecutorError,
    ExecutorResources, GraphicsPipelineCache, GraphicsPipelineCompiler, GraphicsPipelineDescriptor,
    ShaderProgramDefinition, TextureBinding, UniformValueKind, VertexAttributeBinding,
    VulkanGraphicsPipelineCompiler, apply_descriptor_writes,
    resolve_descriptor_writes_for_bindings,
};
use crate::lighting::{LightingConfig, MAX_POINT_LIGHTS, ShadowMode};
use crate::scene::{
    Camera2D, Camera3D, CircleDraw, DrawSpace, LineDraw, Mesh3D, MeshDraw3D, PrimitiveDraw,
    QuadDraw, RectDraw, Scene, SceneConfig, SceneFrame, ScreenshotResolution, ShapeStyle,
    TextAnchor, TextDraw,
};
use crate::syntax::{BufferHandle, DescriptorSetHandle, ShaderHandle, TextureHandle};

const PRIMITIVE_2D_VERT_SPV: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/primitive_2d.vert.spv"));
const PRIMITIVE_2D_FRAG_SPV: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/primitive_2d.frag.spv"));
const CUBE_3D_VERT_SPV: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/cube_3d.vert.spv"));
const CUBE_3D_FRAG_SPV: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/cube_3d.frag.spv"));
const SHADOW_3D_VERT_SPV: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shadow_3d.vert.spv"));
const TEXT_2D_VERT_SPV: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/text_2d.vert.spv"));
const TEXT_2D_FRAG_SPV: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/text_2d.frag.spv"));
const INTER_FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular.ttf");
const INITIAL_PRIMITIVE_CAPACITY: usize = 1024;
const INITIAL_CUBE_VERTEX_CAPACITY: usize = 36;
const INITIAL_TEXT_GLYPH_CAPACITY: usize = 2048;
const SPHERE_LATITUDE_SEGMENTS: usize = 16;
const SPHERE_LONGITUDE_SEGMENTS: usize = 24;
const MAX_SCENE_CUBES: usize = 256;
const MAX_MATERIAL_DESCRIPTOR_SETS: u32 = 256;
const SHADOW_MAP_SIZE: u32 = 2048;
const RUNTIME_SHADER_2D: ShaderHandle = ShaderHandle(2);
const RUNTIME_SHADER_3D: ShaderHandle = ShaderHandle(1);
const RUNTIME_SHADER_TEXT_2D: ShaderHandle = ShaderHandle(3);
const RUNTIME_DESCRIPTOR_SET_3D: DescriptorSetHandle = DescriptorSetHandle(1);
const RUNTIME_DESCRIPTOR_SET_TEXT_2D: DescriptorSetHandle = DescriptorSetHandle(2);
const RUNTIME_DESCRIPTOR_SET_SHADOW_3D: DescriptorSetHandle = DescriptorSetHandle(3);
const RUNTIME_DESCRIPTOR_SET_MATERIAL_BASE: u32 = 1024;
const RUNTIME_BUFFER_CUBE_SCENE: BufferHandle = BufferHandle(1);
const RUNTIME_BUFFER_CUBE_OBJECTS: BufferHandle = BufferHandle(2);
const RUNTIME_UNIFORM_CUBE_SCENE: &str = "CubeScene";
const RUNTIME_STORAGE_CUBE_OBJECTS: &str = "CubeObjects";
const RUNTIME_TEXTURE_WHITE: TextureHandle = TextureHandle(1);
const RUNTIME_TEXTURE_FONT_ATLAS: TextureHandle = TextureHandle(2);
const RUNTIME_TEXTURE_SHADOW_MAP: TextureHandle = TextureHandle(3);
const RUNTIME_TEXTURE_SLOT_ALBEDO: u32 = 0;
const RUNTIME_TEXTURE_SLOT_TEXT_ATLAS: u32 = 0;
const RUNTIME_TEXTURE_SLOT_SHADOW_MAP: u32 = 0;

#[derive(Debug, Clone, Copy)]
pub struct SampledTextureDescriptorResources {
    pub layout: vk::DescriptorSetLayout,
    pub pool: vk::DescriptorPool,
    pub set: vk::DescriptorSet,
}

pub fn run_scene<T>(scene: T) -> Result<(), ApiError>
where
    T: Scene + 'static,
{
    let config = scene.config();
    run_window_app(SceneApp {
        scene,
        config,
        renderer: None,
        frame: SceneFrame::default(),
        last_frame_time: None,
    })
}

struct SceneApp<T>
where
    T: Scene,
{
    scene: T,
    config: SceneConfig,
    renderer: Option<VulkanRuntime>,
    frame: SceneFrame,
    last_frame_time: Option<Instant>,
}

struct SwapchainBundle {
    swapchain: vk::SwapchainKHR,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    extent: vk::Extent2D,
    format: vk::Format,
}

struct VulkanRuntime {
    _entry: Entry,
    instance: Instance,
    surface_loader: surface::Instance,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    queue_family_index: u32,
    device: Device,
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,
    swapchain_loader: swapchain::Device,
    swapchain: vk::SwapchainKHR,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,
    swapchain_extent: vk::Extent2D,
    swapchain_format: vk::Format,
    msaa_samples: vk::SampleCountFlags,
    color_image: vk::Image,
    color_image_memory: vk::DeviceMemory,
    color_image_view: vk::ImageView,
    depth_format: vk::Format,
    depth_image: vk::Image,
    depth_image_memory: vk::DeviceMemory,
    depth_image_view: vk::ImageView,
    render_pass: vk::RenderPass,
    screenshot_render_pass: vk::RenderPass,
    pipeline_layout_2d: vk::PipelineLayout,
    graphics_pipeline_2d: vk::Pipeline,
    descriptor_set_layout_text_2d: vk::DescriptorSetLayout,
    descriptor_pool_text_2d: vk::DescriptorPool,
    descriptor_set_text_2d: vk::DescriptorSet,
    pipeline_layout_text_2d: vk::PipelineLayout,
    graphics_pipeline_text_2d: vk::Pipeline,
    descriptor_set_layout_3d: vk::DescriptorSetLayout,
    descriptor_pool_3d: vk::DescriptorPool,
    descriptor_set_3d: vk::DescriptorSet,
    shadow_descriptor_set_layout_3d: vk::DescriptorSetLayout,
    shadow_descriptor_pool_3d: vk::DescriptorPool,
    shadow_descriptor_set_3d: vk::DescriptorSet,
    material_descriptor_set_layout_3d: vk::DescriptorSetLayout,
    material_descriptor_pool_3d: vk::DescriptorPool,
    default_material_descriptor_set_3d: vk::DescriptorSet,
    pipeline_layout_3d: vk::PipelineLayout,
    graphics_pipeline_3d: vk::Pipeline,
    shadow_render_pass: vk::RenderPass,
    shadow_pipeline_layout: vk::PipelineLayout,
    shadow_graphics_pipeline: vk::Pipeline,
    shadow_map_image: vk::Image,
    shadow_map_memory: vk::DeviceMemory,
    shadow_map_view: vk::ImageView,
    shadow_map_sampler: vk::Sampler,
    shadow_framebuffer: vk::Framebuffer,
    executor_resources: ExecutorResources,
    graphics_pipeline_cache: GraphicsPipelineCache,
    pipeline_compiler: VulkanGraphicsPipelineCompiler,
    material_descriptor_sets_3d: HashMap<TextureHandle, vk::DescriptorSet>,
    framebuffers: Vec<vk::Framebuffer>,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    primitive_buffer: vk::Buffer,
    primitive_buffer_memory: vk::DeviceMemory,
    primitive_capacity: usize,
    text_glyph_buffer: vk::Buffer,
    text_glyph_buffer_memory: vk::DeviceMemory,
    text_glyph_capacity: usize,
    cube_vertex_buffer: vk::Buffer,
    cube_vertex_buffer_memory: vk::DeviceMemory,
    cube_vertex_capacity: usize,
    cube_scene_buffer: vk::Buffer,
    cube_scene_buffer_memory: vk::DeviceMemory,
    cube_object_buffer: vk::Buffer,
    cube_object_buffer_memory: vk::DeviceMemory,
    white_texture_image: vk::Image,
    white_texture_memory: vk::DeviceMemory,
    white_texture_view: vk::ImageView,
    white_texture_sampler: vk::Sampler,
    font_atlas_image: vk::Image,
    font_atlas_memory: vk::DeviceMemory,
    font_atlas_view: vk::ImageView,
    font_atlas_sampler: vk::Sampler,
    font_atlas_layout: FontAtlasLayout,
    image_available_semaphore: vk::Semaphore,
    render_finished_semaphore: vk::Semaphore,
    in_flight_fence: vk::Fence,
}

#[allow(dead_code)]
struct ScreenshotRenderTarget {
    extent: vk::Extent2D,
    framebuffer: vk::Framebuffer,
    color_image: vk::Image,
    color_image_memory: vk::DeviceMemory,
    color_image_view: vk::ImageView,
    resolve_image: vk::Image,
    resolve_image_memory: vk::DeviceMemory,
    resolve_image_view: vk::ImageView,
    depth_image: vk::Image,
    depth_image_memory: vk::DeviceMemory,
    depth_image_view: vk::ImageView,
    readback_buffer: vk::Buffer,
    readback_memory: vk::DeviceMemory,
}

enum DrawFrameResult {
    Drawn,
    NeedsResize,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct PrimitiveInstance {
    color: [f32; 4],
    data0: [f32; 4],
    data1: [f32; 4],
    data2: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct TextGlyphInstance {
    color: [f32; 4],
    rect: [f32; 4],
    uv_rect: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CubeVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    albedo: [f32; 4],
    object_index: u32,
    material: [f32; 4],
    /// Tangent vector in local space; w = handedness (±1). Default [1,0,0,1] when not computed.
    tangent: [f32; 4],
}

impl CubeVertex {
    /// Construct a vertex with the neutral default tangent `[1, 0, 0, 1]`.
    fn with_default_tangent(
        position: [f32; 3],
        normal: [f32; 3],
        uv: [f32; 2],
        albedo: [f32; 4],
        object_index: u32,
        material: [f32; 4],
    ) -> Self {
        Self { position, normal, uv, albedo, object_index, material, tangent: [1.0, 0.0, 0.0, 1.0] }
    }
}

#[derive(Clone, Copy)]
struct MeshDrawBatch3D {
    first_vertex: u32,
    vertex_count: u32,
    albedo_texture: Option<TextureHandle>,
    normal_texture: Option<TextureHandle>,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CubeViewProjectionPushConstants {
    view_projection: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CubeSceneUniforms {
    camera_position: [f32; 4],
    point_light_positions: [[f32; 4]; MAX_POINT_LIGHTS],
    point_light_colors: [[f32; 4]; MAX_POINT_LIGHTS],
    point_light_shadow_flags: [f32; 4],
    ambient_color: [f32; 4],
    fill_direction: [f32; 4],
    fill_color: [f32; 4],
    material: [f32; 4],
    shadow_params: [f32; 4],
    shadow_view_projection: [[f32; 4]; 4],
    /// Spotlight data: 4 vec4s per light × 4 lights = 16 vec4s.
    /// Per light layout: [pos.xyz, range], [dir.xyz, intensity], [color.xyz, count], [cos_inner, cos_outer, 0, 0]
    spot_lights: [[f32; 4]; 16],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct GpuSceneCube {
    center: [f32; 4],
    half_extents: [f32; 4],
    axis_x: [f32; 4],
    axis_y: [f32; 4],
    axis_z: [f32; 4],
}

#[derive(Clone)]
struct FontAtlasGlyph {
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    metrics: Metrics,
}

#[derive(Clone)]
struct FontAtlasLayout {
    glyphs: HashMap<char, FontAtlasGlyph>,
    kerning: HashMap<(char, char), f32>,
    line_height: f32,
    base_pixel_size: f32,
}

impl PrimitiveInstance {
    fn rect(draw: RectDraw, mapper: FrameSpaceMapper) -> Self {
        let center = mapper.point_to_ndc(draw.center, draw.space);
        let size = mapper.size_to_ndc(draw.size, draw.space);
        let (kind, stroke_width) = rect_style(draw.style);
        Self {
            color: draw.color,
            data0: [center[0], center[1], size[0], size[1]],
            data1: [
                draw.rotation_radians.cos(),
                draw.rotation_radians.sin(),
                0.0,
                0.0,
            ],
            data2: [
                kind,
                mapper.scalar_to_ndc(stroke_width, draw.space),
                0.0,
                0.0,
            ],
        }
    }

    fn circle(draw: CircleDraw, mapper: FrameSpaceMapper) -> Self {
        let diameter = draw.radius * 2.0;
        let center = mapper.point_to_ndc(draw.center, draw.space);
        let size = mapper.size_to_ndc([diameter, diameter], draw.space);
        let (kind, stroke_width) = circle_style(draw.style);
        Self {
            color: draw.color,
            data0: [center[0], center[1], size[0], size[1]],
            data1: [1.0, 0.0, 0.0, 0.0],
            data2: [
                kind,
                mapper.scalar_to_ndc(stroke_width, draw.space),
                0.0,
                0.0,
            ],
        }
    }

    fn line(draw: LineDraw, mapper: FrameSpaceMapper) -> Self {
        let start = mapper.point_to_ndc(draw.start, draw.space);
        let end = mapper.point_to_ndc(draw.end, draw.space);
        let dx = end[0] - start[0];
        let dy = end[1] - start[1];
        let length = (dx * dx + dy * dy).sqrt().max(0.0001);
        let rotation = dy.atan2(dx);
        Self {
            color: draw.color,
            data0: [
                (start[0] + end[0]) * 0.5,
                (start[1] + end[1]) * 0.5,
                length,
                mapper.scalar_to_ndc(draw.thickness, draw.space).max(0.0001),
            ],
            data1: [rotation.cos(), rotation.sin(), 0.0, 0.0],
            data2: [2.0, 0.0, 0.0, 0.0],
        }
    }

    fn quad(draw: QuadDraw, mapper: FrameSpaceMapper) -> Self {
        let p0 = mapper.point_to_ndc(draw.points[0], draw.space);
        let p1 = mapper.point_to_ndc(draw.points[1], draw.space);
        let p2 = mapper.point_to_ndc(draw.points[2], draw.space);
        let p3 = mapper.point_to_ndc(draw.points[3], draw.space);
        Self {
            color: draw.color,
            data0: [p0[0], p0[1], p1[0], p1[1]],
            data1: [p2[0], p2[1], p3[0], p3[1]],
            data2: [5.0, 0.0, 0.0, 0.0],
        }
    }
}

#[derive(Clone, Copy)]
struct FrameSpaceMapper {
    extent: vk::Extent2D,
    center: [f32; 2],
    scale: [f32; 2],
}

impl FrameSpaceMapper {
    fn from_scene(camera: Camera2D, extent: vk::Extent2D) -> Self {
        let aspect = if extent.height == 0 {
            1.0
        } else {
            extent.width as f32 / extent.height as f32
        };
        let view_height = camera.view_height.max(0.0001);
        let view_width = view_height * aspect;

        Self {
            extent,
            center: camera.center,
            scale: [2.0 / view_width, 2.0 / view_height],
        }
    }

    fn point_to_ndc(self, point: [f32; 2], space: DrawSpace) -> [f32; 2] {
        match space {
            DrawSpace::World => [
                (point[0] - self.center[0]) * self.scale[0],
                (point[1] - self.center[1]) * self.scale[1],
            ],
            DrawSpace::Screen => {
                let width = self.extent.width.max(1) as f32;
                let height = self.extent.height.max(1) as f32;
                [
                    ((point[0] / width) * 2.0) - 1.0,
                    1.0 - ((point[1] / height) * 2.0),
                ]
            }
        }
    }

    fn size_to_ndc(self, size: [f32; 2], space: DrawSpace) -> [f32; 2] {
        match space {
            DrawSpace::World => [size[0] * self.scale[0], size[1] * self.scale[1]],
            DrawSpace::Screen => {
                let width = self.extent.width.max(1) as f32;
                let height = self.extent.height.max(1) as f32;
                [(size[0] * 2.0) / width, (size[1] * 2.0) / height]
            }
        }
    }

    fn scalar_to_ndc(self, value: f32, space: DrawSpace) -> f32 {
        match space {
            DrawSpace::World => value * self.scale[1],
            DrawSpace::Screen => {
                let height = self.extent.height.max(1) as f32;
                (value * 2.0) / height
            }
        }
    }
}

fn rect_style(style: ShapeStyle) -> (f32, f32) {
    match style {
        ShapeStyle::Fill => (0.0, 0.0),
        ShapeStyle::Stroke { width } => (3.0, width.max(0.0)),
    }
}

fn circle_style(style: ShapeStyle) -> (f32, f32) {
    match style {
        ShapeStyle::Fill => (1.0, 0.0),
        ShapeStyle::Stroke { width } => (4.0, width.max(0.0)),
    }
}

fn build_primitive_instances(
    frame: &SceneFrame,
    scene_config: &SceneConfig,
    extent: vk::Extent2D,
) -> Vec<PrimitiveInstance> {
    let mapper = FrameSpaceMapper::from_scene(scene_config.camera, extent);
    let mut draws = frame.draws().to_vec();
    draws.sort_by_key(PrimitiveDraw::layer);

    draws
        .into_iter()
        .filter_map(|draw| match draw {
            PrimitiveDraw::Rect(rect) => Some(PrimitiveInstance::rect(rect, mapper)),
            PrimitiveDraw::Circle(circle) => Some(PrimitiveInstance::circle(circle, mapper)),
            PrimitiveDraw::Line(line) => Some(PrimitiveInstance::line(line, mapper)),
            PrimitiveDraw::Quad(quad) => Some(PrimitiveInstance::quad(quad, mapper)),
            PrimitiveDraw::Text(_) => None,
        })
        .collect()
}

fn build_text_glyph_instances(
    frame: &SceneFrame,
    camera: Camera2D,
    font_atlas: &FontAtlasLayout,
    extent: vk::Extent2D,
) -> Vec<TextGlyphInstance> {
    let mapper = FrameSpaceMapper::from_scene(camera, extent);
    let mut text_draws: Vec<TextDraw> = frame
        .draws()
        .iter()
        .filter_map(|draw| match draw {
            PrimitiveDraw::Text(text) => Some(text.clone()),
            _ => None,
        })
        .collect();
    text_draws.sort_by_key(|draw| draw.layer);

    let mut instances = Vec::new();
    for draw in text_draws {
        let glyph_scale = (draw.pixel_size / font_atlas.base_pixel_size.max(1.0)).max(0.01);
        let mut width_px = 0.0;
        let mut previous = None;
        for ch in draw.text.chars() {
            if let Some(prev) = previous {
                width_px +=
                    font_atlas.kerning.get(&(prev, ch)).copied().unwrap_or(0.0) * glyph_scale;
            }
            if let Some(glyph) = font_atlas.glyphs.get(&ch) {
                width_px += glyph.metrics.advance_width * glyph_scale;
            } else if let Some(glyph) = font_atlas.glyphs.get(&'?') {
                width_px += glyph.metrics.advance_width * glyph_scale;
            }
            previous = Some(ch);
        }
        let anchor_offset = match draw.anchor {
            TextAnchor::TopLeft => [0.0, 0.0],
            TextAnchor::Center => [width_px * 0.5, font_atlas.line_height * glyph_scale * 0.5],
        };
        let mut pen_x = draw.position[0] - anchor_offset[0];
        let baseline_y =
            draw.position[1] + font_atlas.line_height * glyph_scale * 0.8 - anchor_offset[1];
        let mut previous = None;

        for ch in draw.text.chars() {
            if let Some(prev) = previous {
                pen_x += font_atlas.kerning.get(&(prev, ch)).copied().unwrap_or(0.0) * glyph_scale;
            }
            let glyph = font_atlas
                .glyphs
                .get(&ch)
                .or_else(|| font_atlas.glyphs.get(&'?'));
            let Some(glyph) = glyph else {
                continue;
            };
            let glyph_width = glyph.metrics.width as f32 * glyph_scale;
            let glyph_height = glyph.metrics.height as f32 * glyph_scale;
            let glyph_x = pen_x + glyph.metrics.xmin as f32 * glyph_scale;
            let glyph_y = baseline_y
                - glyph.metrics.height as f32 * glyph_scale
                - glyph.metrics.ymin as f32 * glyph_scale;
            let center = [glyph_x + glyph_width * 0.5, glyph_y + glyph_height * 0.5];
            let ndc_center = mapper.point_to_ndc(center, draw.space);
            let ndc_size = mapper.size_to_ndc([glyph_width, glyph_height], draw.space);
            if glyph.metrics.width > 0 && glyph.metrics.height > 0 {
                instances.push(TextGlyphInstance {
                    color: draw.color,
                    rect: [ndc_center[0], ndc_center[1], ndc_size[0], ndc_size[1]],
                    uv_rect: [
                        glyph.uv_min[0],
                        glyph.uv_min[1],
                        glyph.uv_max[0],
                        glyph.uv_max[1],
                    ],
                });
            }
            pen_x += (glyph.metrics.advance_width * glyph_scale).max(draw.pixel_size * 0.2);
            previous = Some(ch);
        }
    }
    instances
}

fn build_mesh_vertices(
    frame: &SceneFrame,
    camera: Camera3D,
    lighting: LightingConfig,
    extent: vk::Extent2D,
    camera_jitter_ndc: [f32; 2],
) -> (
    Vec<CubeVertex>,
    Vec<MeshDrawBatch3D>,
    Vec<MeshDrawBatch3D>,
    CubeViewProjectionPushConstants,
    CubeSceneUniforms,
    Vec<GpuSceneCube>,
) {
    let mut vertices = Vec::with_capacity(frame.meshes_3d().len() * 36);
    let mut draw_batches = Vec::with_capacity(frame.meshes_3d().len());
    let mut shadow_draw_batches = Vec::with_capacity(frame.meshes_3d().len());
    let cubes = build_gpu_scene_cubes(frame.meshes_3d());
    let shadow_view_projection = compute_directional_shadow_view_projection(
        &frame
            .meshes_3d()
            .iter()
            .filter(|mesh| mesh_casts_live_shadow(mesh))
            .cloned()
            .collect::<Vec<_>>(),
        camera,
        lighting,
        lighting.shadows.live.max_distance,
    );
    let aspect = if extent.height == 0 {
        1.0
    } else {
        extent.width as f32 / extent.height as f32
    };

    for (mesh_index, mesh) in frame.meshes_3d().iter().cloned().enumerate() {
        let first_vertex = vertices.len() as u32;
        append_mesh_vertices(&mut vertices, mesh_index, &mesh, camera.position);
        let vertex_count = vertices.len() as u32 - first_vertex;
        if vertex_count > 0 {
            let batch = MeshDrawBatch3D {
                first_vertex,
                vertex_count,
                albedo_texture: mesh.material.albedo_texture,
                normal_texture: mesh.material.normal_texture,
            };
            draw_batches.push(batch);
            if mesh_casts_live_shadow(&mesh) {
                shadow_draw_batches.push(batch);
            }
        }
    }

    let mut point_light_positions = [[0.0; 4]; MAX_POINT_LIGHTS];
    let mut point_light_colors = [[0.0; 4]; MAX_POINT_LIGHTS];
    let mut point_light_shadow_flags = [0.0; 4];
    for index in 0..lighting.point_light_count.min(MAX_POINT_LIGHTS) {
        let light = lighting.point_lights[index];
        point_light_positions[index] = [
            light.position[0],
            light.position[1],
            light.position[2],
            light.range,
        ];
        point_light_colors[index] = [
            light.color[0],
            light.color[1],
            light.color[2],
            light.intensity,
        ];
        point_light_shadow_flags[index] = if lighting.point_light_shadow_flags[index] {
            1.0
        } else {
            0.0
        };
    }

    // Pack spotlight data: 4 vec4s per spotlight.
    // Layout per light (base = i * 4):
    //   [base+0]: pos.xyz, range
    //   [base+1]: dir.xyz, intensity
    //   [base+2]: color.xyz, spot_count (only base+2 of light 0 carries the total count)
    //   [base+3]: cos_inner, cos_outer, 0, 0
    let mut spot_lights = [[0.0f32; 4]; 16];
    let spot_count = lighting.spot_light_count.min(crate::lighting::MAX_SPOT_LIGHTS);
    for i in 0..spot_count {
        let sl = &lighting.spot_lights[i];
        let base = i * 4;
        spot_lights[base]     = [sl.position[0], sl.position[1], sl.position[2], sl.range];
        spot_lights[base + 1] = [sl.direction[0], sl.direction[1], sl.direction[2], sl.intensity];
        spot_lights[base + 2] = [sl.color[0], sl.color[1], sl.color[2], spot_count as f32];
        spot_lights[base + 3] = [sl.inner_cos(), sl.outer_cos(), 0.0, 0.0];
    }
    // Always write the count into slot [2].w even when there are no spotlights.
    spot_lights[2][3] = spot_count as f32;

    (
        vertices,
        draw_batches,
        shadow_draw_batches,
        CubeViewProjectionPushConstants {
            view_projection: mul_mat4(
                perspective_lh(
                    camera.fov_y_degrees.to_radians(),
                    aspect,
                    camera.near_clip,
                    camera.far_clip,
                    camera_jitter_ndc,
                ),
                look_at_lh(camera.position, camera.target, camera.up),
            ),
        },
        CubeSceneUniforms {
            camera_position: [
                camera.position[0],
                camera.position[1],
                camera.position[2],
                0.0,
            ],
            point_light_positions,
            point_light_colors,
            point_light_shadow_flags,
            ambient_color: [
                lighting.ambient_color[0],
                lighting.ambient_color[1],
                lighting.ambient_color[2],
                lighting.ambient_intensity,
            ],
            fill_direction: [
                lighting.fill_light.direction[0],
                lighting.fill_light.direction[1],
                lighting.fill_light.direction[2],
                lighting.fill_light.intensity,
            ],
            fill_color: [
                lighting.fill_light.color[0],
                lighting.fill_light.color[1],
                lighting.fill_light.color[2],
                lighting.point_light_count.min(MAX_POINT_LIGHTS) as f32,
            ],
            material: [
                lighting.specular_strength,
                lighting.shininess,
                frame.meshes_3d().len() as f32,
                0.78,
            ],
            shadow_params: [
                if matches!(lighting.shadows.mode, ShadowMode::Live) {
                    lighting.shadows.minimum_visibility
                } else {
                    1.0
                },
                lighting.shadows.bias,
                1.0 / SHADOW_MAP_SIZE as f32,
                lighting.shadows.live.filter_radius.max(0.5),
            ],
            shadow_view_projection,
            spot_lights,
        },
        cubes,
    )
}

fn mesh_casts_live_shadow(mesh: &MeshDraw3D) -> bool {
    !matches!(mesh.mesh, Mesh3D::Plane)
}

fn append_mesh_vertices(
    vertices: &mut Vec<CubeVertex>,
    mesh_index: usize,
    mesh: &MeshDraw3D,
    camera_position: [f32; 3],
) {
    match &mesh.mesh {
        Mesh3D::Cube => append_cube_mesh_vertices(vertices, mesh_index, mesh, camera_position),
        Mesh3D::Plane => append_plane_mesh_vertices(vertices, mesh_index, mesh),
        Mesh3D::Sphere => append_sphere_mesh_vertices(vertices, mesh_index, mesh, camera_position),
        Mesh3D::Custom(asset) => append_custom_mesh_vertices(vertices, mesh_index, mesh, asset),
        // Procedural primitives — generated on demand and routed through the custom path.
        Mesh3D::Cylinder { radial_segments, height_segments } => {
            let asset = crate::scene::primitives::generate_cylinder(
                "cylinder", *radial_segments, *height_segments,
            );
            append_custom_mesh_vertices(vertices, mesh_index, mesh, &asset);
        }
        Mesh3D::Torus { major_segments, minor_segments } => {
            let asset = crate::scene::primitives::generate_torus(
                "torus", *major_segments, *minor_segments,
            );
            append_custom_mesh_vertices(vertices, mesh_index, mesh, &asset);
        }
        Mesh3D::Cone { radial_segments, height_segments } => {
            let asset = crate::scene::primitives::generate_cone(
                "cone", *radial_segments, *height_segments,
            );
            append_custom_mesh_vertices(vertices, mesh_index, mesh, &asset);
        }
        Mesh3D::Capsule { radial_segments, cap_segments } => {
            let asset = crate::scene::primitives::generate_capsule(
                "capsule", *radial_segments, *cap_segments,
            );
            append_custom_mesh_vertices(vertices, mesh_index, mesh, &asset);
        }
        Mesh3D::Icosphere { subdivisions } => {
            let asset = crate::scene::primitives::generate_icosphere("icosphere", *subdivisions);
            append_custom_mesh_vertices(vertices, mesh_index, mesh, &asset);
        }
    }
}

fn mesh_material_vertex_params(mesh: &MeshDraw3D) -> [f32; 4] {
    [
        mesh.material.roughness.clamp(0.0, 1.0),
        mesh.material.metallic.clamp(0.0, 1.0),
        // z = emissive intensity (0 = no emission)
        mesh.material.emissive_intensity.max(0.0),
        // w = normal map flag (>= 0.5 means a normal map is bound at set=3)
        if mesh.material.normal_texture.is_some() { 1.0 } else { 0.0 },
    ]
}

fn append_plane_mesh_vertices(
    vertices: &mut Vec<CubeVertex>,
    mesh_index: usize,
    mesh: &MeshDraw3D,
) {
    let half = [mesh.size[0] * 0.5, mesh.size[1] * 0.5, mesh.size[2] * 0.5];
    let local_normal = [0.0, 1.0, 0.0];

    let p0 = [-half[0], 0.0, -half[2]];
    let p1 = [half[0], 0.0, -half[2]];
    let p2 = [half[0], 0.0, half[2]];
    let p3 = [-half[0], 0.0, half[2]];
    let uv0 = [0.0, 0.0];
    let uv1 = [1.0, 0.0];
    let uv2 = [1.0, 1.0];
    let uv3 = [0.0, 1.0];

    vertices.extend_from_slice(&[
        CubeVertex {
            position: p0,
            normal: local_normal,
            uv: uv0,
            albedo: mesh.color,
            object_index: mesh_index as u32,
            material: mesh_material_vertex_params(mesh),
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        CubeVertex {
            position: p1,
            normal: local_normal,
            uv: uv1,
            albedo: mesh.color,
            object_index: mesh_index as u32,
            material: mesh_material_vertex_params(mesh),
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        CubeVertex {
            position: p2,
            normal: local_normal,
            uv: uv2,
            albedo: mesh.color,
            object_index: mesh_index as u32,
            material: mesh_material_vertex_params(mesh),
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        CubeVertex {
            position: p0,
            normal: local_normal,
            uv: uv0,
            albedo: mesh.color,
            object_index: mesh_index as u32,
            material: mesh_material_vertex_params(mesh),
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        CubeVertex {
            position: p2,
            normal: local_normal,
            uv: uv2,
            albedo: mesh.color,
            object_index: mesh_index as u32,
            material: mesh_material_vertex_params(mesh),
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        CubeVertex {
            position: p3,
            normal: local_normal,
            uv: uv3,
            albedo: mesh.color,
            object_index: mesh_index as u32,
            material: mesh_material_vertex_params(mesh),
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        CubeVertex {
            position: p0,
            normal: [0.0, -1.0, 0.0],
            uv: uv0,
            albedo: mesh.color,
            object_index: mesh_index as u32,
            material: mesh_material_vertex_params(mesh),
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        CubeVertex {
            position: p3,
            normal: [0.0, -1.0, 0.0],
            uv: uv3,
            albedo: mesh.color,
            object_index: mesh_index as u32,
            material: mesh_material_vertex_params(mesh),
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        CubeVertex {
            position: p2,
            normal: [0.0, -1.0, 0.0],
            uv: uv2,
            albedo: mesh.color,
            object_index: mesh_index as u32,
            material: mesh_material_vertex_params(mesh),
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        CubeVertex {
            position: p0,
            normal: [0.0, -1.0, 0.0],
            uv: uv0,
            albedo: mesh.color,
            object_index: mesh_index as u32,
            material: mesh_material_vertex_params(mesh),
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        CubeVertex {
            position: p2,
            normal: [0.0, -1.0, 0.0],
            uv: uv2,
            albedo: mesh.color,
            object_index: mesh_index as u32,
            material: mesh_material_vertex_params(mesh),
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        CubeVertex {
            position: p1,
            normal: [0.0, -1.0, 0.0],
            uv: uv1,
            albedo: mesh.color,
            object_index: mesh_index as u32,
            material: mesh_material_vertex_params(mesh),
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
    ]);
}

fn append_cube_mesh_vertices(
    vertices: &mut Vec<CubeVertex>,
    mesh_index: usize,
    mesh: &MeshDraw3D,
    camera_position: [f32; 3],
) {
    let half = [mesh.size[0] * 0.5, mesh.size[1] * 0.5, mesh.size[2] * 0.5];
    let base_local_corners = [
        [-1.0, -1.0, -1.0],
        [1.0, -1.0, -1.0],
        [1.0, 1.0, -1.0],
        [-1.0, 1.0, -1.0],
        [-1.0, -1.0, 1.0],
        [1.0, -1.0, 1.0],
        [1.0, 1.0, 1.0],
        [-1.0, 1.0, 1.0],
    ];
    let world_corners = base_local_corners.map(|corner| {
        let scaled = [
            corner[0] * half[0],
            corner[1] * half[1],
            corner[2] * half[2],
        ];
        add3(rotate_vector_3d(scaled, mesh.rotation_radians), mesh.center)
    });
    let faces = [
        ([4, 5, 6, 7], [0.0, 0.0, 1.0]),
        ([1, 0, 3, 2], [0.0, 0.0, -1.0]),
        ([0, 4, 7, 3], [-1.0, 0.0, 0.0]),
        ([5, 1, 2, 6], [1.0, 0.0, 0.0]),
        ([3, 7, 6, 2], [0.0, 1.0, 0.0]),
        ([0, 1, 5, 4], [0.0, -1.0, 0.0]),
    ];

    for (indices, normal) in faces {
        let rotated_normal = normalize3(rotate_vector_3d(normal, mesh.rotation_radians));
        let face_center = scale3(
            add3(
                add3(world_corners[indices[0]], world_corners[indices[1]]),
                add3(world_corners[indices[2]], world_corners[indices[3]]),
            ),
            0.25,
        );
        let to_camera = normalize3(sub3(camera_position, face_center));
        if dot3(rotated_normal, to_camera) <= 0.0 {
            continue;
        }

        let p0 = base_local_corners[indices[0]];
        let p1 = base_local_corners[indices[1]];
        let p2 = base_local_corners[indices[2]];
        let p3 = base_local_corners[indices[3]];
        let uv0 = [0.0, 0.0];
        let uv1 = [1.0, 0.0];
        let uv2 = [1.0, 1.0];
        let uv3 = [0.0, 1.0];
        vertices.extend_from_slice(&[
            CubeVertex {
                position: p0,
                normal,
                uv: uv0,
                albedo: mesh.color,
                object_index: mesh_index as u32,
                material: mesh_material_vertex_params(mesh),
            },
            CubeVertex {
                position: p1,
                normal,
                uv: uv1,
                albedo: mesh.color,
                object_index: mesh_index as u32,
                material: mesh_material_vertex_params(mesh),
            },
            CubeVertex {
                position: p2,
                normal,
                uv: uv2,
                albedo: mesh.color,
                object_index: mesh_index as u32,
                material: mesh_material_vertex_params(mesh),
            },
            CubeVertex {
                position: p0,
                normal,
                uv: uv0,
                albedo: mesh.color,
                object_index: mesh_index as u32,
                material: mesh_material_vertex_params(mesh),
            },
            CubeVertex {
                position: p2,
                normal,
                uv: uv2,
                albedo: mesh.color,
                object_index: mesh_index as u32,
                material: mesh_material_vertex_params(mesh),
            },
            CubeVertex {
                position: p3,
                normal,
                uv: uv3,
                albedo: mesh.color,
                object_index: mesh_index as u32,
                material: mesh_material_vertex_params(mesh),
            },
        ]);
    }
}

fn append_sphere_mesh_vertices(
    vertices: &mut Vec<CubeVertex>,
    mesh_index: usize,
    mesh: &MeshDraw3D,
    _camera_position: [f32; 3],
) {
    for lat in 0..SPHERE_LATITUDE_SEGMENTS {
        let v0 = lat as f32 / SPHERE_LATITUDE_SEGMENTS as f32;
        let v1 = (lat + 1) as f32 / SPHERE_LATITUDE_SEGMENTS as f32;
        let theta0 = (v0 * std::f32::consts::PI) - (std::f32::consts::PI * 0.5);
        let theta1 = (v1 * std::f32::consts::PI) - (std::f32::consts::PI * 0.5);

        for lon in 0..SPHERE_LONGITUDE_SEGMENTS {
            let u0 = lon as f32 / SPHERE_LONGITUDE_SEGMENTS as f32;
            let u1 = (lon + 1) as f32 / SPHERE_LONGITUDE_SEGMENTS as f32;
            let phi0 = u0 * std::f32::consts::TAU;
            let phi1 = u1 * std::f32::consts::TAU;

            let p00 = sphere_point(theta0, phi0);
            let p10 = sphere_point(theta0, phi1);
            let p01 = sphere_point(theta1, phi0);
            let p11 = sphere_point(theta1, phi1);

            vertices.extend_from_slice(&[
                CubeVertex {
                    position: p00,
                    normal: p00,
                    uv: [u0, v0],
                    albedo: mesh.color,
                    object_index: mesh_index as u32,
                    material: mesh_material_vertex_params(mesh),
                },
                CubeVertex {
                    position: p10,
                    normal: p10,
                    uv: [u1, v0],
                    albedo: mesh.color,
                    object_index: mesh_index as u32,
                    material: mesh_material_vertex_params(mesh),
                },
                CubeVertex {
                    position: p11,
                    normal: p11,
                    uv: [u1, v1],
                    albedo: mesh.color,
                    object_index: mesh_index as u32,
                    material: mesh_material_vertex_params(mesh),
                },
                CubeVertex {
                    position: p00,
                    normal: p00,
                    uv: [u0, v0],
                    albedo: mesh.color,
                    object_index: mesh_index as u32,
                    material: mesh_material_vertex_params(mesh),
                },
                CubeVertex {
                    position: p11,
                    normal: p11,
                    uv: [u1, v1],
                    albedo: mesh.color,
                    object_index: mesh_index as u32,
                    material: mesh_material_vertex_params(mesh),
                },
                CubeVertex {
                    position: p01,
                    normal: p01,
                    uv: [u0, v1],
                    albedo: mesh.color,
                    object_index: mesh_index as u32,
                    material: mesh_material_vertex_params(mesh),
                },
            ]);
        }
    }
}

fn append_custom_mesh_vertices(
    vertices: &mut Vec<CubeVertex>,
    mesh_index: usize,
    mesh: &MeshDraw3D,
    asset: &std::sync::Arc<crate::scene::MeshAsset3D>,
) {
    vertices.extend(asset.vertices.iter().map(|vertex| CubeVertex {
        position: vertex.position,
        normal: vertex.normal,
        uv: vertex.uv,
        albedo: mesh.color,
        object_index: mesh_index as u32,
        material: mesh_material_vertex_params(mesh),
        tangent: vertex.tangent,
    }));
}

impl<T> WindowApp for SceneApp<T>
where
    T: Scene,
{
    fn window_config(&self) -> WindowConfig {
        self.config.window.clone()
    }

    fn resumed(&mut self, window: &Window) -> Result<(), ApiError> {
        match self.renderer.as_mut() {
            Some(renderer) => renderer.recreate_swapchain(window),
            None => {
                self.renderer = Some(VulkanRuntime::new(window, &self.config.api)?);
                apply_scene_window_preferences(window, &self.config);
                Ok(())
            }
        }
    }

    fn window_event(
        &mut self,
        window: &Window,
        event_loop: &ActiveEventLoop,
        event: &WindowEvent,
    ) -> Result<(), ApiError> {
        self.scene.window_event(window, event);

        match event {
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    apply_scene_window_preferences(window, &self.config);
                    self.renderer_mut()?.recreate_swapchain(window)?;
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                apply_scene_window_preferences(window, &self.config);
                self.renderer_mut()?.recreate_swapchain(window)?;
            }
            WindowEvent::Focused(_) => {
                apply_scene_window_preferences(window, &self.config);
            }
            WindowEvent::RedrawRequested => {
                let size = window.inner_size();
                if size.width == 0 || size.height == 0 {
                    return Ok(());
                }

                let delta_time_seconds = match self.last_frame_time.replace(Instant::now()) {
                    Some(previous) => previous.elapsed().as_secs_f32(),
                    None => 0.0,
                };

                self.scene.update(delta_time_seconds);
                self.config = self.scene.config();
                apply_scene_window_preferences(window, &self.config);
                self.frame.clear();
                self.scene.populate(&mut self.frame);

                let draw_result = {
                    let renderer = self.renderer.as_mut().ok_or(ApiError::Window {
                        reason: "Vulkan runtime is not initialized".to_string(),
                    })?;
                    renderer.draw_frame(&self.config, &self.frame)?
                };

                match draw_result {
                    DrawFrameResult::Drawn => {}
                    DrawFrameResult::NeedsResize => {
                        self.renderer_mut()?.recreate_swapchain(window)?;
                    }
                }
            }
            WindowEvent::Destroyed => {
                event_loop.exit();
            }
            _ => {}
        }

        Ok(())
    }

    fn about_to_wait(&mut self, window: &Window) -> Result<(), ApiError> {
        window.request_redraw();
        Ok(())
    }

    fn exiting(&mut self) {
        if let Some(renderer) = self.renderer.as_ref() {
            renderer.wait_idle();
        }
    }
}

fn apply_scene_window_preferences(window: &Window, config: &SceneConfig) {
    if config.capture_cursor {
        let _ = window.set_cursor_grab(CursorGrabMode::Locked);
        let _ = window.set_cursor_grab(CursorGrabMode::Confined);
        window.set_cursor_visible(false);
    } else {
        let _ = window.set_cursor_grab(CursorGrabMode::None);
        window.set_cursor_visible(true);
    }
}

impl<T> SceneApp<T>
where
    T: Scene,
{
    fn renderer_mut(&mut self) -> Result<&mut VulkanRuntime, ApiError> {
        self.renderer.as_mut().ok_or(ApiError::Window {
            reason: "Vulkan runtime is not initialized".to_string(),
        })
    }
}

impl VulkanRuntime {
    fn new(window: &Window, config: &ApiConfig) -> Result<Self, ApiError> {
        let entry = unsafe {
            Entry::load().map_err(|err| ApiError::Window {
                reason: format!("failed to load Vulkan entry: {err}"),
            })?
        };
        let instance = create_instance(&entry, window, config)?;
        let surface_loader = surface::Instance::new(&entry, &instance);
        let surface = create_surface(&entry, &instance, window)?;

        let (physical_device, queue_family_index) =
            pick_physical_device(&instance, &surface_loader, surface)?;
        let (device, graphics_queue, present_queue) =
            create_logical_device(&instance, physical_device, queue_family_index)?;

        let swapchain_loader = swapchain::Device::new(&instance, &device);
        let swapchain_bundle = create_swapchain_bundle(
            window,
            &device,
            &surface_loader,
            &swapchain_loader,
            surface,
            physical_device,
            queue_family_index,
        )?;
        let depth_format = find_depth_format(&instance, physical_device)?;
        let msaa_samples = choose_msaa_samples(
            &instance,
            physical_device,
            swapchain_bundle.format,
            depth_format,
        );
        let (color_image, color_image_memory, color_image_view) =
            if msaa_samples == vk::SampleCountFlags::TYPE_1 {
                (
                    vk::Image::null(),
                    vk::DeviceMemory::null(),
                    vk::ImageView::null(),
                )
            } else {
                create_color_resources(
                    &instance,
                    &device,
                    physical_device,
                    swapchain_bundle.extent,
                    swapchain_bundle.format,
                    msaa_samples,
                )?
            };
        let (depth_image, depth_image_memory, depth_image_view) = create_depth_resources(
            &instance,
            &device,
            physical_device,
            swapchain_bundle.extent,
            depth_format,
            msaa_samples,
        )?;
        let render_pass = create_render_pass(
            &device,
            swapchain_bundle.format,
            depth_format,
            msaa_samples,
            vk::ImageLayout::PRESENT_SRC_KHR,
        )?;
        let screenshot_render_pass = create_render_pass(
            &device,
            swapchain_bundle.format,
            depth_format,
            msaa_samples,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        )?;
        let descriptor_set_layout_text_2d = create_sampled_texture_descriptor_set_layout(
            &device,
            &[(
                RUNTIME_TEXTURE_SLOT_TEXT_ATLAS,
                vk::ShaderStageFlags::FRAGMENT,
            )],
        )?;
        let descriptor_set_layout_3d = create_cube_descriptor_set_layout(&device)?;
        let shadow_descriptor_set_layout_3d = create_sampled_texture_descriptor_set_layout(
            &device,
            &[(
                RUNTIME_TEXTURE_SLOT_SHADOW_MAP,
                vk::ShaderStageFlags::FRAGMENT,
            )],
        )?;
        let material_descriptor_set_layout_3d = create_sampled_texture_descriptor_set_layout(
            &device,
            &[(RUNTIME_TEXTURE_SLOT_ALBEDO, vk::ShaderStageFlags::FRAGMENT)],
        )?;
        let (
            cube_scene_buffer,
            cube_scene_buffer_memory,
            cube_object_buffer,
            cube_object_buffer_memory,
            descriptor_pool_3d,
            descriptor_set_3d,
        ) = create_cube_descriptor_resources(
            &instance,
            &device,
            physical_device,
            descriptor_set_layout_3d,
        )?;
        let (command_pool, command_buffer) = create_command_resources(&device, queue_family_index)?;
        let shadow_descriptor_resources = create_sampled_texture_descriptor_resources(
            &device,
            shadow_descriptor_set_layout_3d,
            1,
        )?;
        let shadow_descriptor_pool_3d = shadow_descriptor_resources.pool;
        let shadow_descriptor_set_3d = shadow_descriptor_resources.set;
        let (shadow_map_image, shadow_map_memory, shadow_map_view, shadow_map_sampler) =
            create_shadow_map_resources(
                &instance,
                &device,
                physical_device,
                depth_format,
                SHADOW_MAP_SIZE,
            )?;
        let shadow_render_pass = create_shadow_render_pass(&device, depth_format)?;
        let shadow_framebuffer = create_shadow_framebuffer(
            &device,
            shadow_render_pass,
            shadow_map_view,
            SHADOW_MAP_SIZE,
        )?;
        let (font_atlas_layout, font_rgba, font_width, font_height) = build_font_atlas()?;
        let (white_texture_image, white_texture_memory, white_texture_view, white_texture_sampler) =
            create_solid_color_texture(
                &instance,
                &device,
                physical_device,
                command_pool,
                graphics_queue,
                [255, 255, 255, 255],
            )?;
        let (font_atlas_image, font_atlas_memory, font_atlas_view, font_atlas_sampler) =
            create_rgba_texture_from_bytes(
                &instance,
                &device,
                physical_device,
                command_pool,
                graphics_queue,
                &font_rgba,
                font_width,
                font_height,
                "font_atlas",
            )?;
        let text_descriptor_resources =
            create_sampled_texture_descriptor_resources(&device, descriptor_set_layout_text_2d, 1)?;
        let descriptor_pool_text_2d = text_descriptor_resources.pool;
        let descriptor_set_text_2d = text_descriptor_resources.set;
        let material_descriptor_pool_3d = create_material_descriptor_pool_3d(&device)?;
        let default_material_descriptor_set_3d = allocate_descriptor_set(
            &device,
            material_descriptor_pool_3d,
            material_descriptor_set_layout_3d,
            "allocate_descriptor_sets(material_default)",
        )?;
        let mut executor_resources = ExecutorResources::default();
        register_runtime_descriptor_resources_3d(
            &mut executor_resources,
            descriptor_set_3d,
            cube_scene_buffer,
            cube_object_buffer,
        );
        register_sampled_texture_descriptor(
            &mut executor_resources,
            RUNTIME_DESCRIPTOR_SET_TEXT_2D,
            descriptor_set_text_2d,
            &[(RUNTIME_TEXTURE_SLOT_TEXT_ATLAS, 0)],
            &[(
                RUNTIME_TEXTURE_FONT_ATLAS,
                TextureBinding {
                    image_view: font_atlas_view,
                    sampler: font_atlas_sampler,
                    image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                },
            )],
        );
        register_sampled_texture_descriptor(
            &mut executor_resources,
            RUNTIME_DESCRIPTOR_SET_SHADOW_3D,
            shadow_descriptor_set_3d,
            &[(RUNTIME_TEXTURE_SLOT_SHADOW_MAP, 0)],
            &[(
                RUNTIME_TEXTURE_SHADOW_MAP,
                TextureBinding {
                    image_view: shadow_map_view,
                    sampler: shadow_map_sampler,
                    image_layout: vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
                },
            )],
        );
        update_sampled_texture_descriptor(
            &device,
            &executor_resources,
            RUNTIME_DESCRIPTOR_SET_TEXT_2D,
            &[(RUNTIME_TEXTURE_SLOT_TEXT_ATLAS, RUNTIME_TEXTURE_FONT_ATLAS)],
        );
        update_sampled_texture_descriptor(
            &device,
            &executor_resources,
            RUNTIME_DESCRIPTOR_SET_SHADOW_3D,
            &[(RUNTIME_TEXTURE_SLOT_SHADOW_MAP, RUNTIME_TEXTURE_SHADOW_MAP)],
        );
        register_sampled_texture_descriptor(
            &mut executor_resources,
            descriptor_handle_for_texture(None),
            default_material_descriptor_set_3d,
            &[(RUNTIME_TEXTURE_SLOT_ALBEDO, 0)],
            &[(
                RUNTIME_TEXTURE_WHITE,
                TextureBinding {
                    image_view: white_texture_view,
                    sampler: white_texture_sampler,
                    image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                },
            )],
        );
        update_runtime_descriptor_resources_3d(&device, &executor_resources)?;
        update_sampled_texture_descriptor(
            &device,
            &executor_resources,
            descriptor_handle_for_texture(None),
            &[(RUNTIME_TEXTURE_SLOT_ALBEDO, RUNTIME_TEXTURE_WHITE)],
        );
        let mut graphics_pipeline_cache = GraphicsPipelineCache::default();
        let mut pipeline_compiler = VulkanGraphicsPipelineCompiler::new(device.clone());
        let (pipeline_layout_2d, graphics_pipeline_2d) = create_graphics_pipeline_2d(
            &device,
            render_pass,
            msaa_samples,
            &mut pipeline_compiler,
            &mut graphics_pipeline_cache,
            &mut executor_resources,
        )?;
        let (pipeline_layout_text_2d, graphics_pipeline_text_2d) =
            create_graphics_pipeline_text_2d(
                &device,
                render_pass,
                msaa_samples,
                descriptor_set_layout_text_2d,
                &mut pipeline_compiler,
                &mut graphics_pipeline_cache,
                &mut executor_resources,
            )?;
        let (pipeline_layout_3d, graphics_pipeline_3d) = create_graphics_pipeline_3d(
            &device,
            render_pass,
            msaa_samples,
            descriptor_set_layout_3d,
            material_descriptor_set_layout_3d,
            shadow_descriptor_set_layout_3d,
            &mut pipeline_compiler,
            &mut graphics_pipeline_cache,
            &mut executor_resources,
        )?;
        let (shadow_pipeline_layout, shadow_graphics_pipeline) =
            create_shadow_pipeline_3d(&device, shadow_render_pass, descriptor_set_layout_3d)?;
        let framebuffers = create_framebuffers(
            &device,
            render_pass,
            &swapchain_bundle.image_views,
            color_image_view,
            depth_image_view,
            swapchain_bundle.extent,
            msaa_samples,
        )?;
        let (primitive_buffer, primitive_buffer_memory) = create_buffer(
            &instance,
            &device,
            physical_device,
            (INITIAL_PRIMITIVE_CAPACITY * size_of::<PrimitiveInstance>()) as vk::DeviceSize,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;
        let (text_glyph_buffer, text_glyph_buffer_memory) = create_buffer(
            &instance,
            &device,
            physical_device,
            (INITIAL_TEXT_GLYPH_CAPACITY * size_of::<TextGlyphInstance>()) as vk::DeviceSize,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;
        let (cube_vertex_buffer, cube_vertex_buffer_memory) = create_buffer(
            &instance,
            &device,
            physical_device,
            (INITIAL_CUBE_VERTEX_CAPACITY * size_of::<CubeVertex>()) as vk::DeviceSize,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;
        let (image_available_semaphore, render_finished_semaphore, in_flight_fence) =
            create_sync_resources(&device)?;

        Ok(Self {
            _entry: entry,
            instance,
            surface_loader,
            surface,
            physical_device,
            queue_family_index,
            device,
            graphics_queue,
            present_queue,
            swapchain_loader,
            swapchain: swapchain_bundle.swapchain,
            swapchain_images: swapchain_bundle.images,
            swapchain_image_views: swapchain_bundle.image_views,
            swapchain_extent: swapchain_bundle.extent,
            swapchain_format: swapchain_bundle.format,
            msaa_samples,
            color_image,
            color_image_memory,
            color_image_view,
            depth_format,
            depth_image,
            depth_image_memory,
            depth_image_view,
            render_pass,
            screenshot_render_pass,
            pipeline_layout_2d,
            graphics_pipeline_2d,
            descriptor_set_layout_text_2d,
            descriptor_pool_text_2d,
            descriptor_set_text_2d,
            pipeline_layout_text_2d,
            graphics_pipeline_text_2d,
            descriptor_set_layout_3d,
            descriptor_pool_3d,
            descriptor_set_3d,
            shadow_descriptor_set_layout_3d,
            shadow_descriptor_pool_3d,
            shadow_descriptor_set_3d,
            material_descriptor_set_layout_3d,
            material_descriptor_pool_3d,
            default_material_descriptor_set_3d,
            pipeline_layout_3d,
            graphics_pipeline_3d,
            shadow_render_pass,
            shadow_pipeline_layout,
            shadow_graphics_pipeline,
            shadow_map_image,
            shadow_map_memory,
            shadow_map_view,
            shadow_map_sampler,
            shadow_framebuffer,
            executor_resources,
            graphics_pipeline_cache,
            pipeline_compiler,
            material_descriptor_sets_3d: HashMap::new(),
            framebuffers,
            command_pool,
            command_buffer,
            primitive_buffer,
            primitive_buffer_memory,
            primitive_capacity: INITIAL_PRIMITIVE_CAPACITY,
            text_glyph_buffer,
            text_glyph_buffer_memory,
            text_glyph_capacity: INITIAL_TEXT_GLYPH_CAPACITY,
            cube_vertex_buffer,
            cube_vertex_buffer_memory,
            cube_vertex_capacity: INITIAL_CUBE_VERTEX_CAPACITY,
            cube_scene_buffer,
            cube_scene_buffer_memory,
            cube_object_buffer,
            cube_object_buffer_memory,
            white_texture_image,
            white_texture_memory,
            white_texture_view,
            white_texture_sampler,
            font_atlas_image,
            font_atlas_memory,
            font_atlas_view,
            font_atlas_sampler,
            font_atlas_layout,
            image_available_semaphore,
            render_finished_semaphore,
            in_flight_fence,
        })
    }

    fn draw_frame(
        &mut self,
        scene_config: &SceneConfig,
        frame: &SceneFrame,
    ) -> Result<DrawFrameResult, ApiError> {
        vk_result(
            unsafe {
                self.device
                    .wait_for_fences(&[self.in_flight_fence], true, u64::MAX)
            },
            "wait_for_fences",
        )?;
        vk_result(
            unsafe { self.device.reset_fences(&[self.in_flight_fence]) },
            "reset_fences",
        )?;

        let (image_index, suboptimal) = match unsafe {
            self.swapchain_loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.image_available_semaphore,
                vk::Fence::null(),
            )
        } {
            Ok(value) => value,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return Ok(DrawFrameResult::NeedsResize),
            Err(err) => {
                return Err(ApiError::Vulkan {
                    context: "acquire_next_image",
                    result: err,
                });
            }
        };
        if suboptimal {
            return Ok(DrawFrameResult::NeedsResize);
        }

        let primitive_instances =
            build_primitive_instances(frame, scene_config, self.swapchain_extent);
        let text_glyph_instances = build_text_glyph_instances(
            frame,
            scene_config.camera,
            &self.font_atlas_layout,
            self.swapchain_extent,
        );
        let screenshot_requested = scene_config.screenshot_path.is_some();
        let screenshot_sample_count = if screenshot_requested {
            scene_config.screenshot_accumulation_samples.max(1)
        } else {
            1
        };
        let (
            cube_vertices,
            cube_draw_batches,
            shadow_draw_batches,
            _cube_view_projection,
            _cube_scene_uniforms,
            gpu_cubes,
        ) = build_mesh_vertices(
            frame,
            scene_config.camera_3d,
            scene_config.lighting,
            self.swapchain_extent,
            [0.0, 0.0],
        );
        self.ensure_primitive_capacity(primitive_instances.len())?;
        self.upload_primitive_instances(&primitive_instances)?;
        self.ensure_text_glyph_capacity(text_glyph_instances.len())?;
        self.upload_text_glyph_instances(&text_glyph_instances)?;
        self.ensure_cube_vertex_capacity(cube_vertices.len())?;
        self.upload_cube_vertices(&cube_vertices)?;
        self.upload_cube_objects(&gpu_cubes)?;
        let screenshot_readback = if scene_config.screenshot_path.is_some() {
            Some(create_buffer(
                &self.instance,
                &self.device,
                self.physical_device,
                (self.swapchain_extent.width as vk::DeviceSize)
                    * (self.swapchain_extent.height as vk::DeviceSize)
                    * 4,
                vk::BufferUsageFlags::TRANSFER_DST,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )?)
        } else {
            None
        };

        if screenshot_requested {
            let screenshot_byte_len = (self.swapchain_extent.width as usize)
                * (self.swapchain_extent.height as usize)
                * 4;
            let mut screenshot_accumulator = if screenshot_sample_count > 1 {
                Some(vec![0.0_f32; screenshot_byte_len])
            } else {
                None
            };

            for sample_index in 0..screenshot_sample_count {
                let camera_jitter_ndc = if screenshot_sample_count > 1 {
                    screenshot_camera_jitter(sample_index, self.swapchain_extent)
                } else {
                    [0.0, 0.0]
                };
                let (_, _, _, cube_view_projection, cube_scene_uniforms, _) = build_mesh_vertices(
                    frame,
                    scene_config.camera_3d,
                    scene_config.lighting,
                    self.swapchain_extent,
                    camera_jitter_ndc,
                );
                self.upload_cube_scene_uniforms(&cube_scene_uniforms)?;
                let shadow_view_projection = CubeViewProjectionPushConstants {
                    view_projection: cube_scene_uniforms.shadow_view_projection,
                };

                vk_result(
                    unsafe {
                        self.device.reset_command_buffer(
                            self.command_buffer,
                            vk::CommandBufferResetFlags::empty(),
                        )
                    },
                    "reset_command_buffer",
                )?;
                self.record_command_buffer(
                    image_index,
                    scene_config,
                    primitive_instances.len() as u32,
                    text_glyph_instances.len() as u32,
                    &cube_draw_batches,
                    &shadow_draw_batches,
                    cube_view_projection,
                    shadow_view_projection,
                    screenshot_readback.as_ref().map(|(buffer, _)| *buffer),
                )?;

                let command_buffers = [self.command_buffer];
                let mut submit_info = vk::SubmitInfo::default().command_buffers(&command_buffers);
                let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
                let wait_semaphores = [self.image_available_semaphore];
                let signal_semaphores = [self.render_finished_semaphore];
                if sample_index == 0 {
                    submit_info = submit_info
                        .wait_semaphores(&wait_semaphores)
                        .wait_dst_stage_mask(&wait_stages);
                }
                if sample_index == screenshot_sample_count - 1 {
                    submit_info = submit_info.signal_semaphores(&signal_semaphores);
                }

                vk_result(
                    unsafe {
                        self.device.queue_submit(
                            self.graphics_queue,
                            &[submit_info],
                            self.in_flight_fence,
                        )
                    },
                    "queue_submit",
                )?;
                vk_result(
                    unsafe {
                        self.device
                            .wait_for_fences(&[self.in_flight_fence], true, u64::MAX)
                    },
                    "wait_for_fences(screenshot_sample)",
                )?;

                if let (Some((_, memory)), Some(accumulator)) = (
                    screenshot_readback.as_ref(),
                    screenshot_accumulator.as_mut(),
                ) {
                    let rgba = read_screenshot_rgba_from_memory(
                        &self.device,
                        *memory,
                        self.swapchain_extent,
                        self.swapchain_format,
                    )?;
                    accumulate_screenshot_rgba(accumulator, &rgba);
                }

                if sample_index + 1 < screenshot_sample_count {
                    vk_result(
                        unsafe { self.device.reset_fences(&[self.in_flight_fence]) },
                        "reset_fences(screenshot_sample)",
                    )?;
                }
            }

            if let (Some(path), Some((buffer, memory))) =
                (scene_config.screenshot_path.as_ref(), screenshot_readback)
            {
                let save_result = if let Some(accumulator) = screenshot_accumulator.as_ref() {
                    let rgba =
                        resolve_accumulated_screenshot_rgba(accumulator, screenshot_sample_count);
                    save_screenshot_rgba(
                        &rgba,
                        self.swapchain_extent,
                        screenshot_output_extent(scene_config.screenshot_resolution),
                        path,
                    )
                } else {
                    save_screenshot_from_buffer(
                        &self.device,
                        memory,
                        self.swapchain_extent,
                        self.swapchain_format,
                        screenshot_output_extent(scene_config.screenshot_resolution),
                        path,
                    )
                };
                unsafe {
                    self.device.destroy_buffer(buffer, None);
                    self.device.free_memory(memory, None);
                }
                save_result?;
            }
        } else {
            let (_, _, _, cube_view_projection, cube_scene_uniforms, _) = build_mesh_vertices(
                frame,
                scene_config.camera_3d,
                scene_config.lighting,
                self.swapchain_extent,
                [0.0, 0.0],
            );
            self.upload_cube_scene_uniforms(&cube_scene_uniforms)?;
            let shadow_view_projection = CubeViewProjectionPushConstants {
                view_projection: cube_scene_uniforms.shadow_view_projection,
            };

            vk_result(
                unsafe {
                    self.device.reset_command_buffer(
                        self.command_buffer,
                        vk::CommandBufferResetFlags::empty(),
                    )
                },
                "reset_command_buffer",
            )?;
            self.record_command_buffer(
                image_index,
                scene_config,
                primitive_instances.len() as u32,
                text_glyph_instances.len() as u32,
                &cube_draw_batches,
                &shadow_draw_batches,
                cube_view_projection,
                shadow_view_projection,
                None,
            )?;

            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let wait_semaphores = [self.image_available_semaphore];
            let signal_semaphores = [self.render_finished_semaphore];
            let command_buffers = [self.command_buffer];
            let submit_infos = [vk::SubmitInfo::default()
                .wait_semaphores(&wait_semaphores)
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(&command_buffers)
                .signal_semaphores(&signal_semaphores)];

            vk_result(
                unsafe {
                    self.device.queue_submit(
                        self.graphics_queue,
                        &submit_infos,
                        self.in_flight_fence,
                    )
                },
                "queue_submit",
            )?;
        }

        let signal_semaphores = [self.render_finished_semaphore];
        let swapchains = [self.swapchain];
        let image_indices = [image_index];
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&signal_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);
        match unsafe {
            self.swapchain_loader
                .queue_present(self.present_queue, &present_info)
        } {
            Ok(suboptimal) if suboptimal => Ok(DrawFrameResult::NeedsResize),
            Ok(_) => Ok(DrawFrameResult::Drawn),
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => Ok(DrawFrameResult::NeedsResize),
            Err(err) => Err(ApiError::Vulkan {
                context: "queue_present",
                result: err,
            }),
        }
    }

    fn record_command_buffer(
        &mut self,
        image_index: u32,
        scene_config: &SceneConfig,
        primitive_count: u32,
        text_glyph_count: u32,
        cube_draw_batches: &[MeshDrawBatch3D],
        shadow_draw_batches: &[MeshDrawBatch3D],
        cube_view_projection: CubeViewProjectionPushConstants,
        shadow_view_projection: CubeViewProjectionPushConstants,
        screenshot_readback_buffer: Option<vk::Buffer>,
    ) -> Result<(), ApiError> {
        let begin_info = vk::CommandBufferBeginInfo::default();
        vk_result(
            unsafe {
                self.device
                    .begin_command_buffer(self.command_buffer, &begin_info)
            },
            "begin_command_buffer",
        )?;

        if matches!(scene_config.lighting.shadows.mode, ShadowMode::Live)
            && !shadow_draw_batches.is_empty()
        {
            let shadow_clear_values = [vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            }];
            let shadow_render_pass_begin = vk::RenderPassBeginInfo::default()
                .render_pass(self.shadow_render_pass)
                .framebuffer(self.shadow_framebuffer)
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: vk::Extent2D {
                        width: SHADOW_MAP_SIZE,
                        height: SHADOW_MAP_SIZE,
                    },
                })
                .clear_values(&shadow_clear_values);
            unsafe {
                self.device.cmd_begin_render_pass(
                    self.command_buffer,
                    &shadow_render_pass_begin,
                    vk::SubpassContents::INLINE,
                );
                let shadow_viewports = [vk::Viewport::default()
                    .x(0.0)
                    .y(0.0)
                    .width(SHADOW_MAP_SIZE as f32)
                    .height(SHADOW_MAP_SIZE as f32)
                    .min_depth(0.0)
                    .max_depth(1.0)];
                let shadow_scissors = [vk::Rect2D::default()
                    .offset(vk::Offset2D { x: 0, y: 0 })
                    .extent(vk::Extent2D {
                        width: SHADOW_MAP_SIZE,
                        height: SHADOW_MAP_SIZE,
                    })];
                let cube_vertex_buffers = [self.cube_vertex_buffer];
                let offsets = [0_u64];
                self.device
                    .cmd_set_viewport(self.command_buffer, 0, &shadow_viewports);
                self.device
                    .cmd_set_scissor(self.command_buffer, 0, &shadow_scissors);
                self.device.cmd_bind_pipeline(
                    self.command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.shadow_graphics_pipeline,
                );
                self.device.cmd_bind_descriptor_sets(
                    self.command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.shadow_pipeline_layout,
                    0,
                    &[self.descriptor_set_3d],
                    &[],
                );
                self.device.cmd_bind_vertex_buffers(
                    self.command_buffer,
                    0,
                    &cube_vertex_buffers,
                    &offsets,
                );
                self.device.cmd_push_constants(
                    self.command_buffer,
                    self.shadow_pipeline_layout,
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    cube_view_projection_as_bytes(&shadow_view_projection),
                );
                for batch in shadow_draw_batches {
                    self.device.cmd_draw(
                        self.command_buffer,
                        batch.vertex_count,
                        1,
                        batch.first_vertex,
                        0,
                    );
                }
                self.device.cmd_end_render_pass(self.command_buffer);
            }
        }

        let clear_values = if self.msaa_samples == vk::SampleCountFlags::TYPE_1 {
            vec![
                vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: scene_config.clear_color,
                    },
                },
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 1.0,
                        stencil: 0,
                    },
                },
            ]
        } else {
            vec![
                vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: scene_config.clear_color,
                    },
                },
                vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [0.0, 0.0, 0.0, 0.0],
                    },
                },
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 1.0,
                        stencil: 0,
                    },
                },
            ]
        };
        let render_pass_begin = vk::RenderPassBeginInfo::default()
            .render_pass(self.render_pass)
            .framebuffer(self.framebuffers[image_index as usize])
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: self.swapchain_extent,
            })
            .clear_values(&clear_values);
        unsafe {
            self.device.cmd_begin_render_pass(
                self.command_buffer,
                &render_pass_begin,
                vk::SubpassContents::INLINE,
            );
            let viewports = [vk::Viewport::default()
                .x(0.0)
                .y(self.swapchain_extent.height as f32)
                .width(self.swapchain_extent.width as f32)
                // Use a negative Vulkan viewport height so clip-space +Y stays visually up.
                .height(-(self.swapchain_extent.height as f32))
                .min_depth(0.0)
                .max_depth(1.0)];
            let scissors = [vk::Rect2D::default()
                .offset(vk::Offset2D { x: 0, y: 0 })
                .extent(self.swapchain_extent)];
            self.device
                .cmd_set_viewport(self.command_buffer, 0, &viewports);
            self.device
                .cmd_set_scissor(self.command_buffer, 0, &scissors);
            if !cube_draw_batches.is_empty() {
                let cube_vertex_buffers = [self.cube_vertex_buffer];
                let offsets = [0_u64];
                self.device.cmd_bind_pipeline(
                    self.command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.graphics_pipeline_3d,
                );
                self.device.cmd_bind_descriptor_sets(
                    self.command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipeline_layout_3d,
                    0,
                    &[self.descriptor_set_3d],
                    &[],
                );
                self.device.cmd_bind_descriptor_sets(
                    self.command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipeline_layout_3d,
                    2,
                    &[self.shadow_descriptor_set_3d],
                    &[],
                );
                self.device.cmd_bind_vertex_buffers(
                    self.command_buffer,
                    0,
                    &cube_vertex_buffers,
                    &offsets,
                );
                self.device.cmd_push_constants(
                    self.command_buffer,
                    self.pipeline_layout_3d,
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    cube_view_projection_as_bytes(&cube_view_projection),
                );
                for batch in cube_draw_batches {
                    let material_descriptor_set =
                        self.ensure_material_descriptor_set_3d(batch.albedo_texture)?;
                    self.device.cmd_bind_descriptor_sets(
                        self.command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.pipeline_layout_3d,
                        1,
                        &[material_descriptor_set],
                        &[],
                    );
                    self.device.cmd_draw(
                        self.command_buffer,
                        batch.vertex_count,
                        1,
                        batch.first_vertex,
                        0,
                    );
                }
            }

            if primitive_count > 0 {
                let vertex_buffers = [self.primitive_buffer];
                let offsets = [0_u64];
                self.device.cmd_bind_pipeline(
                    self.command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.graphics_pipeline_2d,
                );
                self.device.cmd_bind_vertex_buffers(
                    self.command_buffer,
                    0,
                    &vertex_buffers,
                    &offsets,
                );
                self.device
                    .cmd_draw(self.command_buffer, 6, primitive_count, 0, 0);
            }

            if text_glyph_count > 0 {
                let vertex_buffers = [self.text_glyph_buffer];
                let offsets = [0_u64];
                self.device.cmd_bind_pipeline(
                    self.command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.graphics_pipeline_text_2d,
                );
                self.device.cmd_bind_descriptor_sets(
                    self.command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipeline_layout_text_2d,
                    0,
                    &[self.descriptor_set_text_2d],
                    &[],
                );
                self.device.cmd_bind_vertex_buffers(
                    self.command_buffer,
                    0,
                    &vertex_buffers,
                    &offsets,
                );
                self.device
                    .cmd_draw(self.command_buffer, 6, text_glyph_count, 0, 0);
            }

            self.device.cmd_end_render_pass(self.command_buffer);
            if let Some(readback_buffer) = screenshot_readback_buffer {
                record_screenshot_copy_commands(
                    &self.device,
                    self.command_buffer,
                    self.swapchain_images[image_index as usize],
                    readback_buffer,
                    self.swapchain_extent,
                );
            }
        }

        vk_result(
            unsafe { self.device.end_command_buffer(self.command_buffer) },
            "end_command_buffer",
        )
    }

    fn ensure_material_descriptor_set_3d(
        &mut self,
        texture_handle: Option<TextureHandle>,
    ) -> Result<vk::DescriptorSet, ApiError> {
        let Some(texture_handle) = texture_handle else {
            return Ok(self.default_material_descriptor_set_3d);
        };

        if let Some(descriptor_set) = self.material_descriptor_sets_3d.get(&texture_handle) {
            return Ok(*descriptor_set);
        }
        if !self
            .executor_resources
            .textures
            .contains_key(&texture_handle)
        {
            return Ok(self.default_material_descriptor_set_3d);
        }

        let descriptor_set = allocate_descriptor_set(
            &self.device,
            self.material_descriptor_pool_3d,
            self.material_descriptor_set_layout_3d,
            "allocate_descriptor_sets(material_3d)",
        )?;
        let descriptor_handle = descriptor_handle_for_texture(Some(texture_handle));
        register_sampled_texture_descriptor(
            &mut self.executor_resources,
            descriptor_handle,
            descriptor_set,
            &[(RUNTIME_TEXTURE_SLOT_ALBEDO, 0)],
            &[],
        );
        let writes = update_sampled_texture_descriptor(
            &self.device,
            &self.executor_resources,
            descriptor_handle,
            &[(RUNTIME_TEXTURE_SLOT_ALBEDO, texture_handle)],
        );
        if writes == 0 {
            return Err(ApiError::InvalidConfig {
                reason: format!(
                    "material descriptor resolution produced no writes for texture handle {}",
                    texture_handle.0
                ),
            });
        }
        self.material_descriptor_sets_3d
            .insert(texture_handle, descriptor_set);
        Ok(descriptor_set)
    }

    fn ensure_primitive_capacity(&mut self, required: usize) -> Result<(), ApiError> {
        if required <= self.primitive_capacity {
            return Ok(());
        }

        let new_capacity = required.next_power_of_two().max(INITIAL_PRIMITIVE_CAPACITY);
        let (buffer, memory) = create_buffer(
            &self.instance,
            &self.device,
            self.physical_device,
            (new_capacity * size_of::<PrimitiveInstance>()) as vk::DeviceSize,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        unsafe {
            if self.primitive_buffer != vk::Buffer::null() {
                self.device.destroy_buffer(self.primitive_buffer, None);
            }
            if self.primitive_buffer_memory != vk::DeviceMemory::null() {
                self.device.free_memory(self.primitive_buffer_memory, None);
            }
        }

        self.primitive_buffer = buffer;
        self.primitive_buffer_memory = memory;
        self.primitive_capacity = new_capacity;
        Ok(())
    }

    fn ensure_text_glyph_capacity(&mut self, required: usize) -> Result<(), ApiError> {
        if required <= self.text_glyph_capacity {
            return Ok(());
        }

        let new_capacity = required
            .next_power_of_two()
            .max(INITIAL_TEXT_GLYPH_CAPACITY);
        let (buffer, memory) = create_buffer(
            &self.instance,
            &self.device,
            self.physical_device,
            (new_capacity * size_of::<TextGlyphInstance>()) as vk::DeviceSize,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        unsafe {
            if self.text_glyph_buffer != vk::Buffer::null() {
                self.device.destroy_buffer(self.text_glyph_buffer, None);
            }
            if self.text_glyph_buffer_memory != vk::DeviceMemory::null() {
                self.device.free_memory(self.text_glyph_buffer_memory, None);
            }
        }

        self.text_glyph_buffer = buffer;
        self.text_glyph_buffer_memory = memory;
        self.text_glyph_capacity = new_capacity;
        Ok(())
    }

    fn upload_primitive_instances(&self, primitives: &[PrimitiveInstance]) -> Result<(), ApiError> {
        if primitives.is_empty() {
            return Ok(());
        }

        let upload_size = std::mem::size_of_val(primitives) as vk::DeviceSize;
        let mapped = vk_result(
            unsafe {
                self.device.map_memory(
                    self.primitive_buffer_memory,
                    0,
                    upload_size,
                    vk::MemoryMapFlags::empty(),
                )
            },
            "map_memory(primitive_buffer)",
        )?;

        unsafe {
            std::ptr::copy_nonoverlapping(
                primitives.as_ptr().cast::<u8>(),
                mapped.cast::<u8>(),
                upload_size as usize,
            );
            self.device.unmap_memory(self.primitive_buffer_memory);
        }

        Ok(())
    }

    fn upload_text_glyph_instances(&self, glyphs: &[TextGlyphInstance]) -> Result<(), ApiError> {
        if glyphs.is_empty() {
            return Ok(());
        }

        let upload_size = std::mem::size_of_val(glyphs) as vk::DeviceSize;
        let mapped = vk_result(
            unsafe {
                self.device.map_memory(
                    self.text_glyph_buffer_memory,
                    0,
                    upload_size,
                    vk::MemoryMapFlags::empty(),
                )
            },
            "map_memory(text_glyph_buffer)",
        )?;

        unsafe {
            std::ptr::copy_nonoverlapping(
                glyphs.as_ptr().cast::<u8>(),
                mapped.cast::<u8>(),
                upload_size as usize,
            );
            self.device.unmap_memory(self.text_glyph_buffer_memory);
        }

        Ok(())
    }

    fn ensure_cube_vertex_capacity(&mut self, required: usize) -> Result<(), ApiError> {
        if required <= self.cube_vertex_capacity {
            return Ok(());
        }

        let new_capacity = required
            .next_power_of_two()
            .max(INITIAL_CUBE_VERTEX_CAPACITY);
        let (buffer, memory) = create_buffer(
            &self.instance,
            &self.device,
            self.physical_device,
            (new_capacity * size_of::<CubeVertex>()) as vk::DeviceSize,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        unsafe {
            if self.cube_vertex_buffer != vk::Buffer::null() {
                self.device.destroy_buffer(self.cube_vertex_buffer, None);
            }
            if self.cube_vertex_buffer_memory != vk::DeviceMemory::null() {
                self.device
                    .free_memory(self.cube_vertex_buffer_memory, None);
            }
        }

        self.cube_vertex_buffer = buffer;
        self.cube_vertex_buffer_memory = memory;
        self.cube_vertex_capacity = new_capacity;
        Ok(())
    }

    fn upload_cube_vertices(&self, vertices: &[CubeVertex]) -> Result<(), ApiError> {
        if vertices.is_empty() {
            return Ok(());
        }

        let upload_size = std::mem::size_of_val(vertices) as vk::DeviceSize;
        let mapped = vk_result(
            unsafe {
                self.device.map_memory(
                    self.cube_vertex_buffer_memory,
                    0,
                    upload_size,
                    vk::MemoryMapFlags::empty(),
                )
            },
            "map_memory(cube_vertex_buffer)",
        )?;

        unsafe {
            std::ptr::copy_nonoverlapping(
                vertices.as_ptr().cast::<u8>(),
                mapped.cast::<u8>(),
                upload_size as usize,
            );
            self.device.unmap_memory(self.cube_vertex_buffer_memory);
        }

        Ok(())
    }

    fn upload_cube_scene_uniforms(&self, uniforms: &CubeSceneUniforms) -> Result<(), ApiError> {
        let upload_size = size_of::<CubeSceneUniforms>() as vk::DeviceSize;
        let mapped = vk_result(
            unsafe {
                self.device.map_memory(
                    self.cube_scene_buffer_memory,
                    0,
                    upload_size,
                    vk::MemoryMapFlags::empty(),
                )
            },
            "map_memory(cube_scene_buffer)",
        )?;

        unsafe {
            std::ptr::copy_nonoverlapping(
                (uniforms as *const CubeSceneUniforms).cast::<u8>(),
                mapped.cast::<u8>(),
                upload_size as usize,
            );
            self.device.unmap_memory(self.cube_scene_buffer_memory);
        }

        Ok(())
    }

    fn upload_cube_objects(&self, cubes: &[GpuSceneCube]) -> Result<(), ApiError> {
        if cubes.len() > MAX_SCENE_CUBES {
            return Err(ApiError::InvalidConfig {
                reason: format!(
                    "scene contains {} cubes but the shader buffer supports at most {}",
                    cubes.len(),
                    MAX_SCENE_CUBES
                ),
            });
        }
        if cubes.is_empty() {
            return Ok(());
        }

        let upload_size = std::mem::size_of_val(cubes) as vk::DeviceSize;
        let mapped = vk_result(
            unsafe {
                self.device.map_memory(
                    self.cube_object_buffer_memory,
                    0,
                    upload_size,
                    vk::MemoryMapFlags::empty(),
                )
            },
            "map_memory(cube_object_buffer)",
        )?;

        unsafe {
            std::ptr::copy_nonoverlapping(
                cubes.as_ptr().cast::<u8>(),
                mapped.cast::<u8>(),
                upload_size as usize,
            );
            self.device.unmap_memory(self.cube_object_buffer_memory);
        }

        Ok(())
    }

    fn recreate_swapchain(&mut self, window: &Window) -> Result<(), ApiError> {
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }

        vk_result(
            unsafe { self.device.device_wait_idle() },
            "device_wait_idle",
        )?;
        self.destroy_swapchain_dependent_resources();

        let swapchain_bundle = create_swapchain_bundle(
            window,
            &self.device,
            &self.surface_loader,
            &self.swapchain_loader,
            self.surface,
            self.physical_device,
            self.queue_family_index,
        )?;
        let (color_image, color_image_memory, color_image_view) =
            if self.msaa_samples == vk::SampleCountFlags::TYPE_1 {
                (
                    vk::Image::null(),
                    vk::DeviceMemory::null(),
                    vk::ImageView::null(),
                )
            } else {
                create_color_resources(
                    &self.instance,
                    &self.device,
                    self.physical_device,
                    swapchain_bundle.extent,
                    swapchain_bundle.format,
                    self.msaa_samples,
                )?
            };
        let (depth_image, depth_image_memory, depth_image_view) = create_depth_resources(
            &self.instance,
            &self.device,
            self.physical_device,
            swapchain_bundle.extent,
            self.depth_format,
            self.msaa_samples,
        )?;
        let render_pass = create_render_pass(
            &self.device,
            swapchain_bundle.format,
            self.depth_format,
            self.msaa_samples,
            vk::ImageLayout::PRESENT_SRC_KHR,
        )?;
        let screenshot_render_pass = create_render_pass(
            &self.device,
            swapchain_bundle.format,
            self.depth_format,
            self.msaa_samples,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        )?;
        let (pipeline_layout_2d, graphics_pipeline_2d) = create_graphics_pipeline_2d(
            &self.device,
            render_pass,
            self.msaa_samples,
            &mut self.pipeline_compiler,
            &mut self.graphics_pipeline_cache,
            &mut self.executor_resources,
        )?;
        let (pipeline_layout_text_2d, graphics_pipeline_text_2d) =
            create_graphics_pipeline_text_2d(
                &self.device,
                render_pass,
                self.msaa_samples,
                self.descriptor_set_layout_text_2d,
                &mut self.pipeline_compiler,
                &mut self.graphics_pipeline_cache,
                &mut self.executor_resources,
            )?;
        let (pipeline_layout_3d, graphics_pipeline_3d) = create_graphics_pipeline_3d(
            &self.device,
            render_pass,
            self.msaa_samples,
            self.descriptor_set_layout_3d,
            self.material_descriptor_set_layout_3d,
            self.shadow_descriptor_set_layout_3d,
            &mut self.pipeline_compiler,
            &mut self.graphics_pipeline_cache,
            &mut self.executor_resources,
        )?;
        let framebuffers = create_framebuffers(
            &self.device,
            render_pass,
            &swapchain_bundle.image_views,
            color_image_view,
            depth_image_view,
            swapchain_bundle.extent,
            self.msaa_samples,
        )?;

        self.swapchain = swapchain_bundle.swapchain;
        self.swapchain_images = swapchain_bundle.images;
        self.swapchain_image_views = swapchain_bundle.image_views;
        self.swapchain_extent = swapchain_bundle.extent;
        self.swapchain_format = swapchain_bundle.format;
        self.color_image = color_image;
        self.color_image_memory = color_image_memory;
        self.color_image_view = color_image_view;
        self.depth_image = depth_image;
        self.depth_image_memory = depth_image_memory;
        self.depth_image_view = depth_image_view;
        self.render_pass = render_pass;
        self.screenshot_render_pass = screenshot_render_pass;
        self.pipeline_layout_2d = pipeline_layout_2d;
        self.graphics_pipeline_2d = graphics_pipeline_2d;
        self.pipeline_layout_text_2d = pipeline_layout_text_2d;
        self.graphics_pipeline_text_2d = graphics_pipeline_text_2d;
        self.pipeline_layout_3d = pipeline_layout_3d;
        self.graphics_pipeline_3d = graphics_pipeline_3d;
        self.framebuffers = framebuffers;

        Ok(())
    }

    fn destroy_swapchain_dependent_resources(&mut self) {
        unsafe {
            for framebuffer in self.framebuffers.drain(..) {
                self.device.destroy_framebuffer(framebuffer, None);
            }
            self.destroy_runtime_cached_pipeline(RUNTIME_SHADER_2D);
            self.graphics_pipeline_2d = vk::Pipeline::null();
            if self.pipeline_layout_2d != vk::PipelineLayout::null() {
                self.device
                    .destroy_pipeline_layout(self.pipeline_layout_2d, None);
                self.pipeline_layout_2d = vk::PipelineLayout::null();
            }
            self.destroy_runtime_cached_pipeline(RUNTIME_SHADER_TEXT_2D);
            self.graphics_pipeline_text_2d = vk::Pipeline::null();
            if self.pipeline_layout_text_2d != vk::PipelineLayout::null() {
                self.device
                    .destroy_pipeline_layout(self.pipeline_layout_text_2d, None);
                self.pipeline_layout_text_2d = vk::PipelineLayout::null();
            }
            self.destroy_runtime_cached_pipeline(RUNTIME_SHADER_3D);
            self.graphics_pipeline_3d = vk::Pipeline::null();
            if self.pipeline_layout_3d != vk::PipelineLayout::null() {
                self.device
                    .destroy_pipeline_layout(self.pipeline_layout_3d, None);
                self.pipeline_layout_3d = vk::PipelineLayout::null();
            }
            if self.render_pass != vk::RenderPass::null() {
                self.device.destroy_render_pass(self.render_pass, None);
                self.render_pass = vk::RenderPass::null();
            }
            if self.screenshot_render_pass != vk::RenderPass::null() {
                self.device
                    .destroy_render_pass(self.screenshot_render_pass, None);
                self.screenshot_render_pass = vk::RenderPass::null();
            }
            if self.color_image_view != vk::ImageView::null() {
                self.device.destroy_image_view(self.color_image_view, None);
                self.color_image_view = vk::ImageView::null();
            }
            if self.color_image != vk::Image::null() {
                self.device.destroy_image(self.color_image, None);
                self.color_image = vk::Image::null();
            }
            if self.color_image_memory != vk::DeviceMemory::null() {
                self.device.free_memory(self.color_image_memory, None);
                self.color_image_memory = vk::DeviceMemory::null();
            }
            if self.depth_image_view != vk::ImageView::null() {
                self.device.destroy_image_view(self.depth_image_view, None);
                self.depth_image_view = vk::ImageView::null();
            }
            if self.depth_image != vk::Image::null() {
                self.device.destroy_image(self.depth_image, None);
                self.depth_image = vk::Image::null();
            }
            if self.depth_image_memory != vk::DeviceMemory::null() {
                self.device.free_memory(self.depth_image_memory, None);
                self.depth_image_memory = vk::DeviceMemory::null();
            }
            for image_view in self.swapchain_image_views.drain(..) {
                self.device.destroy_image_view(image_view, None);
            }
            if self.swapchain != vk::SwapchainKHR::null() {
                self.swapchain_loader
                    .destroy_swapchain(self.swapchain, None);
                self.swapchain = vk::SwapchainKHR::null();
            }
        }
    }

    fn destroy_runtime_cached_pipeline(&mut self, shader: ShaderHandle) {
        if let Ok(program_context) = self.pipeline_compiler.pipeline_cache_context(shader) {
            self.graphics_pipeline_cache.destroy_program_pipelines(
                &self.device,
                &mut self.executor_resources,
                shader,
                program_context,
            );
        } else {
            self.graphics_pipeline_cache.destroy_shader_pipelines(
                &self.device,
                &mut self.executor_resources,
                shader,
            );
        }
    }

    fn wait_idle(&self) {
        let _ = unsafe { self.device.device_wait_idle() };
    }
}

impl Drop for VulkanRuntime {
    fn drop(&mut self) {
        let _ = unsafe { self.device.device_wait_idle() };

        unsafe {
            self.device
                .destroy_semaphore(self.image_available_semaphore, None);
            self.device
                .destroy_semaphore(self.render_finished_semaphore, None);
            self.device.destroy_fence(self.in_flight_fence, None);
            self.device.destroy_buffer(self.primitive_buffer, None);
            self.device.free_memory(self.primitive_buffer_memory, None);
            self.device.destroy_buffer(self.text_glyph_buffer, None);
            self.device.free_memory(self.text_glyph_buffer_memory, None);
            self.device.destroy_buffer(self.cube_vertex_buffer, None);
            self.device
                .free_memory(self.cube_vertex_buffer_memory, None);
            self.device.destroy_buffer(self.cube_scene_buffer, None);
            self.device.free_memory(self.cube_scene_buffer_memory, None);
            self.device.destroy_buffer(self.cube_object_buffer, None);
            self.device
                .free_memory(self.cube_object_buffer_memory, None);
            if self.white_texture_sampler != vk::Sampler::null() {
                self.device
                    .destroy_sampler(self.white_texture_sampler, None);
            }
            if self.white_texture_view != vk::ImageView::null() {
                self.device
                    .destroy_image_view(self.white_texture_view, None);
            }
            if self.white_texture_image != vk::Image::null() {
                self.device.destroy_image(self.white_texture_image, None);
            }
            if self.white_texture_memory != vk::DeviceMemory::null() {
                self.device.free_memory(self.white_texture_memory, None);
            }
            if self.font_atlas_sampler != vk::Sampler::null() {
                self.device.destroy_sampler(self.font_atlas_sampler, None);
            }
            if self.font_atlas_view != vk::ImageView::null() {
                self.device.destroy_image_view(self.font_atlas_view, None);
            }
            if self.font_atlas_image != vk::Image::null() {
                self.device.destroy_image(self.font_atlas_image, None);
            }
            if self.font_atlas_memory != vk::DeviceMemory::null() {
                self.device.free_memory(self.font_atlas_memory, None);
            }
            if self.shadow_map_sampler != vk::Sampler::null() {
                self.device.destroy_sampler(self.shadow_map_sampler, None);
            }
            if self.shadow_framebuffer != vk::Framebuffer::null() {
                self.device
                    .destroy_framebuffer(self.shadow_framebuffer, None);
            }
            if self.shadow_map_view != vk::ImageView::null() {
                self.device.destroy_image_view(self.shadow_map_view, None);
            }
            if self.shadow_map_image != vk::Image::null() {
                self.device.destroy_image(self.shadow_map_image, None);
            }
            if self.shadow_map_memory != vk::DeviceMemory::null() {
                self.device.free_memory(self.shadow_map_memory, None);
            }
            if self.shadow_graphics_pipeline != vk::Pipeline::null() {
                self.device
                    .destroy_pipeline(self.shadow_graphics_pipeline, None);
            }
            if self.shadow_pipeline_layout != vk::PipelineLayout::null() {
                self.device
                    .destroy_pipeline_layout(self.shadow_pipeline_layout, None);
            }
            if self.shadow_render_pass != vk::RenderPass::null() {
                self.device
                    .destroy_render_pass(self.shadow_render_pass, None);
            }
            self.device
                .destroy_descriptor_pool(self.descriptor_pool_text_2d, None);
            self.device
                .destroy_descriptor_set_layout(self.descriptor_set_layout_text_2d, None);
            self.device
                .destroy_descriptor_pool(self.shadow_descriptor_pool_3d, None);
            self.device
                .destroy_descriptor_set_layout(self.shadow_descriptor_set_layout_3d, None);
            self.device
                .destroy_descriptor_pool(self.material_descriptor_pool_3d, None);
            self.device
                .destroy_descriptor_set_layout(self.material_descriptor_set_layout_3d, None);
            self.device
                .destroy_descriptor_pool(self.descriptor_pool_3d, None);
            self.device
                .destroy_descriptor_set_layout(self.descriptor_set_layout_3d, None);
            self.device.destroy_command_pool(self.command_pool, None);

            self.destroy_swapchain_dependent_resources();
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}

fn create_instance(
    entry: &Entry,
    window: &Window,
    config: &ApiConfig,
) -> Result<Instance, ApiError> {
    let application_name =
        CString::new(config.application_name.as_str()).map_err(|_| ApiError::InvalidConfig {
            reason: "application_name cannot contain nul characters".to_string(),
        })?;
    let engine_name =
        CString::new(config.engine_name.as_str()).map_err(|_| ApiError::InvalidConfig {
            reason: "engine_name cannot contain nul characters".to_string(),
        })?;

    let app_info = vk::ApplicationInfo::default()
        .application_name(application_name.as_c_str())
        .application_version(vk::make_api_version(0, 0, 1, 0))
        .engine_name(engine_name.as_c_str())
        .engine_version(vk::make_api_version(0, 0, 1, 0))
        .api_version(vk::API_VERSION_1_3);

    let display_handle = window.display_handle().map_err(|err| ApiError::Window {
        reason: format!("failed to fetch display handle: {err}"),
    })?;
    let extension_names = ash_window::enumerate_required_extensions(display_handle.as_raw())
        .map_err(|err| ApiError::Vulkan {
            context: "enumerate_required_extensions",
            result: err,
        })?;

    let mut layer_cstrings = Vec::new();
    let mut layer_ptrs = Vec::new();
    if config.enable_validation {
        let layer =
            CString::new("VK_LAYER_KHRONOS_validation").map_err(|_| ApiError::InvalidConfig {
                reason: "validation layer name contains nul characters".to_string(),
            })?;
        layer_ptrs.push(layer.as_ptr());
        layer_cstrings.push(layer);
    }

    let instance_info = vk::InstanceCreateInfo::default()
        .application_info(&app_info)
        .enabled_extension_names(extension_names)
        .enabled_layer_names(&layer_ptrs);

    let instance = vk_result(
        unsafe { entry.create_instance(&instance_info, None) },
        "create_instance",
    )?;

    drop(layer_cstrings);
    Ok(instance)
}

fn create_surface(
    entry: &Entry,
    instance: &Instance,
    window: &Window,
) -> Result<vk::SurfaceKHR, ApiError> {
    let display_handle = window.display_handle().map_err(|err| ApiError::Window {
        reason: format!("failed to fetch display handle: {err}"),
    })?;
    let window_handle = window.window_handle().map_err(|err| ApiError::Window {
        reason: format!("failed to fetch window handle: {err}"),
    })?;

    vk_result(
        unsafe {
            ash_window::create_surface(
                entry,
                instance,
                display_handle.as_raw(),
                window_handle.as_raw(),
                None,
            )
        },
        "create_surface",
    )
}

fn pick_physical_device(
    instance: &Instance,
    surface_loader: &surface::Instance,
    surface: vk::SurfaceKHR,
) -> Result<(vk::PhysicalDevice, u32), ApiError> {
    let physical_devices = vk_result(
        unsafe { instance.enumerate_physical_devices() },
        "enumerate_physical_devices",
    )?;

    for physical_device in physical_devices {
        let queue_family_properties =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
        for (index, family) in queue_family_properties.iter().enumerate() {
            if !family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                continue;
            }

            let supports_present = vk_result(
                unsafe {
                    surface_loader.get_physical_device_surface_support(
                        physical_device,
                        index as u32,
                        surface,
                    )
                },
                "get_physical_device_surface_support",
            )?;
            if supports_present {
                return Ok((physical_device, index as u32));
            }
        }
    }

    Err(ApiError::InvalidConfig {
        reason: "no physical device with graphics+present queue support found".to_string(),
    })
}

fn create_logical_device(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    queue_family_index: u32,
) -> Result<(Device, vk::Queue, vk::Queue), ApiError> {
    let queue_priorities = [1.0_f32];
    let queue_info = [vk::DeviceQueueCreateInfo::default()
        .queue_family_index(queue_family_index)
        .queue_priorities(&queue_priorities)];
    let enabled_device_extensions = [swapchain::NAME.as_ptr()];

    let device_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(&queue_info)
        .enabled_extension_names(&enabled_device_extensions);

    let device = vk_result(
        unsafe { instance.create_device(physical_device, &device_info, None) },
        "create_device",
    )?;
    let graphics_queue = unsafe { device.get_device_queue(queue_family_index, 0) };
    let present_queue = graphics_queue;
    Ok((device, graphics_queue, present_queue))
}

fn create_swapchain_bundle(
    window: &Window,
    device: &Device,
    surface_loader: &surface::Instance,
    swapchain_loader: &swapchain::Device,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    queue_family_index: u32,
) -> Result<SwapchainBundle, ApiError> {
    let capabilities = vk_result(
        unsafe {
            surface_loader.get_physical_device_surface_capabilities(physical_device, surface)
        },
        "get_physical_device_surface_capabilities",
    )?;
    let formats = vk_result(
        unsafe { surface_loader.get_physical_device_surface_formats(physical_device, surface) },
        "get_physical_device_surface_formats",
    )?;
    let present_modes = vk_result(
        unsafe {
            surface_loader.get_physical_device_surface_present_modes(physical_device, surface)
        },
        "get_physical_device_surface_present_modes",
    )?;

    let chosen_format = choose_surface_format(&formats).ok_or(ApiError::InvalidConfig {
        reason: "surface reported zero formats".to_string(),
    })?;
    if present_modes.is_empty() {
        return Err(ApiError::InvalidConfig {
            reason: "surface reported zero present modes".to_string(),
        });
    }
    let chosen_present_mode = choose_present_mode(&present_modes);
    let extent = choose_surface_extent(window, capabilities);

    let mut image_count = capabilities.min_image_count + 1;
    if capabilities.max_image_count > 0 && image_count > capabilities.max_image_count {
        image_count = capabilities.max_image_count;
    }

    let queue_family_indices = [queue_family_index];
    let swapchain_info = vk::SwapchainCreateInfoKHR::default()
        .surface(surface)
        .min_image_count(image_count)
        .image_color_space(chosen_format.color_space)
        .image_format(chosen_format.format)
        .image_extent(extent)
        .image_array_layers(1)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC)
        .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
        .queue_family_indices(&queue_family_indices)
        .pre_transform(capabilities.current_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(chosen_present_mode)
        .clipped(true);
    let swapchain = vk_result(
        unsafe { swapchain_loader.create_swapchain(&swapchain_info, None) },
        "create_swapchain",
    )?;
    let swapchain_images = vk_result(
        unsafe { swapchain_loader.get_swapchain_images(swapchain) },
        "get_swapchain_images",
    )?;

    let mut image_views = Vec::with_capacity(swapchain_images.len());
    for image in &swapchain_images {
        let subresource_range = vk::ImageSubresourceRange::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .base_mip_level(0)
            .level_count(1)
            .base_array_layer(0)
            .layer_count(1);

        let image_view_info = vk::ImageViewCreateInfo::default()
            .image(*image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(chosen_format.format)
            .subresource_range(subresource_range);

        let image_view = vk_result(
            unsafe { device.create_image_view(&image_view_info, None) },
            "create_image_view",
        )?;
        image_views.push(image_view);
    }

    Ok(SwapchainBundle {
        swapchain,
        images: swapchain_images,
        image_views,
        extent,
        format: chosen_format.format,
    })
}

fn create_render_pass(
    device: &Device,
    color_format: vk::Format,
    depth_format: vk::Format,
    msaa_samples: vk::SampleCountFlags,
    final_color_layout: vk::ImageLayout,
) -> Result<vk::RenderPass, ApiError> {
    if msaa_samples == vk::SampleCountFlags::TYPE_1 {
        let color_attachment = vk::AttachmentDescription::default()
            .format(color_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(final_color_layout);
        let depth_attachment = vk::AttachmentDescription::default()
            .format(depth_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let color_attachment_ref = [vk::AttachmentReference::default()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
        let depth_attachment_ref = vk::AttachmentReference::default()
            .attachment(1)
            .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let subpasses = [vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachment_ref)
            .depth_stencil_attachment(&depth_attachment_ref)];
        let dependencies = [vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_stage_mask(
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            )
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            )];
        let attachments = [color_attachment, depth_attachment];
        let render_pass_info = vk::RenderPassCreateInfo::default()
            .attachments(&attachments)
            .subpasses(&subpasses)
            .dependencies(&dependencies);
        return vk_result(
            unsafe { device.create_render_pass(&render_pass_info, None) },
            "create_render_pass",
        );
    }

    let color_attachment = vk::AttachmentDescription::default()
        .format(color_format)
        .samples(msaa_samples)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::DONT_CARE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
    let resolve_attachment = vk::AttachmentDescription::default()
        .format(color_format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::DONT_CARE)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(final_color_layout);
    let depth_attachment = vk::AttachmentDescription::default()
        .format(depth_format)
        .samples(msaa_samples)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::DONT_CARE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
    let color_attachment_ref = [vk::AttachmentReference::default()
        .attachment(0)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
    let resolve_attachment_ref = [vk::AttachmentReference::default()
        .attachment(1)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
    let depth_attachment_ref = vk::AttachmentReference::default()
        .attachment(2)
        .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
    let subpasses = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_attachment_ref)
        .resolve_attachments(&resolve_attachment_ref)
        .depth_stencil_attachment(&depth_attachment_ref)];
    let dependencies = [vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_stage_mask(
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
        )
        .dst_access_mask(
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        )];
    let attachments = [color_attachment, resolve_attachment, depth_attachment];

    let render_pass_info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments)
        .subpasses(&subpasses)
        .dependencies(&dependencies);

    vk_result(
        unsafe { device.create_render_pass(&render_pass_info, None) },
        "create_render_pass",
    )
}

fn create_graphics_pipeline_2d(
    device: &Device,
    render_pass: vk::RenderPass,
    msaa_samples: vk::SampleCountFlags,
    pipeline_compiler: &mut VulkanGraphicsPipelineCompiler,
    graphics_pipeline_cache: &mut GraphicsPipelineCache,
    executor_resources: &mut ExecutorResources,
) -> Result<(vk::PipelineLayout, vk::Pipeline), ApiError> {
    let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default();

    let pipeline_layout = vk_result(
        unsafe { device.create_pipeline_layout(&pipeline_layout_info, None) },
        "create_pipeline_layout",
    )?;
    let compile_result = compile_runtime_graphics_pipeline_2d(
        device,
        render_pass,
        pipeline_layout,
        msaa_samples,
        pipeline_compiler,
        graphics_pipeline_cache,
        executor_resources,
    );
    let graphics_pipeline = match compile_result {
        Ok(pipeline) => pipeline,
        Err(err) => {
            unsafe {
                device.destroy_pipeline_layout(pipeline_layout, None);
            }
            return Err(err);
        }
    };

    Ok((pipeline_layout, graphics_pipeline))
}

fn compile_runtime_graphics_pipeline_2d(
    device: &Device,
    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    msaa_samples: vk::SampleCountFlags,
    pipeline_compiler: &mut VulkanGraphicsPipelineCompiler,
    graphics_pipeline_cache: &mut GraphicsPipelineCache,
    executor_resources: &mut ExecutorResources,
) -> Result<vk::Pipeline, ApiError> {
    let _ = device;
    pipeline_compiler.set_sample_count(msaa_samples);
    pipeline_compiler.register_shader_program(
        RUNTIME_SHADER_2D,
        ShaderProgramDefinition {
            vertex_spirv: read_spirv_words(PRIMITIVE_2D_VERT_SPV)?,
            fragment_spirv: read_spirv_words(PRIMITIVE_2D_FRAG_SPV)?,
            layout: pipeline_layout,
            render_pass,
            subpass: 0,
        },
    );

    let descriptor = GraphicsPipelineDescriptor {
        topology: vk::PrimitiveTopology::TRIANGLE_LIST,
        render_state: crate::syntax::BLEND,
        vertex_attributes: vec![
            VertexAttributeBinding {
                index: 0,
                binding: 0,
                size: 4,
                attrib_type: crate::syntax::VertexAttribType::Float32,
                normalized: false,
                stride: size_of::<PrimitiveInstance>() as i32,
                offset_bytes: 0,
                enabled: true,
                divisor: 1,
            },
            VertexAttributeBinding {
                index: 1,
                binding: 0,
                size: 4,
                attrib_type: crate::syntax::VertexAttribType::Float32,
                normalized: false,
                stride: size_of::<PrimitiveInstance>() as i32,
                offset_bytes: 16,
                enabled: true,
                divisor: 1,
            },
            VertexAttributeBinding {
                index: 2,
                binding: 0,
                size: 4,
                attrib_type: crate::syntax::VertexAttribType::Float32,
                normalized: false,
                stride: size_of::<PrimitiveInstance>() as i32,
                offset_bytes: 32,
                enabled: true,
                divisor: 1,
            },
            VertexAttributeBinding {
                index: 3,
                binding: 0,
                size: 4,
                attrib_type: crate::syntax::VertexAttribType::Float32,
                normalized: false,
                stride: size_of::<PrimitiveInstance>() as i32,
                offset_bytes: 48,
                enabled: true,
                divisor: 1,
            },
        ],
    };
    let (_, binding, _) = graphics_pipeline_cache
        .get_or_compile(
            executor_resources,
            pipeline_compiler,
            RUNTIME_SHADER_2D,
            &descriptor,
        )
        .map_err(executor_error_to_api_error)?;
    Ok(binding.pipeline)
}

fn create_graphics_pipeline_text_2d(
    device: &Device,
    render_pass: vk::RenderPass,
    msaa_samples: vk::SampleCountFlags,
    descriptor_set_layout: vk::DescriptorSetLayout,
    pipeline_compiler: &mut VulkanGraphicsPipelineCompiler,
    graphics_pipeline_cache: &mut GraphicsPipelineCache,
    executor_resources: &mut ExecutorResources,
) -> Result<(vk::PipelineLayout, vk::Pipeline), ApiError> {
    let set_layouts = [descriptor_set_layout];
    let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts);
    let pipeline_layout = vk_result(
        unsafe { device.create_pipeline_layout(&pipeline_layout_info, None) },
        "create_pipeline_layout(text_2d)",
    )?;
    let compile_result = compile_runtime_graphics_pipeline_text_2d(
        device,
        render_pass,
        pipeline_layout,
        msaa_samples,
        pipeline_compiler,
        graphics_pipeline_cache,
        executor_resources,
    );
    let graphics_pipeline = match compile_result {
        Ok(pipeline) => pipeline,
        Err(err) => {
            unsafe {
                device.destroy_pipeline_layout(pipeline_layout, None);
            }
            return Err(err);
        }
    };
    Ok((pipeline_layout, graphics_pipeline))
}

fn compile_runtime_graphics_pipeline_text_2d(
    device: &Device,
    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    msaa_samples: vk::SampleCountFlags,
    pipeline_compiler: &mut VulkanGraphicsPipelineCompiler,
    graphics_pipeline_cache: &mut GraphicsPipelineCache,
    executor_resources: &mut ExecutorResources,
) -> Result<vk::Pipeline, ApiError> {
    let _ = device;
    pipeline_compiler.set_sample_count(msaa_samples);
    pipeline_compiler.register_shader_program(
        RUNTIME_SHADER_TEXT_2D,
        ShaderProgramDefinition {
            vertex_spirv: read_spirv_words(TEXT_2D_VERT_SPV)?,
            fragment_spirv: read_spirv_words(TEXT_2D_FRAG_SPV)?,
            layout: pipeline_layout,
            render_pass,
            subpass: 0,
        },
    );

    let descriptor = GraphicsPipelineDescriptor {
        topology: vk::PrimitiveTopology::TRIANGLE_LIST,
        render_state: crate::syntax::BLEND,
        vertex_attributes: vec![
            VertexAttributeBinding {
                index: 0,
                binding: 0,
                size: 4,
                attrib_type: crate::syntax::VertexAttribType::Float32,
                normalized: false,
                stride: size_of::<TextGlyphInstance>() as i32,
                offset_bytes: 0,
                enabled: true,
                divisor: 1,
            },
            VertexAttributeBinding {
                index: 1,
                binding: 0,
                size: 4,
                attrib_type: crate::syntax::VertexAttribType::Float32,
                normalized: false,
                stride: size_of::<TextGlyphInstance>() as i32,
                offset_bytes: 16,
                enabled: true,
                divisor: 1,
            },
            VertexAttributeBinding {
                index: 2,
                binding: 0,
                size: 4,
                attrib_type: crate::syntax::VertexAttribType::Float32,
                normalized: false,
                stride: size_of::<TextGlyphInstance>() as i32,
                offset_bytes: 32,
                enabled: true,
                divisor: 1,
            },
        ],
    };
    let (_, binding, _) = graphics_pipeline_cache
        .get_or_compile(
            executor_resources,
            pipeline_compiler,
            RUNTIME_SHADER_TEXT_2D,
            &descriptor,
        )
        .map_err(executor_error_to_api_error)?;
    Ok(binding.pipeline)
}

fn create_graphics_pipeline_3d(
    device: &Device,
    render_pass: vk::RenderPass,
    msaa_samples: vk::SampleCountFlags,
    descriptor_set_layout: vk::DescriptorSetLayout,
    material_descriptor_set_layout: vk::DescriptorSetLayout,
    shadow_descriptor_set_layout: vk::DescriptorSetLayout,
    pipeline_compiler: &mut VulkanGraphicsPipelineCompiler,
    graphics_pipeline_cache: &mut GraphicsPipelineCache,
    executor_resources: &mut ExecutorResources,
) -> Result<(vk::PipelineLayout, vk::Pipeline), ApiError> {
    let push_constant_ranges = [vk::PushConstantRange::default()
        .stage_flags(vk::ShaderStageFlags::VERTEX)
        .offset(0)
        .size(size_of::<CubeViewProjectionPushConstants>() as u32)];
    let set_layouts = [
        descriptor_set_layout,
        material_descriptor_set_layout,
        shadow_descriptor_set_layout,
    ];
    let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default()
        .set_layouts(&set_layouts)
        .push_constant_ranges(&push_constant_ranges);

    let pipeline_layout = vk_result(
        unsafe { device.create_pipeline_layout(&pipeline_layout_info, None) },
        "create_pipeline_layout(3d)",
    )?;
    let compile_result = compile_runtime_graphics_pipeline_3d(
        device,
        render_pass,
        pipeline_layout,
        msaa_samples,
        pipeline_compiler,
        graphics_pipeline_cache,
        executor_resources,
    );
    let graphics_pipeline = match compile_result {
        Ok(pipeline) => pipeline,
        Err(err) => {
            unsafe {
                device.destroy_pipeline_layout(pipeline_layout, None);
            }
            return Err(err);
        }
    };

    Ok((pipeline_layout, graphics_pipeline))
}

fn compile_runtime_graphics_pipeline_3d(
    device: &Device,
    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    msaa_samples: vk::SampleCountFlags,
    pipeline_compiler: &mut VulkanGraphicsPipelineCompiler,
    graphics_pipeline_cache: &mut GraphicsPipelineCache,
    executor_resources: &mut ExecutorResources,
) -> Result<vk::Pipeline, ApiError> {
    let _ = device;
    pipeline_compiler.set_sample_count(msaa_samples);
    pipeline_compiler.register_shader_program(
        RUNTIME_SHADER_3D,
        ShaderProgramDefinition {
            vertex_spirv: read_spirv_words(CUBE_3D_VERT_SPV)?,
            fragment_spirv: read_spirv_words(CUBE_3D_FRAG_SPV)?,
            layout: pipeline_layout,
            render_pass,
            subpass: 0,
        },
    );

    let descriptor = GraphicsPipelineDescriptor {
        topology: vk::PrimitiveTopology::TRIANGLE_LIST,
        render_state: crate::syntax::DEPTH_TEST,
        vertex_attributes: vec![
            VertexAttributeBinding {
                index: 0,
                binding: 0,
                size: 3,
                attrib_type: crate::syntax::VertexAttribType::Float32,
                normalized: false,
                stride: size_of::<CubeVertex>() as i32,
                offset_bytes: 0,
                enabled: true,
                divisor: 0,
            },
            VertexAttributeBinding {
                index: 1,
                binding: 0,
                size: 3,
                attrib_type: crate::syntax::VertexAttribType::Float32,
                normalized: false,
                stride: size_of::<CubeVertex>() as i32,
                offset_bytes: 12,
                enabled: true,
                divisor: 0,
            },
            VertexAttributeBinding {
                index: 2,
                binding: 0,
                size: 2,
                attrib_type: crate::syntax::VertexAttribType::Float32,
                normalized: false,
                stride: size_of::<CubeVertex>() as i32,
                offset_bytes: 24,
                enabled: true,
                divisor: 0,
            },
            VertexAttributeBinding {
                index: 3,
                binding: 0,
                size: 4,
                attrib_type: crate::syntax::VertexAttribType::Float32,
                normalized: false,
                stride: size_of::<CubeVertex>() as i32,
                offset_bytes: 32,
                enabled: true,
                divisor: 0,
            },
            VertexAttributeBinding {
                index: 4,
                binding: 0,
                size: 1,
                attrib_type: crate::syntax::VertexAttribType::UnsignedInt,
                normalized: false,
                stride: size_of::<CubeVertex>() as i32,
                offset_bytes: 48,
                enabled: true,
                divisor: 0,
            },
            VertexAttributeBinding {
                index: 5,
                binding: 0,
                size: 4,
                attrib_type: crate::syntax::VertexAttribType::Float32,
                normalized: false,
                stride: size_of::<CubeVertex>() as i32,
                offset_bytes: 52,
                enabled: true,
                divisor: 0,
            },
            // location 6: tangent (vec4, offset 68 = 52 + 16)
            VertexAttributeBinding {
                index: 6,
                binding: 0,
                size: 4,
                attrib_type: crate::syntax::VertexAttribType::Float32,
                normalized: false,
                stride: size_of::<CubeVertex>() as i32,
                offset_bytes: 68,
                enabled: true,
                divisor: 0,
            },
        ],
    };
    let (_, binding, _) = graphics_pipeline_cache
        .get_or_compile(
            executor_resources,
            pipeline_compiler,
            RUNTIME_SHADER_3D,
            &descriptor,
        )
        .map_err(executor_error_to_api_error)?;
    Ok(binding.pipeline)
}

fn create_framebuffers(
    device: &Device,
    render_pass: vk::RenderPass,
    image_views: &[vk::ImageView],
    color_image_view: vk::ImageView,
    depth_image_view: vk::ImageView,
    extent: vk::Extent2D,
    msaa_samples: vk::SampleCountFlags,
) -> Result<Vec<vk::Framebuffer>, ApiError> {
    let mut framebuffers = Vec::with_capacity(image_views.len());
    for image_view in image_views {
        let attachments = if msaa_samples == vk::SampleCountFlags::TYPE_1 {
            vec![*image_view, depth_image_view]
        } else {
            vec![color_image_view, *image_view, depth_image_view]
        };
        let framebuffer_info = vk::FramebufferCreateInfo::default()
            .render_pass(render_pass)
            .attachments(&attachments)
            .width(extent.width)
            .height(extent.height)
            .layers(1);
        let framebuffer = vk_result(
            unsafe { device.create_framebuffer(&framebuffer_info, None) },
            "create_framebuffer",
        )?;
        framebuffers.push(framebuffer);
    }
    Ok(framebuffers)
}

fn create_command_resources(
    device: &Device,
    queue_family_index: u32,
) -> Result<(vk::CommandPool, vk::CommandBuffer), ApiError> {
    let command_pool_info = vk::CommandPoolCreateInfo::default()
        .queue_family_index(queue_family_index)
        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
    let command_pool = vk_result(
        unsafe { device.create_command_pool(&command_pool_info, None) },
        "create_command_pool",
    )?;

    let command_buffer_info = vk::CommandBufferAllocateInfo::default()
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(1);
    let command_buffers = vk_result(
        unsafe { device.allocate_command_buffers(&command_buffer_info) },
        "allocate_command_buffers",
    )?;
    Ok((command_pool, command_buffers[0]))
}

fn create_sync_resources(
    device: &Device,
) -> Result<(vk::Semaphore, vk::Semaphore, vk::Fence), ApiError> {
    let semaphore_info = vk::SemaphoreCreateInfo::default();
    let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

    let image_available_semaphore = vk_result(
        unsafe { device.create_semaphore(&semaphore_info, None) },
        "create_semaphore(image_available)",
    )?;
    let render_finished_semaphore = vk_result(
        unsafe { device.create_semaphore(&semaphore_info, None) },
        "create_semaphore(render_finished)",
    )?;
    let in_flight_fence = vk_result(
        unsafe { device.create_fence(&fence_info, None) },
        "create_fence",
    )?;

    Ok((
        image_available_semaphore,
        render_finished_semaphore,
        in_flight_fence,
    ))
}

fn create_depth_resources(
    instance: &Instance,
    device: &Device,
    physical_device: vk::PhysicalDevice,
    extent: vk::Extent2D,
    format: vk::Format,
    samples: vk::SampleCountFlags,
) -> Result<(vk::Image, vk::DeviceMemory, vk::ImageView), ApiError> {
    let image_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(format)
        .extent(vk::Extent3D {
            width: extent.width.max(1),
            height: extent.height.max(1),
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(1)
        .samples(samples)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED);
    let image = vk_result(
        unsafe { device.create_image(&image_info, None) },
        "create_image(depth)",
    )?;

    let memory_requirements = unsafe { device.get_image_memory_requirements(image) };
    let memory_type_index = find_memory_type(
        instance,
        physical_device,
        memory_requirements.memory_type_bits,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )?;
    let allocation_info = vk::MemoryAllocateInfo::default()
        .allocation_size(memory_requirements.size)
        .memory_type_index(memory_type_index);
    let memory = match vk_result(
        unsafe { device.allocate_memory(&allocation_info, None) },
        "allocate_memory(depth)",
    ) {
        Ok(memory) => memory,
        Err(err) => {
            unsafe { device.destroy_image(image, None) };
            return Err(err);
        }
    };
    if let Err(err) = vk_result(
        unsafe { device.bind_image_memory(image, memory, 0) },
        "bind_image_memory(depth)",
    ) {
        unsafe {
            device.free_memory(memory, None);
            device.destroy_image(image, None);
        }
        return Err(err);
    }

    let subresource_range = vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::DEPTH)
        .base_mip_level(0)
        .level_count(1)
        .base_array_layer(0)
        .layer_count(1);
    let image_view_info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(format)
        .subresource_range(subresource_range);
    let image_view = match vk_result(
        unsafe { device.create_image_view(&image_view_info, None) },
        "create_image_view(depth)",
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

    Ok((image, memory, image_view))
}

fn create_color_resources(
    instance: &Instance,
    device: &Device,
    physical_device: vk::PhysicalDevice,
    extent: vk::Extent2D,
    format: vk::Format,
    samples: vk::SampleCountFlags,
) -> Result<(vk::Image, vk::DeviceMemory, vk::ImageView), ApiError> {
    let image_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(format)
        .extent(vk::Extent3D {
            width: extent.width.max(1),
            height: extent.height.max(1),
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(1)
        .samples(samples)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED);
    let image = vk_result(
        unsafe { device.create_image(&image_info, None) },
        "create_image(color_msaa)",
    )?;

    let memory_requirements = unsafe { device.get_image_memory_requirements(image) };
    let memory_type_index = find_memory_type(
        instance,
        physical_device,
        memory_requirements.memory_type_bits,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )?;
    let allocation_info = vk::MemoryAllocateInfo::default()
        .allocation_size(memory_requirements.size)
        .memory_type_index(memory_type_index);
    let memory = match vk_result(
        unsafe { device.allocate_memory(&allocation_info, None) },
        "allocate_memory(color_msaa)",
    ) {
        Ok(memory) => memory,
        Err(err) => {
            unsafe { device.destroy_image(image, None) };
            return Err(err);
        }
    };
    if let Err(err) = vk_result(
        unsafe { device.bind_image_memory(image, memory, 0) },
        "bind_image_memory(color_msaa)",
    ) {
        unsafe {
            device.free_memory(memory, None);
            device.destroy_image(image, None);
        }
        return Err(err);
    }

    let subresource_range = vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .base_mip_level(0)
        .level_count(1)
        .base_array_layer(0)
        .layer_count(1);
    let image_view_info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(format)
        .subresource_range(subresource_range);
    let image_view = match vk_result(
        unsafe { device.create_image_view(&image_view_info, None) },
        "create_image_view(color_msaa)",
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

    Ok((image, memory, image_view))
}

#[allow(dead_code)]
fn create_screenshot_color_target(
    instance: &Instance,
    device: &Device,
    physical_device: vk::PhysicalDevice,
    extent: vk::Extent2D,
    format: vk::Format,
) -> Result<(vk::Image, vk::DeviceMemory, vk::ImageView), ApiError> {
    let image_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(format)
        .extent(vk::Extent3D {
            width: extent.width.max(1),
            height: extent.height.max(1),
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED);
    let image = vk_result(
        unsafe { device.create_image(&image_info, None) },
        "create_image(screenshot_color)",
    )?;
    let memory_requirements = unsafe { device.get_image_memory_requirements(image) };
    let memory_type_index = find_memory_type(
        instance,
        physical_device,
        memory_requirements.memory_type_bits,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )?;
    let allocation_info = vk::MemoryAllocateInfo::default()
        .allocation_size(memory_requirements.size)
        .memory_type_index(memory_type_index);
    let memory = match vk_result(
        unsafe { device.allocate_memory(&allocation_info, None) },
        "allocate_memory(screenshot_color)",
    ) {
        Ok(memory) => memory,
        Err(err) => {
            unsafe { device.destroy_image(image, None) };
            return Err(err);
        }
    };
    if let Err(err) = vk_result(
        unsafe { device.bind_image_memory(image, memory, 0) },
        "bind_image_memory(screenshot_color)",
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
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1),
        );
    let view = match vk_result(
        unsafe { device.create_image_view(&view_info, None) },
        "create_image_view(screenshot_color)",
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

#[allow(dead_code)]
fn create_screenshot_render_target(
    instance: &Instance,
    device: &Device,
    physical_device: vk::PhysicalDevice,
    render_pass: vk::RenderPass,
    color_format: vk::Format,
    depth_format: vk::Format,
    extent: vk::Extent2D,
    msaa_samples: vk::SampleCountFlags,
) -> Result<ScreenshotRenderTarget, ApiError> {
    let (color_image, color_image_memory, color_image_view) =
        if msaa_samples == vk::SampleCountFlags::TYPE_1 {
            (
                vk::Image::null(),
                vk::DeviceMemory::null(),
                vk::ImageView::null(),
            )
        } else {
            create_color_resources(
                instance,
                device,
                physical_device,
                extent,
                color_format,
                msaa_samples,
            )?
        };
    let (resolve_image, resolve_image_memory, resolve_image_view) =
        create_screenshot_color_target(instance, device, physical_device, extent, color_format)?;
    let (depth_image, depth_image_memory, depth_image_view) = create_depth_resources(
        instance,
        device,
        physical_device,
        extent,
        depth_format,
        msaa_samples,
    )?;
    let attachments = if msaa_samples == vk::SampleCountFlags::TYPE_1 {
        vec![resolve_image_view, depth_image_view]
    } else {
        vec![color_image_view, resolve_image_view, depth_image_view]
    };
    let framebuffer_info = vk::FramebufferCreateInfo::default()
        .render_pass(render_pass)
        .attachments(&attachments)
        .width(extent.width)
        .height(extent.height)
        .layers(1);
    let framebuffer = vk_result(
        unsafe { device.create_framebuffer(&framebuffer_info, None) },
        "create_framebuffer(screenshot)",
    )?;
    let (readback_buffer, readback_memory) = create_buffer(
        instance,
        device,
        physical_device,
        (extent.width as vk::DeviceSize) * (extent.height as vk::DeviceSize) * 4,
        vk::BufferUsageFlags::TRANSFER_DST,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )?;
    Ok(ScreenshotRenderTarget {
        extent,
        framebuffer,
        color_image,
        color_image_memory,
        color_image_view,
        resolve_image,
        resolve_image_memory,
        resolve_image_view,
        depth_image,
        depth_image_memory,
        depth_image_view,
        readback_buffer,
        readback_memory,
    })
}

#[allow(dead_code)]
fn destroy_screenshot_render_target(device: &Device, target: ScreenshotRenderTarget) {
    unsafe {
        device.destroy_buffer(target.readback_buffer, None);
        device.free_memory(target.readback_memory, None);
        device.destroy_framebuffer(target.framebuffer, None);
        device.destroy_image_view(target.resolve_image_view, None);
        device.destroy_image(target.resolve_image, None);
        device.free_memory(target.resolve_image_memory, None);
        device.destroy_image_view(target.depth_image_view, None);
        device.destroy_image(target.depth_image, None);
        device.free_memory(target.depth_image_memory, None);
        if target.color_image_view != vk::ImageView::null() {
            device.destroy_image_view(target.color_image_view, None);
        }
        if target.color_image != vk::Image::null() {
            device.destroy_image(target.color_image, None);
        }
        if target.color_image_memory != vk::DeviceMemory::null() {
            device.free_memory(target.color_image_memory, None);
        }
    }
}

fn create_shadow_render_pass(
    device: &Device,
    depth_format: vk::Format,
) -> Result<vk::RenderPass, ApiError> {
    let depth_attachment = vk::AttachmentDescription::default()
        .format(depth_format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL);
    let depth_attachment_ref = vk::AttachmentReference::default()
        .attachment(0)
        .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
    let subpasses = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .depth_stencil_attachment(&depth_attachment_ref)];
    let dependencies = [
        vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .dst_stage_mask(vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS)
            .src_access_mask(vk::AccessFlags::SHADER_READ)
            .dst_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE),
        vk::SubpassDependency::default()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::LATE_FRAGMENT_TESTS)
            .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .src_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
            .dst_access_mask(vk::AccessFlags::SHADER_READ),
    ];
    let attachments = [depth_attachment];
    let render_pass_info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments)
        .subpasses(&subpasses)
        .dependencies(&dependencies);
    vk_result(
        unsafe { device.create_render_pass(&render_pass_info, None) },
        "create_shadow_render_pass",
    )
}

fn create_shadow_map_resources(
    instance: &Instance,
    device: &Device,
    physical_device: vk::PhysicalDevice,
    format: vk::Format,
    size: u32,
) -> Result<(vk::Image, vk::DeviceMemory, vk::ImageView, vk::Sampler), ApiError> {
    let image_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(format)
        .extent(vk::Extent3D {
            width: size.max(1),
            height: size.max(1),
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED);
    let image = vk_result(
        unsafe { device.create_image(&image_info, None) },
        "create_image(shadow_map)",
    )?;
    let memory_requirements = unsafe { device.get_image_memory_requirements(image) };
    let memory_type_index = find_memory_type(
        instance,
        physical_device,
        memory_requirements.memory_type_bits,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )?;
    let allocation_info = vk::MemoryAllocateInfo::default()
        .allocation_size(memory_requirements.size)
        .memory_type_index(memory_type_index);
    let memory = match vk_result(
        unsafe { device.allocate_memory(&allocation_info, None) },
        "allocate_memory(shadow_map)",
    ) {
        Ok(memory) => memory,
        Err(err) => {
            unsafe { device.destroy_image(image, None) };
            return Err(err);
        }
    };
    if let Err(err) = vk_result(
        unsafe { device.bind_image_memory(image, memory, 0) },
        "bind_image_memory(shadow_map)",
    ) {
        unsafe {
            device.free_memory(memory, None);
            device.destroy_image(image, None);
        }
        return Err(err);
    }

    let subresource_range = vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::DEPTH)
        .base_mip_level(0)
        .level_count(1)
        .base_array_layer(0)
        .layer_count(1);
    let image_view_info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(format)
        .subresource_range(subresource_range);
    let image_view = match vk_result(
        unsafe { device.create_image_view(&image_view_info, None) },
        "create_image_view(shadow_map)",
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

    let sampler_info = vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::LINEAR)
        .min_filter(vk::Filter::LINEAR)
        .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_BORDER)
        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_BORDER)
        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_BORDER)
        .border_color(vk::BorderColor::FLOAT_OPAQUE_WHITE)
        .max_lod(1.0)
        .compare_enable(false);
    let sampler = match vk_result(
        unsafe { device.create_sampler(&sampler_info, None) },
        "create_sampler(shadow_map)",
    ) {
        Ok(sampler) => sampler,
        Err(err) => {
            unsafe {
                device.destroy_image_view(image_view, None);
                device.free_memory(memory, None);
                device.destroy_image(image, None);
            }
            return Err(err);
        }
    };

    Ok((image, memory, image_view, sampler))
}

fn create_shadow_framebuffer(
    device: &Device,
    render_pass: vk::RenderPass,
    depth_image_view: vk::ImageView,
    size: u32,
) -> Result<vk::Framebuffer, ApiError> {
    let attachments = [depth_image_view];
    let framebuffer_info = vk::FramebufferCreateInfo::default()
        .render_pass(render_pass)
        .attachments(&attachments)
        .width(size.max(1))
        .height(size.max(1))
        .layers(1);
    vk_result(
        unsafe { device.create_framebuffer(&framebuffer_info, None) },
        "create_framebuffer(shadow_map)",
    )
}

fn create_shadow_pipeline_3d(
    device: &Device,
    render_pass: vk::RenderPass,
    descriptor_set_layout: vk::DescriptorSetLayout,
) -> Result<(vk::PipelineLayout, vk::Pipeline), ApiError> {
    let push_constant_ranges = [vk::PushConstantRange::default()
        .stage_flags(vk::ShaderStageFlags::VERTEX)
        .offset(0)
        .size(size_of::<CubeViewProjectionPushConstants>() as u32)];
    let set_layouts = [descriptor_set_layout];
    let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default()
        .set_layouts(&set_layouts)
        .push_constant_ranges(&push_constant_ranges);
    let pipeline_layout = vk_result(
        unsafe { device.create_pipeline_layout(&pipeline_layout_info, None) },
        "create_pipeline_layout(shadow_3d)",
    )?;

    let vertex_words = read_spirv_words(SHADOW_3D_VERT_SPV)?;
    let vertex_module_info = vk::ShaderModuleCreateInfo::default().code(&vertex_words);
    let vertex_module = match vk_result(
        unsafe { device.create_shader_module(&vertex_module_info, None) },
        "create_shader_module(shadow_3d.vert)",
    ) {
        Ok(module) => module,
        Err(err) => {
            unsafe { device.destroy_pipeline_layout(pipeline_layout, None) };
            return Err(err);
        }
    };
    let entry_name = CString::new("main").expect("shader entry point");
    let shader_stages = [vk::PipelineShaderStageCreateInfo::default()
        .stage(vk::ShaderStageFlags::VERTEX)
        .module(vertex_module)
        .name(&entry_name)];
    let vertex_binding_descriptions = [vk::VertexInputBindingDescription::default()
        .binding(0)
        .stride(size_of::<CubeVertex>() as u32)
        .input_rate(vk::VertexInputRate::VERTEX)];
    let vertex_attribute_descriptions = [
        vk::VertexInputAttributeDescription::default()
            .location(0)
            .binding(0)
            .format(vk::Format::R32G32B32_SFLOAT)
            .offset(0),
        vk::VertexInputAttributeDescription::default()
            .location(4)
            .binding(0)
            .format(vk::Format::R32_UINT)
            .offset(48),
    ];
    let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&vertex_binding_descriptions)
        .vertex_attribute_descriptions(&vertex_attribute_descriptions);
    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
    let viewport_state = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);
    let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
        .depth_clamp_enable(false)
        .rasterizer_discard_enable(false)
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.0)
        .cull_mode(vk::CullModeFlags::NONE)
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .depth_bias_enable(true)
        .depth_bias_constant_factor(0.6)
        .depth_bias_slope_factor(1.2);
    let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1);
    let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(true)
        .depth_write_enable(true)
        .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL)
        .stencil_test_enable(false);
    let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic_state =
        vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
    let color_blend = vk::PipelineColorBlendStateCreateInfo::default();
    let pipeline_info = [vk::GraphicsPipelineCreateInfo::default()
        .stages(&shader_stages)
        .vertex_input_state(&vertex_input)
        .input_assembly_state(&input_assembly)
        .viewport_state(&viewport_state)
        .rasterization_state(&rasterizer)
        .multisample_state(&multisampling)
        .depth_stencil_state(&depth_stencil)
        .color_blend_state(&color_blend)
        .dynamic_state(&dynamic_state)
        .layout(pipeline_layout)
        .render_pass(render_pass)
        .subpass(0)];
    let pipeline = match unsafe {
        device.create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_info, None)
    } {
        Ok(mut pipelines) => pipelines.remove(0),
        Err((_, result)) => {
            unsafe {
                device.destroy_shader_module(vertex_module, None);
                device.destroy_pipeline_layout(pipeline_layout, None);
            }
            return Err(ApiError::Vulkan {
                context: "create_graphics_pipelines(shadow_3d)",
                result,
            });
        }
    };
    unsafe {
        device.destroy_shader_module(vertex_module, None);
    }
    Ok((pipeline_layout, pipeline))
}

fn create_solid_color_texture(
    instance: &Instance,
    device: &Device,
    physical_device: vk::PhysicalDevice,
    command_pool: vk::CommandPool,
    graphics_queue: vk::Queue,
    rgba: [u8; 4],
) -> Result<(vk::Image, vk::DeviceMemory, vk::ImageView, vk::Sampler), ApiError> {
    create_rgba_texture_from_bytes(
        instance,
        device,
        physical_device,
        command_pool,
        graphics_queue,
        &rgba,
        1,
        1,
        "white_texture",
    )
}

fn create_rgba_texture_from_bytes(
    instance: &Instance,
    device: &Device,
    physical_device: vk::PhysicalDevice,
    command_pool: vk::CommandPool,
    graphics_queue: vk::Queue,
    rgba: &[u8],
    width: u32,
    height: u32,
    _label: &str,
) -> Result<(vk::Image, vk::DeviceMemory, vk::ImageView, vk::Sampler), ApiError> {
    let (staging_buffer, staging_memory) = create_buffer(
        instance,
        device,
        physical_device,
        rgba.len() as vk::DeviceSize,
        vk::BufferUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )?;

    let image_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(vk::Format::R8G8B8A8_UNORM)
        .extent(vk::Extent3D {
            width,
            height,
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED);
    let image = match vk_result(
        unsafe { device.create_image(&image_info, None) },
        "create_image(texture)",
    ) {
        Ok(image) => image,
        Err(err) => {
            unsafe {
                device.destroy_buffer(staging_buffer, None);
                device.free_memory(staging_memory, None);
            }
            return Err(err);
        }
    };

    let memory_requirements = unsafe { device.get_image_memory_requirements(image) };
    let memory_type_index = match find_memory_type(
        instance,
        physical_device,
        memory_requirements.memory_type_bits,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    ) {
        Ok(index) => index,
        Err(err) => {
            unsafe {
                device.destroy_image(image, None);
                device.destroy_buffer(staging_buffer, None);
                device.free_memory(staging_memory, None);
            }
            return Err(err);
        }
    };
    let allocation_info = vk::MemoryAllocateInfo::default()
        .allocation_size(memory_requirements.size)
        .memory_type_index(memory_type_index);
    let image_memory = match vk_result(
        unsafe { device.allocate_memory(&allocation_info, None) },
        "allocate_memory(texture)",
    ) {
        Ok(memory) => memory,
        Err(err) => {
            unsafe {
                device.destroy_image(image, None);
                device.destroy_buffer(staging_buffer, None);
                device.free_memory(staging_memory, None);
            }
            return Err(err);
        }
    };
    if let Err(err) = vk_result(
        unsafe { device.bind_image_memory(image, image_memory, 0) },
        "bind_image_memory(texture)",
    ) {
        unsafe {
            device.free_memory(image_memory, None);
            device.destroy_image(image, None);
            device.destroy_buffer(staging_buffer, None);
            device.free_memory(staging_memory, None);
        }
        return Err(err);
    }

    let mapped = vk_result(
        unsafe {
            device.map_memory(
                staging_memory,
                0,
                rgba.len() as vk::DeviceSize,
                vk::MemoryMapFlags::empty(),
            )
        },
        "map_memory(texture_staging)",
    )?;
    unsafe {
        std::ptr::copy_nonoverlapping(rgba.as_ptr(), mapped.cast::<u8>(), rgba.len());
        device.unmap_memory(staging_memory);
    }

    if let Err(err) = upload_buffer_to_texture(
        device,
        command_pool,
        graphics_queue,
        staging_buffer,
        image,
        width,
        height,
    ) {
        unsafe {
            device.free_memory(image_memory, None);
            device.destroy_image(image, None);
            device.destroy_buffer(staging_buffer, None);
            device.free_memory(staging_memory, None);
        }
        return Err(err);
    }

    unsafe {
        device.destroy_buffer(staging_buffer, None);
        device.free_memory(staging_memory, None);
    }

    let subresource_range = vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .base_mip_level(0)
        .level_count(1)
        .base_array_layer(0)
        .layer_count(1);
    let image_view_info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(vk::Format::R8G8B8A8_UNORM)
        .subresource_range(subresource_range);
    let image_view = match vk_result(
        unsafe { device.create_image_view(&image_view_info, None) },
        "create_image_view(texture)",
    ) {
        Ok(view) => view,
        Err(err) => {
            unsafe {
                device.free_memory(image_memory, None);
                device.destroy_image(image, None);
            }
            return Err(err);
        }
    };

    let sampler_info = vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::LINEAR)
        .min_filter(vk::Filter::LINEAR)
        .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .max_lod(1.0);
    let sampler = match vk_result(
        unsafe { device.create_sampler(&sampler_info, None) },
        "create_sampler(texture)",
    ) {
        Ok(sampler) => sampler,
        Err(err) => {
            unsafe {
                device.destroy_image_view(image_view, None);
                device.free_memory(image_memory, None);
                device.destroy_image(image, None);
            }
            return Err(err);
        }
    };

    Ok((image, image_memory, image_view, sampler))
}

fn build_font_atlas() -> Result<(FontAtlasLayout, Vec<u8>, u32, u32), ApiError> {
    let font = Font::from_bytes(INTER_FONT_BYTES, FontSettings::default()).map_err(|err| {
        ApiError::InvalidConfig {
            reason: format!("failed to load bundled Inter font: {err}"),
        }
    })?;
    let px = 32.0_f32;
    let chars: Vec<char> = (32_u8..=126_u8)
        .map(char::from)
        .chain(std::iter::once('?'))
        .collect();
    let line_metrics = font
        .horizontal_line_metrics(px)
        .ok_or(ApiError::InvalidConfig {
            reason: "font has no horizontal line metrics".into(),
        })?;

    let padding = 4_usize;
    let mut atlas_width = 0_usize;
    let mut atlas_height = 0_usize;
    let mut glyph_bitmaps = Vec::new();

    for ch in chars {
        let (metrics, bitmap) = font.rasterize(ch, px);
        atlas_width += metrics.width + padding;
        atlas_height = atlas_height.max(metrics.height + padding * 2);
        glyph_bitmaps.push((ch, metrics, bitmap));
    }

    atlas_width = atlas_width.max(1);
    atlas_height = atlas_height.max((line_metrics.new_line_size.ceil() as usize) + padding * 2);

    let mut rgba = vec![0_u8; atlas_width * atlas_height * 4];
    let mut glyphs = HashMap::new();
    let mut kerning = HashMap::new();
    let mut pen_x = padding;
    for (ch, metrics, bitmap) in glyph_bitmaps {
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let alpha = bitmap[row * metrics.width + col];
                let atlas_x = pen_x + col;
                let atlas_y = padding + row;
                let dst = (atlas_y * atlas_width + atlas_x) * 4;
                rgba[dst] = 255;
                rgba[dst + 1] = 255;
                rgba[dst + 2] = 255;
                rgba[dst + 3] = alpha;
            }
        }
        let uv_min = [
            pen_x as f32 / atlas_width as f32,
            padding as f32 / atlas_height as f32,
        ];
        let uv_max = [
            (pen_x + metrics.width) as f32 / atlas_width as f32,
            (padding + metrics.height) as f32 / atlas_height as f32,
        ];
        glyphs.insert(
            ch,
            FontAtlasGlyph {
                uv_min,
                uv_max,
                metrics,
            },
        );
        pen_x += metrics.width + padding;
    }
    for &left in glyphs.keys() {
        for &right in glyphs.keys() {
            if let Some(value) = font.horizontal_kern(left, right, px) {
                if value != 0.0 {
                    kerning.insert((left, right), value);
                }
            }
        }
    }

    Ok((
        FontAtlasLayout {
            glyphs,
            kerning,
            line_height: line_metrics.new_line_size,
            base_pixel_size: px,
        },
        rgba,
        atlas_width as u32,
        atlas_height as u32,
    ))
}

fn upload_buffer_to_texture(
    device: &Device,
    command_pool: vk::CommandPool,
    graphics_queue: vk::Queue,
    staging_buffer: vk::Buffer,
    image: vk::Image,
    width: u32,
    height: u32,
) -> Result<(), ApiError> {
    let command_buffer = begin_one_time_commands(device, command_pool)?;
    transition_image_layout(
        device,
        command_buffer,
        image,
        vk::ImageLayout::UNDEFINED,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
    );
    let region = [vk::BufferImageCopy::default()
        .buffer_offset(0)
        .image_subresource(
            vk::ImageSubresourceLayers::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .mip_level(0)
                .base_array_layer(0)
                .layer_count(1),
        )
        .image_extent(vk::Extent3D {
            width,
            height,
            depth: 1,
        })];
    unsafe {
        device.cmd_copy_buffer_to_image(
            command_buffer,
            staging_buffer,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &region,
        );
    }
    transition_image_layout(
        device,
        command_buffer,
        image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    );
    end_one_time_commands(device, command_pool, graphics_queue, command_buffer)
}

fn begin_one_time_commands(
    device: &Device,
    command_pool: vk::CommandPool,
) -> Result<vk::CommandBuffer, ApiError> {
    let alloc_info = vk::CommandBufferAllocateInfo::default()
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(1);
    let mut command_buffers = vk_result(
        unsafe { device.allocate_command_buffers(&alloc_info) },
        "allocate_command_buffers(one_time)",
    )?;
    let command_buffer = command_buffers.remove(0);
    let begin_info =
        vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    vk_result(
        unsafe { device.begin_command_buffer(command_buffer, &begin_info) },
        "begin_command_buffer(one_time)",
    )?;
    Ok(command_buffer)
}

fn end_one_time_commands(
    device: &Device,
    command_pool: vk::CommandPool,
    graphics_queue: vk::Queue,
    command_buffer: vk::CommandBuffer,
) -> Result<(), ApiError> {
    vk_result(
        unsafe { device.end_command_buffer(command_buffer) },
        "end_command_buffer(one_time)",
    )?;
    let submit_info =
        [vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&command_buffer))];
    vk_result(
        unsafe { device.queue_submit(graphics_queue, &submit_info, vk::Fence::null()) },
        "queue_submit(one_time)",
    )?;
    vk_result(
        unsafe { device.queue_wait_idle(graphics_queue) },
        "queue_wait_idle(one_time)",
    )?;
    unsafe {
        device.free_command_buffers(command_pool, &[command_buffer]);
    }
    Ok(())
}

fn transition_image_layout(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) {
    let (src_access_mask, dst_access_mask, src_stage, dst_stage) = match (old_layout, new_layout) {
        (vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL) => (
            vk::AccessFlags::empty(),
            vk::AccessFlags::TRANSFER_WRITE,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
        ),
        (vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL) => (
            vk::AccessFlags::TRANSFER_WRITE,
            vk::AccessFlags::SHADER_READ,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
        ),
        _ => (
            vk::AccessFlags::empty(),
            vk::AccessFlags::empty(),
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
        ),
    };
    let barrier = [vk::ImageMemoryBarrier::default()
        .old_layout(old_layout)
        .new_layout(new_layout)
        .src_access_mask(src_access_mask)
        .dst_access_mask(dst_access_mask)
        .image(image)
        .subresource_range(
            vk::ImageSubresourceRange::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1),
        )];
    unsafe {
        device.cmd_pipeline_barrier(
            command_buffer,
            src_stage,
            dst_stage,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &barrier,
        );
    }
}

fn record_screenshot_copy_commands(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    image: vk::Image,
    buffer: vk::Buffer,
    extent: vk::Extent2D,
) {
    let to_transfer = [vk::ImageMemoryBarrier::default()
        .old_layout(vk::ImageLayout::PRESENT_SRC_KHR)
        .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
        .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
        .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
        .image(image)
        .subresource_range(
            vk::ImageSubresourceRange::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1),
        )];
    unsafe {
        device.cmd_pipeline_barrier(
            command_buffer,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &to_transfer,
        );
    }

    let copy_region = [vk::BufferImageCopy::default()
        .buffer_offset(0)
        .buffer_row_length(0)
        .buffer_image_height(0)
        .image_subresource(
            vk::ImageSubresourceLayers::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .mip_level(0)
                .base_array_layer(0)
                .layer_count(1),
        )
        .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
        .image_extent(vk::Extent3D {
            width: extent.width,
            height: extent.height,
            depth: 1,
        })];
    unsafe {
        device.cmd_copy_image_to_buffer(
            command_buffer,
            image,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            buffer,
            &copy_region,
        );
    }

    let to_present = [vk::ImageMemoryBarrier::default()
        .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
        .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
        .src_access_mask(vk::AccessFlags::TRANSFER_READ)
        .dst_access_mask(vk::AccessFlags::MEMORY_READ)
        .image(image)
        .subresource_range(
            vk::ImageSubresourceRange::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1),
        )];
    unsafe {
        device.cmd_pipeline_barrier(
            command_buffer,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &to_present,
        );
    }
}

fn save_screenshot_from_buffer(
    device: &Device,
    memory: vk::DeviceMemory,
    source_extent: vk::Extent2D,
    format: vk::Format,
    output_extent: vk::Extent2D,
    path: &Path,
) -> Result<(), ApiError> {
    let rgba = read_screenshot_rgba_from_memory(device, memory, source_extent, format)?;
    save_screenshot_rgba(&rgba, source_extent, output_extent, path)
}

fn read_screenshot_rgba_from_memory(
    device: &Device,
    memory: vk::DeviceMemory,
    extent: vk::Extent2D,
    format: vk::Format,
) -> Result<Vec<u8>, ApiError> {
    let byte_len = (extent.width as usize) * (extent.height as usize) * 4;
    let mapped = vk_result(
        unsafe { device.map_memory(memory, 0, byte_len as u64, vk::MemoryMapFlags::empty()) },
        "map_memory(screenshot_readback)",
    )?;
    let raw = unsafe { std::slice::from_raw_parts(mapped.cast::<u8>(), byte_len) };
    let mut rgba = vec![0_u8; byte_len];
    match format {
        vk::Format::B8G8R8A8_UNORM | vk::Format::B8G8R8A8_SRGB => {
            for (src, dst) in raw.chunks_exact(4).zip(rgba.chunks_exact_mut(4)) {
                dst[0] = src[2];
                dst[1] = src[1];
                dst[2] = src[0];
                dst[3] = src[3];
            }
        }
        vk::Format::R8G8B8A8_UNORM | vk::Format::R8G8B8A8_SRGB => {
            rgba.copy_from_slice(raw);
        }
        unsupported => {
            unsafe { device.unmap_memory(memory) };
            return Err(ApiError::Window {
                reason: format!("unsupported screenshot swapchain format: {unsupported:?}"),
            });
        }
    }
    unsafe { device.unmap_memory(memory) };

    Ok(rgba)
}

fn save_screenshot_rgba(
    rgba: &[u8],
    source_extent: vk::Extent2D,
    output_extent: vk::Extent2D,
    path: &Path,
) -> Result<(), ApiError> {
    let output_rgba = if output_extent == source_extent {
        rgba.to_vec()
    } else {
        let image =
            image::RgbaImage::from_raw(source_extent.width, source_extent.height, rgba.to_vec())
                .ok_or(ApiError::Window {
                    reason: format!(
                        "failed to build screenshot image buffer {}x{}",
                        source_extent.width, source_extent.height
                    ),
                })?;
        image::imageops::resize(
            &image,
            output_extent.width,
            output_extent.height,
            image::imageops::FilterType::CatmullRom,
        )
        .into_raw()
    };
    spawn_screenshot_write(output_rgba, output_extent, path.to_path_buf());
    Ok(())
}

fn spawn_screenshot_write(rgba: Vec<u8>, extent: vk::Extent2D, path: PathBuf) {
    thread::spawn(move || {
        if let Some(parent) = path.parent() {
            if let Err(err) = std::fs::create_dir_all(parent) {
                eprintln!(
                    "failed to create screenshot directory {}: {err}",
                    parent.display()
                );
                return;
            }
        }

        if let Err(err) = image::save_buffer_with_format(
            &path,
            &rgba,
            extent.width,
            extent.height,
            ColorType::Rgba8,
            ImageFormat::Png,
        ) {
            eprintln!("failed to write screenshot {}: {err}", path.display());
        }
    });
}

fn accumulate_screenshot_rgba(accumulator: &mut [f32], rgba: &[u8]) {
    for (dst, src) in accumulator.iter_mut().zip(rgba.iter()) {
        *dst += *src as f32;
    }
}

fn resolve_accumulated_screenshot_rgba(accumulator: &[f32], sample_count: u32) -> Vec<u8> {
    let scale = 1.0 / sample_count.max(1) as f32;
    accumulator
        .iter()
        .map(|value| (value * scale).clamp(0.0, 255.0).round() as u8)
        .collect()
}

fn screenshot_output_extent(resolution: ScreenshotResolution) -> vk::Extent2D {
    let [width, height] = resolution.extent();
    vk::Extent2D { width, height }
}

fn create_buffer(
    instance: &Instance,
    device: &Device,
    physical_device: vk::PhysicalDevice,
    size: vk::DeviceSize,
    usage: vk::BufferUsageFlags,
    memory_properties: vk::MemoryPropertyFlags,
) -> Result<(vk::Buffer, vk::DeviceMemory), ApiError> {
    let buffer_info = vk::BufferCreateInfo::default()
        .size(size.max(1))
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    let buffer = vk_result(
        unsafe { device.create_buffer(&buffer_info, None) },
        "create_buffer",
    )?;

    let memory_requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
    let memory_type_index = find_memory_type(
        instance,
        physical_device,
        memory_requirements.memory_type_bits,
        memory_properties,
    )?;
    let allocation_info = vk::MemoryAllocateInfo::default()
        .allocation_size(memory_requirements.size)
        .memory_type_index(memory_type_index);
    let memory = match vk_result(
        unsafe { device.allocate_memory(&allocation_info, None) },
        "allocate_memory(buffer)",
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
        "bind_buffer_memory",
    ) {
        unsafe {
            device.free_memory(memory, None);
            device.destroy_buffer(buffer, None);
        }
        return Err(err);
    }

    Ok((buffer, memory))
}

fn create_cube_descriptor_set_layout(device: &Device) -> Result<vk::DescriptorSetLayout, ApiError> {
    let bindings = [
        vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        vk::DescriptorSetLayoutBinding::default()
            .binding(1)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
    ];
    let layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);

    vk_result(
        unsafe { device.create_descriptor_set_layout(&layout_info, None) },
        "create_descriptor_set_layout(3d)",
    )
}

pub fn create_sampled_texture_descriptor_set_layout(
    device: &Device,
    bindings: &[(u32, vk::ShaderStageFlags)],
) -> Result<vk::DescriptorSetLayout, ApiError> {
    let bindings_vk: Vec<_> = bindings
        .iter()
        .map(|(binding, stage_flags)| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(*binding)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(*stage_flags)
        })
        .collect();
    let layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings_vk);
    vk_result(
        unsafe { device.create_descriptor_set_layout(&layout_info, None) },
        "create_descriptor_set_layout(sampled_texture)",
    )
}

pub fn create_sampled_texture_descriptor_resources(
    device: &Device,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_count: u32,
) -> Result<SampledTextureDescriptorResources, ApiError> {
    let pool_sizes = [vk::DescriptorPoolSize::default()
        .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .descriptor_count(descriptor_count.max(1))];
    let pool_info = vk::DescriptorPoolCreateInfo::default()
        .max_sets(1)
        .pool_sizes(&pool_sizes);
    let descriptor_pool = vk_result(
        unsafe { device.create_descriptor_pool(&pool_info, None) },
        "create_descriptor_pool(sampled_texture)",
    )?;

    let set_layouts = [descriptor_set_layout];
    let alloc_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(&set_layouts);
    let descriptor_set = match vk_result(
        unsafe { device.allocate_descriptor_sets(&alloc_info) },
        "allocate_descriptor_sets(sampled_texture)",
    ) {
        Ok(mut sets) => sets.remove(0),
        Err(err) => {
            unsafe {
                device.destroy_descriptor_pool(descriptor_pool, None);
            }
            return Err(err);
        }
    };

    Ok(SampledTextureDescriptorResources {
        layout: descriptor_set_layout,
        pool: descriptor_pool,
        set: descriptor_set,
    })
}

fn create_material_descriptor_pool_3d(device: &Device) -> Result<vk::DescriptorPool, ApiError> {
    let pool_sizes = [vk::DescriptorPoolSize::default()
        .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .descriptor_count(MAX_MATERIAL_DESCRIPTOR_SETS)];
    let pool_info = vk::DescriptorPoolCreateInfo::default()
        .max_sets(MAX_MATERIAL_DESCRIPTOR_SETS)
        .pool_sizes(&pool_sizes);
    vk_result(
        unsafe { device.create_descriptor_pool(&pool_info, None) },
        "create_descriptor_pool(material_3d)",
    )
}

fn allocate_descriptor_set(
    device: &Device,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
    context: &'static str,
) -> Result<vk::DescriptorSet, ApiError> {
    let set_layouts = [descriptor_set_layout];
    let alloc_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(&set_layouts);
    let mut sets = vk_result(
        unsafe { device.allocate_descriptor_sets(&alloc_info) },
        context,
    )?;
    Ok(sets.remove(0))
}

fn descriptor_handle_for_texture(texture_handle: Option<TextureHandle>) -> DescriptorSetHandle {
    match texture_handle {
        None => DescriptorSetHandle(RUNTIME_DESCRIPTOR_SET_MATERIAL_BASE),
        Some(handle) => DescriptorSetHandle(RUNTIME_DESCRIPTOR_SET_MATERIAL_BASE + handle.0 + 1),
    }
}

fn create_cube_descriptor_resources(
    instance: &Instance,
    device: &Device,
    physical_device: vk::PhysicalDevice,
    descriptor_set_layout: vk::DescriptorSetLayout,
) -> Result<
    (
        vk::Buffer,
        vk::DeviceMemory,
        vk::Buffer,
        vk::DeviceMemory,
        vk::DescriptorPool,
        vk::DescriptorSet,
    ),
    ApiError,
> {
    let (buffer, memory) = create_buffer(
        instance,
        device,
        physical_device,
        size_of::<CubeSceneUniforms>() as vk::DeviceSize,
        vk::BufferUsageFlags::UNIFORM_BUFFER,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )?;
    let (object_buffer, object_memory) = match create_buffer(
        instance,
        device,
        physical_device,
        (MAX_SCENE_CUBES * size_of::<GpuSceneCube>()) as vk::DeviceSize,
        vk::BufferUsageFlags::STORAGE_BUFFER,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    ) {
        Ok(resources) => resources,
        Err(err) => {
            unsafe {
                device.destroy_buffer(buffer, None);
                device.free_memory(memory, None);
            }
            return Err(err);
        }
    };

    let pool_sizes = [
        vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1),
        vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::STORAGE_BUFFER)
            .descriptor_count(1),
    ];
    let pool_info = vk::DescriptorPoolCreateInfo::default()
        .max_sets(1)
        .pool_sizes(&pool_sizes);
    let descriptor_pool = match vk_result(
        unsafe { device.create_descriptor_pool(&pool_info, None) },
        "create_descriptor_pool(3d)",
    ) {
        Ok(pool) => pool,
        Err(err) => {
            unsafe {
                device.destroy_buffer(object_buffer, None);
                device.free_memory(object_memory, None);
                device.destroy_buffer(buffer, None);
                device.free_memory(memory, None);
            }
            return Err(err);
        }
    };

    let set_layouts = [descriptor_set_layout];
    let alloc_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(&set_layouts);
    let descriptor_set = match vk_result(
        unsafe { device.allocate_descriptor_sets(&alloc_info) },
        "allocate_descriptor_sets(3d)",
    ) {
        Ok(mut sets) => sets.remove(0),
        Err(err) => {
            unsafe {
                device.destroy_descriptor_pool(descriptor_pool, None);
                device.destroy_buffer(object_buffer, None);
                device.free_memory(object_memory, None);
                device.destroy_buffer(buffer, None);
                device.free_memory(memory, None);
            }
            return Err(err);
        }
    };

    Ok((
        buffer,
        memory,
        object_buffer,
        object_memory,
        descriptor_pool,
        descriptor_set,
    ))
}

fn register_runtime_descriptor_resources_3d(
    executor_resources: &mut ExecutorResources,
    descriptor_set: vk::DescriptorSet,
    scene_buffer: vk::Buffer,
    object_buffer: vk::Buffer,
) {
    executor_resources.descriptor_sets.insert(
        RUNTIME_DESCRIPTOR_SET_3D,
        DescriptorSetBinding {
            set: descriptor_set,
            layout: DescriptorSetLayoutBindings {
                uniform_buffers_by_name: HashMap::from([(
                    String::from(RUNTIME_UNIFORM_CUBE_SCENE),
                    0,
                )]),
                storage_buffers_by_name: HashMap::from([(
                    String::from(RUNTIME_STORAGE_CUBE_OBJECTS),
                    1,
                )]),
                combined_image_samplers_by_slot: HashMap::new(),
            },
        },
    );
    executor_resources.buffers.insert(
        RUNTIME_BUFFER_CUBE_SCENE,
        BufferBinding {
            buffer: scene_buffer,
            offset: 0,
            range: size_of::<CubeSceneUniforms>() as u64,
        },
    );
    executor_resources.buffers.insert(
        RUNTIME_BUFFER_CUBE_OBJECTS,
        BufferBinding {
            buffer: object_buffer,
            offset: 0,
            range: (MAX_SCENE_CUBES * size_of::<GpuSceneCube>()) as u64,
        },
    );
    executor_resources.named_uniform_buffers.insert(
        String::from(RUNTIME_UNIFORM_CUBE_SCENE),
        RUNTIME_BUFFER_CUBE_SCENE,
    );
    executor_resources.named_storage_buffers.insert(
        String::from(RUNTIME_STORAGE_CUBE_OBJECTS),
        RUNTIME_BUFFER_CUBE_OBJECTS,
    );
}

fn update_runtime_descriptor_resources_3d(
    device: &Device,
    executor_resources: &ExecutorResources,
) -> Result<(), ApiError> {
    let writes = resolve_descriptor_writes_for_bindings(
        executor_resources,
        &[RUNTIME_DESCRIPTOR_SET_3D],
        &[
            (RUNTIME_UNIFORM_CUBE_SCENE, UniformValueKind::BufferBlock),
            (RUNTIME_STORAGE_CUBE_OBJECTS, UniformValueKind::StorageBlock),
        ],
        &[],
    );
    if writes.is_empty() {
        return Err(ApiError::InvalidConfig {
            reason: "runtime 3d descriptor registration did not resolve CubeScene uniform buffer"
                .into(),
        });
    }
    apply_descriptor_writes(device, &writes);
    Ok(())
}

pub fn register_sampled_texture_descriptor(
    executor_resources: &mut ExecutorResources,
    descriptor_handle: DescriptorSetHandle,
    descriptor_set: vk::DescriptorSet,
    slot_bindings: &[(u32, u32)],
    textures: &[(TextureHandle, TextureBinding)],
) {
    executor_resources.descriptor_sets.insert(
        descriptor_handle,
        DescriptorSetBinding {
            set: descriptor_set,
            layout: DescriptorSetLayoutBindings {
                uniform_buffers_by_name: HashMap::new(),
                storage_buffers_by_name: HashMap::new(),
                combined_image_samplers_by_slot: slot_bindings.iter().copied().collect(),
            },
        },
    );
    for (handle, binding) in textures {
        executor_resources.textures.insert(*handle, *binding);
    }
}

pub fn resolve_sampled_texture_descriptor_writes(
    executor_resources: &ExecutorResources,
    descriptor_handle: DescriptorSetHandle,
    textures_by_slot: &[(u32, TextureHandle)],
) -> Vec<crate::executor::DescriptorWritePlan> {
    resolve_descriptor_writes_for_bindings(
        executor_resources,
        &[descriptor_handle],
        &[],
        textures_by_slot,
    )
}

pub fn update_sampled_texture_descriptor(
    device: &Device,
    executor_resources: &ExecutorResources,
    descriptor_handle: DescriptorSetHandle,
    textures_by_slot: &[(u32, TextureHandle)],
) -> usize {
    let writes = resolve_sampled_texture_descriptor_writes(
        executor_resources,
        descriptor_handle,
        textures_by_slot,
    );
    if !writes.is_empty() {
        apply_descriptor_writes(device, &writes);
    }
    writes.len()
}

fn read_spirv_words(spirv_bytes: &[u8]) -> Result<Vec<u32>, ApiError> {
    let mut cursor = Cursor::new(spirv_bytes);
    ash::util::read_spv(&mut cursor).map_err(|err| ApiError::InvalidConfig {
        reason: format!("failed to parse shader SPIR-V: {err}"),
    })
}

fn executor_error_to_api_error(err: ExecutorError) -> ApiError {
    match err {
        ExecutorError::MissingResource {
            resource_type,
            handle,
        } => ApiError::ResourceNotFound {
            resource_type,
            name: handle.to_string(),
        },
        ExecutorError::InvalidState { reason } => ApiError::InvalidConfig { reason },
        ExecutorError::Vulkan { context, result } => ApiError::Vulkan { context, result },
    }
}

fn find_depth_format(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
) -> Result<vk::Format, ApiError> {
    for format in [
        vk::Format::D32_SFLOAT,
        vk::Format::D32_SFLOAT_S8_UINT,
        vk::Format::D24_UNORM_S8_UINT,
    ] {
        let properties =
            unsafe { instance.get_physical_device_format_properties(physical_device, format) };
        if properties
            .optimal_tiling_features
            .contains(vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT)
        {
            return Ok(format);
        }
    }

    Err(ApiError::InvalidConfig {
        reason: "no supported depth format found".to_string(),
    })
}

fn choose_msaa_samples(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    color_format: vk::Format,
    depth_format: vk::Format,
) -> vk::SampleCountFlags {
    let color_properties = match vk_result(
        unsafe {
            instance.get_physical_device_image_format_properties(
                physical_device,
                color_format,
                vk::ImageType::TYPE_2D,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT,
                vk::ImageCreateFlags::empty(),
            )
        },
        "get_physical_device_image_format_properties(color_msaa)",
    ) {
        Ok(properties) => properties,
        Err(_) => return vk::SampleCountFlags::TYPE_1,
    };
    let depth_properties = match vk_result(
        unsafe {
            instance.get_physical_device_image_format_properties(
                physical_device,
                depth_format,
                vk::ImageType::TYPE_2D,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                vk::ImageCreateFlags::empty(),
            )
        },
        "get_physical_device_image_format_properties(depth_msaa)",
    ) {
        Ok(properties) => properties,
        Err(_) => return vk::SampleCountFlags::TYPE_1,
    };
    let supported = color_properties.sample_counts & depth_properties.sample_counts;

    for sample in [
        vk::SampleCountFlags::TYPE_4,
        vk::SampleCountFlags::TYPE_2,
        vk::SampleCountFlags::TYPE_1,
    ] {
        if supported.contains(sample) {
            return sample;
        }
    }

    vk::SampleCountFlags::TYPE_1
}

fn choose_surface_format(formats: &[vk::SurfaceFormatKHR]) -> Option<vk::SurfaceFormatKHR> {
    formats
        .iter()
        .copied()
        .find(|format| {
            format.format == vk::Format::B8G8R8A8_UNORM
                && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
        })
        .or_else(|| formats.first().copied())
}

fn choose_present_mode(present_modes: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
    if present_modes.contains(&vk::PresentModeKHR::MAILBOX) {
        vk::PresentModeKHR::MAILBOX
    } else {
        vk::PresentModeKHR::FIFO
    }
}

fn build_gpu_scene_cubes(meshes: &[MeshDraw3D]) -> Vec<GpuSceneCube> {
    meshes
        .iter()
        .cloned()
        .map(|mesh| {
            let axes = cube_axes(mesh.rotation_radians);
            let thin_slab = is_thin_cube_slab(&mesh);
            let occlusion_weight = match mesh.mesh {
                Mesh3D::Plane => 0.28,
                Mesh3D::Cube if thin_slab => 0.14,
                Mesh3D::Cube
                | Mesh3D::Sphere
                | Mesh3D::Custom(_)
                | Mesh3D::Cylinder { .. }
                | Mesh3D::Torus { .. }
                | Mesh3D::Cone { .. }
                | Mesh3D::Capsule { .. }
                | Mesh3D::Icosphere { .. } => 1.0,
            };
            GpuSceneCube {
                center: [
                    mesh.center[0],
                    mesh.center[1],
                    mesh.center[2],
                    occlusion_weight,
                ],
                half_extents: [
                    mesh.size[0] * 0.5,
                    mesh.size[1] * 0.5,
                    mesh.size[2] * 0.5,
                    0.0,
                ],
                axis_x: [
                    axes[0][0],
                    axes[0][1],
                    axes[0][2],
                    if matches!(mesh.mesh, Mesh3D::Plane | Mesh3D::Torus { .. }) || thin_slab {
                        0.0
                    } else {
                        1.0
                    },
                ],
                axis_y: [axes[1][0], axes[1][1], axes[1][2], 0.0],
                axis_z: [axes[2][0], axes[2][1], axes[2][2], 0.0],
            }
        })
        .collect()
}

fn is_thin_cube_slab(mesh: &MeshDraw3D) -> bool {
    if !matches!(mesh.mesh, Mesh3D::Cube) {
        return false;
    }

    let min_extent = mesh.size[0].min(mesh.size[1]).min(mesh.size[2]);
    let max_extent = mesh.size[0].max(mesh.size[1]).max(mesh.size[2]);
    min_extent <= 0.16 && max_extent >= 1.5
}

fn cube_axes(rotation_radians: [f32; 3]) -> [[f32; 3]; 3] {
    [
        normalize3(rotate_vector_3d([1.0, 0.0, 0.0], rotation_radians)),
        normalize3(rotate_vector_3d([0.0, 1.0, 0.0], rotation_radians)),
        normalize3(rotate_vector_3d([0.0, 0.0, 1.0], rotation_radians)),
    ]
}

fn sphere_point(theta: f32, phi: f32) -> [f32; 3] {
    let cos_theta = theta.cos();
    [cos_theta * phi.cos(), theta.sin(), cos_theta * phi.sin()]
}

fn choose_surface_extent(
    window: &Window,
    capabilities: vk::SurfaceCapabilitiesKHR,
) -> vk::Extent2D {
    if capabilities.current_extent.width != u32::MAX {
        return capabilities.current_extent;
    }

    let size = window.inner_size();
    vk::Extent2D {
        width: size.width.clamp(
            capabilities.min_image_extent.width,
            capabilities.max_image_extent.width,
        ),
        height: size.height.clamp(
            capabilities.min_image_extent.height,
            capabilities.max_image_extent.height,
        ),
    }
}

fn vk_result<T>(result: Result<T, vk::Result>, context: &'static str) -> Result<T, ApiError> {
    result.map_err(|err| ApiError::Vulkan {
        context,
        result: err,
    })
}

fn cube_view_projection_as_bytes(push_constants: &CubeViewProjectionPushConstants) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            (push_constants as *const CubeViewProjectionPushConstants).cast::<u8>(),
            size_of::<CubeViewProjectionPushConstants>(),
        )
    }
}

fn screenshot_camera_jitter(sample_index: u32, extent: vk::Extent2D) -> [f32; 2] {
    let pixel_jitter_x = halton(sample_index + 1, 2) - 0.5;
    let pixel_jitter_y = halton(sample_index + 1, 3) - 0.5;
    [
        (pixel_jitter_x * 2.0) / extent.width.max(1) as f32,
        (pixel_jitter_y * 2.0) / extent.height.max(1) as f32,
    ]
}

fn halton(mut index: u32, base: u32) -> f32 {
    let mut result = 0.0;
    let mut fraction = 1.0 / base as f32;
    while index > 0 {
        result += fraction * (index % base) as f32;
        index /= base;
        fraction /= base as f32;
    }
    result
}

fn perspective_lh(
    fov_y_radians: f32,
    aspect: f32,
    near: f32,
    far: f32,
    jitter_ndc: [f32; 2],
) -> [[f32; 4]; 4] {
    let f = 1.0 / (fov_y_radians * 0.5).tan();
    let range = far - near;
    [
        [f / aspect.max(0.0001), 0.0, jitter_ndc[0], 0.0],
        [0.0, f, jitter_ndc[1], 0.0],
        [0.0, 0.0, far / range, (-near * far) / range],
        [0.0, 0.0, 1.0, 0.0],
    ]
}

fn look_at_lh(eye: [f32; 3], center: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let forward = normalize3(sub3(center, eye));
    let side = normalize3(cross3(up, forward));
    let up = cross3(forward, side);

    [
        [side[0], side[1], side[2], -dot3(side, eye)],
        [up[0], up[1], up[2], -dot3(up, eye)],
        [forward[0], forward[1], forward[2], -dot3(forward, eye)],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn mul_mat4(left: [[f32; 4]; 4], right: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            out[row][col] = left[row][0] * right[0][col]
                + left[row][1] * right[1][col]
                + left[row][2] * right[2][col]
                + left[row][3] * right[3][col];
        }
    }
    out
}

fn orthographic_lh(
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
    near: f32,
    far: f32,
) -> [[f32; 4]; 4] {
    let width = (right - left).max(0.0001);
    let height = (top - bottom).max(0.0001);
    let depth = (far - near).max(0.0001);
    [
        [2.0 / width, 0.0, 0.0, -(right + left) / width],
        [0.0, 2.0 / height, 0.0, -(top + bottom) / height],
        [0.0, 0.0, 1.0 / depth, -near / depth],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn transform_point_mat4(matrix: [[f32; 4]; 4], point: [f32; 3]) -> [f32; 3] {
    let world = [point[0], point[1], point[2], 1.0];
    [
        matrix[0][0] * world[0] + matrix[0][1] * world[1] + matrix[0][2] * world[2] + matrix[0][3],
        matrix[1][0] * world[0] + matrix[1][1] * world[1] + matrix[1][2] * world[2] + matrix[1][3],
        matrix[2][0] * world[0] + matrix[2][1] * world[1] + matrix[2][2] * world[2] + matrix[2][3],
    ]
}

fn compute_directional_shadow_view_projection(
    meshes: &[MeshDraw3D],
    camera: Camera3D,
    lighting: LightingConfig,
    max_distance: f32,
) -> [[f32; 4]; 4] {
    let light_dir = normalize3(lighting.fill_light.direction);
    let mut min_world = [f32::INFINITY; 3];
    let mut max_world = [f32::NEG_INFINITY; 3];
    let mut any = false;

    for mesh in meshes {
        let half = [mesh.size[0] * 0.5, mesh.size[1] * 0.5, mesh.size[2] * 0.5];
        let corners = [
            [-half[0], -half[1], -half[2]],
            [half[0], -half[1], -half[2]],
            [half[0], half[1], -half[2]],
            [-half[0], half[1], -half[2]],
            [-half[0], -half[1], half[2]],
            [half[0], -half[1], half[2]],
            [half[0], half[1], half[2]],
            [-half[0], half[1], half[2]],
        ];
        for corner in corners {
            let world = add3(rotate_vector_3d(corner, mesh.rotation_radians), mesh.center);
            min_world[0] = min_world[0].min(world[0]);
            min_world[1] = min_world[1].min(world[1]);
            min_world[2] = min_world[2].min(world[2]);
            max_world[0] = max_world[0].max(world[0]);
            max_world[1] = max_world[1].max(world[1]);
            max_world[2] = max_world[2].max(world[2]);
            any = true;
        }
    }

    if !any {
        min_world = [-2.0, -2.0, -2.0];
        max_world = [2.0, 2.0, 2.0];
    }

    let center = scale3(add3(min_world, max_world), 0.5);
    let radius = length3(sub3(max_world, min_world))
        .min(max_distance.max(6.0))
        .max(6.0);
    let eye = sub3(center, scale3(light_dir, radius));
    let up = if light_dir[1].abs() > 0.95 {
        [0.0, 0.0, 1.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let view = look_at_lh(eye, center, up);

    let bounds_corners = [
        [min_world[0], min_world[1], min_world[2]],
        [max_world[0], min_world[1], min_world[2]],
        [max_world[0], max_world[1], min_world[2]],
        [min_world[0], max_world[1], min_world[2]],
        [min_world[0], min_world[1], max_world[2]],
        [max_world[0], min_world[1], max_world[2]],
        [max_world[0], max_world[1], max_world[2]],
        [min_world[0], max_world[1], max_world[2]],
        camera.position,
        camera.target,
    ];
    let mut min_light = [f32::INFINITY; 3];
    let mut max_light = [f32::NEG_INFINITY; 3];
    for corner in bounds_corners {
        let light_space = transform_point_mat4(view, corner);
        min_light[0] = min_light[0].min(light_space[0]);
        min_light[1] = min_light[1].min(light_space[1]);
        min_light[2] = min_light[2].min(light_space[2]);
        max_light[0] = max_light[0].max(light_space[0]);
        max_light[1] = max_light[1].max(light_space[1]);
        max_light[2] = max_light[2].max(light_space[2]);
    }
    let margin = 2.0;
    let projection = orthographic_lh(
        min_light[0] - margin,
        max_light[0] + margin,
        min_light[1] - margin,
        max_light[1] + margin,
        (min_light[2] - margin).max(0.1),
        max_light[2] + margin,
    );
    mul_mat4(projection, view)
}

fn rotate_vector_3d(vector: [f32; 3], rotation_radians: [f32; 3]) -> [f32; 3] {
    let [mut x, mut y, mut z] = vector;

    let (sin_x, cos_x) = rotation_radians[0].sin_cos();
    let rotated_y = (y * cos_x) - (z * sin_x);
    let rotated_z = (y * sin_x) + (z * cos_x);
    y = rotated_y;
    z = rotated_z;

    let (sin_y, cos_y) = rotation_radians[1].sin_cos();
    let rotated_x = (x * cos_y) + (z * sin_y);
    let rotated_z = (-x * sin_y) + (z * cos_y);
    x = rotated_x;
    z = rotated_z;

    let (sin_z, cos_z) = rotation_radians[2].sin_cos();
    let rotated_x = (x * cos_z) - (y * sin_z);
    let rotated_y = (x * sin_z) + (y * cos_z);

    [rotated_x, rotated_y, z]
}

fn add3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn sub3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn scale3(vector: [f32; 3], scale: f32) -> [f32; 3] {
    [vector[0] * scale, vector[1] * scale, vector[2] * scale]
}

fn dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
    (left[0] * right[0]) + (left[1] * right[1]) + (left[2] * right[2])
}

fn cross3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        (left[1] * right[2]) - (left[2] * right[1]),
        (left[2] * right[0]) - (left[0] * right[2]),
        (left[0] * right[1]) - (left[1] * right[0]),
    ]
}

fn length3(vector: [f32; 3]) -> f32 {
    dot3(vector, vector).sqrt()
}

fn normalize3(vector: [f32; 3]) -> [f32; 3] {
    let length = length3(vector).max(0.0001);
    [vector[0] / length, vector[1] / length, vector[2] / length]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transform_point(matrix: [[f32; 4]; 4], point: [f32; 3]) -> [f32; 4] {
        let world = [point[0], point[1], point[2], 1.0];
        [
            matrix[0][0] * world[0]
                + matrix[0][1] * world[1]
                + matrix[0][2] * world[2]
                + matrix[0][3] * world[3],
            matrix[1][0] * world[0]
                + matrix[1][1] * world[1]
                + matrix[1][2] * world[2]
                + matrix[1][3] * world[3],
            matrix[2][0] * world[0]
                + matrix[2][1] * world[1]
                + matrix[2][2] * world[2]
                + matrix[2][3] * world[3],
            matrix[3][0] * world[0]
                + matrix[3][1] * world[1]
                + matrix[3][2] * world[2]
                + matrix[3][3] * world[3],
        ]
    }

    #[test]
    fn view_projection_keeps_origin_inside_clip_space() {
        let camera = Camera3D::default();
        let matrix = mul_mat4(
            perspective_lh(
                camera.fov_y_degrees.to_radians(),
                960.0 / 640.0,
                camera.near_clip,
                camera.far_clip,
                [0.0, 0.0],
            ),
            look_at_lh(camera.position, camera.target, camera.up),
        );

        let clip = transform_point(matrix, [0.0, 0.0, 0.0]);
        let ndc = [clip[0] / clip[3], clip[1] / clip[3], clip[2] / clip[3]];

        assert!(clip[3] > 0.0, "w should stay positive: {clip:?}");
        assert!(ndc[0].abs() <= 1.0, "x out of clip: {ndc:?}");
        assert!(ndc[1].abs() <= 1.0, "y out of clip: {ndc:?}");
        assert!((0.0..=1.0).contains(&ndc[2]), "z out of clip: {ndc:?}");
    }

    #[test]
    fn cube_vertex_builder_emits_triangles_for_demo_cube() {
        let mut frame = SceneFrame::default();
        frame.draw_mesh_3d(MeshDraw3D {
            mesh: Mesh3D::Cube,
            center: [0.0, 0.0, 0.0],
            size: [1.05, 1.05, 1.05],
            rotation_radians: [-0.55, -0.35, 0.0],
            color: [0.95, 0.04, 0.04, 1.0],
            material: Default::default(),
        });

        let (vertices, _, _, _, _, _) = build_mesh_vertices(
            &frame,
            Camera3D::default(),
            LightingConfig::default(),
            vk::Extent2D {
                width: 960,
                height: 640,
            },
            [0.0, 0.0],
        );

        assert!(
            !vertices.is_empty(),
            "cube vertex builder emitted no triangles"
        );
        assert_eq!(vertices.len() % 3, 0);
    }

    #[test]
    fn sphere_vertex_builder_emits_triangles_for_demo_sphere() {
        let mut frame = SceneFrame::default();
        frame.draw_mesh_3d(MeshDraw3D {
            mesh: Mesh3D::Sphere,
            center: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_radians: [0.0, 0.0, 0.0],
            color: [0.92, 0.18, 0.18, 1.0],
            material: Default::default(),
        });

        let (vertices, batches, _, _, _, _) = build_mesh_vertices(
            &frame,
            Camera3D::default(),
            LightingConfig::default(),
            vk::Extent2D {
                width: 960,
                height: 640,
            },
            [0.0, 0.0],
        );

        assert!(
            !vertices.is_empty(),
            "sphere vertex builder emitted no triangles"
        );
        assert_eq!(vertices.len() % 3, 0);
        assert_eq!(batches.len(), 1);
        assert!(batches[0].vertex_count > 0);
    }

    #[test]
    fn mesh_batches_preserve_material_texture_handles() {
        let mut frame = SceneFrame::default();
        frame.draw_mesh_3d(MeshDraw3D {
            material: crate::scene::MeshMaterial3D {
                albedo_texture: Some(TextureHandle(9)),
                roughness: 0.5,
                metallic: 0.0,
            },
            ..MeshDraw3D::default()
        });

        let (_, batches, _, _, _, _) = build_mesh_vertices(
            &frame,
            Camera3D::default(),
            LightingConfig::default(),
            vk::Extent2D {
                width: 960,
                height: 640,
            },
            [0.0, 0.0],
        );

        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].albedo_texture, Some(TextureHandle(9)));
        assert!(batches[0].vertex_count > 0);
    }

    #[test]
    fn sampled_texture_descriptor_registration_resolves_combined_image_write() {
        let mut resources = ExecutorResources::default();
        register_sampled_texture_descriptor(
            &mut resources,
            DescriptorSetHandle(7),
            vk::DescriptorSet::null(),
            &[(0, 3)],
            &[(
                TextureHandle(11),
                TextureBinding {
                    image_view: vk::ImageView::null(),
                    sampler: vk::Sampler::null(),
                    image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                },
            )],
        );

        let writes = resolve_sampled_texture_descriptor_writes(
            &resources,
            DescriptorSetHandle(7),
            &[(0, TextureHandle(11))],
        );

        assert_eq!(
            writes,
            vec![crate::executor::DescriptorWritePlan::CombinedImageSampler {
                descriptor_set: vk::DescriptorSet::null(),
                binding: 3,
                image_view: vk::ImageView::null(),
                sampler: vk::Sampler::null(),
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }]
        );
    }
}

fn find_memory_type(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    type_filter: u32,
    required_properties: vk::MemoryPropertyFlags,
) -> Result<u32, ApiError> {
    let memory_properties =
        unsafe { instance.get_physical_device_memory_properties(physical_device) };

    for index in 0..memory_properties.memory_type_count {
        let memory_type = memory_properties.memory_types[index as usize];
        let supported = (type_filter & (1 << index)) != 0;
        if supported && memory_type.property_flags.contains(required_properties) {
            return Ok(index);
        }
    }

    Err(ApiError::InvalidConfig {
        reason: format!(
            "no compatible memory type found for properties {:?}",
            required_properties
        ),
    })
}
