use crate::core::ApiConfig;
use crate::runtime::run_scene;
use crate::scene::{Camera2D, Scene, SceneConfig, SceneFrame};

pub fn run_square_demo() -> Result<(), crate::core::ApiError> {
    run_scene(SquareScene::default())
}

#[derive(Default)]
struct SquareScene;

impl Scene for SquareScene {
    fn config(&self) -> SceneConfig {
        let mut api = ApiConfig::default();
        api.application_name = "Square Demo".to_string();
        api.enable_validation = false;

        let mut config = SceneConfig::default();
        config.window.title = "nu Demo: Red Square".to_string();
        config.window.width = 900;
        config.window.height = 600;
        config.api = api;
        config.clear_color = [0.05, 0.05, 0.07, 1.0];
        config.camera = Camera2D {
            center: [0.0, 0.0],
            view_height: 2.0,
        };
        config
    }

    fn populate(&mut self, frame: &mut SceneFrame) {
        let mut canvas = frame.canvas();
        canvas.line([-0.8, -0.55], [0.8, -0.55], 0.035, [0.2, 0.7, 1.0, 1.0], 0);
        canvas.fill_rect([0.0, 0.0], [0.55, 0.55], 0.2, [1.0, 0.0, 0.0, 0.92], 1);
        canvas.fill_rect(
            [0.18, -0.08],
            [0.42, 0.42],
            -0.35,
            [1.0, 0.55, 0.1, 0.45],
            2,
        );
        canvas.stroke_rect(
            [0.0, 0.0],
            [0.78, 0.78],
            0.2,
            [1.0, 0.85, 0.25, 0.95],
            0.05,
            4,
        );
        canvas.fill_circle([-0.45, 0.38], 0.12, [0.95, 0.85, 0.2, 0.9], 3);
        canvas.stroke_circle([0.52, 0.32], 0.16, [0.25, 0.95, 0.6, 0.95], 0.04, 3);

        let mut ui = frame.ui_canvas();
        ui.fill_rect(
            [135.0, 78.0],
            [210.0, 56.0],
            0.0,
            [0.04, 0.06, 0.1, 0.82],
            10,
        );
        ui.stroke_rect(
            [135.0, 78.0],
            [210.0, 56.0],
            0.0,
            [0.95, 0.35, 0.2, 0.95],
            3.0,
            11,
        );
        ui.line([40.0, 42.0], [230.0, 42.0], 2.0, [1.0, 0.45, 0.2, 0.85], 12);
        ui.fill_circle([232.0, 78.0], 10.0, [0.95, 0.35, 0.2, 0.95], 12);
    }
}
