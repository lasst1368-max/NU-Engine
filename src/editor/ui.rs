use super::SceneEditor;
use crate::app::WindowConfig;
use crate::core::{ApiConfig, ApiError};
use crate::engine::{
    EngineError, HotReloadManager, LightKind, NuMeshSection, NuSceneDocument, NuTransform,
    ReloadBatch, SceneSyntax, load_obj_mesh_asset,
};
use crate::lighting::{DirectionalLight, LightingConfig, PointLight};
use crate::run_scene;
use crate::scene::{
    Camera2D, Camera3D, Canvas2D, Mesh3D, MeshAsset3D, MeshDraw3D, MeshMaterial3D, Scene,
    SceneConfig, SceneFrame,
};
use rfd::FileDialog;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

const WINDOW_WIDTH: u32 = 1600;
const WINDOW_HEIGHT: u32 = 900;
const TOP_BAR_HEIGHT: f32 = 54.0;
const LEFT_PANEL_WIDTH: f32 = 250.0;
const RIGHT_PANEL_WIDTH: f32 = 320.0;
const PANEL_PAD: f32 = 14.0;
const BUTTON_H: f32 = 32.0;
const UI_LAYER_PANEL: i32 = 2000;
const UI_LAYER_TEXT: i32 = 2010;

const C_PANEL: [f32; 4] = [0.08, 0.09, 0.12, 0.92];
const C_PANEL_ALT: [f32; 4] = [0.11, 0.12, 0.16, 0.95];
const C_BUTTON: [f32; 4] = [0.16, 0.17, 0.23, 0.96];
const C_BUTTON_HOVER: [f32; 4] = [0.23, 0.24, 0.31, 0.98];
const C_BUTTON_ACTIVE: [f32; 4] = [0.74, 0.12, 0.12, 0.98];
const C_TEXT: [f32; 4] = [0.90, 0.92, 0.96, 1.0];
const C_TEXT_MUTED: [f32; 4] = [0.64, 0.68, 0.76, 1.0];
const C_TEXT_ACCENT: [f32; 4] = [1.0, 0.88, 0.52, 1.0];
const C_OUTLINE: [f32; 4] = [0.28, 0.30, 0.36, 1.0];
const C_OK: [f32; 4] = [0.56, 0.88, 0.68, 1.0];
const C_WARN: [f32; 4] = [1.0, 0.72, 0.34, 1.0];
#[derive(Debug)]
pub enum EditorUiError {
    Engine(EngineError),
    Api(ApiError),
}

impl Display for EditorUiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Engine(error) => write!(f, "{error}"),
            Self::Api(error) => write!(f, "{error}"),
        }
    }
}

impl Error for EditorUiError {}

impl From<EngineError> for EditorUiError {
    fn from(value: EngineError) -> Self {
        Self::Engine(value)
    }
}

impl From<ApiError> for EditorUiError {
    fn from(value: ApiError) -> Self {
        Self::Api(value)
    }
}

