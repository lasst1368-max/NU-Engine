use crate::core::ApiConfig;
use crate::lighting::{DirectionalLight, LightingConfig, PointLight, ShadowConfig};
use crate::runtime::run_scene;
use crate::scene::{Camera2D, Camera3D, Mesh3D, MeshDraw3D, Scene, SceneConfig, SceneFrame};

pub fn run_spinning_block_demo() -> Result<(), crate::core::ApiError> {
    run_scene(SpinningBlockScene::default())
}

#[derive(Default)]
struct SpinningBlockScene {
    elapsed_seconds: f32,
}

impl Scene for SpinningBlockScene {
    fn config(&self) -> SceneConfig {
        let mut api = ApiConfig::default();
        api.application_name = "Rotating Red Cube Demo".to_string();
        api.enable_validation = false;

        let mut config = SceneConfig::default();
        config.window.title = "nu Demo: Rotating Red Cube".to_string();
        config.window.width = 960;
        config.window.height = 640;
        config.api = api;
        config.clear_color = [0.02, 0.02, 0.03, 1.0];
        config.camera = Camera2D::new([0.0, 0.0], 2.4);
        config.camera_3d = Camera3D {
            position: [0.0, -0.7, -4.2],
            target: [0.0, 0.2, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_degrees: 55.0,
            near_clip: 0.1,
            far_clip: 100.0,
        };
        config.lighting = LightingConfig {
            ambient_color: [0.22, 0.24, 0.28],
            ambient_intensity: 0.24,
            point_light: PointLight {
                position: [1.1, -2.2, -2.4],
                color: [1.0, 0.97, 0.93],
                intensity: 1.42,
                range: 9.0,
            },
            fill_light: DirectionalLight {
                direction: [-0.55, -0.85, -0.2],
                color: [0.30, 0.35, 0.44],
                intensity: 0.28,
            },
            shadows: ShadowConfig {
                minimum_visibility: 0.10,
                bias: 0.03,
                point_light_radius: 0.08,
                point_samples: 1,
                directional_spread: 0.10,
                directional_samples: 3,
            },
            specular_strength: 0.08,
            shininess: 48.0,
        };
        config
    }

    fn update(&mut self, delta_time_seconds: f32) {
        self.elapsed_seconds += delta_time_seconds;
    }

    fn populate(&mut self, frame: &mut SceneFrame) {
        frame.draw_mesh_3d(MeshDraw3D {
            mesh: Mesh3D::Cube,
            center: [0.0, 0.83, 0.0],
            size: [4.8, 0.12, 4.8],
            rotation_radians: [0.0, 0.0, 0.0],
            color: [0.16, 0.16, 0.18, 1.0],
            material: Default::default(),
        });
        frame.draw_mesh_3d(MeshDraw3D {
            mesh: Mesh3D::Cube,
            center: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_radians: [-0.62, -0.88 - self.elapsed_seconds * 0.22, 0.0],
            color: [0.95, 0.04, 0.04, 1.0],
            material: Default::default(),
        });
    }
}
