use crate::backend::{GraphicsBackendKind, VULKAN_BACKEND_INFO};
use crate::core::ApiConfig;
use crate::lighting::{
    DirectionalLight, LightingConfig, LiveShadowConfig, ShadowConfig, ShadowMode,
};
use crate::run_scene;
use crate::scene::{Camera2D, Camera3D, Mesh3D, MeshDraw3D, Scene, SceneConfig, SceneFrame};
use crate::syntax::opengl::BackendContext;
use crate::syntax::{
    BufferTarget, BufferUsage, CommonGfxContext, FramebufferAttachment, IndexType,
    RenderStateFlags, RenderbufferHandle, VertexAttribType,
};
use std::collections::HashMap;
use std::ffi::{CStr, c_char};
use std::sync::Arc;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

pub const NU_FFI_CLEAR_COLOR_BIT: u32 = 1 << 0;
pub const NU_FFI_CLEAR_DEPTH_BIT: u32 = 1 << 1;
pub const NU_FFI_CLEAR_STENCIL_BIT: u32 = 1 << 2;

pub const NU_FFI_RENDER_STATE_DEPTH_TEST: u32 = 1 << 0;
pub const NU_FFI_RENDER_STATE_BLEND: u32 = 1 << 1;
pub const NU_FFI_RENDER_STATE_CULL_FACE: u32 = 1 << 2;

pub const NU_FFI_BUFFER_TARGET_ARRAY: u32 = 1;
pub const NU_FFI_BUFFER_TARGET_ELEMENT_ARRAY: u32 = 2;
pub const NU_FFI_BUFFER_TARGET_UNIFORM: u32 = 3;

pub const NU_FFI_BUFFER_USAGE_STATIC_DRAW: u32 = 1;
pub const NU_FFI_BUFFER_USAGE_DYNAMIC_DRAW: u32 = 2;
pub const NU_FFI_BUFFER_USAGE_STREAM_DRAW: u32 = 3;

pub const NU_FFI_VERTEX_ATTRIB_FLOAT32: u32 = 1;
pub const NU_FFI_VERTEX_ATTRIB_UNSIGNED_SHORT: u32 = 2;
pub const NU_FFI_VERTEX_ATTRIB_UNSIGNED_INT: u32 = 3;

pub const NU_FFI_INDEX_TYPE_U16: u32 = 1;
pub const NU_FFI_INDEX_TYPE_U32: u32 = 2;

pub const NU_FFI_TOPOLOGY_TRIANGLES: u32 = 1;
pub const NU_FFI_TOPOLOGY_LINES: u32 = 2;
pub const NU_FFI_TOPOLOGY_POINTS: u32 = 3;

pub const NU_FFI_ATTACHMENT_COLOR0: u32 = 1;
pub const NU_FFI_ATTACHMENT_DEPTH: u32 = 2;
pub const NU_FFI_ATTACHMENT_DEPTH_STENCIL: u32 = 3;
pub const NU_FFI_TEXTURE_FORMAT_RGBA8: u32 = 1;
pub const NU_FFI_BACKEND_KIND_VULKAN: u32 = GraphicsBackendKind::Vulkan as u32;
pub const NU_FFI_BACKEND_KIND_DX12: u32 = GraphicsBackendKind::Dx12 as u32;
pub const NU_FFI_BACKEND_KIND_METAL: u32 = GraphicsBackendKind::Metal as u32;

const NU_FFI_BACKEND_NAME_VULKAN: &[u8] = b"vulkan\0";
const NU_FFI_BACKEND_DLL_VULKAN: &[u8] = b"nu-vlk.dll\0";
const NU_FFI_BACKEND_DISPLAY_VULKAN: &[u8] = b"NU Vulkan\0";

