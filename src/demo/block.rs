use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::ApiConfig;
use crate::lighting::{
    DirectionalLight, LightingConfig, LiveShadowConfig, PointLight, ShadowConfig, ShadowMode,
};
use crate::runtime::run_scene;
use crate::scene::{
    Camera2D, Camera3D, Mesh3D, MeshAsset3D, MeshDraw3D, MeshVertex3D, Scene, SceneConfig,
    SceneFrame, ScreenshotResolution,
};
use winit::event::{ElementState, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

pub fn run_spinning_block_demo() -> Result<(), crate::core::ApiError> {
    run_scene(SpinningBlockScene::default())
}

const SPINNING_CUBE_SCREENSHOT_SAMPLES: u32 = 32;

struct SpinningBlockScene {
    elapsed_seconds: f32,
    screenshot_request: RefCell<Option<PathBuf>>,
    screenshot_flash_frames: Cell<u32>,
    screenshot_resolution: Cell<ScreenshotResolution>,
    screenshot_cube_mesh: Arc<MeshAsset3D>,
}

impl Default for SpinningBlockScene {
    fn default() -> Self {
        Self {
            elapsed_seconds: 0.0,
            screenshot_request: RefCell::new(None),
            screenshot_flash_frames: Cell::new(0),
            screenshot_resolution: Cell::new(ScreenshotResolution::K4),
            screenshot_cube_mesh: create_subdivided_box_mesh_asset("spinning_cube_flash_box", 64),
        }
    }
}

impl Scene for SpinningBlockScene {
    fn config(&self) -> SceneConfig {
        let mut api = ApiConfig::default();
        api.application_name = "NU Spinning Cube".to_string();
        api.enable_validation = false;

        let mut config = SceneConfig::default();
        config.window.title = "nu Demo: Spinning Cube".to_string();
        config.window.width = 960;
        config.window.height = 640;
        config.api = api;
        config.clear_color = [0.01, 0.01, 0.02, 1.0];
        config.camera = Camera2D::new([0.0, 0.0], 2.4);
        config.camera_3d = Camera3D {
            position: [0.0, -0.70, -4.20],
            target: [0.0, 0.18, 0.0],
            up: [0.0, -1.0, 0.0],
            fov_y_degrees: 55.0,
            near_clip: 0.1,
            far_clip: 100.0,
        };
        config.lighting = showcase_lighting(self.screenshot_flash_active());
        config.screenshot_path = self.screenshot_request.borrow_mut().take();
        config.screenshot_accumulation_samples = if config.screenshot_path.is_some() {
            SPINNING_CUBE_SCREENSHOT_SAMPLES
        } else {
            1
        };
        config.screenshot_resolution = self.screenshot_resolution.get();
        config
    }

    fn update(&mut self, delta_time_seconds: f32) {
        self.elapsed_seconds += delta_time_seconds.min(0.1);
        let frames = self.screenshot_flash_frames.get();
        if frames > 0 {
            self.screenshot_flash_frames.set(frames - 1);
        }
    }

    fn window_event(&mut self, _window: &Window, event: &WindowEvent) {
        if let WindowEvent::KeyboardInput { event, .. } = event {
            if event.state == ElementState::Pressed {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match code {
                        KeyCode::F12 => self.queue_screenshot_capture(),
                        KeyCode::Digit2 => self.screenshot_resolution.set(ScreenshotResolution::K2),
                        KeyCode::Digit4 => self.screenshot_resolution.set(ScreenshotResolution::K4),
                        KeyCode::Digit8 => self.screenshot_resolution.set(ScreenshotResolution::K8),
                        KeyCode::Digit9 => {
                            self.screenshot_resolution.set(ScreenshotResolution::K16)
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn populate(&mut self, frame: &mut SceneFrame) {
        let cube_rotation = [-0.62, -0.88 - self.elapsed_seconds * 0.92, 0.0];
        let screenshot_flash = self.screenshot_flash_active();
        let screenshot_cube_mesh = if screenshot_flash {
            Mesh3D::Custom(self.screenshot_cube_mesh.clone())
        } else {
            Mesh3D::Cube
        };
        let cube_material = if screenshot_flash {
            crate::scene::MeshMaterial3D {
                roughness: 0.10,
                metallic: 0.94,
                ..Default::default()
            }
        } else {
            crate::scene::MeshMaterial3D {
                roughness: 0.28,
                metallic: 0.14,
                ..Default::default()
            }
        };

        frame.draw_mesh_3d(MeshDraw3D {
            mesh: Mesh3D::Cube,
            center: [0.0, 0.83, 0.0],
            size: [4.8, 0.12, 4.8],
            rotation_radians: [0.0, 0.0, 0.0],
            color: [0.17, 0.17, 0.18, 1.0],
            material: crate::scene::MeshMaterial3D {
                roughness: 0.88,
                metallic: 0.01,
                ..Default::default()
            },
        });

        frame.draw_mesh_3d(MeshDraw3D {
            mesh: Mesh3D::Cube,
            center: [0.0, 0.0, 2.35],
            size: [4.4, 2.8, 0.10],
            rotation_radians: [0.0, 0.0, 0.0],
            color: [0.10, 0.10, 0.12, 1.0],
            material: crate::scene::MeshMaterial3D {
                roughness: 0.92,
                metallic: 0.02,
                ..Default::default()
            },
        });

        frame.draw_mesh_3d(MeshDraw3D {
            mesh: screenshot_cube_mesh,
            center: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_radians: cube_rotation,
            color: [0.96, 0.05, 0.05, 1.0],
            material: cube_material,
        });

        if !screenshot_flash {
            let mut ui = frame.ui_canvas();
            ui.text(
                [18.0, 20.0],
                18.0,
                "NU SPINNING CUBE",
                [0.90, 0.92, 0.96, 1.0],
                2500,
            );
            ui.text(
                [18.0, 44.0],
                15.0,
                &format!(
                    "F12 SCREENSHOT  2/4/8/9 RES  {}",
                    screenshot_resolution_label(self.screenshot_resolution.get())
                ),
                [0.68, 0.74, 0.82, 1.0],
                2500,
            );
        }
    }
}

fn showcase_lighting(screenshot_flash: bool) -> LightingConfig {
    let mut lighting = LightingConfig::default();
    lighting.ambient_color = [0.18, 0.19, 0.22];
    lighting.ambient_intensity = if screenshot_flash { 0.34 } else { 0.28 };
    lighting.fill_light = DirectionalLight {
        direction: [-0.16, -0.93, 0.34],
        color: [0.38, 0.39, 0.45],
        intensity: if screenshot_flash { 0.72 } else { 0.52 },
    };
    lighting.shadows = ShadowConfig {
        mode: ShadowMode::Live,
        minimum_visibility: if screenshot_flash { 0.02 } else { 0.08 },
        bias: if screenshot_flash { 0.005 } else { 0.008 },
        live: LiveShadowConfig {
            max_distance: 20.0,
            filter_radius: if screenshot_flash { 2.55 } else { 1.75 },
        },
    };
    lighting.specular_strength = if screenshot_flash { 0.12 } else { 0.10 };
    lighting.shininess = if screenshot_flash { 72.0 } else { 58.0 };
    lighting.clear_point_lights();
    let _ = lighting.push_point_light(
        PointLight {
            position: [1.55, -2.55, -2.95],
            color: [1.0, 0.97, 0.93],
            intensity: if screenshot_flash { 2.60 } else { 2.25 },
            range: 10.5,
        },
        false,
    );
    let _ = lighting.push_point_light(
        PointLight {
            position: [0.0, -0.10, 1.70],
            color: [0.50, 0.10, 0.10],
            intensity: if screenshot_flash { 0.22 } else { 0.14 },
            range: 4.2,
        },
        false,
    );
    lighting
}

impl SpinningBlockScene {
    fn screenshot_flash_active(&self) -> bool {
        self.screenshot_flash_frames.get() > 0
    }

    fn queue_screenshot_capture(&self) {
        let screenshots_dir = PathBuf::from("screenshots");
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let path = screenshots_dir.join(format!("nu_spinning_cube_{timestamp}.png"));
        *self.screenshot_request.borrow_mut() = Some(path);
        self.screenshot_flash_frames.set(2);
    }
}

fn create_subdivided_box_mesh_asset(name: &str, subdivisions: usize) -> Arc<MeshAsset3D> {
    let subdivisions = subdivisions.max(1);
    let mut vertices = Vec::with_capacity(subdivisions * subdivisions * 6 * 6);
    append_subdivided_face(
        &mut vertices,
        [-1.0, -1.0, 1.0],
        [2.0, 0.0, 0.0],
        [0.0, 2.0, 0.0],
        [0.0, 0.0, 1.0],
        subdivisions,
    );
    append_subdivided_face(
        &mut vertices,
        [1.0, -1.0, -1.0],
        [-2.0, 0.0, 0.0],
        [0.0, 2.0, 0.0],
        [0.0, 0.0, -1.0],
        subdivisions,
    );
    append_subdivided_face(
        &mut vertices,
        [-1.0, -1.0, -1.0],
        [0.0, 0.0, 2.0],
        [0.0, 2.0, 0.0],
        [-1.0, 0.0, 0.0],
        subdivisions,
    );
    append_subdivided_face(
        &mut vertices,
        [1.0, -1.0, 1.0],
        [0.0, 2.0, 0.0],
        [0.0, 0.0, -2.0],
        [1.0, 0.0, 0.0],
        subdivisions,
    );
    append_subdivided_face(
        &mut vertices,
        [-1.0, 1.0, 1.0],
        [2.0, 0.0, 0.0],
        [0.0, 0.0, -2.0],
        [0.0, 1.0, 0.0],
        subdivisions,
    );
    append_subdivided_face(
        &mut vertices,
        [-1.0, -1.0, -1.0],
        [2.0, 0.0, 0.0],
        [0.0, 0.0, 2.0],
        [0.0, -1.0, 0.0],
        subdivisions,
    );

    Arc::new(MeshAsset3D {
        name: name.to_string(),
        vertices: Arc::<[MeshVertex3D]>::from(vertices),
        base_size: [2.0, 2.0, 2.0],
    })
}

fn append_subdivided_face(
    vertices: &mut Vec<MeshVertex3D>,
    origin: [f32; 3],
    axis_u: [f32; 3],
    axis_v: [f32; 3],
    normal: [f32; 3],
    subdivisions: usize,
) {
    let step = 1.0 / subdivisions as f32;
    for y in 0..subdivisions {
        for x in 0..subdivisions {
            let u0 = x as f32 * step;
            let u1 = (x + 1) as f32 * step;
            let v0 = y as f32 * step;
            let v1 = (y + 1) as f32 * step;

            let p00 = face_point(origin, axis_u, axis_v, u0, v0);
            let p10 = face_point(origin, axis_u, axis_v, u1, v0);
            let p11 = face_point(origin, axis_u, axis_v, u1, v1);
            let p01 = face_point(origin, axis_u, axis_v, u0, v1);

            vertices.extend_from_slice(&[
                MeshVertex3D {
                    position: p00,
                    normal,
                    uv: [u0, v0],
                },
                MeshVertex3D {
                    position: p10,
                    normal,
                    uv: [u1, v0],
                },
                MeshVertex3D {
                    position: p11,
                    normal,
                    uv: [u1, v1],
                },
                MeshVertex3D {
                    position: p00,
                    normal,
                    uv: [u0, v0],
                },
                MeshVertex3D {
                    position: p11,
                    normal,
                    uv: [u1, v1],
                },
                MeshVertex3D {
                    position: p01,
                    normal,
                    uv: [u0, v1],
                },
            ]);
        }
    }
}

fn face_point(origin: [f32; 3], axis_u: [f32; 3], axis_v: [f32; 3], u: f32, v: f32) -> [f32; 3] {
    [
        origin[0] + axis_u[0] * u + axis_v[0] * v,
        origin[1] + axis_u[1] * u + axis_v[1] * v,
        origin[2] + axis_u[2] * u + axis_v[2] * v,
    ]
}

fn screenshot_resolution_label(resolution: ScreenshotResolution) -> &'static str {
    match resolution {
        ScreenshotResolution::K2 => "2K",
        ScreenshotResolution::K4 => "4K",
        ScreenshotResolution::K8 => "8K",
        ScreenshotResolution::K16 => "16K",
    }
}