pub fn run_basic_scene_editor(scene_path: Option<impl AsRef<Path>>) -> Result<(), EditorUiError> {
    let scene = BasicEditorScene::new(scene_path.map(|path| path.as_ref().to_path_buf()))?;
    run_scene(scene)?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorMode {
    Scene,
    Meshes,
    Lights,
    Materials,
}

struct BasicEditorScene {
    editor: SceneEditor,
    hot_reload: Option<HotReloadManager>,
    status: String,
    status_color: [f32; 4],
    last_reload: Option<ReloadBatch>,
    mode: EditorMode,
    selected_mesh: Option<String>,
    selected_light: Option<String>,
    selected_material: Option<String>,
    mouse_pos: [f32; 2],
    click_pending: bool,
    left_mouse_down: bool,
    right_mouse_down: bool,
    window_size: [u32; 2],
    drag_state: DragState,
    obj_mesh_cache: HashMap<PathBuf, Result<Arc<MeshAsset3D>, String>>,
}

#[derive(Debug, Clone, Copy)]
enum DragState {
    None,
    MoveMesh {
        plane_y: f32,
        offset: [f32; 3],
    },
    OrbitCamera {
        last_mouse: [f32; 2],
        pivot: [f32; 3],
    },
}

impl BasicEditorScene {
    fn new(scene_path: Option<PathBuf>) -> Result<Self, EngineError> {
        let (mut editor, hot_reload, status, status_color) = match scene_path {
            Some(path) => match SceneEditor::open(&path) {
                Ok(editor) => (
                    editor,
                    HotReloadManager::open(&path).ok(),
                    format!("OPENED {}", path.display()),
                    C_OK,
                ),
                Err(error) => {
                    let mut editor = SceneEditor::new_empty("untitled");
                    Self::seed_defaults(&mut editor);
                    (editor, None, format!("OPEN FAILED: {error}"), C_WARN)
                }
            },
            None => {
                let mut editor = SceneEditor::new_empty("untitled");
                Self::seed_defaults(&mut editor);
                (editor, None, "NEW SCENE".to_string(), C_OK)
            }
        };

        if editor.document().materials.is_empty() || editor.document().meshes.is_empty() {
            Self::seed_defaults(&mut editor);
        }

        let mut scene = Self {
            editor,
            hot_reload,
            status,
            status_color,
            last_reload: None,
            mode: EditorMode::Meshes,
            selected_mesh: None,
            selected_light: None,
            selected_material: None,
            mouse_pos: [0.0, 0.0],
            click_pending: false,
            left_mouse_down: false,
            right_mouse_down: false,
            window_size: [WINDOW_WIDTH, WINDOW_HEIGHT],
            drag_state: DragState::None,
            obj_mesh_cache: HashMap::new(),
        };
        scene.ensure_selection();
        Ok(scene)
    }

    fn seed_defaults(editor: &mut SceneEditor) {
        if editor.document().lights.is_empty() {
            editor.upsert_light(
                "key",
                LightKind::Point,
                [4.5, 6.0, -4.0],
                [1.0, 0.95, 0.88],
                1.2,
            );
        }
        if editor.document().materials.is_empty() {
            editor.upsert_material(
                "default_material",
                "lit.vert",
                "lit.frag",
                [0.92, 0.12, 0.12],
                0.45,
                None,
            );
        }
        if editor.document().meshes.is_empty() {
            editor.upsert_mesh(
                "cube",
                "cube",
                None,
                "default_material",
                None,
                NuTransform {
                    position: [0.0, 1.0, 0.0],
                    rotation_degrees: [20.0, 35.0, 0.0],
                    scale: [1.2, 1.2, 1.2],
                },
            );
            editor.upsert_mesh(
                "floor",
                "plane",
                None,
                "default_material",
                None,
                NuTransform {
                    position: [0.0, 0.0, 0.0],
                    rotation_degrees: [0.0, 0.0, 0.0],
                    scale: [8.0, 1.0, 8.0],
                },
            );
        }
    }

    fn ensure_selection(&mut self) {
        if self.selected_mesh.is_none() {
            self.selected_mesh = self.editor.document().meshes.keys().next().cloned();
        }
        if self.selected_light.is_none() {
            self.selected_light = self.editor.document().lights.keys().next().cloned();
        }
        if self.selected_material.is_none() {
            self.selected_material = self.editor.document().materials.keys().next().cloned();
        }
    }

    fn set_status(&mut self, message: impl Into<String>, color: [f32; 4]) {
        self.status = message.into();
        self.status_color = color;
    }

    fn open_from_dialog(&mut self) {
        let Some(path) = FileDialog::new()
            .add_filter("nu scene", &["nuscene"])
            .pick_file()
        else {
            return;
        };
        match SceneEditor::open(&path) {
            Ok(mut editor) => {
                if editor.document().materials.is_empty() || editor.document().meshes.is_empty() {
                    Self::seed_defaults(&mut editor);
                }
                self.editor = editor;
                self.obj_mesh_cache.clear();
                self.hot_reload = HotReloadManager::open(&path).ok();
                self.last_reload = None;
                self.selected_mesh = None;
                self.selected_light = None;
                self.selected_material = None;
                self.ensure_selection();
                self.set_status(format!("OPENED {}", path.display()), C_OK);
            }
            Err(error) => self.set_status(format!("OPEN FAILED: {error}"), C_WARN),
        }
    }

    fn save_current(&mut self) {
        if self.editor.scene_path().is_none() {
            self.save_as_dialog();
            return;
        }
        match self.editor.save() {
            Ok(()) => {
                self.rebind_hot_reload();
                self.set_status("SAVED", C_OK);
            }
            Err(error) => self.set_status(format!("SAVE FAILED: {error}"), C_WARN),
        }
    }

    fn save_as_dialog(&mut self) {
        let default_name = self
            .editor
            .scene_path()
            .and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| "scene.nuscene".to_string());
        let Some(path) = FileDialog::new()
            .add_filter("nu scene", &["nuscene"])
            .set_file_name(&default_name)
            .save_file()
        else {
            return;
        };
        match self.editor.save_as(&path) {
            Ok(()) => {
                self.rebind_hot_reload();
                self.set_status(format!("SAVED {}", path.display()), C_OK);
            }
            Err(error) => self.set_status(format!("SAVE FAILED: {error}"), C_WARN),
        }
    }

    fn rebind_hot_reload(&mut self) {
        let Some(path) = self.editor.scene_path().map(PathBuf::from) else {
            return;
        };
        self.hot_reload = HotReloadManager::open(&path).ok();
        if let Some(hot_reload) = &mut self.hot_reload {
            self.last_reload = hot_reload.reload_now().ok();
        }
    }

    fn force_reload(&mut self) {
        let Some(hot_reload) = &mut self.hot_reload else {
            self.set_status("RELOAD REQUIRES A SAVED SCENE", C_WARN);
            return;
        };
        match hot_reload.reload_now() {
            Ok(batch) => {
                if let Some(scene) = &batch.scene {
                    self.editor.replace_document(scene.clone());
                }
                self.last_reload = Some(batch.clone());
                self.selected_mesh = None;
                self.selected_light = None;
                self.selected_material = None;
                self.ensure_selection();
                self.set_status(
                    format!(
                        "RELOADED {} SH / {} TX",
                        batch.shaders.len(),
                        batch.textures.len()
                    ),
                    C_OK,
                );
            }
            Err(error) => self.set_status(format!("RELOAD FAILED: {error}"), C_WARN),
        }
    }

    fn cycle_syntax(&mut self) {
        let syntax = match self.editor.document().scene.syntax {
            SceneSyntax::OpenGl => SceneSyntax::Vulkan,
            SceneSyntax::Vulkan => SceneSyntax::Raw,
            SceneSyntax::Raw => SceneSyntax::OpenGl,
        };
        self.editor.set_syntax(syntax);
        self.set_status(format!("SYNTAX {}", syntax_label(syntax)), C_OK);
    }

    fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Digit1 => self.mode = EditorMode::Scene,
            KeyCode::Digit2 => self.mode = EditorMode::Meshes,
            KeyCode::Digit3 => self.mode = EditorMode::Lights,
            KeyCode::Digit4 => self.mode = EditorMode::Materials,
            KeyCode::F2 => self.save_current(),
            KeyCode::F5 => self.force_reload(),
            KeyCode::Tab => {
                self.mode = match self.mode {
                    EditorMode::Scene => EditorMode::Meshes,
                    EditorMode::Meshes => EditorMode::Lights,
                    EditorMode::Lights => EditorMode::Materials,
                    EditorMode::Materials => EditorMode::Scene,
                };
            }
            _ => self.handle_shortcuts(code),
        }
    }

    fn handle_shortcuts(&mut self, code: KeyCode) {
        if self.handle_camera_shortcuts(code) {
            return;
        }
        match self.mode {
            EditorMode::Meshes => {
                let Some(name) = self.selected_mesh.clone() else {
                    return;
                };
                let Some(mesh) = self.editor.document_mut().meshes.get_mut(&name) else {
                    return;
                };
                match code {
                    KeyCode::KeyJ => mesh.transform.position[0] -= 0.1,
                    KeyCode::KeyL => mesh.transform.position[0] += 0.1,
                    KeyCode::KeyI => mesh.transform.position[2] -= 0.1,
                    KeyCode::KeyK => mesh.transform.position[2] += 0.1,
                    KeyCode::KeyU => mesh.transform.position[1] += 0.1,
                    KeyCode::KeyO => mesh.transform.position[1] -= 0.1,
                    KeyCode::KeyQ => mesh.transform.rotation_degrees[1] -= 5.0,
                    KeyCode::KeyE => mesh.transform.rotation_degrees[1] += 5.0,
                    KeyCode::KeyZ => {
                        for value in &mut mesh.transform.scale {
                            *value = (*value - 0.1).max(0.1);
                        }
                    }
                    KeyCode::KeyX => {
                        for value in &mut mesh.transform.scale {
                            *value += 0.1;
                        }
                    }
                    _ => return,
                }
                self.set_status(format!("EDITED {}", name.to_uppercase()), C_OK);
            }
            EditorMode::Lights => {
                let Some(name) = self.selected_light.clone() else {
                    return;
                };
                let Some(light) = self.editor.document_mut().lights.get_mut(&name) else {
                    return;
                };
                match code {
                    KeyCode::KeyJ => light.position[0] -= 0.1,
                    KeyCode::KeyL => light.position[0] += 0.1,
                    KeyCode::KeyI => light.position[2] -= 0.1,
                    KeyCode::KeyK => light.position[2] += 0.1,
                    KeyCode::KeyU => light.position[1] += 0.1,
                    KeyCode::KeyO => light.position[1] -= 0.1,
                    KeyCode::Minus => light.intensity = (light.intensity - 0.05).max(0.0),
                    KeyCode::Equal => light.intensity += 0.05,
                    KeyCode::KeyT => {
                        light.kind = match light.kind {
                            LightKind::Point => LightKind::Directional,
                            LightKind::Directional => LightKind::Point,
                        };
                    }
                    _ => return,
                }
                self.set_status(format!("EDITED {}", name.to_uppercase()), C_OK);
            }
            _ => {}
        }
    }

    fn handle_camera_shortcuts(&mut self, code: KeyCode) -> bool {
        let delta = match code {
            KeyCode::ArrowLeft | KeyCode::KeyA => Some([-0.25, 0.0, 0.0]),
            KeyCode::ArrowRight | KeyCode::KeyD => Some([0.25, 0.0, 0.0]),
            KeyCode::ArrowUp | KeyCode::KeyW => Some([0.0, 0.0, 0.35]),
            KeyCode::ArrowDown | KeyCode::KeyS => Some([0.0, 0.0, -0.35]),
            KeyCode::PageUp => Some([0.0, 0.25, 0.0]),
            KeyCode::PageDown => Some([0.0, -0.25, 0.0]),
            _ => None,
        };
        let Some(delta) = delta else {
            return false;
        };
        translate_document_camera(self.editor.document_mut(), delta);
        self.set_status("CAMERA MOVED", C_OK);
        true
    }

    fn point_in_rect(&self, rect: UiRect) -> bool {
        self.mouse_pos[0] >= rect.x
            && self.mouse_pos[0] <= rect.x + rect.w
            && self.mouse_pos[1] >= rect.y
            && self.mouse_pos[1] <= rect.y + rect.h
    }

    fn viewport_rect(&self) -> UiRect {
        UiRect {
            x: LEFT_PANEL_WIDTH,
            y: TOP_BAR_HEIGHT,
            w: (self.window_size[0] as f32 - LEFT_PANEL_WIDTH - RIGHT_PANEL_WIDTH).max(1.0),
            h: (self.window_size[1] as f32 - TOP_BAR_HEIGHT).max(1.0),
        }
    }

    fn point_in_viewport(&self) -> bool {
        self.point_in_rect(self.viewport_rect())
    }

    fn current_orbit_pivot(&mut self) -> [f32; 3] {
        let scene_base_dir = self.scene_base_dir();
        let document = self.editor.document();
        if let Some(name) = self.selected_mesh.clone() {
            if let Some(mesh) = document.meshes.get(&name) {
                let draw = resolve_mesh_draw(
                    document,
                    mesh,
                    0,
                    scene_base_dir.as_deref(),
                    &mut self.obj_mesh_cache,
                );
                return draw.center;
            }
        }
        document.camera.target
    }

    fn scene_base_dir(&self) -> Option<PathBuf> {
        self.editor
            .scene_path()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
    }

    fn pick_mesh_in_viewport(&mut self) -> Option<String> {
        let viewport = self.viewport_rect();
        let mut best: Option<(String, f32)> = None;
        let scene_base_dir = self.scene_base_dir();
        let document = self.editor.document();
        for (name, mesh) in &document.meshes {
            if mesh.geometry.eq_ignore_ascii_case("plane") {
                continue;
            }
            let draw = resolve_mesh_draw(
                document,
                mesh,
                0,
                scene_base_dir.as_deref(),
                &mut self.obj_mesh_cache,
            );
            let Some(screen) = project_world_to_screen(
                document.camera.position,
                document.camera.target,
                document.camera.fov_degrees,
                viewport,
                draw.center,
            ) else {
                continue;
            };
            let radius = projected_mesh_radius(
                document.camera.position,
                document.camera.target,
                document.camera.fov_degrees,
                viewport,
                draw.center,
                draw.size,
            );
            let dx = self.mouse_pos[0] - screen[0];
            let dy = self.mouse_pos[1] - screen[1];
            let distance_sq = dx * dx + dy * dy;
            if distance_sq <= radius * radius {
                match &best {
                    Some((_, best_distance_sq)) if distance_sq >= *best_distance_sq => {}
                    _ => best = Some((name.clone(), distance_sq)),
                }
            }
        }
        best.map(|(name, _)| name)
    }

    fn begin_mesh_drag(&mut self) {
        if self.mode != EditorMode::Meshes || !self.point_in_viewport() {
            return;
        }
        if let Some(name) = self.pick_mesh_in_viewport() {
            self.selected_mesh = Some(name.clone());
        }
        let Some(name) = self.selected_mesh.clone() else {
            return;
        };
        let Some(mesh) = self.editor.document().meshes.get(&name) else {
            return;
        };
        let Some((ray_origin, ray_dir)) = viewport_ray(
            self.editor.document().camera.position,
            self.editor.document().camera.target,
            self.editor.document().camera.fov_degrees,
            self.viewport_rect(),
            self.mouse_pos,
        ) else {
            return;
        };
        let plane_y = mesh.transform.position[1];
        let Some(hit) = intersect_ray_with_horizontal_plane(ray_origin, ray_dir, plane_y) else {
            return;
        };
        self.drag_state = DragState::MoveMesh {
            plane_y,
            offset: sub3(mesh.transform.position, hit),
        };
        self.set_status(format!("SELECTED {}", name.to_uppercase()), C_OK);
    }

    fn update_drag(&mut self) {
        match self.drag_state {
            DragState::MoveMesh { plane_y, offset } => {
                let Some(name) = self.selected_mesh.clone() else {
                    self.drag_state = DragState::None;
                    return;
                };
                let camera = self.editor.document().camera.clone();
                let Some((ray_origin, ray_dir)) = viewport_ray(
                    camera.position,
                    camera.target,
                    camera.fov_degrees,
                    self.viewport_rect(),
                    self.mouse_pos,
                ) else {
                    return;
                };
                let Some(hit) = intersect_ray_with_horizontal_plane(ray_origin, ray_dir, plane_y)
                else {
                    return;
                };
                if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&name) {
                    mesh.transform.position[0] = hit[0] + offset[0];
                    mesh.transform.position[2] = hit[2] + offset[2];
                }
            }
            DragState::OrbitCamera { last_mouse, pivot } => {
                let delta = [
                    self.mouse_pos[0] - last_mouse[0],
                    self.mouse_pos[1] - last_mouse[1],
                ];
                if delta[0].abs() < f32::EPSILON && delta[1].abs() < f32::EPSILON {
                    return;
                }
                orbit_document_camera(self.editor.document_mut(), pivot, delta);
                self.drag_state = DragState::OrbitCamera {
                    last_mouse: self.mouse_pos,
                    pivot,
                };
            }
            DragState::None => {}
        }
    }

    fn button(
        &mut self,
        canvas: &mut Canvas2D<'_>,
        rect: UiRect,
        label: &str,
        active: bool,
    ) -> bool {
        let hovered = self.point_in_rect(rect);
        let fill = if active {
            C_BUTTON_ACTIVE
        } else if hovered {
            C_BUTTON_HOVER
        } else {
            C_BUTTON
        };
        draw_box(canvas, rect, fill, C_OUTLINE, UI_LAYER_PANEL);
        draw_text_centered(
            canvas,
            rect,
            2.0,
            label,
            if active { C_TEXT_ACCENT } else { C_TEXT },
            UI_LAYER_TEXT,
        );
        self.click_pending && hovered
    }

    fn add_mesh(&mut self, geometry: &str) {
        let name = unique_name(
            geometry,
            self.editor.document().meshes.keys().map(String::as_str),
        );
        let material = self
            .editor
            .document()
            .materials
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "default_material".to_string());
        self.editor.upsert_mesh(
            &name,
            geometry,
            None,
            material,
            None,
            NuTransform::default(),
        );
        self.selected_mesh = Some(name.clone());
        self.mode = EditorMode::Meshes;
        self.set_status(format!("ADDED {}", name.to_uppercase()), C_OK);
    }

    fn add_obj_mesh(&mut self) {
        let Some(path) = FileDialog::new()
            .add_filter("wavefront obj", &["obj"])
            .pick_file()
        else {
            return;
        };
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("obj_mesh");
        let name = unique_name(
            stem,
            self.editor.document().meshes.keys().map(String::as_str),
        );
        let material = self
            .editor
            .document()
            .materials
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "default_material".to_string());
        self.editor.upsert_mesh(
            &name,
            "obj",
            Some(path.clone()),
            material,
            None,
            NuTransform::default(),
        );
        self.selected_mesh = Some(name.clone());
        self.mode = EditorMode::Meshes;
        self.set_status(format!("IMPORTED {}", name.to_uppercase()), C_OK);
    }

    fn add_light(&mut self) {
        let name = unique_name(
            "light",
            self.editor.document().lights.keys().map(String::as_str),
        );
        self.editor.upsert_light(
            &name,
            LightKind::Point,
            [3.0, 5.0, -3.0],
            [1.0, 1.0, 1.0],
            1.0,
        );
        self.selected_light = Some(name.clone());
        self.mode = EditorMode::Lights;
        self.set_status(format!("ADDED {}", name.to_uppercase()), C_OK);
    }

    fn add_material(&mut self) {
        let name = unique_name(
            "material",
            self.editor.document().materials.keys().map(String::as_str),
        );
        self.editor
            .upsert_material(&name, "lit.vert", "lit.frag", [1.0, 1.0, 1.0], 0.5, None);
        self.selected_material = Some(name.clone());
        self.mode = EditorMode::Materials;
        self.set_status(format!("ADDED {}", name.to_uppercase()), C_OK);
    }

    fn remove_selected(&mut self) {
        match self.mode {
            EditorMode::Meshes => {
                if let Some(name) = self.selected_mesh.clone() {
                    self.editor.document_mut().meshes.remove(&name);
                    self.selected_mesh = None;
                    self.ensure_selection();
                    self.set_status(format!("REMOVED {}", name.to_uppercase()), C_WARN);
                }
            }
            EditorMode::Lights => {
                if let Some(name) = self.selected_light.clone() {
                    self.editor.document_mut().lights.remove(&name);
                    self.selected_light = None;
                    self.ensure_selection();
                    self.set_status(format!("REMOVED {}", name.to_uppercase()), C_WARN);
                }
            }
            EditorMode::Materials => {
                if let Some(name) = self.selected_material.clone() {
                    self.editor.document_mut().materials.remove(&name);
                    self.selected_material = None;
                    self.ensure_selection();
                    self.set_status(format!("REMOVED {}", name.to_uppercase()), C_WARN);
                }
            }
            EditorMode::Scene => {}
        }
    }

    fn draw_top_bar(&mut self, canvas: &mut Canvas2D<'_>) {
        let width = self.window_size[0] as f32;
        draw_box(
            canvas,
            UiRect {
                x: 0.0,
                y: 0.0,
                w: width,
                h: TOP_BAR_HEIGHT,
            },
            C_PANEL,
            C_OUTLINE,
            UI_LAYER_PANEL,
        );

        let icon_rect = UiRect {
            x: PANEL_PAD,
            y: 8.0,
            w: 30.0,
            h: 38.0,
        };
        draw_nu_icon(canvas, icon_rect, UI_LAYER_TEXT);
        draw_text(
            canvas,
            [icon_rect.x + icon_rect.w + 10.0, 17.0],
            2.1,
            "nu Editor",
            C_TEXT,
            UI_LAYER_TEXT,
        );

        let buttons = [
            ("OPEN", 78.0),
            ("SAVE", 78.0),
            ("SAVEAS", 86.0),
            ("RELOAD", 86.0),
        ];
        let mut x = 150.0;
        for (label, width) in buttons {
            let rect = UiRect {
                x,
                y: 11.0,
                w: width,
                h: BUTTON_H,
            };
            if self.button(canvas, rect, label, false) {
                match label {
                    "OPEN" => self.open_from_dialog(),
                    "SAVE" => self.save_current(),
                    "SAVEAS" => self.save_as_dialog(),
                    "RELOAD" => self.force_reload(),
                    _ => {}
                }
            }
            x += rect.w + 10.0;
        }

        let syntax_rect = UiRect {
            x,
            y: 11.0,
            w: 120.0,
            h: BUTTON_H,
        };
        if self.button(
            canvas,
            syntax_rect,
            &format!("S {}", syntax_label(self.editor.document().scene.syntax)),
            false,
        ) {
            self.cycle_syntax();
        }

        let path_text = self
            .editor
            .scene_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "UNSAVED SCENE".to_string());
        draw_text(
            canvas,
            [syntax_rect.x + syntax_rect.w + 16.0, 18.0],
            2.0,
            &truncate_middle(&path_text, 44),
            C_TEXT_MUTED,
            UI_LAYER_TEXT,
        );
    }

    fn draw_left_panel(&mut self, canvas: &mut Canvas2D<'_>) {
        let rect = UiRect {
            x: 0.0,
            y: TOP_BAR_HEIGHT,
            w: LEFT_PANEL_WIDTH,
            h: self.window_size[1] as f32 - TOP_BAR_HEIGHT,
        };
        draw_box(canvas, rect, C_PANEL_ALT, C_OUTLINE, UI_LAYER_PANEL);
        draw_text(
            canvas,
            [14.0, TOP_BAR_HEIGHT + 12.0],
            2.0,
            "OUTLINER",
            C_TEXT,
            UI_LAYER_TEXT,
        );

        let modes = [
            (EditorMode::Scene, "SCENE"),
            (EditorMode::Meshes, "MESH"),
            (EditorMode::Lights, "LIGHT"),
            (EditorMode::Materials, "MAT"),
        ];
        let mut mx = PANEL_PAD;
        for (mode, label) in modes {
            let r = UiRect {
                x: mx,
                y: TOP_BAR_HEIGHT + 38.0,
                w: 52.0,
                h: BUTTON_H,
            };
            if self.button(canvas, r, label, self.mode == mode) {
                self.mode = mode;
            }
            mx += 56.0;
        }

        let mut y = TOP_BAR_HEIGHT + 84.0;
        match self.mode {
            EditorMode::Scene => {
                draw_text(
                    canvas,
                    [14.0, y],
                    2.0,
                    &format!("NAME {}", self.editor.document().scene.name),
                    C_TEXT_MUTED,
                    UI_LAYER_TEXT,
                );
                y += 20.0;
                draw_text(
                    canvas,
                    [14.0, y],
                    2.0,
                    &format!("MESH {}", self.editor.document().meshes.len()),
                    C_TEXT_MUTED,
                    UI_LAYER_TEXT,
                );
                y += 20.0;
                draw_text(
                    canvas,
                    [14.0, y],
                    2.0,
                    &format!("LIGHT {}", self.editor.document().lights.len()),
                    C_TEXT_MUTED,
                    UI_LAYER_TEXT,
                );
                y += 20.0;
                draw_text(
                    canvas,
                    [14.0, y],
                    2.0,
                    &format!("MAT {}", self.editor.document().materials.len()),
                    C_TEXT_MUTED,
                    UI_LAYER_TEXT,
                );
            }
            EditorMode::Meshes => {
                let mesh_names: Vec<String> =
                    self.editor.document().meshes.keys().cloned().collect();
                for mesh_name in mesh_names {
                    let selected = self.selected_mesh.as_deref() == Some(mesh_name.as_str());
                    let r = UiRect {
                        x: PANEL_PAD,
                        y,
                        w: LEFT_PANEL_WIDTH - PANEL_PAD * 2.0,
                        h: BUTTON_H,
                    };
                    if self.button(canvas, r, &mesh_name, selected) {
                        self.selected_mesh = Some(mesh_name);
                    }
                    y += BUTTON_H + 8.0;
                }
                let add_cube = UiRect {
                    x: PANEL_PAD,
                    y: rect.y + rect.h - 92.0,
                    w: 104.0,
                    h: BUTTON_H,
                };
                let add_plane = UiRect {
                    x: PANEL_PAD + 114.0,
                    y: rect.y + rect.h - 92.0,
                    w: 104.0,
                    h: BUTTON_H,
                };
                let add_sphere = UiRect {
                    x: PANEL_PAD,
                    y: rect.y + rect.h - 52.0,
                    w: 104.0,
                    h: BUTTON_H,
                };
                let add_obj = UiRect {
                    x: PANEL_PAD + 114.0,
                    y: rect.y + rect.h - 52.0,
                    w: 104.0,
                    h: BUTTON_H,
                };
                if self.button(canvas, add_cube, "+ CUBE", false) {
                    self.add_mesh("cube");
                }
                if self.button(canvas, add_plane, "+ PLANE", false) {
                    self.add_mesh("plane");
                }
                if self.button(canvas, add_sphere, "+ SPHERE", false) {
                    self.add_mesh("sphere");
                }
                if self.button(canvas, add_obj, "+ OBJ", false) {
                    self.add_obj_mesh();
                }
            }
            EditorMode::Lights => {
                let light_names: Vec<String> =
                    self.editor.document().lights.keys().cloned().collect();
                for light_name in light_names {
                    let selected = self.selected_light.as_deref() == Some(light_name.as_str());
                    let r = UiRect {
                        x: PANEL_PAD,
                        y,
                        w: LEFT_PANEL_WIDTH - PANEL_PAD * 2.0,
                        h: BUTTON_H,
                    };
                    if self.button(canvas, r, &light_name, selected) {
                        self.selected_light = Some(light_name);
                    }
                    y += BUTTON_H + 8.0;
                }
                if self.button(
                    canvas,
                    UiRect {
                        x: PANEL_PAD,
                        y: rect.y + rect.h - 92.0,
                        w: LEFT_PANEL_WIDTH - PANEL_PAD * 2.0,
                        h: BUTTON_H,
                    },
                    "+ LIGHT",
                    false,
                ) {
                    self.add_light();
                }
            }
            EditorMode::Materials => {
                let material_names: Vec<String> =
                    self.editor.document().materials.keys().cloned().collect();
                for material_name in material_names {
                    let selected =
                        self.selected_material.as_deref() == Some(material_name.as_str());
                    let r = UiRect {
                        x: PANEL_PAD,
                        y,
                        w: LEFT_PANEL_WIDTH - PANEL_PAD * 2.0,
                        h: BUTTON_H,
                    };
                    if self.button(canvas, r, &material_name, selected) {
                        self.selected_material = Some(material_name);
                    }
                    y += BUTTON_H + 8.0;
                }
                if self.button(
                    canvas,
                    UiRect {
                        x: PANEL_PAD,
                        y: rect.y + rect.h - 92.0,
                        w: LEFT_PANEL_WIDTH - PANEL_PAD * 2.0,
                        h: BUTTON_H,
                    },
                    "+ MAT",
                    false,
                ) {
                    self.add_material();
                }
            }
        }
    }

    fn draw_right_panel(&mut self, canvas: &mut Canvas2D<'_>) {
        let rect = UiRect {
            x: self.window_size[0] as f32 - RIGHT_PANEL_WIDTH,
            y: TOP_BAR_HEIGHT,
            w: RIGHT_PANEL_WIDTH,
            h: self.window_size[1] as f32 - TOP_BAR_HEIGHT,
        };
        draw_box(canvas, rect, C_PANEL_ALT, C_OUTLINE, UI_LAYER_PANEL);
        draw_text(
            canvas,
            [rect.x + 14.0, rect.y + 12.0],
            2.0,
            "INSPECTOR",
            C_TEXT,
            UI_LAYER_TEXT,
        );

        let mut y = rect.y + 42.0;
        match self.mode {
            EditorMode::Scene => {
                draw_text(
                    canvas,
                    [rect.x + 14.0, y],
                    2.0,
                    &format!(
                        "SYNTAX {}",
                        syntax_label(self.editor.document().scene.syntax)
                    ),
                    C_TEXT_MUTED,
                    UI_LAYER_TEXT,
                );
                y += 22.0;
                draw_text(
                    canvas,
                    [rect.x + 14.0, y],
                    2.0,
                    "F2 SAVE",
                    C_TEXT_MUTED,
                    UI_LAYER_TEXT,
                );
                y += 18.0;
                draw_text(
                    canvas,
                    [rect.x + 14.0, y],
                    2.0,
                    "F5 RELOAD",
                    C_TEXT_MUTED,
                    UI_LAYER_TEXT,
                );
                y += 18.0;
                draw_text(
                    canvas,
                    [rect.x + 14.0, y],
                    2.0,
                    "1 2 3 4 MODE",
                    C_TEXT_MUTED,
                    UI_LAYER_TEXT,
                );
            }
            EditorMode::Meshes => self.draw_mesh_inspector(canvas, rect, &mut y),
            EditorMode::Lights => self.draw_light_inspector(canvas, rect, &mut y),
            EditorMode::Materials => self.draw_material_inspector(canvas, rect, &mut y),
        }

        let footer_y = rect.y + rect.h - 96.0;
        draw_text(
            canvas,
            [rect.x + 14.0, footer_y],
            2.0,
            &truncate_middle(&self.status, 34),
            self.status_color,
            UI_LAYER_TEXT,
        );
        if let Some(batch) = &self.last_reload {
            draw_text(
                canvas,
                [rect.x + 14.0, footer_y + 18.0],
                2.0,
                &format!("{} SH {} TX", batch.shaders.len(), batch.textures.len()),
                C_TEXT_MUTED,
                UI_LAYER_TEXT,
            );
        }
        if self.mode != EditorMode::Scene
            && self.button(
                canvas,
                UiRect {
                    x: rect.x + 14.0,
                    y: rect.y + rect.h - 48.0,
                    w: rect.w - 28.0,
                    h: BUTTON_H,
                },
                "REMOVE",
                false,
            )
        {
            self.remove_selected();
        }
    }

    fn draw_viewport_overlay(&mut self, canvas: &mut Canvas2D<'_>) {
        let viewport = self.viewport_rect();
        canvas.stroke_rect(
            [viewport.x + viewport.w * 0.5, viewport.y + viewport.h * 0.5],
            [viewport.w, viewport.h],
            0.0,
            C_OUTLINE,
            1.0,
            UI_LAYER_PANEL,
        );
        draw_text(
            canvas,
            [viewport.x + 14.0, viewport.y + 12.0],
            1.8,
            "LMB SELECT/DRAG  RMB ORBIT  WHEEL ZOOM  ARROWS/WASD CAMERA",
            C_TEXT_MUTED,
            UI_LAYER_TEXT,
        );

        if let Some(name) = &self.selected_mesh {
            let scene_base_dir = self.scene_base_dir();
            let document = self.editor.document();
            if let Some(mesh) = document.meshes.get(name) {
                let draw = resolve_mesh_draw(
                    document,
                    mesh,
                    0,
                    scene_base_dir.as_deref(),
                    &mut self.obj_mesh_cache,
                );
                if let Some(screen) = project_world_to_screen(
                    document.camera.position,
                    document.camera.target,
                    document.camera.fov_degrees,
                    viewport,
                    draw.center,
                ) {
                    canvas.stroke_rect(
                        screen,
                        [28.0, 28.0],
                        0.0,
                        C_TEXT_ACCENT,
                        2.0,
                        UI_LAYER_TEXT,
                    );
                    draw_text(
                        canvas,
                        [screen[0] + 18.0, screen[1] - 10.0],
                        1.8,
                        name,
                        C_TEXT_ACCENT,
                        UI_LAYER_TEXT,
                    );
                }
            }
        }
    }

    fn draw_mesh_inspector(&mut self, canvas: &mut Canvas2D<'_>, rect: UiRect, y: &mut f32) {
        let Some(name) = self.selected_mesh.clone() else {
            draw_text(
                canvas,
                [rect.x + 14.0, *y],
                2.0,
                "NO MESH",
                C_TEXT_MUTED,
                UI_LAYER_TEXT,
            );
            return;
        };
        let Some(mesh_snapshot) = self.editor.document().meshes.get(&name).cloned() else {
            return;
        };
        let material_names: Vec<String> =
            self.editor.document().materials.keys().cloned().collect();
        let mut toggle_geo = false;
        let mut next_mat = false;
        let mut pick_obj = false;
        let mut clear_obj = false;
        draw_text(
            canvas,
            [rect.x + 14.0, *y],
            2.0,
            &name,
            C_TEXT,
            UI_LAYER_TEXT,
        );
        *y += 24.0;
        if self.button(
            canvas,
            UiRect {
                x: rect.x + 14.0,
                y: *y,
                w: rect.w - 28.0,
                h: BUTTON_H,
            },
            &format!("G {}", mesh_snapshot.geometry),
            false,
        ) {
            toggle_geo = true;
        }
        *y += BUTTON_H + 8.0;
        if self.button(
            canvas,
            UiRect {
                x: rect.x + 14.0,
                y: *y,
                w: rect.w - 28.0,
                h: BUTTON_H,
            },
            &format!("M {}", mesh_snapshot.material),
            false,
        ) {
            next_mat = true;
        }
        *y += BUTTON_H + 8.0;
        draw_text(
            canvas,
            [rect.x + 14.0, *y],
            2.0,
            &format!("P {}", format_vec3(mesh_snapshot.transform.position)),
            C_TEXT_MUTED,
            UI_LAYER_TEXT,
        );
        *y += 18.0;
        draw_text(
            canvas,
            [rect.x + 14.0, *y],
            2.0,
            &format!(
                "R {}",
                format_vec3(mesh_snapshot.transform.rotation_degrees)
            ),
            C_TEXT_MUTED,
            UI_LAYER_TEXT,
        );
        *y += 18.0;
        draw_text(
            canvas,
            [rect.x + 14.0, *y],
            2.0,
            &format!("S {}", format_vec3(mesh_snapshot.transform.scale)),
            C_TEXT_MUTED,
            UI_LAYER_TEXT,
        );
        *y += 24.0;
        if mesh_snapshot.geometry.eq_ignore_ascii_case("obj") {
            draw_text(
                canvas,
                [rect.x + 14.0, *y],
                2.0,
                &format!(
                    "SRC {}",
                    truncate_middle(
                        &mesh_snapshot
                            .source
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "MISSING".to_string()),
                        26
                    )
                ),
                C_TEXT_MUTED,
                UI_LAYER_TEXT,
            );
            *y += 18.0;
            let pick_rect = UiRect {
                x: rect.x + 14.0,
                y: *y,
                w: (rect.w - 38.0) * 0.5,
                h: BUTTON_H,
            };
            let clear_rect = UiRect {
                x: pick_rect.x + pick_rect.w + 10.0,
                y: *y,
                w: pick_rect.w,
                h: BUTTON_H,
            };
            pick_obj = self.button(canvas, pick_rect, "PICK OBJ", false);
            clear_obj = self.button(canvas, clear_rect, "CLEAR OBJ", false);
            *y += BUTTON_H + 12.0;
        }
        draw_text(
            canvas,
            [rect.x + 14.0, *y],
            2.0,
            "DRAG OR I/J/K/L/U/O MOVE",
            C_TEXT_MUTED,
            UI_LAYER_TEXT,
        );
        *y += 18.0;
        draw_text(
            canvas,
            [rect.x + 14.0, *y],
            2.0,
            "Q/E ROTATE",
            C_TEXT_MUTED,
            UI_LAYER_TEXT,
        );
        *y += 18.0;
        draw_text(
            canvas,
            [rect.x + 14.0, *y],
            2.0,
            "Z/X SCALE",
            C_TEXT_MUTED,
            UI_LAYER_TEXT,
        );
        if toggle_geo {
            if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&name) {
                mesh.geometry = if mesh.geometry.eq_ignore_ascii_case("cube") {
                    "plane".to_string()
                } else if mesh.geometry.eq_ignore_ascii_case("plane") {
                    "sphere".to_string()
                } else {
                    "cube".to_string()
                };
            }
        }
        if pick_obj {
            if let Some(path) = FileDialog::new()
                .add_filter("wavefront obj", &["obj"])
                .pick_file()
            {
                if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&name) {
                    mesh.geometry = "obj".to_string();
                    mesh.source = Some(path.clone());
                }
                self.obj_mesh_cache.remove(&resolve_scene_asset_path(
                    self.scene_base_dir().as_deref(),
                    &path,
                ));
                self.set_status(format!("LINKED {}", name.to_uppercase()), C_OK);
            }
        }
        if clear_obj {
            let scene_base_dir = self.scene_base_dir();
            if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&name) {
                if let Some(source) = mesh.source.take() {
                    self.obj_mesh_cache.remove(&resolve_scene_asset_path(
                        scene_base_dir.as_deref(),
                        &source,
                    ));
                }
                mesh.geometry = "cube".to_string();
            }
            self.set_status(format!("UNLINKED {}", name.to_uppercase()), C_WARN);
        }
        if next_mat && !material_names.is_empty() {
            if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&name) {
                mesh.material = cycle_name(&material_names, &mesh.material, 1);
            }
        }
    }

    fn draw_light_inspector(&mut self, canvas: &mut Canvas2D<'_>, rect: UiRect, y: &mut f32) {
        let Some(name) = self.selected_light.clone() else {
            draw_text(
                canvas,
                [rect.x + 14.0, *y],
                2.0,
                "NO LIGHT",
                C_TEXT_MUTED,
                UI_LAYER_TEXT,
            );
            return;
        };
        let Some(light_snapshot) = self.editor.document().lights.get(&name).cloned() else {
            return;
        };
        let mut toggle_kind = false;
        draw_text(
            canvas,
            [rect.x + 14.0, *y],
            2.0,
            &name,
            C_TEXT,
            UI_LAYER_TEXT,
        );
        *y += 24.0;
        if self.button(
            canvas,
            UiRect {
                x: rect.x + 14.0,
                y: *y,
                w: rect.w - 28.0,
                h: BUTTON_H,
            },
            &format!("TYPE {}", light_kind_label(light_snapshot.kind)),
            false,
        ) {
            toggle_kind = true;
        }
        *y += BUTTON_H + 8.0;
        draw_text(
            canvas,
            [rect.x + 14.0, *y],
            2.0,
            &format!("P {}", format_vec3(light_snapshot.position)),
            C_TEXT_MUTED,
            UI_LAYER_TEXT,
        );
        *y += 18.0;
        draw_text(
            canvas,
            [rect.x + 14.0, *y],
            2.0,
            &format!("I {:.2}", light_snapshot.intensity),
            C_TEXT_MUTED,
            UI_LAYER_TEXT,
        );
        *y += 24.0;
        draw_text(
            canvas,
            [rect.x + 14.0, *y],
            2.0,
            "I/J/K/L/U/O MOVE",
            C_TEXT_MUTED,
            UI_LAYER_TEXT,
        );
        *y += 18.0;
        draw_text(
            canvas,
            [rect.x + 14.0, *y],
            2.0,
            "-/= INTENSITY",
            C_TEXT_MUTED,
            UI_LAYER_TEXT,
        );
        *y += 18.0;
        draw_text(
            canvas,
            [rect.x + 14.0, *y],
            2.0,
            "T TOGGLE",
            C_TEXT_MUTED,
            UI_LAYER_TEXT,
        );
        if toggle_kind {
            if let Some(light) = self.editor.document_mut().lights.get_mut(&name) {
                light.kind = match light.kind {
                    LightKind::Point => LightKind::Directional,
                    LightKind::Directional => LightKind::Point,
                };
            }
        }
    }

    fn draw_material_inspector(&mut self, canvas: &mut Canvas2D<'_>, rect: UiRect, y: &mut f32) {
        let Some(name) = self.selected_material.clone() else {
            draw_text(
                canvas,
                [rect.x + 14.0, *y],
                2.0,
                "NO MAT",
                C_TEXT_MUTED,
                UI_LAYER_TEXT,
            );
            return;
        };
        let Some(material_snapshot) = self.editor.document().materials.get(&name).cloned() else {
            return;
        };
        let mut pick_vert = false;
        let mut pick_frag = false;
        let mut pick_tex = false;
        let mut clear_tex = false;
        let rough_down;
        let rough_up;
        draw_text(
            canvas,
            [rect.x + 14.0, *y],
            2.0,
            &name,
            C_TEXT,
            UI_LAYER_TEXT,
        );
        *y += 24.0;
        if self.button(
            canvas,
            UiRect {
                x: rect.x + 14.0,
                y: *y,
                w: rect.w - 28.0,
                h: BUTTON_H,
            },
            "PICK VERT",
            false,
        ) {
            pick_vert = true;
        }
        *y += BUTTON_H + 8.0;
        if self.button(
            canvas,
            UiRect {
                x: rect.x + 14.0,
                y: *y,
                w: rect.w - 28.0,
                h: BUTTON_H,
            },
            "PICK FRAG",
            false,
        ) {
            pick_frag = true;
        }
        *y += BUTTON_H + 8.0;
        if self.button(
            canvas,
            UiRect {
                x: rect.x + 14.0,
                y: *y,
                w: rect.w - 28.0,
                h: BUTTON_H,
            },
            "PICK TEX",
            false,
        ) {
            pick_tex = true;
        }
        *y += BUTTON_H + 8.0;
        if self.button(
            canvas,
            UiRect {
                x: rect.x + 14.0,
                y: *y,
                w: rect.w - 28.0,
                h: BUTTON_H,
            },
            "CLEAR TEX",
            false,
        ) {
            clear_tex = true;
        }
        *y += BUTTON_H + 8.0;
        let down = UiRect {
            x: rect.x + 14.0,
            y: *y,
            w: 44.0,
            h: BUTTON_H,
        };
        let up = UiRect {
            x: rect.x + 66.0,
            y: *y,
            w: 44.0,
            h: BUTTON_H,
        };
        rough_down = self.button(canvas, down, "R-", false);
        rough_up = self.button(canvas, up, "R+", false);
        draw_text(
            canvas,
            [rect.x + 128.0, *y + 10.0],
            2.0,
            &format!("ROUGH {:.2}", material_snapshot.roughness),
            C_TEXT_MUTED,
            UI_LAYER_TEXT,
        );
        *y += BUTTON_H + 12.0;
        draw_text(
            canvas,
            [rect.x + 14.0, *y],
            2.0,
            &format!("COLOR {}", format_vec3(material_snapshot.color)),
            C_TEXT_MUTED,
            UI_LAYER_TEXT,
        );
        if rough_down {
            if let Some(material) = self.editor.document_mut().materials.get_mut(&name) {
                material.roughness = (material.roughness - 0.05).max(0.0);
            }
        }
        if rough_up {
            if let Some(material) = self.editor.document_mut().materials.get_mut(&name) {
                material.roughness = (material.roughness + 0.05).min(1.0);
            }
        }
        if pick_vert {
            if let Some(path) = FileDialog::new()
                .add_filter("vertex shader", &["vert", "glsl", "spv"])
                .pick_file()
            {
                if let Some(material) = self.editor.document_mut().materials.get_mut(&name) {
                    material.shader_vertex = path;
                }
            }
        }
        if pick_frag {
            if let Some(path) = FileDialog::new()
                .add_filter("fragment shader", &["frag", "glsl", "spv"])
                .pick_file()
            {
                if let Some(material) = self.editor.document_mut().materials.get_mut(&name) {
                    material.shader_fragment = path;
                }
            }
        }
        if pick_tex {
            if let Some(path) = FileDialog::new()
                .add_filter("albedo texture", &["png", "jpg", "jpeg", "bmp", "tga"])
                .pick_file()
            {
                if let Some(material) = self.editor.document_mut().materials.get_mut(&name) {
                    material.albedo_texture = Some(path);
                }
            }
        }
        if clear_tex {
            if let Some(material) = self.editor.document_mut().materials.get_mut(&name) {
                material.albedo_texture = None;
            }
        }
    }
}