#[unsafe(no_mangle)]
pub extern "C" fn nu_ffi_backend_kind() -> u32 {
    VULKAN_BACKEND_INFO.kind as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn nu_ffi_backend_name() -> *const c_char {
    NU_FFI_BACKEND_NAME_VULKAN.as_ptr().cast::<c_char>()
}

#[unsafe(no_mangle)]
pub extern "C" fn nu_ffi_backend_dll_name() -> *const c_char {
    NU_FFI_BACKEND_DLL_VULKAN.as_ptr().cast::<c_char>()
}

#[unsafe(no_mangle)]
pub extern "C" fn nu_ffi_backend_display_name() -> *const c_char {
    NU_FFI_BACKEND_DISPLAY_VULKAN.as_ptr().cast::<c_char>()
}

#[repr(C)]
pub struct NuGlScratchContext {
    inner: BackendContext,
    scratch: ScratchGlState,
}

impl NuGlScratchContext {
    fn new() -> Self {
        Self {
            inner: BackendContext::new(),
            scratch: ScratchGlState::default(),
        }
    }
}

#[derive(Debug, Clone)]
struct ScratchBufferData {
    bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
struct ScratchTexture2D {
    width: u32,
    height: u32,
    rgba8: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
struct ScratchVertexAttribute {
    size: i32,
    attrib_type: VertexAttribType,
    normalized: bool,
    stride: i32,
    offset_bytes: u64,
    buffer: u32,
    enabled: bool,
}

#[derive(Debug, Clone, Default)]
struct ScratchVertexArrayState {
    element_buffer: u32,
    attributes: HashMap<u32, ScratchVertexAttribute>,
}

#[derive(Debug, Clone, Copy)]
struct ScratchDrawCall {
    topology: ash::vk::PrimitiveTopology,
    first_vertex: u32,
    vertex_count: u32,
    index_count: u32,
    index_type: Option<IndexType>,
    index_offset_bytes: u64,
}

#[derive(Debug, Clone)]
struct ScratchGlState {
    clear_color: [f32; 4],
    sun_direction: [f32; 3],
    current_vertex_array: u32,
    current_array_buffer: u32,
    active_texture_slot: u32,
    bound_textures: HashMap<u32, u32>,
    buffers: HashMap<u32, ScratchBufferData>,
    textures: HashMap<u32, ScratchTexture2D>,
    vertex_arrays: HashMap<u32, ScratchVertexArrayState>,
    last_draw: Option<ScratchDrawCall>,
}

impl Default for ScratchGlState {
    fn default() -> Self {
        Self {
            clear_color: [0.53, 0.81, 0.92, 1.0],
            sun_direction: [-0.45, 0.82, -0.35],
            current_vertex_array: 0,
            current_array_buffer: 0,
            active_texture_slot: 0,
            bound_textures: HashMap::new(),
            buffers: HashMap::new(),
            textures: HashMap::new(),
            vertex_arrays: HashMap::new(),
            last_draw: None,
        }
    }
}

#[derive(Debug, Clone)]
struct ScratchPreviewMeshPart {
    asset: Arc<crate::scene::MeshAsset3D>,
    face_key: i32,
    color: [f32; 4],
}

#[derive(Debug, Clone, Copy)]
struct ScratchAimTarget {
    place_cell: Option<[i32; 3]>,
    remove_cell: Option<[i32; 3]>,
    hit_cell: Option<[i32; 3]>,
    hit_normal: Option<[i32; 3]>,
}

#[derive(Debug, Clone, Copy)]
struct ScratchBlockHit {
    cell: [i32; 3],
    normal: [i32; 3],
    distance: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScratchBlockVariant {
    Grass,
    Stone,
    Sand,
}

#[derive(Debug, Clone, Copy)]
struct ScratchPlacedBlock {
    cell: [i32; 3],
    variant: ScratchBlockVariant,
}

#[derive(Clone)]
struct ScratchPreviewScene {
    title: String,
    width: u32,
    height: u32,
    clear_color: [f32; 4],
    sun_direction: [f32; 3],
    mesh_parts: Arc<[ScratchPreviewMeshPart]>,
    camera_position: [f32; 3],
    yaw_radians: f32,
    pitch_radians: f32,
    move_forward: bool,
    move_backward: bool,
    move_left: bool,
    move_right: bool,
    jump_requested: bool,
    vertical_velocity: f32,
    on_ground: bool,
    turn_left: bool,
    turn_right: bool,
    turn_up: bool,
    turn_down: bool,
    mouse_captured: bool,
    current_variant: ScratchBlockVariant,
    terrain_blocks: Vec<[i32; 3]>,
    placed_blocks: Vec<ScratchPlacedBlock>,
}

impl Scene for ScratchPreviewScene {
    fn config(&self) -> SceneConfig {
        let mut api = ApiConfig::default();
        api.application_name = "nu C++ Scratch Preview".to_string();
        api.enable_validation = false;

        let mut lighting = LightingConfig::default();
        lighting.clear_point_lights();
        lighting.ambient_color = [0.58, 0.66, 0.78];
        lighting.ambient_intensity = 0.42;
        lighting.fill_light = DirectionalLight {
            direction: self.sun_direction,
            color: [1.0, 0.95, 0.84],
            intensity: 1.15,
        };
        lighting.shadows = ShadowConfig {
            mode: ShadowMode::Live,
            minimum_visibility: 0.10,
            bias: 0.002,
            live: LiveShadowConfig {
                max_distance: 18.0,
                filter_radius: 2.0,
            },
        };
        lighting.specular_strength = 0.08;
        lighting.shininess = 32.0;
        let forward = self.forward_vector();

        SceneConfig {
            window: crate::app::WindowConfig {
                title: self.title.clone(),
                width: self.width.max(320),
                height: self.height.max(240),
            },
            api,
            clear_color: self.clear_color,
            camera: Camera2D::default(),
            camera_3d: Camera3D {
                position: self.camera_position,
                target: add3(self.camera_position, forward),
                up: [0.0, 1.0, 0.0],
                fov_y_degrees: 50.0,
                near_clip: 0.1,
                far_clip: 100.0,
            },
            lighting,
            screenshot_path: None,
            screenshot_accumulation_samples: 1,
            screenshot_resolution: crate::scene::ScreenshotResolution::K4,
            capture_cursor: self.mouse_captured,
        }
    }

    fn update(&mut self, delta_time_seconds: f32) {
        let turn_speed = 1.7;
        if self.turn_left {
            self.yaw_radians -= turn_speed * delta_time_seconds;
        }
        if self.turn_right {
            self.yaw_radians += turn_speed * delta_time_seconds;
        }
        if self.turn_up {
            self.pitch_radians =
                (self.pitch_radians + turn_speed * delta_time_seconds).clamp(-1.2, 1.2);
        }
        if self.turn_down {
            self.pitch_radians =
                (self.pitch_radians - turn_speed * delta_time_seconds).clamp(-1.2, 1.2);
        }

        let forward = self.forward_vector();
        let flat_forward = normalize3([forward[0], 0.0, forward[2]]);
        let right = [flat_forward[2], 0.0, -flat_forward[0]];
        let move_speed = 4.4 * delta_time_seconds;
        let mut desired_position = self.camera_position;
        if self.move_forward {
            desired_position = add3(desired_position, scale3(flat_forward, move_speed));
        }
        if self.move_backward {
            desired_position = add3(desired_position, scale3(flat_forward, -move_speed));
        }
        if self.move_left {
            desired_position = add3(desired_position, scale3(right, -move_speed));
        }
        if self.move_right {
            desired_position = add3(desired_position, scale3(right, move_speed));
        }
        if self.jump_requested && self.on_ground {
            self.vertical_velocity = 5.8;
            self.on_ground = false;
        }
        self.jump_requested = false;
        self.vertical_velocity -= 18.0 * delta_time_seconds;
        desired_position[1] = self.camera_position[1] + self.vertical_velocity * delta_time_seconds;
        let (resolved_position, on_ground) = self.resolve_camera_collision(desired_position);
        self.camera_position = resolved_position;
        self.on_ground = on_ground;
        if on_ground && self.vertical_velocity < 0.0 {
            self.vertical_velocity = 0.0;
        }
    }

    fn window_event(&mut self, window: &Window, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                if let PhysicalKey::Code(code) = event.physical_key {
                    match code {
                        KeyCode::KeyW => self.move_forward = pressed,
                        KeyCode::KeyS => self.move_backward = pressed,
                        KeyCode::KeyA => self.move_left = pressed,
                        KeyCode::KeyD => self.move_right = pressed,
                        KeyCode::ArrowLeft => self.turn_left = pressed,
                        KeyCode::ArrowRight => self.turn_right = pressed,
                        KeyCode::ArrowUp => self.turn_up = pressed,
                        KeyCode::ArrowDown => self.turn_down = pressed,
                        KeyCode::Escape if pressed => self.mouse_captured = false,
                        KeyCode::Space if pressed => self.jump_requested = true,
                        KeyCode::Digit1 if pressed => {
                            self.current_variant = ScratchBlockVariant::Grass
                        }
                        KeyCode::Digit2 if pressed => {
                            self.current_variant = ScratchBlockVariant::Stone
                        }
                        KeyCode::Digit3 if pressed => {
                            self.current_variant = ScratchBlockVariant::Sand
                        }
                        KeyCode::KeyE if pressed => self.place_block_at_aim(),
                        KeyCode::KeyQ if pressed => self.remove_block_at_aim(),
                        _ => {}
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if !self.mouse_captured {
                    self.capture_mouse(window);
                    return;
                }
                self.place_block_at_aim();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                if !self.mouse_captured {
                    self.capture_mouse(window);
                    return;
                }
                self.remove_block_at_aim();
            }
            WindowEvent::CursorMoved { position, .. } => {
                if !self.mouse_captured {
                    return;
                }
                let center = [self.width as f32 * 0.5, self.height as f32 * 0.5];
                let delta_x = position.x as f32 - center[0];
                let delta_y = position.y as f32 - center[1];
                if delta_x.abs() > 0.25 || delta_y.abs() > 0.25 {
                    let sensitivity = 0.0045;
                    self.yaw_radians += delta_x * sensitivity;
                    self.pitch_radians =
                        (self.pitch_radians - delta_y * sensitivity).clamp(-1.2, 1.2);
                    let _ = window.set_cursor_position(PhysicalPosition::new(
                        center[0] as f64,
                        center[1] as f64,
                    ));
                }
            }
            WindowEvent::Resized(size) => {
                self.width = size.width;
                self.height = size.height;
                let _ = window.set_cursor_position(PhysicalPosition::new(
                    self.width as f64 * 0.5,
                    self.height as f64 * 0.5,
                ));
            }
            WindowEvent::Focused(true) => {
                if self.mouse_captured {
                    self.recenter_cursor(window);
                }
            }
            WindowEvent::Focused(false) => {
                self.mouse_captured = false;
            }
            _ => {}
        }
    }

    fn populate(&mut self, frame: &mut SceneFrame) {
        frame.draw_mesh_3d(MeshDraw3D {
            mesh: Mesh3D::Plane,
            center: [0.0, 0.0, 0.0],
            size: [40.0, 1.0, 40.0],
            rotation_radians: [0.0, 0.0, 0.0],
            color: [0.56, 0.64, 0.52, 1.0],
            material: Default::default(),
        });

        for cell in &self.terrain_blocks {
            self.draw_block(frame, *cell, ScratchBlockVariant::Grass);
        }
        for block in &self.placed_blocks {
            self.draw_block(frame, block.cell, block.variant);
        }

        if let Some(target) = self.aim_target() {
            if let (Some(hit_cell), Some(hit_normal)) = (target.hit_cell, target.hit_normal) {
                let rotation_radians = face_normal_to_plane_rotation(hit_normal);
                let block_center = [
                    hit_cell[0] as f32,
                    hit_cell[1] as f32 + 0.5,
                    hit_cell[2] as f32,
                ];
                let face_center = add3(block_center, scale3(int3_to_f32(hit_normal), 0.505));
                frame.draw_mesh_3d(MeshDraw3D {
                    mesh: Mesh3D::Plane,
                    center: face_center,
                    size: [1.03, 1.0, 1.03],
                    rotation_radians,
                    color: [1.0, 0.96, 0.55, 0.78],
                    material: Default::default(),
                });
            }
            if let Some(place_cell) = target.place_cell {
                frame.draw_mesh_3d(MeshDraw3D {
                    mesh: Mesh3D::Plane,
                    center: [
                        place_cell[0] as f32,
                        place_cell[1] as f32 + 0.015,
                        place_cell[2] as f32,
                    ],
                    size: [0.82, 1.0, 0.82],
                    rotation_radians: [0.0, 0.0, 0.0],
                    color: [1.0, 0.93, 0.35, 0.72],
                    material: Default::default(),
                });
            }
            if let Some(remove_cell) = target.remove_cell {
                let remove_center = [
                    remove_cell[0] as f32,
                    remove_cell[1] as f32 + 1.01,
                    remove_cell[2] as f32,
                ];
                frame.draw_mesh_3d(MeshDraw3D {
                    mesh: Mesh3D::Plane,
                    center: remove_center,
                    size: [0.9, 1.0, 0.9],
                    rotation_radians: [0.0, 0.0, 0.0],
                    color: [1.0, 0.28, 0.22, 0.64],
                    material: Default::default(),
                });
            }
        }

        frame.draw_text(crate::scene::TextDraw {
            position: [16.0, 16.0],
            text: if self.mouse_captured {
                "WASD walk  mouse-look  Space jump  LMB place  RMB remove  Esc release"
                    .to_string()
            } else {
                "Click window to capture mouse  WASD walk  Space jump  LMB place  RMB remove  Esc release"
                    .to_string()
            },
            pixel_size: 18.0,
            color: [1.0, 1.0, 1.0, 1.0],
            layer: 1000,
            space: crate::scene::DrawSpace::Screen,
            anchor: crate::scene::TextAnchor::TopLeft,
        });
        self.draw_hotbar(frame);

        let crosshair_center = [self.width as f32 * 0.5, self.height as f32 * 0.5];
        frame.draw_line(crate::scene::LineDraw {
            start: [crosshair_center[0] - 8.0, crosshair_center[1]],
            end: [crosshair_center[0] + 8.0, crosshair_center[1]],
            thickness: 2.0,
            color: [1.0, 1.0, 1.0, 0.9],
            layer: 1001,
            space: crate::scene::DrawSpace::Screen,
        });
        frame.draw_line(crate::scene::LineDraw {
            start: [crosshair_center[0], crosshair_center[1] - 8.0],
            end: [crosshair_center[0], crosshair_center[1] + 8.0],
            thickness: 2.0,
            color: [1.0, 1.0, 1.0, 0.9],
            layer: 1001,
            space: crate::scene::DrawSpace::Screen,
        });
    }
}

impl ScratchBlockVariant {
    fn label(self) -> &'static str {
        match self {
            Self::Grass => "GRASS",
            Self::Stone => "STONE",
            Self::Sand => "SAND",
        }
    }

    fn ui_color(self) -> [f32; 4] {
        match self {
            Self::Grass => [0.55, 0.82, 0.45, 1.0],
            Self::Stone => [0.78, 0.80, 0.84, 1.0],
            Self::Sand => [0.95, 0.88, 0.60, 1.0],
        }
    }

    fn face_color(self, face_key: i32, fallback: [f32; 4]) -> [f32; 4] {
        match self {
            Self::Grass => match face_key {
                2 => [0.38, 0.69, 0.28, 1.0],
                -2 => [0.34, 0.23, 0.13, 1.0],
                _ => [0.50, 0.39, 0.23, 1.0],
            },
            Self::Stone => match face_key {
                2 => [0.70, 0.71, 0.73, 1.0],
                -2 => [0.43, 0.44, 0.47, 1.0],
                _ => [0.58, 0.59, 0.62, 1.0],
            },
            Self::Sand => match face_key {
                2 => [0.92, 0.84, 0.58, 1.0],
                -2 => [0.63, 0.55, 0.33, 1.0],
                _ => [0.82, 0.73, 0.46, 1.0],
            },
        }
        .with_alpha(fallback[3])
    }
}

trait WithAlpha {
    fn with_alpha(self, alpha: f32) -> Self;
}

impl WithAlpha for [f32; 4] {
    fn with_alpha(mut self, alpha: f32) -> Self {
        self[3] = alpha;
        self
    }
}

impl ScratchPreviewScene {
    fn recenter_cursor(&self, window: &Window) {
        let _ = window.set_cursor_position(PhysicalPosition::new(
            self.width as f64 * 0.5,
            self.height as f64 * 0.5,
        ));
    }

    fn capture_mouse(&mut self, window: &Window) {
        self.mouse_captured = true;
        self.recenter_cursor(window);
    }

    fn draw_hotbar(&self, frame: &mut SceneFrame) {
        let mut canvas = frame.ui_canvas();
        let slot_size = [92.0, 52.0];
        let gap = 10.0;
        let total_width = slot_size[0] * 3.0 + gap * 2.0;
        let start_x = self.width as f32 * 0.5 - total_width * 0.5 + slot_size[0] * 0.5;
        let center_y = self.height as f32 - 54.0;
        let variants = [
            ScratchBlockVariant::Grass,
            ScratchBlockVariant::Stone,
            ScratchBlockVariant::Sand,
        ];
        for (index, variant) in variants.into_iter().enumerate() {
            let center = [start_x + index as f32 * (slot_size[0] + gap), center_y];
            let selected = variant == self.current_variant;
            canvas.fill_rect(
                center,
                slot_size,
                0.0,
                if selected {
                    [0.20, 0.18, 0.12, 0.92]
                } else {
                    [0.05, 0.05, 0.06, 0.78]
                },
                1100,
            );
            canvas.stroke_rect(
                center,
                slot_size,
                0.0,
                if selected {
                    [1.0, 0.93, 0.35, 1.0]
                } else {
                    [0.65, 0.66, 0.70, 0.9]
                },
                if selected { 4.0 } else { 2.0 },
                1101,
            );
            canvas.text_centered(
                [center[0], center[1] - 8.0],
                16.0,
                format!("{}", index + 1),
                [1.0, 1.0, 1.0, 1.0],
                1102,
            );
            canvas.text_centered(
                [center[0], center[1] + 10.0],
                18.0,
                variant.label(),
                variant.ui_color(),
                1102,
            );
        }
    }

    fn draw_block(&self, frame: &mut SceneFrame, cell: [i32; 3], variant: ScratchBlockVariant) {
        let center = [cell[0] as f32, cell[1] as f32 + 0.5, cell[2] as f32];
        for part in self.mesh_parts.iter() {
            frame.draw_mesh_3d(MeshDraw3D {
                mesh: Mesh3D::Custom(part.asset.clone()),
                center,
                size: [1.0, 1.0, 1.0],
                rotation_radians: [0.0, 0.0, 0.0],
                color: variant.face_color(part.face_key, part.color),
                material: Default::default(),
            });
        }
    }

    fn forward_vector(&self) -> [f32; 3] {
        let cos_pitch = self.pitch_radians.cos();
        [
            self.yaw_radians.sin() * cos_pitch,
            self.pitch_radians.sin(),
            self.yaw_radians.cos() * cos_pitch,
        ]
    }

    fn aim_target(&self) -> Option<ScratchAimTarget> {
        let origin = self.camera_position;
        let direction = normalize3(self.forward_vector());
        if let Some(hit) = self.raycast_blocks(origin, direction, 12.0) {
            let adjacent_cell = [
                hit.cell[0] + hit.normal[0],
                hit.cell[1] + hit.normal[1],
                hit.cell[2] + hit.normal[2],
            ];
            let place_cell = (!self.is_occupied(adjacent_cell)).then_some(adjacent_cell);
            let remove_cell = self
                .placed_blocks
                .iter()
                .any(|block| block.cell == hit.cell)
                .then_some(hit.cell);
            return Some(ScratchAimTarget {
                place_cell,
                remove_cell,
                hit_cell: Some(hit.cell),
                hit_normal: Some(hit.normal),
            });
        }

        self.raycast_ground(origin, direction, 12.0)
            .filter(|place_cell| !self.is_occupied(*place_cell))
            .map(|place_cell| ScratchAimTarget {
                place_cell: Some(place_cell),
                remove_cell: None,
                hit_cell: None,
                hit_normal: None,
            })
    }

    fn place_block_at_aim(&mut self) {
        if let Some(target) = self.aim_target() {
            if let Some(place_cell) = target.place_cell {
                if !self.is_occupied(place_cell) {
                    self.placed_blocks.push(ScratchPlacedBlock {
                        cell: place_cell,
                        variant: self.current_variant,
                    });
                }
            }
        }
    }

    fn remove_block_at_aim(&mut self) {
        let Some(target) = self.aim_target() else {
            return;
        };
        let Some(remove_cell) = target.remove_cell else {
            return;
        };
        self.placed_blocks.retain(|block| block.cell != remove_cell);
    }

    fn is_occupied(&self, cell: [i32; 3]) -> bool {
        self.terrain_blocks.contains(&cell)
            || self.placed_blocks.iter().any(|block| block.cell == cell)
    }

    fn resolve_camera_collision(&self, desired: [f32; 3]) -> ([f32; 3], bool) {
        let mut resolved = self.camera_position;
        let mut x_candidate = resolved;
        x_candidate[0] = desired[0];
        if !self.camera_collides(x_candidate) {
            resolved[0] = desired[0];
        }
        let mut z_candidate = resolved;
        z_candidate[2] = desired[2];
        if !self.camera_collides(z_candidate) {
            resolved[2] = desired[2];
        }
        let foot_target = desired[1] - 1.72;
        let support_height = self.support_height_at([resolved[0], resolved[2]]);
        let mut on_ground = false;
        if foot_target <= support_height {
            resolved[1] = support_height + 1.72;
            on_ground = true;
        } else {
            resolved[1] = desired[1];
        }
        (resolved, on_ground)
    }

    fn camera_collides(&self, position: [f32; 3]) -> bool {
        let radius = 0.22;
        let feet_y = position[1] - 1.72;
        let head_y = feet_y + 1.8;
        self.terrain_blocks
            .iter()
            .copied()
            .chain(self.placed_blocks.iter().map(|block| block.cell))
            .any(|cell| {
                let block_min = [cell[0] as f32 - 0.5, cell[1] as f32, cell[2] as f32 - 0.5];
                let block_max = [
                    cell[0] as f32 + 0.5,
                    cell[1] as f32 + 1.0,
                    cell[2] as f32 + 0.5,
                ];
                let vertical_overlap = feet_y < block_max[1] && head_y > block_min[1];
                let horizontal_overlap = position[0] + radius > block_min[0]
                    && position[0] - radius < block_max[0]
                    && position[2] + radius > block_min[2]
                    && position[2] - radius < block_max[2];
                vertical_overlap && horizontal_overlap
            })
    }

    fn support_height_at(&self, position: [f32; 2]) -> f32 {
        let radius = 0.22;
        let mut support_height: f32 = 0.0;
        for cell in self
            .terrain_blocks
            .iter()
            .copied()
            .chain(self.placed_blocks.iter().map(|block| block.cell))
        {
            let block_min = [cell[0] as f32 - 0.5, cell[2] as f32 - 0.5];
            let block_max = [cell[0] as f32 + 0.5, cell[2] as f32 + 0.5];
            let overlaps = position[0] + radius > block_min[0]
                && position[0] - radius < block_max[0]
                && position[1] + radius > block_min[1]
                && position[1] - radius < block_max[1];
            if overlaps {
                support_height = support_height.max(cell[1] as f32 + 1.0);
            }
        }
        support_height
    }

    fn raycast_blocks(
        &self,
        origin: [f32; 3],
        direction: [f32; 3],
        max_distance: f32,
    ) -> Option<ScratchBlockHit> {
        let mut nearest_hit: Option<ScratchBlockHit> = None;
        for cell in self
            .terrain_blocks
            .iter()
            .copied()
            .chain(self.placed_blocks.iter().map(|block| block.cell))
        {
            let Some((distance, normal)) = ray_aabb_intersection(origin, direction, cell) else {
                continue;
            };
            if distance > max_distance {
                continue;
            }
            let should_replace = match nearest_hit {
                Some(existing) => distance < existing.distance,
                None => true,
            };
            if should_replace {
                nearest_hit = Some(ScratchBlockHit {
                    cell,
                    normal,
                    distance,
                });
            }
        }
        nearest_hit
    }

    fn raycast_ground(
        &self,
        origin: [f32; 3],
        direction: [f32; 3],
        max_distance: f32,
    ) -> Option<[i32; 3]> {
        if direction[1].abs() < 0.0001 {
            return None;
        }
        let distance = -origin[1] / direction[1];
        if !(0.0..=max_distance).contains(&distance) {
            return None;
        }
        let hit = add3(origin, scale3(direction, distance));
        let cell = [
            (hit[0] + 0.5).floor() as i32,
            0,
            (hit[2] + 0.5).floor() as i32,
        ];
        ((-8..=8).contains(&cell[0]) && (-8..=8).contains(&cell[2])).then_some(cell)
    }
}

fn build_preview_terrain() -> Vec<[i32; 3]> {
    vec![
        [-2, 0, -2],
        [-1, 0, -2],
        [0, 0, -2],
        [1, 0, -2],
        [-2, 0, -1],
        [-1, 0, -1],
        [0, 0, -1],
        [1, 0, -1],
        [-2, 0, 0],
        [-1, 0, 0],
        [0, 0, 0],
        [1, 0, 0],
        [-1, 1, -1],
        [0, 1, -1],
        [0, 1, 0],
    ]
}

fn extract_preview_scene(
    ctx: &NuGlScratchContext,
    title: String,
    width: u32,
    height: u32,
) -> Option<ScratchPreviewScene> {
    let mesh_parts = build_preview_mesh_parts(&ctx.scratch);
    if mesh_parts.is_empty() {
        return None;
    }

    Some(ScratchPreviewScene {
        title,
        width,
        height,
        clear_color: ctx.scratch.clear_color,
        sun_direction: ctx.scratch.sun_direction,
        mesh_parts: Arc::<[ScratchPreviewMeshPart]>::from(mesh_parts),
        camera_position: [0.35, 1.72, -6.4],
        yaw_radians: 0.0,
        pitch_radians: -0.22,
        move_forward: false,
        move_backward: false,
        move_left: false,
        move_right: false,
        jump_requested: false,
        vertical_velocity: 0.0,
        on_ground: true,
        turn_left: false,
        turn_right: false,
        turn_up: false,
        turn_down: false,
        mouse_captured: false,
        current_variant: ScratchBlockVariant::Grass,
        terrain_blocks: build_preview_terrain(),
        placed_blocks: vec![
            ScratchPlacedBlock {
                cell: [2, 0, -1],
                variant: ScratchBlockVariant::Grass,
            },
            ScratchPlacedBlock {
                cell: [2, 1, -1],
                variant: ScratchBlockVariant::Stone,
            },
        ],
    })
}

fn build_preview_mesh_parts(state: &ScratchGlState) -> Vec<ScratchPreviewMeshPart> {
    let Some(draw) = state.last_draw else {
        return Vec::new();
    };
    if draw.topology != ash::vk::PrimitiveTopology::TRIANGLE_LIST {
        return Vec::new();
    }

    let Some(vao) = state.vertex_arrays.get(&state.current_vertex_array) else {
        return Vec::new();
    };
    let Some(positions) = vao.attributes.get(&0).copied() else {
        return Vec::new();
    };
    let normals = vao.attributes.get(&1).copied();
    let uvs = vao.attributes.get(&2).copied();
    if !positions.enabled {
        return Vec::new();
    }

    let index_stream = build_index_stream(state, vao, draw);
    if index_stream.is_empty() {
        return Vec::new();
    }

    let texture = state
        .bound_textures
        .get(&0)
        .and_then(|texture_id| state.textures.get(texture_id));

    let mut groups: HashMap<i32, Vec<crate::scene::MeshVertex3D>> = HashMap::new();
    let mut group_uvs: HashMap<i32, Vec<[f32; 2]>> = HashMap::new();
    for triangle in index_stream.chunks_exact(3) {
        let mut triangle_vertices = Vec::with_capacity(3);
        for vertex_index in triangle {
            let position = read_vec3_attribute(state, positions, *vertex_index).unwrap_or([0.0; 3]);
            let normal = normals
                .and_then(|attribute| read_vec3_attribute(state, attribute, *vertex_index))
                .unwrap_or([0.0, 1.0, 0.0]);
            let uv = uvs
                .and_then(|attribute| read_vec2_attribute(state, attribute, *vertex_index))
                .unwrap_or([0.5, 0.5]);
            triangle_vertices.push(crate::scene::MeshVertex3D {
                position,
                normal,
                uv,
            });
        }

        let face_key = classify_face(&triangle_vertices);
        groups
            .entry(face_key)
            .or_default()
            .extend(triangle_vertices.clone());
        group_uvs
            .entry(face_key)
            .or_default()
            .extend(triangle_vertices.iter().map(|vertex| vertex.uv));
    }

    let mut parts = Vec::new();
    for (face_key, vertices) in groups {
        if vertices.is_empty() {
            continue;
        }
        let vertices = normalize_preview_face_vertices(face_key, &vertices);
        let sampled = sample_group_color(texture, group_uvs.get(&face_key).map_or(&[], |uvs| uvs));
        let base_size = compute_base_size(&vertices);
        let asset = Arc::new(crate::scene::MeshAsset3D {
            name: format!("scratch_face_{face_key}"),
            vertices: Arc::<[crate::scene::MeshVertex3D]>::from(vertices),
            base_size,
        });
        parts.push(ScratchPreviewMeshPart {
            asset,
            face_key,
            color: sampled,
        });
    }
    parts
}

fn build_index_stream(
    state: &ScratchGlState,
    vao: &ScratchVertexArrayState,
    draw: ScratchDrawCall,
) -> Vec<u32> {
    if let Some(index_type) = draw.index_type {
        let Some(buffer) = state.buffers.get(&vao.element_buffer) else {
            return Vec::new();
        };
        let bytes_per_index = match index_type {
            IndexType::U16 => 2,
            IndexType::U32 => 4,
        };
        let mut indices = Vec::with_capacity(draw.index_count as usize);
        for i in 0..draw.index_count {
            let byte_index = draw.index_offset_bytes as usize + i as usize * bytes_per_index;
            let Some(value) = (match index_type {
                IndexType::U16 => buffer
                    .bytes
                    .get(byte_index..byte_index + 2)
                    .map(|slice| u16::from_le_bytes([slice[0], slice[1]]) as u32),
                IndexType::U32 => buffer
                    .bytes
                    .get(byte_index..byte_index + 4)
                    .map(|slice| u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]])),
            }) else {
                return Vec::new();
            };
            indices.push(value);
        }
        indices
    } else {
        (draw.first_vertex..draw.first_vertex + draw.vertex_count).collect()
    }
}

