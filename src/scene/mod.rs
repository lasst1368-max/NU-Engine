pub mod primitives;
pub mod sculpt;

use crate::app::WindowConfig;
use crate::core::ApiConfig;
use crate::lighting::LightingConfig;
use crate::syntax::TextureHandle;
use std::path::PathBuf;
use std::sync::Arc;
use winit::event::WindowEvent;
use winit::window::Window;

#[derive(Debug, Clone)]
pub struct SceneConfig {
    pub window: WindowConfig,
    pub api: ApiConfig,
    pub clear_color: [f32; 4],
    pub camera: Camera2D,
    pub camera_3d: Camera3D,
    pub lighting: LightingConfig,
    pub screenshot_path: Option<PathBuf>,
    pub screenshot_accumulation_samples: u32,
    pub screenshot_resolution: ScreenshotResolution,
    pub capture_cursor: bool,
}

impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            window: WindowConfig::default(),
            api: ApiConfig::default(),
            clear_color: [0.05, 0.05, 0.07, 1.0],
            camera: Camera2D::default(),
            camera_3d: Camera3D::default(),
            lighting: LightingConfig::default(),
            screenshot_path: None,
            screenshot_accumulation_samples: 1,
            screenshot_resolution: ScreenshotResolution::K4,
            capture_cursor: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenshotResolution {
    K2,
    K4,
    K8,
    K16,
}