impl Scene for BasicEditorScene {
    fn config(&self) -> SceneConfig {
        let document = self.editor.document();
        let mut lighting = LightingConfig::default();
        if let Some(environment) = &document.environment {
            lighting.ambient_color = environment.ambient_color;
            lighting.ambient_intensity = environment.ambient_intensity;
        }
        for light in document.lights.values() {
            match light.kind {
                LightKind::Point => {
                    lighting.point_light = PointLight {
                        position: light.position,
                        color: light.color,
                        intensity: light.intensity,
                        range: 18.0,
                    };
                    break;
                }
                LightKind::Directional => {
                    lighting.fill_light = DirectionalLight {
                        direction: normalize3([
                            -light.position[0],
                            -light.position[1],
                            -light.position[2],
                        ]),
                        color: light.color,
                        intensity: light.intensity,
                    };
                }
            }
        }
        let mut api = ApiConfig::default();
        api.enable_validation = false;

        SceneConfig {
            window: WindowConfig {
                title: "nu Editor".to_string(),
                width: WINDOW_WIDTH,
                height: WINDOW_HEIGHT,
            },
            api,
            clear_color: [0.02, 0.025, 0.035, 1.0],
            camera: Camera2D::default(),
            camera_3d: Camera3D {
                position: document.camera.position,
                target: document.camera.target,
                up: [0.0, 1.0, 0.0],
                fov_y_degrees: document.camera.fov_degrees,
                near_clip: 0.1,
                far_clip: 200.0,
            },
            lighting,
        }
    }