fn read_vec3_attribute(
    state: &ScratchGlState,
    attribute: ScratchVertexAttribute,
    vertex_index: u32,
) -> Option<[f32; 3]> {
    let buffer = state.buffers.get(&attribute.buffer)?;
    let component_size = match attribute.attrib_type {
        VertexAttribType::Float32 => 4,
        VertexAttribType::UnsignedShort => 2,
        VertexAttribType::UnsignedInt => 4,
    };
    let stride = if attribute.stride <= 0 {
        attribute.size.max(0) as usize * component_size
    } else {
        attribute.stride as usize
    };
    let base = attribute.offset_bytes as usize + vertex_index as usize * stride;
    let mut out = [0.0_f32; 3];
    for (component, item) in out.iter_mut().enumerate() {
        *item = read_attribute_component(&buffer.bytes, attribute, base, component)?;
    }
    Some(out)
}

fn read_vec2_attribute(
    state: &ScratchGlState,
    attribute: ScratchVertexAttribute,
    vertex_index: u32,
) -> Option<[f32; 2]> {
    let buffer = state.buffers.get(&attribute.buffer)?;
    let component_size = match attribute.attrib_type {
        VertexAttribType::Float32 => 4,
        VertexAttribType::UnsignedShort => 2,
        VertexAttribType::UnsignedInt => 4,
    };
    let stride = if attribute.stride <= 0 {
        attribute.size.max(0) as usize * component_size
    } else {
        attribute.stride as usize
    };
    let base = attribute.offset_bytes as usize + vertex_index as usize * stride;
    Some([
        read_attribute_component(&buffer.bytes, attribute, base, 0)?,
        read_attribute_component(&buffer.bytes, attribute, base, 1)?,
    ])
}

