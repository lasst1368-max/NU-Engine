use crate::app::WindowConfig;
use crate::core::ApiConfig;
use crate::lighting::LightingConfig;
use crate::syntax::TextureHandle;
use std::sync::Arc;
use winit::event::WindowEvent;

#[derive(Debug, Clone)]
pub struct SceneConfig {
    pub window: WindowConfig,
    pub api: ApiConfig,
    pub clear_color: [f32; 4],
    pub camera: Camera2D,
    pub camera_3d: Camera3D,
    pub lighting: LightingConfig,
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

pub trait Scene {
    fn config(&self) -> SceneConfig;

    fn update(&mut self, _delta_time_seconds: f32) {}

    fn window_event(&mut self, _event: &WindowEvent) {}

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
}

#[derive(Debug, Clone)]
pub struct MeshAsset3D {
    pub name: String,
    pub vertices: Arc<[MeshVertex3D]>,
    pub base_size: [f32; 3],
}

#[derive(Debug, Clone)]
pub enum Mesh3D {
    Cube,
    Plane,
    Sphere,
    Custom(Arc<MeshAsset3D>),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MeshMaterial3D {
    pub albedo_texture: Option<TextureHandle>,
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