    fn window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = [position.x as f32, position.y as f32];
                if !matches!(self.drag_state, DragState::None) {
                    self.update_drag();
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => match state {
                ElementState::Pressed => {
                    self.left_mouse_down = true;
                    self.click_pending = false;
                    self.begin_mesh_drag();
                }
                ElementState::Released => {
                    self.click_pending =
                        self.left_mouse_down && matches!(self.drag_state, DragState::None);
                    self.left_mouse_down = false;
                    if matches!(self.drag_state, DragState::MoveMesh { .. }) {
                        self.drag_state = DragState::None;
                    }
                }
            },
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Right,
                ..
            } => match state {
                ElementState::Pressed => {
                    self.right_mouse_down = true;
                    if self.point_in_viewport() {
                        let pivot = self.current_orbit_pivot();
                        self.drag_state = DragState::OrbitCamera {
                            last_mouse: self.mouse_pos,
                            pivot,
                        };
                    }
                }
                ElementState::Released => {
                    self.right_mouse_down = false;
                    if matches!(self.drag_state, DragState::OrbitCamera { .. }) {
                        self.drag_state = DragState::None;
                    }
                }
            },
            WindowEvent::MouseWheel { delta, .. } => {
                if self.point_in_viewport() {
                    let amount = match delta {
                        MouseScrollDelta::LineDelta(_, y) => *y * 0.6,
                        MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.02,
                    };
                    if amount.abs() > f32::EPSILON {
                        zoom_document_camera(self.editor.document_mut(), amount);
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    if let PhysicalKey::Code(code) = event.physical_key {
                        self.handle_key(code);
                    }
                }
            }
            WindowEvent::Resized(size) => {
                self.window_size = [size.width, size.height];
            }
            _ => {}
        }
    }

    fn populate(&mut self, frame: &mut SceneFrame) {
        self.ensure_selection();
        let scene_base_dir = self.scene_base_dir();
        let document = self.editor.document();
        populate_scene_preview(
            frame,
            document,
            self.selected_mesh.as_deref(),
            scene_base_dir.as_deref(),
            &mut self.obj_mesh_cache,
        );
        let mut canvas = frame.ui_canvas();
        self.draw_viewport_overlay(&mut canvas);
        self.draw_top_bar(&mut canvas);
        self.draw_left_panel(&mut canvas);
        self.draw_right_panel(&mut canvas);
        self.click_pending = false;
    }
}