impl ScreenshotResolution {
    pub fn extent(self) -> [u32; 2] {
        match self {
            Self::K2 => [2560, 1440],
            Self::K4 => [3840, 2160],
            Self::K8 => [7680, 4320],
            Self::K16 => [15360, 8640],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Camera2D {
    pub center: [f32; 2],
    pub view_height: f32,
}

impl Camera2D {
    pub fn new(center: [f32; 2], view_height: f32) -> Self {
        Self {
            center,
            view_height: view_height.max(0.0001),
        }
    }

    pub fn view_width(self, aspect_ratio: f32) -> f32 {
        self.view_height * aspect_ratio.max(0.0001)
    }

    pub fn pan(&mut self, delta: [f32; 2]) {
        self.center[0] += delta[0];
        self.center[1] += delta[1];
    }

    pub fn zoom(&mut self, zoom_factor: f32) {
        self.view_height = (self.view_height / zoom_factor.max(0.0001)).max(0.0001);
    }

    pub fn world_to_ndc(self, point: [f32; 2], aspect_ratio: f32) -> [f32; 2] {
        let view_width = self.view_width(aspect_ratio);
        [
            ((point[0] - self.center[0]) * 2.0) / view_width,
            ((point[1] - self.center[1]) * 2.0) / self.view_height,
        ]
    }

    pub fn ndc_to_world(self, point: [f32; 2], aspect_ratio: f32) -> [f32; 2] {
        let half_width = self.view_width(aspect_ratio) * 0.5;
        let half_height = self.view_height * 0.5;
        [
            self.center[0] + point[0] * half_width,
            self.center[1] + point[1] * half_height,
        ]
    }
}

impl Default for Camera2D {
    fn default() -> Self {
        Self::new([0.0, 0.0], 2.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Camera3D {
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub fov_y_degrees: f32,
    pub near_clip: f32,
    pub far_clip: f32,
}

impl Default for Camera3D {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, -3.5],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_degrees: 55.0,
            near_clip: 0.1,
            far_clip: 100.0,
        }
    }
}

impl Camera3D {
    /// Unit vector pointing from the camera toward the target.
    pub fn forward(&self) -> [f32; 3] {
        cam3_normalize(cam3_sub(self.target, self.position))
    }

    /// Unit vector pointing to the camera's right (perpendicular to forward and up).
    pub fn right(&self) -> [f32; 3] {
        cam3_normalize(cam3_cross(self.forward(), self.up))
    }

    /// Row-major view matrix (Vulkan convention: right-handed, Y-down clip space).
    /// Rows are [right, up_corrected, -forward] with translation baked into the last row.
    pub fn view_matrix(&self) -> [[f32; 4]; 4] {
        let f = cam3_normalize(cam3_sub(self.target, self.position));
        let r = cam3_normalize(cam3_cross(f, self.up));
        let u = cam3_cross(r, f);
        [
            [r[0], u[0], -f[0], 0.0],
            [r[1], u[1], -f[1], 0.0],
            [r[2], u[2], -f[2], 0.0],
            [
                -cam3_dot(r, self.position),
                -cam3_dot(u, self.position),
                cam3_dot(f, self.position),
                1.0,
            ],
        ]
    }

    /// Row-major perspective projection matrix (Vulkan clip space: Z in [0, 1], Y-down).
    pub fn projection_matrix(&self, aspect_ratio: f32) -> [[f32; 4]; 4] {
        let fov_rad = self.fov_y_degrees.to_radians();
        let tan_half = (fov_rad * 0.5).tan();
        let near = self.near_clip;
        let far = self.far_clip;
        let range = near - far;
        [
            [1.0 / (aspect_ratio * tan_half), 0.0, 0.0, 0.0],
            [0.0, -1.0 / tan_half, 0.0, 0.0], // Y-flip for Vulkan
            [0.0, 0.0, far / range, -1.0],
            [0.0, 0.0, (near * far) / range, 0.0],
        ]
    }

    /// Orbit (tumble) the camera around the target by `yaw_delta_degrees` (horizontal)
    /// and `pitch_delta_degrees` (vertical). Pitch is clamped to avoid gimbal lock.
    pub fn orbit(&mut self, yaw_delta_degrees: f32, pitch_delta_degrees: f32) {
        let dir = cam3_sub(self.position, self.target);
        let radius = cam3_length(dir).max(0.0001);
        let current_yaw = dir[2].atan2(dir[0]);
        let current_pitch = (dir[1] / radius).clamp(-1.0, 1.0).asin();
        let new_yaw = current_yaw + yaw_delta_degrees.to_radians();
        let new_pitch = (current_pitch + pitch_delta_degrees.to_radians())
            .clamp(
                -std::f32::consts::FRAC_PI_2 + 0.02,
                std::f32::consts::FRAC_PI_2 - 0.02,
            );
        let cos_pitch = new_pitch.cos();
        self.position = [
            self.target[0] + radius * cos_pitch * new_yaw.cos(),
            self.target[1] + radius * new_pitch.sin(),
            self.target[2] + radius * cos_pitch * new_yaw.sin(),
        ];
    }

    /// Move the camera along the view axis (positive = toward target).
    /// The target stays fixed; the camera approaches or retreats.
    pub fn dolly(&mut self, distance: f32) {
        let dir = cam3_normalize(cam3_sub(self.target, self.position));
        self.position = cam3_add(self.position, cam3_scale(dir, distance));
    }

    /// Truck the camera (and target) sideways and vertically without changing orientation.
    pub fn strafe(&mut self, right_delta: f32, up_delta: f32) {
        let r = self.right();
        let u = self.up;
        let delta = cam3_add(cam3_scale(r, right_delta), cam3_scale(u, up_delta));
        self.position = cam3_add(self.position, delta);
        self.target = cam3_add(self.target, delta);
    }

    /// Narrow or widen the field of view (zoom in/out). `factor > 1` zooms in.
    /// Field of view is clamped to [5°, 150°].
    pub fn zoom(&mut self, factor: f32) {
        self.fov_y_degrees = (self.fov_y_degrees / factor.max(0.01)).clamp(5.0, 150.0);
    }

    /// Compute a `Frustum` from the current camera state given a viewport aspect ratio.
    /// Useful for CPU-side frustum culling before issuing draw calls.
    pub fn frustum(&self, aspect_ratio: f32) -> Frustum {
        Frustum::from_camera(self, aspect_ratio)
    }
}

/// Six-plane view frustum for CPU-side visibility culling.
/// Planes are stored as `[a, b, c, d]` (normal xyz + offset d) in world space,
/// where a point P is *inside* the frustum if `dot(plane.xyz, P) + d >= 0` for all planes.
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    planes: [[f32; 4]; 6],
}

impl Frustum {
    /// Build a frustum directly from a view-projection matrix (row-major, as returned by
    /// `Camera3D::view_matrix()` × `projection_matrix()`).
    pub fn from_view_proj(vp: [[f32; 4]; 4]) -> Self {
        // Gribb-Hartmann extraction from a row-major VP matrix.
        let p = |row: usize, col: usize| vp[row][col];
        let plane = |a: f32, b: f32, c: f32, d: f32| -> [f32; 4] {
            let len = (a * a + b * b + c * c).sqrt().max(0.0001);
            [a / len, b / len, c / len, d / len]
        };
        Self {
            planes: [
                // Left:   col3 + col0
                plane(p(0,3)+p(0,0), p(1,3)+p(1,0), p(2,3)+p(2,0), p(3,3)+p(3,0)),
                // Right:  col3 - col0
                plane(p(0,3)-p(0,0), p(1,3)-p(1,0), p(2,3)-p(2,0), p(3,3)-p(3,0)),
                // Bottom: col3 + col1
                plane(p(0,3)+p(0,1), p(1,3)+p(1,1), p(2,3)+p(2,1), p(3,3)+p(3,1)),
                // Top:    col3 - col1
                plane(p(0,3)-p(0,1), p(1,3)-p(1,1), p(2,3)-p(2,1), p(3,3)-p(3,1)),
                // Near:   col3 + col2
                plane(p(0,3)+p(0,2), p(1,3)+p(1,2), p(2,3)+p(2,2), p(3,3)+p(3,2)),
                // Far:    col3 - col2
                plane(p(0,3)-p(0,2), p(1,3)-p(1,2), p(2,3)-p(2,2), p(3,3)-p(3,2)),
            ],
        }
    }

    pub fn from_camera(camera: &Camera3D, aspect_ratio: f32) -> Self {
        let view = camera.view_matrix();
        let proj = camera.projection_matrix(aspect_ratio);
        let vp = mat4_mul(proj, view);
        Self::from_view_proj(vp)
    }

    /// Returns `true` if the sphere (center + radius) intersects or is inside the frustum.
    pub fn test_sphere(&self, center: [f32; 3], radius: f32) -> bool {
        for plane in &self.planes {
            let dist = plane[0] * center[0]
                + plane[1] * center[1]
                + plane[2] * center[2]
                + plane[3];
            if dist < -radius {
                return false;
            }
        }
        true
    }

    /// Returns `true` if the AABB (min/max corners) intersects or is inside the frustum.
    /// Uses the positive-vertex (p-vertex) test for each plane.
    pub fn test_aabb(&self, min: [f32; 3], max: [f32; 3]) -> bool {
        for plane in &self.planes {
            // Pick the corner most in the direction of the plane normal (p-vertex).
            let px = if plane[0] >= 0.0 { max[0] } else { min[0] };
            let py = if plane[1] >= 0.0 { max[1] } else { min[1] };
            let pz = if plane[2] >= 0.0 { max[2] } else { min[2] };
            if plane[0] * px + plane[1] * py + plane[2] * pz + plane[3] < 0.0 {
                return false;
            }
        }
        true
    }
}

// ── Private math helpers for Camera3D / Frustum ─────────────────────────────

fn cam3_add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn cam3_sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cam3_scale(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

fn cam3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cam3_length(v: [f32; 3]) -> f32 {
    cam3_dot(v, v).sqrt()
}

fn cam3_normalize(v: [f32; 3]) -> [f32; 3] {
    let len = cam3_length(v).max(0.0001);
    [v[0] / len, v[1] / len, v[2] / len]
}

fn cam3_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Multiply two row-major 4×4 matrices: result = lhs × rhs.
fn mat4_mul(lhs: [[f32; 4]; 4], rhs: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0f32; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            for k in 0..4 {
                out[row][col] += lhs[row][k] * rhs[k][col];
            }
        }
    }
    out
}

pub trait Scene {
    fn config(&self) -> SceneConfig;