fn read_attribute_component(
    bytes: &[u8],
    attribute: ScratchVertexAttribute,
    base: usize,
    component: usize,
) -> Option<f32> {
    let offset = match attribute.attrib_type {
        VertexAttribType::Float32 => base + component * 4,
        VertexAttribType::UnsignedShort => base + component * 2,
        VertexAttribType::UnsignedInt => base + component * 4,
    };
    match attribute.attrib_type {
        VertexAttribType::Float32 => bytes
            .get(offset..offset + 4)
            .map(|slice| f32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]])),
        VertexAttribType::UnsignedShort => bytes.get(offset..offset + 2).map(|slice| {
            let value = u16::from_le_bytes([slice[0], slice[1]]) as f32;
            if attribute.normalized {
                value / 65535.0
            } else {
                value
            }
        }),
        VertexAttribType::UnsignedInt => bytes.get(offset..offset + 4).map(|slice| {
            let value = u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]) as f32;
            if attribute.normalized {
                value / u32::MAX as f32
            } else {
                value
            }
        }),
    }
}

fn classify_face(vertices: &[crate::scene::MeshVertex3D]) -> i32 {
    let mut normal = [0.0_f32; 3];
    for vertex in vertices {
        normal = add3(normal, vertex.normal);
    }
    let abs = [normal[0].abs(), normal[1].abs(), normal[2].abs()];
    if abs[0] >= abs[1] && abs[0] >= abs[2] {
        if normal[0] >= 0.0 { 1 } else { -1 }
    } else if abs[1] >= abs[2] {
        if normal[1] >= 0.0 { 2 } else { -2 }
    } else if normal[2] >= 0.0 {
        3
    } else {
        -3
    }
}