fn populate_scene_preview(
    frame: &mut SceneFrame,
    document: &NuSceneDocument,
    selected_mesh: Option<&str>,
    scene_base_dir: Option<&Path>,
    obj_mesh_cache: &mut HashMap<PathBuf, Result<Arc<MeshAsset3D>, String>>,
) {
    draw_editor_grid(frame);
    for (mesh_name, mesh) in &document.meshes {
        let mut draw = resolve_mesh_draw(document, mesh, 0, scene_base_dir, obj_mesh_cache);
        if selected_mesh == Some(mesh_name.as_str()) {
            draw.color = brighten_color(draw.color, 0.18);
        }
        frame.draw_mesh_3d(draw);
    }
}

fn draw_editor_grid(frame: &mut SceneFrame) {
    const GRID_EXTENT: i32 = 12;
    const GRID_STEP: f32 = 1.0;
    const GRID_Y: f32 = -0.01;
    const GRID_THICKNESS: f32 = 0.03;
    let span = GRID_EXTENT as f32 * GRID_STEP;

    for index in -GRID_EXTENT..=GRID_EXTENT {
        let position = index as f32 * GRID_STEP;
        let major = index == 0 || index % 5 == 0;
        let color = if index == 0 {
            [0.65, 0.16, 0.16, 1.0]
        } else if major {
            [0.24, 0.26, 0.30, 1.0]
        } else {
            [0.14, 0.15, 0.18, 1.0]
        };

        frame.draw_mesh_3d(MeshDraw3D {
            mesh: Mesh3D::Plane,
            center: [position, GRID_Y, 0.0],
            size: [GRID_THICKNESS, 1.0, span * 2.0],
            rotation_radians: [0.0, 0.0, 0.0],
            color,
            material: MeshMaterial3D::default(),
        });
        frame.draw_mesh_3d(MeshDraw3D {
            mesh: Mesh3D::Plane,
            center: [0.0, GRID_Y, position],
            size: [span * 2.0, 1.0, GRID_THICKNESS],
            rotation_radians: [0.0, 0.0, 0.0],
            color: if index == 0 {
                [0.16, 0.30, 0.65, 1.0]
            } else {
                color
            },
            material: MeshMaterial3D::default(),
        });
    }
}

