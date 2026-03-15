use crate::core::ApiConfig;
use crate::lighting::{
    DirectionalLight, LightingConfig, LiveShadowConfig, PointLight, ShadowConfig, ShadowMode,
};
use crate::physics::{BodyHandle, ColliderShape, PhysicsConfig, PhysicsWorld, RigidBody};
use crate::runtime::run_scene;
use crate::scene::{Camera2D, Camera3D, Mesh3D, MeshDraw3D, Scene, SceneConfig, SceneFrame};

pub fn run_physics_demo() -> Result<(), crate::core::ApiError> {
    run_scene(PhysicsDropScene::new())
}

struct PhysicsDropScene {
    world: PhysicsWorld,
    cube_body: BodyHandle,
    sphere_body: BodyHandle,
    accumulator: f32,
}

impl PhysicsDropScene {
    fn new() -> Self {
        let mut world = PhysicsWorld::new(PhysicsConfig::default());
        world.insert_body(RigidBody::static_body(
            [0.0, 0.0, 0.0],
            ColliderShape::Plane {
                normal: [0.0, 1.0, 0.0],
                offset: 0.0,
            },
        ));
        let cube_body = world.insert_body(RigidBody::dynamic_body(
            [-0.55, 3.8, 0.0],
            1.0,
            ColliderShape::Cuboid {
                half_extents: [0.5, 0.5, 0.5],
            },
        ));
        let sphere_body = world.insert_body(RigidBody::dynamic_body(
            [0.75, 5.2, 0.0],
            0.8,
            ColliderShape::Sphere { radius: 0.42 },
        ));
        Self {
            world,
            cube_body,
            sphere_body,
            accumulator: 0.0,
        }
    }
}

impl Scene for PhysicsDropScene {
    fn config(&self) -> SceneConfig {
        let mut api = ApiConfig::default();
        api.application_name = "NU Physics Detection Demo".to_string();
        api.enable_validation = false;

        let mut config = SceneConfig::default();
        config.window.title = "nu Demo: Physics".to_string();
        config.window.width = 1040;
        config.window.height = 680;
        config.api = api;
        config.clear_color = [0.025, 0.028, 0.036, 1.0];
        config.camera = Camera2D::new([0.0, 0.0], 2.4);
        config.camera_3d = Camera3D {
            position: [0.0, 2.0, -8.5],
            target: [0.0, 1.4, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_degrees: 48.0,
            near_clip: 0.1,
            far_clip: 100.0,
        };
        config.lighting = LightingConfig {
            ambient_color: [0.18, 0.20, 0.24],
            ambient_intensity: 0.28,
            point_lights: [PointLight::default(); crate::lighting::MAX_POINT_LIGHTS],
            point_light_shadow_flags: [false; crate::lighting::MAX_POINT_LIGHTS],
            point_light_count: 1,
            fill_light: DirectionalLight {
                direction: [-0.35, -0.9, -0.2],
                color: [0.26, 0.32, 0.40],
                intensity: 0.25,
            },
            shadows: ShadowConfig {
                mode: ShadowMode::Live,
                minimum_visibility: 0.12,
                bias: 0.03,
                live: LiveShadowConfig {
                    max_distance: 32.0,
                    filter_radius: 1.4,
                },
            },
            specular_strength: 0.08,
            shininess: 40.0,
        };
        config.lighting.point_lights[0] = PointLight {
            position: [2.8, 5.4, -4.2],
            color: [1.0, 0.97, 0.92],
            intensity: 1.35,
            range: 16.0,
        };
        config.lighting.point_light_shadow_flags[0] = true;
        config
    }

    fn update(&mut self, delta_time_seconds: f32) {
        const FIXED_STEP: f32 = 1.0 / 120.0;
        self.accumulator = (self.accumulator + delta_time_seconds.min(0.1)).min(0.25);
        while self.accumulator >= FIXED_STEP {
            self.world.step(FIXED_STEP);
            self.accumulator -= FIXED_STEP;
        }
    }

    fn populate(&mut self, frame: &mut SceneFrame) {
        frame.draw_mesh_3d(MeshDraw3D {
            mesh: Mesh3D::Cube,
            center: [0.0, -0.06, 0.0],
            size: [8.0, 0.12, 8.0],
            rotation_radians: [0.0, 0.0, 0.0],
            color: [0.17, 0.18, 0.20, 1.0],
            material: Default::default(),
        });

        if let Some(body) = self.world.body(self.cube_body) {
            frame.draw_mesh_3d(MeshDraw3D {
                mesh: Mesh3D::Cube,
                center: body.position,
                size: [1.0, 1.0, 1.0],
                rotation_radians: body.rotation_radians,
                color: [0.95, 0.08, 0.08, 1.0],
                material: Default::default(),
            });
        }

        if let Some(body) = self.world.body(self.sphere_body) {
            frame.draw_mesh_3d(MeshDraw3D {
                mesh: Mesh3D::Sphere,
                center: body.position,
                size: [0.84, 0.84, 0.84],
                rotation_radians: body.rotation_radians,
                color: [0.86, 0.32, 0.18, 1.0],
                material: Default::default(),
            });
        }

        let mut ui = frame.ui_canvas();
        ui.text(
            [18.0, 20.0],
            18.0,
            "PHYSICS: DETECTION + SIMPLE CONTACT RESOLUTION",
            [0.88, 0.90, 0.94, 1.0],
            2100,
        );
        ui.text(
            [18.0, 44.0],
            15.0,
            format!("BODIES {}", self.world.bodies().count()),
            [0.62, 0.68, 0.76, 1.0],
            2100,
        );
    }
}