fn sample_group_color(texture: Option<&ScratchTexture2D>, uvs: &[[f32; 2]]) -> [f32; 4] {
    let Some(texture) = texture else {
        return [0.72, 0.66, 0.58, 1.0];
    };
    if uvs.is_empty() || texture.width == 0 || texture.height == 0 {
        return [0.72, 0.66, 0.58, 1.0];
    }
    let mut uv = [0.0_f32; 2];
    for value in uvs {
        uv[0] += value[0];
        uv[1] += value[1];
    }
    uv[0] /= uvs.len() as f32;
    uv[1] /= uvs.len() as f32;
    let x = ((uv[0].clamp(0.0, 0.9999)) * texture.width as f32) as usize;
    let y = ((1.0 - uv[1].clamp(0.0, 0.9999)) * texture.height as f32) as usize;
    let px = x.min(texture.width.saturating_sub(1) as usize);
    let py = y.min(texture.height.saturating_sub(1) as usize);
    let index = (py * texture.width as usize + px) * 4;
    if index + 3 >= texture.rgba8.len() {
        return [0.72, 0.66, 0.58, 1.0];
    }
    [
        texture.rgba8[index] as f32 / 255.0,
        texture.rgba8[index + 1] as f32 / 255.0,
        texture.rgba8[index + 2] as f32 / 255.0,
        texture.rgba8[index + 3] as f32 / 255.0,
    ]
}

fn compute_base_size(vertices: &[crate::scene::MeshVertex3D]) -> [f32; 3] {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for vertex in vertices {
        for axis in 0..3 {
            min[axis] = min[axis].min(vertex.position[axis]);
            max[axis] = max[axis].max(vertex.position[axis]);
        }
    }
    [
        (max[0] - min[0]).max(0.001),
        (max[1] - min[1]).max(0.001),
        (max[2] - min[2]).max(0.001),
    ]
}

fn normalize_preview_face_vertices(
    face_key: i32,
    vertices: &[crate::scene::MeshVertex3D],
) -> Vec<crate::scene::MeshVertex3D> {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for vertex in vertices {
        for axis in 0..3 {
            min[axis] = min[axis].min(vertex.position[axis]);
            max[axis] = max[axis].max(vertex.position[axis]);
        }
    }
    let center = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];
    let extents = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
    let locked_axis = match face_key.abs() {
        1 => Some((0, face_key.signum() as f32)),
        2 => Some((1, face_key.signum() as f32)),
        3 => Some((2, face_key.signum() as f32)),
        _ => None,
    };

    vertices
        .iter()
        .cloned()
        .map(|mut vertex| {
            for axis in 0..3 {
                if let Some((locked, sign)) = locked_axis {
                    if axis == locked {
                        vertex.position[axis] = sign;
                        continue;
                    }
                }
                vertex.position[axis] = if extents[axis] > 0.0001 {
                    ((vertex.position[axis] - center[axis]) / (extents[axis] * 0.5))
                        .clamp(-1.0, 1.0)
                } else {
                    0.0
                };
            }
            vertex
        })
        .collect()
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn int3_to_f32(value: [i32; 3]) -> [f32; 3] {
    [value[0] as f32, value[1] as f32, value[2] as f32]
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if length <= 0.0001 {
        [0.0, 0.0, 1.0]
    } else {
        [v[0] / length, v[1] / length, v[2] / length]
    }
}