fn resolve_mesh_draw(
    scene: &NuSceneDocument,
    mesh: &NuMeshSection,
    depth: usize,
    scene_base_dir: Option<&Path>,
    obj_mesh_cache: &mut HashMap<PathBuf, Result<Arc<MeshAsset3D>, String>>,
) -> MeshDraw3D {
    let mut center = mesh.transform.position;
    let mut rotation = [
        mesh.transform.rotation_degrees[0].to_radians(),
        mesh.transform.rotation_degrees[1].to_radians(),
        mesh.transform.rotation_degrees[2].to_radians(),
    ];
    let mut scale = mesh.transform.scale;
    if depth < 16 {
        if let Some(parent) = &mesh.parent {
            if let Some(parent_name) = parent.strip_prefix("mesh.") {
                if let Some(parent_mesh) = scene.meshes.get(parent_name) {
                    let parent_draw = resolve_mesh_draw(
                        scene,
                        parent_mesh,
                        depth + 1,
                        scene_base_dir,
                        obj_mesh_cache,
                    );
                    center = add3(parent_draw.center, center);
                    rotation = add3(rotation, parent_draw.rotation_radians);
                    scale = mul3(scale, parent_draw.size);
                }
            }
        }
    }
    let material = scene.materials.get(&mesh.material);
    let mesh_kind = if mesh.geometry.eq_ignore_ascii_case("plane") {
        Mesh3D::Plane
    } else if mesh.geometry.eq_ignore_ascii_case("sphere") {
        Mesh3D::Sphere
    } else if mesh.geometry.eq_ignore_ascii_case("obj") {
        if let Some(source) = &mesh.source {
            match resolve_obj_mesh_asset(scene_base_dir, source, obj_mesh_cache) {
                Ok(asset) => {
                    scale = mul3(scale, asset.base_size);
                    Mesh3D::Custom(asset)
                }
                Err(_) => Mesh3D::Cube,
            }
        } else {
            Mesh3D::Cube
        }
    } else {
        Mesh3D::Cube
    };
    let color = match &mesh_kind {
        Mesh3D::Plane => material.map_or([0.22, 0.24, 0.28, 1.0], |material| {
            [
                material.color[0] * 0.35,
                material.color[1] * 0.35,
                material.color[2] * 0.35,
                1.0,
            ]
        }),
        Mesh3D::Cube => material.map_or([0.85, 0.15, 0.15, 1.0], |material| {
            [material.color[0], material.color[1], material.color[2], 1.0]
        }),
        Mesh3D::Sphere => material.map_or([0.84, 0.20, 0.18, 1.0], |material| {
            [material.color[0], material.color[1], material.color[2], 1.0]
        }),
        Mesh3D::Custom(_) => material.map_or([0.84, 0.20, 0.18, 1.0], |material| {
            [material.color[0], material.color[1], material.color[2], 1.0]
        }),
    };
    MeshDraw3D {
        mesh: mesh_kind,
        center,
        size: scale,
        rotation_radians: rotation,
        color,
        material: MeshMaterial3D::default(),
    }
}