    fn update(&mut self, _delta_time_seconds: f32) {}

    fn window_event(&mut self, _window: &Window, _event: &WindowEvent) {}

    fn populate(&mut self, frame: &mut SceneFrame);
}

#[derive(Debug, Clone, Copy)]
pub enum ShapeStyle {
    Fill,
    Stroke { width: f32 },
}

impl Default for ShapeStyle {
    fn default() -> Self {
        Self::Fill
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawSpace {
    World,
    Screen,
}

impl Default for DrawSpace {
    fn default() -> Self {
        Self::World
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RectDraw {
    pub center: [f32; 2],
    pub size: [f32; 2],
    pub rotation_radians: f32,
    pub color: [f32; 4],
    pub layer: i32,
    pub style: ShapeStyle,
    pub space: DrawSpace,
}

impl Default for RectDraw {
    fn default() -> Self {
        Self {
            center: [0.0, 0.0],
            size: [0.5, 0.5],
            rotation_radians: 0.0,
            color: [1.0, 0.0, 0.0, 1.0],
            layer: 0,
            style: ShapeStyle::Fill,
            space: DrawSpace::World,
        }
    }
}

pub type SquareDraw = RectDraw;

#[derive(Debug, Clone, Copy)]
pub struct CircleDraw {
    pub center: [f32; 2],
    pub radius: f32,
    pub color: [f32; 4],
    pub layer: i32,
    pub style: ShapeStyle,
    pub space: DrawSpace,
}

impl Default for CircleDraw {
    fn default() -> Self {
        Self {
            center: [0.0, 0.0],
            radius: 0.25,
            color: [1.0, 1.0, 1.0, 1.0],
            layer: 0,
            style: ShapeStyle::Fill,
            space: DrawSpace::World,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LineDraw {
    pub start: [f32; 2],
    pub end: [f32; 2],
    pub thickness: f32,
    pub color: [f32; 4],
    pub layer: i32,
    pub space: DrawSpace,
}

impl Default for LineDraw {
    fn default() -> Self {
        Self {
            start: [-0.5, 0.0],
            end: [0.5, 0.0],
            thickness: 0.05,
            color: [1.0, 1.0, 1.0, 1.0],
            layer: 0,
            space: DrawSpace::World,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct QuadDraw {
    pub points: [[f32; 2]; 4],
    pub color: [f32; 4],
    pub layer: i32,
    pub space: DrawSpace,
}

impl Default for QuadDraw {
    fn default() -> Self {
        Self {
            points: [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]],
            color: [1.0, 1.0, 1.0, 1.0],
            layer: 0,
            space: DrawSpace::World,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAnchor {
    TopLeft,
    Center,
}

impl Default for TextAnchor {
    fn default() -> Self {
        Self::TopLeft
    }
}

#[derive(Debug, Clone)]
pub struct TextDraw {
    pub position: [f32; 2],
    pub text: String,
    pub pixel_size: f32,
    pub color: [f32; 4],
    pub layer: i32,
    pub space: DrawSpace,
    pub anchor: TextAnchor,
}

impl Default for TextDraw {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            text: String::new(),
            pixel_size: 16.0,
            color: [1.0, 1.0, 1.0, 1.0],
            layer: 0,
            space: DrawSpace::Screen,
            anchor: TextAnchor::TopLeft,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MeshVertex3D {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    /// Tangent vector in local space. The `w` component encodes bitangent handedness
    /// (-1.0 or +1.0). Supply `[1.0, 0.0, 0.0, 1.0]` when no tangent data is available.
    pub tangent: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct MeshAsset3D {
    pub name: String,
    pub vertices: Arc<[MeshVertex3D]>,
    pub base_size: [f32; 3],
}

#[derive(Debug, Clone)]
pub enum Mesh3D {
    // ── Built-in primitives ─────────────────────────────────────────────
    Cube,
    Plane,
    Sphere,
    /// Cylinder aligned on the Y-axis with solid end caps.
    /// Fields: `(radial_segments, height_segments)` — defaults `(24, 1)` are good for most uses.
    Cylinder { radial_segments: u32, height_segments: u32 },
    /// Torus (donut) lying in the XZ-plane.
    /// Fields: `(major_segments, minor_segments)` — defaults `(32, 16)`.
    Torus { major_segments: u32, minor_segments: u32 },
    /// Cone with base at y=-1 and apex at y=+1.
    /// Fields: `(radial_segments, height_segments)`.
    Cone { radial_segments: u32, height_segments: u32 },
    /// Capsule — cylinder body with hemispherical caps.
    /// Fields: `(radial_segments, cap_segments)`.
    Capsule { radial_segments: u32, cap_segments: u32 },
    /// Icosphere — smooth sphere via icosahedron subdivision.
    /// `subdivisions = 0` → raw icosahedron (20 triangles); each step quadruples triangle count.
    Icosphere { subdivisions: u32 },
    // ── User-supplied ───────────────────────────────────────────────────
    Custom(Arc<MeshAsset3D>),
}

impl Mesh3D {
    /// Returns a cylinder with sensible defaults.
    pub fn cylinder() -> Self {
        Self::Cylinder { radial_segments: 24, height_segments: 1 }
    }

    /// Returns a torus with sensible defaults.
    pub fn torus() -> Self {
        Self::Torus { major_segments: 32, minor_segments: 16 }
    }

    /// Returns a cone with sensible defaults.
    pub fn cone() -> Self {
        Self::Cone { radial_segments: 24, height_segments: 1 }
    }

    /// Returns a capsule with sensible defaults.
    pub fn capsule() -> Self {
        Self::Capsule { radial_segments: 16, cap_segments: 8 }
    }

    /// Returns a smooth icosphere (subdivision level 3 ≈ 1280 triangles).
    pub fn icosphere() -> Self {
        Self::Icosphere { subdivisions: 3 }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MeshMaterial3D {
    pub albedo_texture: Option<TextureHandle>,
    /// Optional tangent-space normal map. When `Some`, `draw_material.w` is set to 1.0
    /// so the GPU shader samples `set=3` for per-pixel normals.
    pub normal_texture: Option<TextureHandle>,
    pub roughness: f32,
    pub metallic: f32,
    /// Emissive glow multiplier sent to the GPU as `draw_material.z`.
    /// 0.0 = no emission. Values above 1.0 produce HDR glow (tonemapped by ACES).
    pub emissive_intensity: f32,
}

impl Default for MeshMaterial3D {
    fn default() -> Self {
        Self {
            albedo_texture: None,
            normal_texture: None,
            roughness: 0.5,
            metallic: 0.0,
            emissive_intensity: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MeshDraw3D {
    pub mesh: Mesh3D,
    pub center: [f32; 3],
    pub size: [f32; 3],
    pub rotation_radians: [f32; 3],
    pub color: [f32; 4],
    pub material: MeshMaterial3D,
}

impl Default for MeshDraw3D {
    fn default() -> Self {
        Self {
            mesh: Mesh3D::Cube,
            center: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_radians: [0.0, 0.0, 0.0],
            color: [0.95, 0.05, 0.05, 1.0],
            material: MeshMaterial3D::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CubeDraw3D {
    pub center: [f32; 3],
    pub size: [f32; 3],
    pub rotation_radians: [f32; 3],
    pub color: [f32; 4],
    pub material: MeshMaterial3D,
}

impl Default for CubeDraw3D {
    fn default() -> Self {
        Self {
            center: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_radians: [0.0, 0.0, 0.0],
            color: [0.95, 0.05, 0.05, 1.0],
            material: MeshMaterial3D::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SphereDraw3D {
    pub center: [f32; 3],
    pub diameter: f32,
    pub color: [f32; 4],
    pub material: MeshMaterial3D,
}

impl Default for SphereDraw3D {
    fn default() -> Self {
        Self {
            center: [0.0, 0.0, 0.0],
            diameter: 1.0,
            color: [0.95, 0.05, 0.05, 1.0],
            material: MeshMaterial3D::default(),
        }
    }
}

impl From<CubeDraw3D> for MeshDraw3D {
    fn from(value: CubeDraw3D) -> Self {
        Self {
            mesh: Mesh3D::Cube,
            center: value.center,
            size: value.size,
            rotation_radians: value.rotation_radians,
            color: value.color,
            material: value.material,
        }
    }
}

impl From<SphereDraw3D> for MeshDraw3D {
    fn from(value: SphereDraw3D) -> Self {
        Self {
            mesh: Mesh3D::Sphere,
            center: value.center,
            size: [value.diameter, value.diameter, value.diameter],
            rotation_radians: [0.0, 0.0, 0.0],
            color: value.color,
            material: value.material,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PrimitiveDraw {
    Rect(RectDraw),
    Circle(CircleDraw),
    Line(LineDraw),
    Quad(QuadDraw),
    Text(TextDraw),
}

impl PrimitiveDraw {
    pub fn layer(&self) -> i32 {
        match self {
            Self::Rect(draw) => draw.layer,
            Self::Circle(draw) => draw.layer,
            Self::Line(draw) => draw.layer,
            Self::Quad(draw) => draw.layer,
            Self::Text(draw) => draw.layer,
        }
    }
}

#[derive(Debug, Default)]
pub struct SceneFrame {
    draws: Vec<PrimitiveDraw>,
    meshes_3d: Vec<MeshDraw3D>,
}

pub struct Canvas2D<'a> {
    frame: &'a mut SceneFrame,
    space: DrawSpace,
}

impl SceneFrame {
    pub fn clear(&mut self) {
        self.draws.clear();
        self.meshes_3d.clear();
    }

    pub fn canvas(&mut self) -> Canvas2D<'_> {
        self.canvas_in(DrawSpace::World)
    }

    pub fn ui_canvas(&mut self) -> Canvas2D<'_> {
        self.canvas_in(DrawSpace::Screen)
    }

    pub fn canvas_in(&mut self, space: DrawSpace) -> Canvas2D<'_> {
        Canvas2D { frame: self, space }
    }

    pub fn draw_rect(&mut self, draw: RectDraw) {
        self.draws.push(PrimitiveDraw::Rect(draw));
    }

    pub fn draw_square(&mut self, draw: SquareDraw) {
        self.draw_rect(draw);
    }

    pub fn draw_circle(&mut self, draw: CircleDraw) {
        self.draws.push(PrimitiveDraw::Circle(draw));
    }

    pub fn draw_line(&mut self, draw: LineDraw) {
        self.draws.push(PrimitiveDraw::Line(draw));
    }

    pub fn draw_quad(&mut self, draw: QuadDraw) {
        self.draws.push(PrimitiveDraw::Quad(draw));
    }

    pub fn draw_text(&mut self, draw: TextDraw) {
        self.draws.push(PrimitiveDraw::Text(draw));
    }

    pub fn draw_mesh_3d(&mut self, draw: MeshDraw3D) {
        self.meshes_3d.push(draw);
    }

    pub fn draw_cube_3d(&mut self, draw: CubeDraw3D) {
        self.draw_mesh_3d(draw.into());
    }

    pub fn draw_sphere_3d(&mut self, draw: SphereDraw3D) {
        self.draw_mesh_3d(draw.into());
    }

    pub(crate) fn draws(&self) -> &[PrimitiveDraw] {
        &self.draws
    }

    pub(crate) fn meshes_3d(&self) -> &[MeshDraw3D] {
        &self.meshes_3d
    }
}

impl Canvas2D<'_> {
    pub fn space(&self) -> DrawSpace {
        self.space
    }

    pub fn fill_rect(
        &mut self,
        center: [f32; 2],
        size: [f32; 2],
        rotation_radians: f32,
        color: [f32; 4],
        layer: i32,
    ) {
        self.frame.draw_rect(RectDraw {
            center,
            size,
            rotation_radians,
            color,
            layer,
            style: ShapeStyle::Fill,
            space: self.space,
        });
    }

    pub fn stroke_rect(
        &mut self,
        center: [f32; 2],
        size: [f32; 2],
        rotation_radians: f32,
        color: [f32; 4],
        width: f32,
        layer: i32,
    ) {
        self.frame.draw_rect(RectDraw {
            center,
            size,
            rotation_radians,
            color,
            layer,
            style: ShapeStyle::Stroke { width },
            space: self.space,
        });
    }

    pub fn fill_circle(&mut self, center: [f32; 2], radius: f32, color: [f32; 4], layer: i32) {
        self.frame.draw_circle(CircleDraw {
            center,
            radius,
            color,
            layer,
            style: ShapeStyle::Fill,
            space: self.space,
        });
    }

    pub fn stroke_circle(
        &mut self,
        center: [f32; 2],
        radius: f32,
        color: [f32; 4],
        width: f32,
        layer: i32,
    ) {
        self.frame.draw_circle(CircleDraw {
            center,
            radius,
            color,
            layer,
            style: ShapeStyle::Stroke { width },
            space: self.space,
        });
    }

    pub fn line(
        &mut self,
        start: [f32; 2],
        end: [f32; 2],
        thickness: f32,
        color: [f32; 4],
        layer: i32,
    ) {
        self.frame.draw_line(LineDraw {
            start,
            end,
            thickness,
            color,
            layer,
            space: self.space,
        });
    }

    pub fn fill_quad(&mut self, points: [[f32; 2]; 4], color: [f32; 4], layer: i32) {
        self.frame.draw_quad(QuadDraw {
            points,
            color,
            layer,
            space: self.space,
        });
    }

    pub fn text(
        &mut self,
        position: [f32; 2],
        pixel_size: f32,
        text: impl Into<String>,
        color: [f32; 4],
        layer: i32,
    ) {
        self.frame.draw_text(TextDraw {
            position,
            text: text.into(),
            pixel_size,
            color,
            layer,
            space: self.space,
            anchor: TextAnchor::TopLeft,
        });
    }

    pub fn text_centered(
        &mut self,
        position: [f32; 2],
        pixel_size: f32,
        text: impl Into<String>,
        color: [f32; 4],
        layer: i32,
    ) {
        self.frame.draw_text(TextDraw {
            position,
            text: text.into(),
            pixel_size,
            color,
            layer,
            space: self.space,
            anchor: TextAnchor::Center,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_frame_collects_square_draws() {
        let mut frame = SceneFrame::default();
        frame.draw_square(SquareDraw::default());
        frame.draw_circle(CircleDraw::default());
        frame.draw_line(LineDraw::default());
        frame.draw_quad(QuadDraw::default());
        frame.draw_text(TextDraw::default());
        frame.draw_mesh_3d(MeshDraw3D::default());

        assert_eq!(frame.draws().len(), 5);
        assert_eq!(frame.meshes_3d().len(), 1);
    }

    #[test]
    fn camera_3d_view_matrix_look_along_neg_z() {
        // Camera at (0,0,-5) looking at origin → forward is +Z.
        let camera = Camera3D {
            position: [0.0, 0.0, -5.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_degrees: 60.0,
            near_clip: 0.1,
            far_clip: 100.0,
        };
        let view = camera.view_matrix();
        // The view matrix fourth row encodes translation; w component should be 1.
        assert!((view[3][3] - 1.0).abs() < 0.0001);
    }

    #[test]
    fn camera_3d_orbit_preserves_distance() {
        let mut camera = Camera3D::default();
        let dist_before = {
            let d = cam3_sub(camera.position, camera.target);
            cam3_length(d)
        };
        camera.orbit(45.0, 15.0);
        let dist_after = {
            let d = cam3_sub(camera.position, camera.target);
            cam3_length(d)
        };
        assert!((dist_before - dist_after).abs() < 0.001);
    }

    #[test]
    fn camera_3d_dolly_moves_toward_target() {
        let mut camera = Camera3D::default();
        let dist_before = cam3_length(cam3_sub(camera.position, camera.target));
        camera.dolly(1.0);
        let dist_after = cam3_length(cam3_sub(camera.position, camera.target));
        assert!(dist_after < dist_before);
    }

    #[test]
    fn frustum_contains_target_point() {
        let camera = Camera3D {
            position: [0.0, 0.0, -10.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_degrees: 60.0,
            near_clip: 0.1,
            far_clip: 100.0,
        };
        let frustum = camera.frustum(16.0 / 9.0);
        // The target is at the center of the view — it must be inside the frustum.
        assert!(frustum.test_sphere([0.0, 0.0, 0.0], 0.1));
    }

    #[test]
    fn frustum_rejects_point_behind_camera() {
        let camera = Camera3D {
            position: [0.0, 0.0, -10.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_degrees: 60.0,
            near_clip: 0.1,
            far_clip: 100.0,
        };
        let frustum = camera.frustum(16.0 / 9.0);
        // A point 50 units behind the camera (farther along -Z than position).
        assert!(!frustum.test_sphere([0.0, 0.0, -60.0], 0.5));
    }

    #[test]
    fn camera_round_trips_world_space() {
        let camera = Camera2D::new([2.0, -1.0], 4.0);
        let point = [3.0, 0.5];
        let ndc = camera.world_to_ndc(point, 16.0 / 9.0);
        let rebuilt = camera.ndc_to_world(ndc, 16.0 / 9.0);

        assert!((rebuilt[0] - point[0]).abs() < 0.0001);
        assert!((rebuilt[1] - point[1]).abs() < 0.0001);
    }

    #[test]
    fn canvas_helpers_emit_primitives() {
        let mut frame = SceneFrame::default();
        let mut canvas = frame.canvas();
        canvas.fill_rect([0.0, 0.0], [1.0, 1.0], 0.0, [1.0, 0.0, 0.0, 0.5], 1);
        canvas.stroke_circle([0.5, 0.0], 0.25, [0.0, 1.0, 0.0, 1.0], 0.05, 2);
        canvas.line([-1.0, 0.0], [1.0, 0.0], 0.02, [0.0, 0.5, 1.0, 1.0], 0);
        canvas.fill_quad(
            [[-0.5, -0.5], [0.2, -0.4], [0.4, 0.3], [-0.4, 0.2]],
            [1.0, 1.0, 1.0, 1.0],
            3,
        );
        canvas.text([8.0, 12.0], 16.0, "hello", [1.0, 1.0, 1.0, 1.0], 4);

        assert_eq!(frame.draws().len(), 5);
    }

    #[test]
    fn ui_canvas_emits_screen_space_primitives() {
        let mut frame = SceneFrame::default();
        let mut canvas = frame.ui_canvas();
        canvas.fill_rect([120.0, 64.0], [200.0, 40.0], 0.0, [1.0, 1.0, 1.0, 1.0], 5);

        match frame.draws()[0] {
            PrimitiveDraw::Rect(draw) => assert_eq!(draw.space, DrawSpace::Screen),
            _ => panic!("expected rect draw"),
        }
    }
}