fn scale3(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

fn face_normal_to_plane_rotation(normal: [i32; 3]) -> [f32; 3] {
    match normal {
        [1, 0, 0] | [-1, 0, 0] => [0.0, 0.0, std::f32::consts::FRAC_PI_2],
        [0, 0, 1] | [0, 0, -1] => [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
        _ => [0.0, 0.0, 0.0],
    }
}

fn ray_aabb_intersection(
    origin: [f32; 3],
    direction: [f32; 3],
    cell: [i32; 3],
) -> Option<(f32, [i32; 3])> {
    let min = [cell[0] as f32 - 0.5, cell[1] as f32, cell[2] as f32 - 0.5];
    let max = [
        cell[0] as f32 + 0.5,
        cell[1] as f32 + 1.0,
        cell[2] as f32 + 0.5,
    ];
    let mut t_min = f32::NEG_INFINITY;
    let mut t_max = f32::INFINITY;
    let mut hit_normal = [0, 0, 0];

    for axis in 0..3 {
        if direction[axis].abs() < 0.0001 {
            if origin[axis] < min[axis] || origin[axis] > max[axis] {
                return None;
            }
            continue;
        }

        let inv = 1.0 / direction[axis];
        let t1 = (min[axis] - origin[axis]) * inv;
        let t2 = (max[axis] - origin[axis]) * inv;
        let (near_t, far_t, near_normal) = if t1 <= t2 {
            (t1, t2, axis_normal(axis, -1))
        } else {
            (t2, t1, axis_normal(axis, 1))
        };

        if near_t > t_min {
            t_min = near_t;
            hit_normal = near_normal;
        }
        t_max = t_max.min(far_t);
        if t_min > t_max {
            return None;
        }
    }

    (t_min >= 0.0).then_some((t_min, hit_normal))
}

fn axis_normal(axis: usize, direction: i32) -> [i32; 3] {
    match axis {
        0 => [direction, 0, 0],
        1 => [0, direction, 0],
        _ => [0, 0, direction],
    }
}

fn decode_clear_flags(bits: u32) -> crate::syntax::ClearFlags {
    let mut flags = crate::syntax::ClearFlags::empty();
    if bits & NU_FFI_CLEAR_COLOR_BIT != 0 {
        flags |= crate::syntax::CLEAR_COLOR;
    }
    if bits & NU_FFI_CLEAR_DEPTH_BIT != 0 {
        flags |= crate::syntax::CLEAR_DEPTH;
    }
    if bits & NU_FFI_CLEAR_STENCIL_BIT != 0 {
        flags |= crate::syntax::CLEAR_STENCIL;
    }
    flags
}

fn decode_render_state(flag: u32) -> Option<RenderStateFlags> {
    match flag {
        NU_FFI_RENDER_STATE_DEPTH_TEST => Some(crate::syntax::DEPTH_TEST),
        NU_FFI_RENDER_STATE_BLEND => Some(crate::syntax::BLEND),
        NU_FFI_RENDER_STATE_CULL_FACE => Some(crate::syntax::CULL_FACE),
        _ => None,
    }
}

fn decode_buffer_target(value: u32) -> Option<BufferTarget> {
    match value {
        NU_FFI_BUFFER_TARGET_ARRAY => Some(BufferTarget::Array),
        NU_FFI_BUFFER_TARGET_ELEMENT_ARRAY => Some(BufferTarget::ElementArray),
        NU_FFI_BUFFER_TARGET_UNIFORM => Some(BufferTarget::Uniform),
        _ => None,
    }
}

fn decode_buffer_usage(value: u32) -> Option<BufferUsage> {
    match value {
        NU_FFI_BUFFER_USAGE_STATIC_DRAW => Some(BufferUsage::StaticDraw),
        NU_FFI_BUFFER_USAGE_DYNAMIC_DRAW => Some(BufferUsage::DynamicDraw),
        NU_FFI_BUFFER_USAGE_STREAM_DRAW => Some(BufferUsage::StreamDraw),
        _ => None,
    }
}

fn decode_vertex_attrib_type(value: u32) -> Option<VertexAttribType> {
    match value {
        NU_FFI_VERTEX_ATTRIB_FLOAT32 => Some(VertexAttribType::Float32),
        NU_FFI_VERTEX_ATTRIB_UNSIGNED_SHORT => Some(VertexAttribType::UnsignedShort),
        NU_FFI_VERTEX_ATTRIB_UNSIGNED_INT => Some(VertexAttribType::UnsignedInt),
        _ => None,
    }
}

fn decode_index_type(value: u32) -> Option<IndexType> {
    match value {
        NU_FFI_INDEX_TYPE_U16 => Some(IndexType::U16),
        NU_FFI_INDEX_TYPE_U32 => Some(IndexType::U32),
        _ => None,
    }
}

fn decode_topology(value: u32) -> Option<ash::vk::PrimitiveTopology> {
    match value {
        NU_FFI_TOPOLOGY_TRIANGLES => Some(ash::vk::PrimitiveTopology::TRIANGLE_LIST),
        NU_FFI_TOPOLOGY_LINES => Some(ash::vk::PrimitiveTopology::LINE_LIST),
        NU_FFI_TOPOLOGY_POINTS => Some(ash::vk::PrimitiveTopology::POINT_LIST),
        _ => None,
    }
}

fn decode_attachment(value: u32) -> Option<FramebufferAttachment> {
    match value {
        NU_FFI_ATTACHMENT_COLOR0 => Some(FramebufferAttachment::Color(0)),
        NU_FFI_ATTACHMENT_DEPTH => Some(FramebufferAttachment::Depth),
        NU_FFI_ATTACHMENT_DEPTH_STENCIL => Some(FramebufferAttachment::DepthStencil),
        _ => None,
    }
}

unsafe fn context_mut<'a>(ctx: *mut NuGlScratchContext) -> Option<&'a mut NuGlScratchContext> {
    if ctx.is_null() {
        None
    } else {
        Some(unsafe { &mut *ctx })
    }
}

fn read_c_string(name: *const c_char) -> Option<String> {
    if name.is_null() {
        return None;
    }
    Some(
        unsafe { CStr::from_ptr(name) }
            .to_string_lossy()
            .into_owned(),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn nu_ffi_gl_context_create() -> *mut NuGlScratchContext {
    Box::into_raw(Box::new(NuGlScratchContext::new()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_context_destroy(ctx: *mut NuGlScratchContext) {
    if !ctx.is_null() {
        unsafe { drop(Box::from_raw(ctx)) };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_context_reset(ctx: *mut NuGlScratchContext) {
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.reset_commands();
        ctx.scratch = ScratchGlState::default();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_command_count(ctx: *mut NuGlScratchContext) -> u64 {
    unsafe { context_mut(ctx) }
        .map(|ctx| ctx.inner.command_count() as u64)
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_preview_window(
    ctx: *mut NuGlScratchContext,
    title: *const c_char,
    width: u32,
    height: u32,
) -> bool {
    let Some(ctx) = (unsafe { context_mut(ctx) }) else {
        return false;
    };
    let title = read_c_string(title).unwrap_or_else(|| "nu C++ Scratch Preview".to_string());
    let Some(scene) = extract_preview_scene(ctx, title, width, height) else {
        return false;
    };
    match run_scene(scene) {
        Ok(()) => true,
        Err(err) => {
            eprintln!("nu_ffi_gl_preview_window error: {err}");
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_clear_color(
    ctx: *mut NuGlScratchContext,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
) {
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.clear_color(r, g, b, a);
        ctx.scratch.clear_color = [r, g, b, a];
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_clear(ctx: *mut NuGlScratchContext, flags: u32) {
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.clear(decode_clear_flags(flags));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_viewport(
    ctx: *mut NuGlScratchContext,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) {
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.viewport(x, y, width, height);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_enable(ctx: *mut NuGlScratchContext, state: u32) -> bool {
    let Some(flag) = decode_render_state(state) else {
        return false;
    };
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.enable(flag);
        return true;
    }
    false
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_disable(ctx: *mut NuGlScratchContext, state: u32) -> bool {
    let Some(flag) = decode_render_state(state) else {
        return false;
    };
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.disable(flag);
        return true;
    }
    false
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_use_program(ctx: *mut NuGlScratchContext, shader: u32) {
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.use_program(shader);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_bind_vertex_array(ctx: *mut NuGlScratchContext, mesh: u32) {
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.bind_vertex_array(mesh);
        ctx.scratch.current_vertex_array = mesh;
        ctx.scratch.vertex_arrays.entry(mesh).or_default();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_bind_framebuffer(
    ctx: *mut NuGlScratchContext,
    framebuffer: u32,
) {
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.bind_framebuffer(framebuffer);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_bind_buffer(
    ctx: *mut NuGlScratchContext,
    target: u32,
    buffer: u32,
) -> bool {
    let Some(target) = decode_buffer_target(target) else {
        return false;
    };
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.bind_buffer(target, buffer);
        match target {
            BufferTarget::Array => ctx.scratch.current_array_buffer = buffer,
            BufferTarget::ElementArray => {
                ctx.scratch
                    .vertex_arrays
                    .entry(ctx.scratch.current_vertex_array)
                    .or_default()
                    .element_buffer = buffer;
            }
            BufferTarget::Uniform => {}
        }
        return true;
    }
    false
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_bind_buffer_base(
    ctx: *mut NuGlScratchContext,
    target: u32,
    index: u32,
    buffer: u32,
) -> bool {
    let Some(target) = decode_buffer_target(target) else {
        return false;
    };
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.bind_buffer_base(target, index, buffer);
        return true;
    }
    false
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_active_texture(ctx: *mut NuGlScratchContext, slot: u32) {
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.active_texture(slot);
        ctx.scratch.active_texture_slot = slot;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_bind_texture_2d(ctx: *mut NuGlScratchContext, texture: u32) {
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.bind_texture_2d(texture);
        ctx.scratch
            .bound_textures
            .insert(ctx.scratch.active_texture_slot, texture);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_buffer_data(
    ctx: *mut NuGlScratchContext,
    target: u32,
    size_bytes: u64,
    data: *const u8,
    usage: u32,
) -> bool {
    let Some(target) = decode_buffer_target(target) else {
        return false;
    };
    let Some(usage) = decode_buffer_usage(usage) else {
        return false;
    };
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.buffer_data(target, size_bytes, (), usage);
        let bound_buffer = match target {
            BufferTarget::Array => ctx.scratch.current_array_buffer,
            BufferTarget::ElementArray => {
                ctx.scratch
                    .vertex_arrays
                    .entry(ctx.scratch.current_vertex_array)
                    .or_default()
                    .element_buffer
            }
            BufferTarget::Uniform => 0,
        };
        if bound_buffer != 0 {
            let bytes = if data.is_null() || size_bytes == 0 {
                vec![0_u8; size_bytes as usize]
            } else {
                unsafe { std::slice::from_raw_parts(data, size_bytes as usize) }.to_vec()
            };
            ctx.scratch
                .buffers
                .insert(bound_buffer, ScratchBufferData { bytes });
        }
        return true;
    }
    false
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_buffer_sub_data(
    ctx: *mut NuGlScratchContext,
    target: u32,
    offset_bytes: u64,
    size_bytes: u64,
    data: *const u8,
) -> bool {
    let Some(target) = decode_buffer_target(target) else {
        return false;
    };
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner
            .buffer_sub_data(target, offset_bytes, size_bytes, ());
        let bound_buffer = match target {
            BufferTarget::Array => ctx.scratch.current_array_buffer,
            BufferTarget::ElementArray => {
                ctx.scratch
                    .vertex_arrays
                    .entry(ctx.scratch.current_vertex_array)
                    .or_default()
                    .element_buffer
            }
            BufferTarget::Uniform => 0,
        };
        if bound_buffer != 0 {
            let entry = ctx
                .scratch
                .buffers
                .entry(bound_buffer)
                .or_insert(ScratchBufferData { bytes: Vec::new() });
            let end = offset_bytes as usize + size_bytes as usize;
            if entry.bytes.len() < end {
                entry.bytes.resize(end, 0);
            }
            if !data.is_null() && size_bytes > 0 {
                let source = unsafe { std::slice::from_raw_parts(data, size_bytes as usize) };
                entry.bytes[offset_bytes as usize..end].copy_from_slice(source);
            }
        }
        return true;
    }
    false
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_vertex_attrib_pointer(
    ctx: *mut NuGlScratchContext,
    index: u32,
    size: i32,
    attrib_type: u32,
    normalized: bool,
    stride: i32,
    offset_bytes: u64,
) -> bool {
    let Some(attrib_type) = decode_vertex_attrib_type(attrib_type) else {
        return false;
    };
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner
            .vertex_attrib_pointer(index, size, attrib_type, normalized, stride, offset_bytes);
        ctx.scratch
            .vertex_arrays
            .entry(ctx.scratch.current_vertex_array)
            .or_default()
            .attributes
            .insert(
                index,
                ScratchVertexAttribute {
                    size,
                    attrib_type,
                    normalized,
                    stride,
                    offset_bytes,
                    buffer: ctx.scratch.current_array_buffer,
                    enabled: false,
                },
            );
        return true;
    }
    false
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_enable_vertex_attrib_array(
    ctx: *mut NuGlScratchContext,
    index: u32,
) {
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.enable_vertex_attrib_array(index);
        if let Some(attribute) = ctx
            .scratch
            .vertex_arrays
            .entry(ctx.scratch.current_vertex_array)
            .or_default()
            .attributes
            .get_mut(&index)
        {
            attribute.enabled = true;
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_disable_vertex_attrib_array(
    ctx: *mut NuGlScratchContext,
    index: u32,
) {
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.disable_vertex_attrib_array(index);
        if let Some(attribute) = ctx
            .scratch
            .vertex_arrays
            .entry(ctx.scratch.current_vertex_array)
            .or_default()
            .attributes
            .get_mut(&index)
        {
            attribute.enabled = false;
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_vertex_attrib_divisor(
    ctx: *mut NuGlScratchContext,
    index: u32,
    divisor: u32,
) {
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.vertex_attrib_divisor(index, divisor);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_framebuffer_texture_2d(
    ctx: *mut NuGlScratchContext,
    attachment: u32,
    texture: u32,
    level: i32,
) -> bool {
    let Some(attachment) = decode_attachment(attachment) else {
        return false;
    };
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.framebuffer_texture_2d(attachment, texture, level);
        return true;
    }
    false
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_tex_image_2d_rgba8(
    ctx: *mut NuGlScratchContext,
    texture: u32,
    width: u32,
    height: u32,
    pixels: *const u8,
) -> bool {
    if width == 0 || height == 0 || pixels.is_null() {
        return false;
    }
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        let byte_count = width as usize * height as usize * 4;
        let rgba8 = unsafe { std::slice::from_raw_parts(pixels, byte_count) }.to_vec();
        ctx.scratch.textures.insert(
            texture,
            ScratchTexture2D {
                width,
                height,
                rgba8,
            },
        );
        return true;
    }
    false
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_framebuffer_renderbuffer(
    ctx: *mut NuGlScratchContext,
    attachment: u32,
    renderbuffer: u32,
) -> bool {
    let Some(attachment) = decode_attachment(attachment) else {
        return false;
    };
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner
            .framebuffer_renderbuffer(attachment, RenderbufferHandle(renderbuffer));
        return true;
    }
    false
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_draw_arrays(
    ctx: *mut NuGlScratchContext,
    topology: u32,
    first: u32,
    count: u32,
) -> bool {
    let Some(topology) = decode_topology(topology) else {
        return false;
    };
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.draw_arrays(topology, first, count);
        ctx.scratch.last_draw = Some(ScratchDrawCall {
            topology,
            first_vertex: first,
            vertex_count: count,
            index_count: 0,
            index_type: None,
            index_offset_bytes: 0,
        });
        return true;
    }
    false
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_draw_elements(
    ctx: *mut NuGlScratchContext,
    topology: u32,
    count: u32,
    index_type: u32,
    offset_bytes: u64,
) -> bool {
    let Some(topology) = decode_topology(topology) else {
        return false;
    };
    let Some(index_type) = decode_index_type(index_type) else {
        return false;
    };
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner
            .draw_elements_typed(topology, count, index_type, offset_bytes);
        ctx.scratch.last_draw = Some(ScratchDrawCall {
            topology,
            first_vertex: 0,
            vertex_count: 0,
            index_count: count,
            index_type: Some(index_type),
            index_offset_bytes: offset_bytes,
        });
        return true;
    }
    false
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_uniform_mat4(
    ctx: *mut NuGlScratchContext,
    name: *const c_char,
    values: *const f32,
) -> bool {
    let Some(name) = read_c_string(name) else {
        return false;
    };
    if values.is_null() {
        return false;
    }
    let value_slice = unsafe { std::slice::from_raw_parts(values, 16) };
    let mut matrix = [[0.0_f32; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            matrix[row][col] = value_slice[row * 4 + col];
        }
    }
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.set_uniform_mat4(&name, matrix);
        return true;
    }
    false
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_uniform_vec3(
    ctx: *mut NuGlScratchContext,
    name: *const c_char,
    x: f32,
    y: f32,
    z: f32,
) -> bool {
    let Some(name) = read_c_string(name) else {
        return false;
    };
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        ctx.inner.set_uniform_vec3(&name, [x, y, z]);
        if name == "u_sunDirection" {
            ctx.scratch.sun_direction = [x, y, z];
        }
        return true;
    }
    false
}

unsafe fn write_generated_ids(
    ids: *mut u32,
    count: u32,
    fill: impl FnOnce(&mut BackendContext, u32, &mut [u32]),
    ctx: *mut NuGlScratchContext,
) -> u32 {
    if ids.is_null() || count == 0 {
        return 0;
    }
    let Some(ctx) = (unsafe { context_mut(ctx) }) else {
        return 0;
    };
    let ids_slice = unsafe { std::slice::from_raw_parts_mut(ids, count as usize) };
    fill(&mut ctx.inner, count, ids_slice);
    count
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_gen_buffers(
    ctx: *mut NuGlScratchContext,
    count: u32,
    ids: *mut u32,
) -> u32 {
    unsafe {
        write_generated_ids(
            ids,
            count,
            |inner, n, slice| inner.gen_buffers(n, slice),
            ctx,
        )
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_gen_textures(
    ctx: *mut NuGlScratchContext,
    count: u32,
    ids: *mut u32,
) -> u32 {
    unsafe {
        write_generated_ids(
            ids,
            count,
            |inner, n, slice| inner.gen_textures(n, slice),
            ctx,
        )
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_gen_vertex_arrays(
    ctx: *mut NuGlScratchContext,
    count: u32,
    ids: *mut u32,
) -> u32 {
    unsafe {
        write_generated_ids(
            ids,
            count,
            |inner, n, slice| inner.gen_vertex_arrays(n, slice),
            ctx,
        )
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_gen_framebuffers(
    ctx: *mut NuGlScratchContext,
    count: u32,
    ids: *mut u32,
) -> u32 {
    unsafe {
        write_generated_ids(
            ids,
            count,
            |inner, n, slice| inner.gen_framebuffers(n, slice),
            ctx,
        )
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_gen_renderbuffers(
    ctx: *mut NuGlScratchContext,
    count: u32,
    ids: *mut u32,
) -> u32 {
    unsafe {
        write_generated_ids(
            ids,
            count,
            |inner, n, slice| inner.gen_renderbuffers(n, slice),
            ctx,
        )
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_delete_buffers(
    ctx: *mut NuGlScratchContext,
    count: u32,
    ids: *const u32,
) {
    if ids.is_null() || count == 0 {
        return;
    }
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        let ids_slice = unsafe { std::slice::from_raw_parts(ids, count as usize) };
        ctx.inner.delete_buffers(count, ids_slice);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_delete_textures(
    ctx: *mut NuGlScratchContext,
    count: u32,
    ids: *const u32,
) {
    if ids.is_null() || count == 0 {
        return;
    }
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        let ids_slice = unsafe { std::slice::from_raw_parts(ids, count as usize) };
        ctx.inner.delete_textures(count, ids_slice);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_delete_vertex_arrays(
    ctx: *mut NuGlScratchContext,
    count: u32,
    ids: *const u32,
) {
    if ids.is_null() || count == 0 {
        return;
    }
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        let ids_slice = unsafe { std::slice::from_raw_parts(ids, count as usize) };
        ctx.inner.delete_vertex_arrays(count, ids_slice);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_delete_framebuffers(
    ctx: *mut NuGlScratchContext,
    count: u32,
    ids: *const u32,
) {
    if ids.is_null() || count == 0 {
        return;
    }
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        let ids_slice = unsafe { std::slice::from_raw_parts(ids, count as usize) };
        ctx.inner.delete_framebuffers(count, ids_slice);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nu_ffi_gl_delete_renderbuffers(
    ctx: *mut NuGlScratchContext,
    count: u32,
    ids: *const u32,
) {
    if ids.is_null() || count == 0 {
        return;
    }
    if let Some(ctx) = unsafe { context_mut(ctx) } {
        let ids_slice = unsafe { std::slice::from_raw_parts(ids, count as usize) };
        ctx.inner.delete_renderbuffers(count, ids_slice);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_gl_context_records_basic_commands() {
        let ctx = nu_ffi_gl_context_create();
        unsafe {
            nu_ffi_gl_clear_color(ctx, 0.1, 0.2, 0.3, 1.0);
            nu_ffi_gl_clear(ctx, NU_FFI_CLEAR_COLOR_BIT | NU_FFI_CLEAR_DEPTH_BIT);
            assert_eq!(nu_ffi_gl_command_count(ctx), 2);
            nu_ffi_gl_context_reset(ctx);
            assert_eq!(nu_ffi_gl_command_count(ctx), 0);
            nu_ffi_gl_context_destroy(ctx);
        }
    }

    #[test]
    fn ffi_gl_context_accepts_draw_commands() {
        let ctx = nu_ffi_gl_context_create();
        unsafe {
            assert!(nu_ffi_gl_bind_buffer(ctx, NU_FFI_BUFFER_TARGET_ARRAY, 7));
            assert!(nu_ffi_gl_draw_arrays(ctx, NU_FFI_TOPOLOGY_TRIANGLES, 0, 36));
            assert_eq!(nu_ffi_gl_command_count(ctx), 2);
            nu_ffi_gl_context_destroy(ctx);
        }
    }

    #[test]
    fn scratch_preview_scene_extracts_basic_render_state() {
        let ctx = nu_ffi_gl_context_create();
        unsafe {
            let vertices: [f32; 9] = [
                -0.5, 0.0, 0.0, //
                0.5, 0.0, 0.0, //
                0.0, 1.0, 0.0,
            ];
            nu_ffi_gl_clear_color(ctx, 0.2, 0.3, 0.4, 1.0);
            nu_ffi_gl_bind_vertex_array(ctx, 1);
            assert!(nu_ffi_gl_bind_buffer(ctx, NU_FFI_BUFFER_TARGET_ARRAY, 2));
            assert!(nu_ffi_gl_buffer_data(
                ctx,
                NU_FFI_BUFFER_TARGET_ARRAY,
                std::mem::size_of_val(&vertices) as u64,
                vertices.as_ptr() as *const u8,
                NU_FFI_BUFFER_USAGE_STATIC_DRAW
            ));
            assert!(nu_ffi_gl_vertex_attrib_pointer(
                ctx,
                0,
                3,
                NU_FFI_VERTEX_ATTRIB_FLOAT32,
                false,
                12,
                0
            ));
            nu_ffi_gl_enable_vertex_attrib_array(ctx, 0);
            assert!(nu_ffi_gl_uniform_vec3(
                ctx,
                c"u_sunDirection".as_ptr(),
                -0.2,
                0.9,
                -0.3
            ));
            assert!(nu_ffi_gl_draw_arrays(ctx, NU_FFI_TOPOLOGY_TRIANGLES, 0, 3));

            let ctx_ref = context_mut(ctx).expect("context should exist");
            let preview =
                extract_preview_scene(ctx_ref, "nu C++ Scratch Preview".to_string(), 1280, 720)
                    .expect("draw commands should produce a preview scene");
            assert_eq!(preview.clear_color, [0.2, 0.3, 0.4, 1.0]);
            assert_eq!(preview.sun_direction, [-0.2, 0.9, -0.3]);
            assert_eq!(preview.width, 1280);
            assert_eq!(preview.height, 720);

            nu_ffi_gl_context_destroy(ctx);
        }
    }
}