fn resolve_obj_mesh_asset(
    scene_base_dir: Option<&Path>,
    source: &Path,
    obj_mesh_cache: &mut HashMap<PathBuf, Result<Arc<MeshAsset3D>, String>>,
) -> Result<Arc<MeshAsset3D>, String> {
    let resolved_path = resolve_scene_asset_path(scene_base_dir, source);
    let entry = obj_mesh_cache
        .entry(resolved_path.clone())
        .or_insert_with(|| load_obj_mesh_asset(&resolved_path).map_err(|error| error.to_string()));
    entry.clone()
}

fn resolve_scene_asset_path(scene_base_dir: Option<&Path>, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else if let Some(base_dir) = scene_base_dir {
        base_dir.join(path)
    } else {
        path.to_path_buf()
    }
}

#[derive(Clone, Copy)]
struct UiRect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

fn draw_box(
    canvas: &mut Canvas2D<'_>,
    rect: UiRect,
    fill: [f32; 4],
    outline: [f32; 4],
    layer: i32,
) {
    canvas.fill_rect(
        [rect.x + rect.w * 0.5, rect.y + rect.h * 0.5],
        [rect.w, rect.h],
        0.0,
        fill,
        layer,
    );
    canvas.stroke_rect(
        [rect.x + rect.w * 0.5, rect.y + rect.h * 0.5],
        [rect.w, rect.h],
        0.0,
        outline,
        1.0,
        layer + 1,
    );
}

fn draw_text(
    canvas: &mut Canvas2D<'_>,
    origin: [f32; 2],
    scale: f32,
    text: &str,
    color: [f32; 4],
    layer: i32,
) {
    let pixel_size = (scale * 9.0).round().clamp(12.0, 32.0);
    canvas.text(origin, pixel_size, text, color, layer);
}

fn draw_text_centered(
    canvas: &mut Canvas2D<'_>,
    rect: UiRect,
    scale: f32,
    text: &str,
    color: [f32; 4],
    layer: i32,
) {
    let pixel_size = (scale * 9.0).round().clamp(12.0, 32.0);
    canvas.text_centered(
        [rect.x + rect.w * 0.5, rect.y + rect.h * 0.5],
        pixel_size,
        text,
        color,
        layer,
    );
}

fn draw_nu_icon(canvas: &mut Canvas2D<'_>, rect: UiRect, layer: i32) {
    let map = |x: f32, y: f32| -> [f32; 2] { [rect.x + rect.w * x, rect.y + rect.h * y] };

    let top = [
        map(0.5, 0.02),
        map(0.98, 0.24),
        map(0.5, 0.46),
        map(0.02, 0.24),
    ];
    let left = [
        map(0.02, 0.24),
        map(0.5, 0.46),
        map(0.5, 0.98),
        map(0.02, 0.76),
    ];
    let right = [
        map(0.5, 0.46),
        map(0.98, 0.24),
        map(0.98, 0.76),
        map(0.5, 0.98),
    ];

    canvas.fill_quad(left, [0.53, 0.0, 0.0, 1.0], layer);
    canvas.fill_quad(right, [0.93, 0.13, 0.08, 1.0], layer + 1);
    canvas.fill_quad(top, [1.0, 0.44, 0.33, 1.0], layer + 2);

    let edge = [1.0, 1.0, 1.0, 0.92];
    canvas.line(top[0], top[2], 1.0, edge, layer + 3);
    canvas.line(top[0], top[1], 1.0, edge, layer + 3);
    canvas.line(top[1], top[2], 1.0, edge, layer + 3);
    canvas.line(top[2], top[3], 1.0, edge, layer + 3);
    canvas.line(top[3], top[0], 1.0, edge, layer + 3);
    canvas.line(left[0], left[3], 1.0, edge, layer + 3);
    canvas.line(left[3], left[2], 1.0, edge, layer + 3);
    canvas.line(left[2], left[1], 1.0, edge, layer + 3);
    canvas.line(right[1], right[2], 1.0, edge, layer + 3);
    canvas.line(right[2], right[3], 1.0, edge, layer + 3);
}

fn unique_name<'a>(base: &str, existing: impl Iterator<Item = &'a str>) -> String {
    let existing: Vec<&str> = existing.collect();
    if !existing.iter().any(|name| name.eq_ignore_ascii_case(base)) {
        return base.to_string();
    }
    for index in 1..1000 {
        let candidate = format!("{}_{}", base, index);
        if !existing
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&candidate))
        {
            return candidate;
        }
    }
    format!("{}_x", base)
}

fn cycle_name(names: &[String], current: &str, step: isize) -> String {
    if names.is_empty() {
        return current.to_string();
    }
    let index = names
        .iter()
        .position(|name| name.eq_ignore_ascii_case(current))
        .unwrap_or(0) as isize;
    let next = (index + step).rem_euclid(names.len() as isize) as usize;
    names[next].clone()
}

fn truncate_middle(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        return text.to_string();
    }
    let left_len = (max_len.saturating_sub(3)) / 2;
    let right_len = max_len.saturating_sub(3) - left_len;
    let left: String = text.chars().take(left_len).collect();
    let right: String = text
        .chars()
        .rev()
        .take(right_len)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{left}...{right}")
}

fn brighten_color(color: [f32; 4], amount: f32) -> [f32; 4] {
    [
        (color[0] + amount).min(1.0),
        (color[1] + amount).min(1.0),
        (color[2] + amount).min(1.0),
        color[3],
    ]
}

fn format_vec3(value: [f32; 3]) -> String {
    format!("{:.2} {:.2} {:.2}", value[0], value[1], value[2])
}

fn syntax_label(syntax: SceneSyntax) -> &'static str {
    match syntax {
        SceneSyntax::OpenGl => "OPENGL",
        SceneSyntax::Vulkan => "VULKAN",
        SceneSyntax::Raw => "RAW",
    }
}

fn light_kind_label(kind: LightKind) -> &'static str {
    match kind {
        LightKind::Point => "POINT",
        LightKind::Directional => "DIR",
    }
}

fn project_world_to_screen(
    camera_position: [f32; 3],
    camera_target: [f32; 3],
    fov_degrees: f32,
    viewport: UiRect,
    point: [f32; 3],
) -> Option<[f32; 2]> {
    let aspect = (viewport.w / viewport.h.max(1.0)).max(0.0001);
    let (forward, right, up) = camera_basis(camera_position, camera_target);
    let local = sub3(point, camera_position);
    let x = dot3(local, right);
    let y = dot3(local, up);
    let z = dot3(local, forward);
    if z <= 0.01 {
        return None;
    }

    let tan_half = (fov_degrees.to_radians() * 0.5).tan().max(0.0001);
    let ndc_x = x / (z * tan_half * aspect);
    let ndc_y = y / (z * tan_half);
    if ndc_x.abs() > 1.2 || ndc_y.abs() > 1.2 {
        return None;
    }

    Some([
        viewport.x + (ndc_x * 0.5 + 0.5) * viewport.w,
        viewport.y + (0.5 - ndc_y * 0.5) * viewport.h,
    ])
}

fn projected_mesh_radius(
    camera_position: [f32; 3],
    camera_target: [f32; 3],
    fov_degrees: f32,
    viewport: UiRect,
    center: [f32; 3],
    size: [f32; 3],
) -> f32 {
    let (forward, _, _) = camera_basis(camera_position, camera_target);
    let depth = dot3(sub3(center, camera_position), forward).max(0.1);
    let world_radius = size[0].max(size[1]).max(size[2]) * 0.65;
    let tan_half = (fov_degrees.to_radians() * 0.5).tan().max(0.0001);
    (world_radius / (depth * tan_half)) * viewport.h * 0.5
}

fn viewport_ray(
    camera_position: [f32; 3],
    camera_target: [f32; 3],
    fov_degrees: f32,
    viewport: UiRect,
    mouse: [f32; 2],
) -> Option<([f32; 3], [f32; 3])> {
    if viewport.w <= 0.0 || viewport.h <= 0.0 {
        return None;
    }
    let aspect = (viewport.w / viewport.h.max(1.0)).max(0.0001);
    let ndc_x = ((mouse[0] - viewport.x) / viewport.w) * 2.0 - 1.0;
    let ndc_y = 1.0 - ((mouse[1] - viewport.y) / viewport.h) * 2.0;
    let tan_half = (fov_degrees.to_radians() * 0.5).tan().max(0.0001);
    let (forward, right, up) = camera_basis(camera_position, camera_target);
    let direction = normalize3(add3(
        add3(forward, scale3(right, ndc_x * tan_half * aspect)),
        scale3(up, ndc_y * tan_half),
    ));
    Some((camera_position, direction))
}

fn camera_basis(
    camera_position: [f32; 3],
    camera_target: [f32; 3],
) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let forward = normalize3(sub3(camera_target, camera_position));
    let world_up = [0.0, 1.0, 0.0];
    let mut right = normalize3(cross3(world_up, forward));
    if length3(right) <= 0.0001 {
        right = [1.0, 0.0, 0.0];
    }
    let up = normalize3(cross3(forward, right));
    (forward, right, up)
}

fn intersect_ray_with_horizontal_plane(
    ray_origin: [f32; 3],
    ray_direction: [f32; 3],
    plane_y: f32,
) -> Option<[f32; 3]> {
    let denom = ray_direction[1];
    if denom.abs() < 0.0001 {
        return None;
    }
    let t = (plane_y - ray_origin[1]) / denom;
    if t <= 0.0 {
        return None;
    }
    Some(add3(ray_origin, scale3(ray_direction, t)))
}

fn orbit_document_camera(document: &mut NuSceneDocument, pivot: [f32; 3], delta: [f32; 2]) {
    let offset = sub3(document.camera.position, pivot);
    let radius = length3(offset).max(0.25);
    let mut yaw = offset[0].atan2(offset[2]);
    let mut pitch = (offset[1] / radius).asin();
    yaw += delta[0] * 0.01;
    pitch = (pitch - delta[1] * 0.01).clamp(-1.3, 1.3);
    let horizontal = radius * pitch.cos();
    document.camera.target = pivot;
    document.camera.position = [
        pivot[0] + horizontal * yaw.sin(),
        pivot[1] + radius * pitch.sin(),
        pivot[2] + horizontal * yaw.cos(),
    ];
}

fn zoom_document_camera(document: &mut NuSceneDocument, amount: f32) {
    let offset = sub3(document.camera.position, document.camera.target);
    let distance = length3(offset).max(0.25);
    let next_distance = (distance - amount).clamp(1.0, 80.0);
    let direction = normalize3(offset);
    document.camera.position = add3(document.camera.target, scale3(direction, next_distance));
}

fn translate_document_camera(document: &mut NuSceneDocument, local_delta: [f32; 3]) {
    let view_forward = normalize3(sub3(document.camera.target, document.camera.position));
    let forward = normalize3([view_forward[0], 0.0, view_forward[2]]);
    let right = normalize3(cross3(forward, [0.0, 1.0, 0.0]));
    let up = [0.0, 1.0, 0.0];
    let world_delta = add3(
        add3(scale3(right, local_delta[0]), scale3(up, local_delta[1])),
        scale3(forward, local_delta[2]),
    );
    document.camera.position = add3(document.camera.position, world_delta);
    document.camera.target = add3(document.camera.target, world_delta);
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale3(v: [f32; 3], scale: f32) -> [f32; 3] {
    [v[0] * scale, v[1] * scale, v[2] * scale]
}

fn mul3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length3(v: [f32; 3]) -> f32 {
    dot3(v, v).sqrt()
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let length = length3(v);
    if length <= 0.0001 {
        [0.0, 1.0, 0.0]
    } else {
        [v[0] / length, v[1] / length, v[2] / length]
    }
}
