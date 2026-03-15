use super::SceneEditor;
use crate::app::WindowConfig;
use crate::core::{ApiConfig, ApiError};
use crate::engine::{
    EngineError, HotReloadManager, LightKind, NuMeshScriptSection, NuMeshSection,
    NuPhysicsBodyKind, NuPhysicsColliderKind, NuPhysicsSection, NuSceneDocument, NuTransform,
    ReloadBatch, SceneSyntax, load_obj_mesh_asset, publish_reload_batch_events,
    world::NuSceneWorld,
};
use crate::event::{EngineEvent, EventBus, EventDeliveryMode};
use crate::lighting::{DirectionalLight, LightingConfig, PointLight, ShadowMode};
use crate::resource::{AssetHandle, AssetKind, AssetManager, AssetState};
use crate::run_scene;
use crate::scene::{
    Camera2D, Camera3D, Canvas2D, Mesh3D, MeshAsset3D, MeshDraw3D, MeshMaterial3D, Scene,
    SceneConfig, SceneFrame,
};
use crate::script::{NaMoveDirection, NaScriptProgram, parse_na_script};
use rfd::FileDialog;
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

const WINDOW_WIDTH: u32 = 1600;
const WINDOW_HEIGHT: u32 = 900;
const TOP_BAR_HEIGHT: f32 = 54.0;
const LEFT_PANEL_WIDTH: f32 = 250.0;
const RIGHT_PANEL_WIDTH: f32 = 320.0;
const PANEL_PAD: f32 = 14.0;
const BUTTON_H: f32 = 32.0;
const UI_LAYER_PANEL: i32 = 2000;
const UI_LAYER_TEXT: i32 = 2010;
const UNDO_HISTORY_LIMIT: usize = 128;
const SCREENSHOT_SHADOW_BOOST_FRAMES: u32 = 12;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MeshToolMode {
    Move,
    Deform,
    Pivot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GizmoSpace {
    Local,
    World,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GizmoAxis {
    X,
    Y,
    Z,
}

struct BasicEditorScene {
    editor: SceneEditor,
    hot_reload: Option<HotReloadManager>,
    status: String,
    status_color: [f32; 4],
    last_reload: Option<ReloadBatch>,
    mode: EditorMode,
    selected_mesh: Option<String>,
    selection_pivot_world: Option<[f32; 3]>,
    selected_light: Option<String>,
    selected_material: Option<String>,
    mouse_pos: [f32; 2],
    click_pending: bool,
    left_mouse_down: bool,
    right_mouse_down: bool,
    modifiers_state: ModifiersState,
    window_size: [u32; 2],
    drag_state: DragState,
    mesh_tool_mode: MeshToolMode,
    gizmo_space: GizmoSpace,
    show_physics_debug: bool,
    live_shadows_enabled: bool,
    preview_selected_light_only: bool,
    screenshot_shadow_boost_frames: u32,
    pending_screenshot_path: RefCell<Option<PathBuf>>,
    event_bus: EventBus,
    asset_manager: AssetManager,
    mesh_asset_handles: HashMap<PathBuf, AssetHandle>,
    obj_mesh_cache: HashMap<PathBuf, Result<Arc<MeshAsset3D>, String>>,
    undo_stack: Vec<NuSceneDocument>,
    redo_stack: Vec<NuSceneDocument>,
    play_session: Option<PlaySession>,
    player_input: PlayerInputState,
}

#[derive(Debug, Clone)]
enum DragState {
    None,
    Gizmo {
        mesh_name: String,
        axis: GizmoAxis,
        mode: MeshToolMode,
        space: GizmoSpace,
        start_mouse: [f32; 2],
        start_position: [f32; 3],
        start_scale: [f32; 3],
        start_rotation_radians: [f32; 3],
        start_pivot_world: [f32; 3],
        pixels_per_unit: f32,
    },
    LightGizmo {
        light_name: String,
        axis: GizmoAxis,
        start_mouse: [f32; 2],
        start_position: [f32; 3],
        pixels_per_unit: f32,
    },
    OrbitCamera {
        last_mouse: [f32; 2],
        pivot: [f32; 3],
    },
}

#[derive(Debug, Clone, Copy)]
struct GizmoAxisVisual {
    axis: GizmoAxis,
    screen_start: [f32; 2],
    screen_end: [f32; 2],
    world_direction: [f32; 3],
    color: [f32; 4],
    pixels_per_unit: f32,
}

#[derive(Debug, Clone)]
struct PlaySession {
    saved_document: NuSceneDocument,
    controlled_mesh: String,
    na_script: Option<NaScriptProgram>,
    cpp_script: Option<PathBuf>,
    attach_player_camera: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct PlayerInputState {
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
}

impl BasicEditorScene {
    fn compiled_world(&self) -> Option<NuSceneWorld> {
        self.editor.document().compile_world().ok()
    }

    fn resolved_mesh_draw_by_name(&mut self, mesh_name: &str) -> Option<MeshDraw3D> {
        let scene_base_dir = self.scene_base_dir();
        if let Some(world) = self.compiled_world() {
            if let Some(entity) = world.mesh_entity(mesh_name) {
                return Some(resolve_mesh_draw_from_world(
                    &world,
                    entity,
                    scene_base_dir.as_deref(),
                    &mut self.obj_mesh_cache,
                ));
            }
        }
        let document = self.editor.document();
        let mesh = document.meshes.get(mesh_name)?;
        Some(resolve_mesh_draw(
            document,
            mesh,
            0,
            scene_base_dir.as_deref(),
            &mut self.obj_mesh_cache,
        ))
    }

    fn sync_editor_mesh_assets(&mut self) {
        let scene_base_dir = self.scene_base_dir();
        let mut unique_paths = std::collections::BTreeSet::new();
        for mesh in self.editor.document().meshes.values() {
            if !mesh.geometry.eq_ignore_ascii_case("obj") {
                continue;
            }
            let Some(source) = &mesh.source else {
                continue;
            };
            unique_paths.insert(resolve_scene_asset_path(scene_base_dir.as_deref(), source));
        }

        let previous_handles = std::mem::take(&mut self.mesh_asset_handles);
        let mut next_handles = HashMap::new();
        for path in unique_paths {
            let handle = self.asset_manager.register(
                AssetKind::Mesh,
                path.to_string_lossy().into_owned(),
                Some(path.clone()),
            );
            let _ = self.asset_manager.mark_state(handle, AssetState::Unloaded);
            next_handles.insert(path, handle);
        }

        for (path, handle) in previous_handles {
            if !next_handles.contains_key(&path) {
                self.obj_mesh_cache.remove(&path);
            }
            let _ = self.asset_manager.release(handle);
        }

        self.mesh_asset_handles = next_handles;
    }

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
            selection_pivot_world: None,
            selected_light: None,
            selected_material: None,
            mouse_pos: [0.0, 0.0],
            click_pending: false,
            left_mouse_down: false,
            right_mouse_down: false,
            modifiers_state: ModifiersState::default(),
            window_size: [WINDOW_WIDTH, WINDOW_HEIGHT],
            drag_state: DragState::None,
            mesh_tool_mode: MeshToolMode::Move,
            gizmo_space: GizmoSpace::Local,
            show_physics_debug: true,
            live_shadows_enabled: true,
            preview_selected_light_only: false,
            screenshot_shadow_boost_frames: 0,
            pending_screenshot_path: RefCell::new(None),
            event_bus: EventBus::default(),
            asset_manager: AssetManager::default(),
            mesh_asset_handles: HashMap::new(),
            obj_mesh_cache: HashMap::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            play_session: None,
            player_input: PlayerInputState::default(),
        };
        scene.sync_editor_mesh_assets();
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
                true,
            );
        }
        if editor.document().materials.is_empty() {
            editor.upsert_material(
                "default_material",
                "lit.vert",
                "lit.frag",
                [0.92, 0.12, 0.12],
                0.45,
                0.0,
                None,
            );
        }
        if editor.document().meshes.is_empty() {
            editor.upsert_mesh(
                "car",
                "cube",
                None,
                "default_material",
                None,
                NuTransform {
                    position: [0.0, 1.0, 0.0],
                    rotation_degrees: [20.0, 35.0, 0.0],
                    scale: [1.2, 1.2, 1.2],
                },
                None,
            );
            if let Some(mesh) = editor.document_mut().meshes.get_mut("car") {
                mesh.script = Some(NuMeshScriptSection {
                    na_script: Some(PathBuf::from("scripts/player_controller.na")),
                    cpp_script: Some(PathBuf::from("scripts/player_controller.cpp")),
                    player_camera: true,
                });
            }
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
                None,
            );
        }
    }

    fn ensure_selection(&mut self) {
        if self.selected_mesh.is_none() {
            let preferred = if self.editor.document().meshes.contains_key("car") {
                Some("car".to_string())
            } else {
                self.editor
                    .document()
                    .meshes
                    .keys()
                    .find(|name| !name.eq_ignore_ascii_case("floor"))
                    .cloned()
            };
            self.set_selected_mesh(
                preferred.or_else(|| self.editor.document().meshes.keys().next().cloned()),
            );
        }
        if self.selected_light.is_none() {
            self.selected_light = self.editor.document().lights.keys().next().cloned();
        }
        if self.selected_material.is_none() {
            self.selected_material = self.editor.document().materials.keys().next().cloned();
        }
    }

    fn selected_mesh_entity_id(&self, mesh_name: &str) -> Option<u64> {
        let world = self.compiled_world()?;
        world.mesh_entity(mesh_name)
    }

    fn set_selected_mesh(&mut self, selected_mesh: Option<String>) {
        self.selected_mesh = selected_mesh.clone();
        self.refresh_selection_pivot();
        self.event_bus.publish(
            EngineEvent::EntitySelected {
                entity: selected_mesh
                    .as_deref()
                    .and_then(|mesh_name| self.selected_mesh_entity_id(mesh_name)),
            },
            EventDeliveryMode::Immediate,
        );
    }

    fn publish_transform_event_for_mesh(&mut self, mesh_name: &str) {
        if self.selected_mesh.as_deref() == Some(mesh_name) {
            self.refresh_selection_pivot();
        }
        if let Some(entity) = self.selected_mesh_entity_id(mesh_name) {
            self.event_bus.publish(
                EngineEvent::EntityTransformed { entity },
                EventDeliveryMode::Immediate,
            );
        }
    }

    fn set_status(&mut self, message: impl Into<String>, color: [f32; 4]) {
        self.status = message.into();
        self.status_color = color;
    }

    fn trigger_screenshot_shadow_boost(&mut self) {
        self.screenshot_shadow_boost_frames = SCREENSHOT_SHADOW_BOOST_FRAMES;
        self.set_status("SCREENSHOT SHADOW BOOST", C_OK);
    }

    fn screenshot_shadow_boost_active(&self) -> bool {
        self.screenshot_shadow_boost_frames > 0
    }

    fn queue_screenshot_capture(&mut self) {
        let base_dir = self
            .scene_base_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let screenshots_dir = base_dir.join("screenshots");
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let path = screenshots_dir.join(format!("nu_capture_{timestamp}.png"));
        *self.pending_screenshot_path.borrow_mut() = Some(path.clone());
        self.trigger_screenshot_shadow_boost();
        self.set_status(format!("SCREENSHOT RENDER {}", path.display()), C_OK);
    }

    fn clear_history(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    fn record_undo_snapshot(&mut self) {
        let current = self.editor.document().clone();
        if self.undo_stack.last() == Some(&current) {
            self.redo_stack.clear();
            return;
        }
        if self.undo_stack.len() >= UNDO_HISTORY_LIMIT {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(current);
        self.redo_stack.clear();
    }

    fn restore_document_state(&mut self, document: NuSceneDocument) {
        self.editor.replace_document(document);
        self.sync_editor_mesh_assets();
        self.play_session = None;
        self.player_input = PlayerInputState::default();
        if self
            .selected_mesh
            .as_ref()
            .is_some_and(|name| !self.editor.document().meshes.contains_key(name))
        {
            self.selected_mesh = None;
        }
        if self
            .selected_light
            .as_ref()
            .is_some_and(|name| !self.editor.document().lights.contains_key(name))
        {
            self.selected_light = None;
        }
        if self
            .selected_material
            .as_ref()
            .is_some_and(|name| !self.editor.document().materials.contains_key(name))
        {
            self.selected_material = None;
        }
        self.drag_state = DragState::None;
        self.refresh_selection_pivot();
        self.ensure_selection();
    }

    fn undo(&mut self) {
        let Some(previous) = self.undo_stack.pop() else {
            self.set_status("NOTHING TO UNDO", C_WARN);
            return;
        };
        let current = self.editor.document().clone();
        if self.redo_stack.len() >= UNDO_HISTORY_LIMIT {
            self.redo_stack.remove(0);
        }
        self.redo_stack.push(current);
        self.restore_document_state(previous);
        self.set_status("UNDO", C_OK);
    }

    fn redo(&mut self) {
        let Some(next) = self.redo_stack.pop() else {
            self.set_status("NOTHING TO REDO", C_WARN);
            return;
        };
        let current = self.editor.document().clone();
        if self.undo_stack.len() >= UNDO_HISTORY_LIMIT {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(current);
        self.restore_document_state(next);
        self.set_status("REDO", C_OK);
    }

    fn handle_history_shortcut(&mut self, code: KeyCode) -> bool {
        if !self.modifiers_state.control_key() {
            return false;
        }
        match code {
            KeyCode::KeyZ if self.modifiers_state.shift_key() => {
                self.redo();
                true
            }
            KeyCode::KeyZ => {
                self.undo();
                true
            }
            KeyCode::KeyY => {
                self.redo();
                true
            }
            _ => false,
        }
    }

    fn refresh_selection_pivot(&mut self) {
        let Some(name) = self.selected_mesh.clone() else {
            self.selection_pivot_world = None;
            return;
        };
        self.selection_pivot_world = self.resolved_mesh_draw_by_name(&name).map(|draw| {
            let pivot_offset = self
                .editor
                .document()
                .meshes
                .get(&name)
                .map(|mesh| mesh.pivot_offset)
                .unwrap_or([0.0, 0.0, 0.0]);
            add3(draw.center, pivot_offset)
        });
    }

    fn is_playing(&self) -> bool {
        self.play_session.is_some()
    }

    fn preferred_play_mesh(&self) -> Option<String> {
        if let Some(selected) = &self.selected_mesh {
            if self.editor.document().meshes.contains_key(selected) {
                return Some(selected.clone());
            }
        }
        if let Some((name, _)) = self.editor.document().meshes.iter().find(|(_, mesh)| {
            mesh.script
                .as_ref()
                .is_some_and(|script| script.player_camera)
        }) {
            return Some(name.clone());
        }
        if let Some((name, _)) = self
            .editor
            .document()
            .meshes
            .iter()
            .find(|(_, mesh)| mesh.script.is_some())
        {
            return Some(name.clone());
        }
        if self.editor.document().meshes.contains_key("car") {
            return Some("car".to_string());
        }
        self.editor
            .document()
            .meshes
            .keys()
            .find(|name| !name.eq_ignore_ascii_case("floor"))
            .cloned()
    }

    fn load_play_script(
        &self,
        mesh_name: &str,
    ) -> Result<(Option<NaScriptProgram>, Option<PathBuf>, bool), String> {
        let Some(mesh) = self.editor.document().meshes.get(mesh_name) else {
            return Err(format!("play mesh `{mesh_name}` does not exist"));
        };
        let Some(script) = &mesh.script else {
            return Ok((None, None, false));
        };
        let scene_base_dir = self.scene_base_dir();
        let na_script = if let Some(path) = &script.na_script {
            let resolved = resolve_scene_asset_path(scene_base_dir.as_deref(), path);
            let source = std::fs::read_to_string(&resolved)
                .map_err(|error| format!("failed to read {}: {error}", resolved.display()))?;
            Some(parse_na_script(&source).map_err(|error| {
                format!("failed to parse NAScript {}: {error}", resolved.display())
            })?)
        } else {
            None
        };
        let cpp_script = script
            .cpp_script
            .as_ref()
            .map(|path| resolve_scene_asset_path(scene_base_dir.as_deref(), path));
        let attach_player_camera = script.player_camera
            || na_script
                .as_ref()
                .is_some_and(|program| program.attach_player_camera);
        Ok((na_script, cpp_script, attach_player_camera))
    }

    fn update_play_camera(&mut self) {
        let Some((controlled_mesh, attach_player_camera)) =
            self.play_session.as_ref().map(|session| {
                (
                    session.controlled_mesh.clone(),
                    session.attach_player_camera,
                )
            })
        else {
            return;
        };
        if !attach_player_camera {
            return;
        }
        let Some(mesh) = self.editor.document().meshes.get(&controlled_mesh) else {
            return;
        };
        let yaw = mesh.transform.rotation_degrees[1].to_radians();
        let forward = [yaw.sin(), 0.0, yaw.cos()];
        let camera_offset = [-forward[0] * 4.0, 1.9, -forward[2] * 4.0];
        self.editor.set_camera(
            add3(mesh.transform.position, camera_offset),
            add3(mesh.transform.position, [0.0, 1.0, 0.0]),
            60.0,
        );
    }

    fn start_play_mode(&mut self) {
        if self.play_session.is_some() {
            return;
        }
        let Some(controlled_mesh) = self.preferred_play_mesh() else {
            self.set_status("PLAY REQUIRES A MESH", C_WARN);
            return;
        };
        let (na_script, cpp_script, attach_player_camera) =
            match self.load_play_script(&controlled_mesh) {
                Ok(result) => result,
                Err(error) => {
                    self.set_status(error, C_WARN);
                    return;
                }
            };
        self.play_session = Some(PlaySession {
            saved_document: self.editor.document().clone(),
            controlled_mesh: controlled_mesh.clone(),
            na_script,
            cpp_script: cpp_script.clone(),
            attach_player_camera,
        });
        self.player_input = PlayerInputState::default();
        self.drag_state = DragState::None;
        self.set_selected_mesh(Some(controlled_mesh.clone()));
        self.update_play_camera();
        let script_mode = match (
            self.play_session
                .as_ref()
                .and_then(|s| s.na_script.as_ref()),
            cpp_script.as_ref(),
        ) {
            (Some(_), Some(_)) => "NASCRIPT + C++ ATTACHED",
            (Some(_), None) => "NASCRIPT",
            (None, Some(_)) => "C++ ATTACHED",
            (None, None) => "BUILTIN",
        };
        self.set_status(
            format!("PLAY {} {}", controlled_mesh.to_uppercase(), script_mode),
            C_OK,
        );
    }

    fn stop_play_mode(&mut self) {
        let Some(session) = self.play_session.take() else {
            return;
        };
        self.editor.replace_document(session.saved_document);
        self.sync_editor_mesh_assets();
        self.player_input = PlayerInputState::default();
        self.drag_state = DragState::None;
        self.set_selected_mesh(Some(session.controlled_mesh));
        self.selected_light = None;
        self.selected_material = None;
        self.ensure_selection();
        self.refresh_selection_pivot();
        self.set_status("PLAY STOPPED", C_WARN);
    }

    fn toggle_play_mode(&mut self) {
        if self.is_playing() {
            self.stop_play_mode();
        } else {
            self.start_play_mode();
        }
    }

    fn handle_play_input_key(&mut self, code: KeyCode, pressed: bool) -> bool {
        if !self.is_playing() {
            return false;
        }
        match code {
            KeyCode::KeyW => self.player_input.forward = pressed,
            KeyCode::KeyS => self.player_input.backward = pressed,
            KeyCode::KeyA => self.player_input.left = pressed,
            KeyCode::KeyD => self.player_input.right = pressed,
            _ => return false,
        }
        true
    }

    fn update_play_mode(&mut self, delta_time_seconds: f32) {
        let Some((controlled_mesh, na_script)) = self
            .play_session
            .as_ref()
            .map(|session| (session.controlled_mesh.clone(), session.na_script.clone()))
        else {
            return;
        };
        let input = self.player_input;
        let mut changed = false;
        if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&controlled_mesh) {
            let yaw = mesh.transform.rotation_degrees[1].to_radians();
            let movement = play_movement_delta(na_script.as_ref(), input, delta_time_seconds, yaw);
            if length3(movement) > 0.0001 {
                mesh.transform.position = add3(mesh.transform.position, movement);
                mesh.transform.rotation_degrees[1] = movement[0].atan2(movement[2]).to_degrees();
                changed = true;
            }
        } else {
            self.stop_play_mode();
            self.set_status("PLAY TARGET LOST", C_WARN);
            return;
        }
        if changed {
            self.publish_transform_event_for_mesh(&controlled_mesh);
        }
        self.update_play_camera();
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
                self.clear_history();
                self.play_session = None;
                self.player_input = PlayerInputState::default();
                self.sync_editor_mesh_assets();
                self.hot_reload = HotReloadManager::open(&path).ok();
                self.last_reload = None;
                self.set_selected_mesh(None);
                self.selected_light = None;
                self.selected_material = None;
                self.ensure_selection();
                self.event_bus.publish(
                    EngineEvent::SceneLoaded {
                        scene_name: self.editor.document().scene.name.clone(),
                    },
                    EventDeliveryMode::Immediate,
                );
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
            if let Ok(batch) = hot_reload.reload_now() {
                publish_reload_batch_events(
                    &batch,
                    &mut self.event_bus,
                    EventDeliveryMode::Immediate,
                );
                self.last_reload = Some(batch);
            }
        }
    }

    fn force_reload(&mut self) {
        let Some(hot_reload) = &mut self.hot_reload else {
            self.set_status("RELOAD REQUIRES A SAVED SCENE", C_WARN);
            return;
        };
        match hot_reload.reload_now_with_events(&mut self.event_bus, EventDeliveryMode::Immediate) {
            Ok(batch) => {
                if let Some(scene) = &batch.scene {
                    self.editor.replace_document(scene.clone());
                    self.clear_history();
                    self.play_session = None;
                    self.player_input = PlayerInputState::default();
                }
                self.sync_editor_mesh_assets();
                self.last_reload = Some(batch.clone());
                self.set_selected_mesh(None);
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
        self.record_undo_snapshot();
        self.editor.set_syntax(syntax);
        self.set_status(format!("SYNTAX {}", syntax_label(syntax)), C_OK);
    }

    fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Digit1 => self.mode = EditorMode::Scene,
            KeyCode::Digit2 => self.mode = EditorMode::Meshes,
            KeyCode::Digit3 => self.mode = EditorMode::Lights,
            KeyCode::Digit4 => self.mode = EditorMode::Materials,
            KeyCode::KeyG => {
                self.mesh_tool_mode = MeshToolMode::Move;
                self.set_status("TOOL MOVE", C_OK);
            }
            KeyCode::KeyB => {
                self.mesh_tool_mode = MeshToolMode::Deform;
                self.set_status("TOOL DEFORM", C_OK);
            }
            KeyCode::KeyP => {
                self.mesh_tool_mode = MeshToolMode::Pivot;
                self.set_status("TOOL PIVOT", C_OK);
            }
            KeyCode::KeyV => {
                self.gizmo_space = match self.gizmo_space {
                    GizmoSpace::Local => GizmoSpace::World,
                    GizmoSpace::World => GizmoSpace::Local,
                };
                self.set_status(
                    match self.gizmo_space {
                        GizmoSpace::Local => "GIZMO LOCAL",
                        GizmoSpace::World => "GIZMO WORLD",
                    },
                    C_OK,
                );
            }
            KeyCode::F2 => self.save_current(),
            KeyCode::F5 => self.force_reload(),
            KeyCode::F6 => {
                self.show_physics_debug = !self.show_physics_debug;
                self.set_status(
                    if self.show_physics_debug {
                        "PHYSICS DEBUG ON"
                    } else {
                        "PHYSICS DEBUG OFF"
                    },
                    C_OK,
                );
            }
            KeyCode::F7 => {
                self.live_shadows_enabled = !self.live_shadows_enabled;
                self.set_status(
                    if self.live_shadows_enabled {
                        "LIVE SHADOWS ON"
                    } else {
                        "LIVE SHADOWS OFF"
                    },
                    C_OK,
                );
            }
            KeyCode::F8 => self.toggle_play_mode(),
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
                if !matches!(
                    code,
                    KeyCode::KeyJ
                        | KeyCode::KeyL
                        | KeyCode::KeyI
                        | KeyCode::KeyK
                        | KeyCode::KeyU
                        | KeyCode::KeyO
                        | KeyCode::KeyQ
                        | KeyCode::KeyE
                        | KeyCode::KeyZ
                        | KeyCode::KeyX
                ) {
                    return;
                }
                self.record_undo_snapshot();
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
                self.publish_transform_event_for_mesh(&name);
                self.set_status(format!("EDITED {}", name.to_uppercase()), C_OK);
            }
            EditorMode::Lights => {
                let Some(name) = self.selected_light.clone() else {
                    return;
                };
                if !matches!(
                    code,
                    KeyCode::KeyJ
                        | KeyCode::KeyL
                        | KeyCode::KeyI
                        | KeyCode::KeyK
                        | KeyCode::KeyU
                        | KeyCode::KeyO
                        | KeyCode::Minus
                        | KeyCode::Equal
                        | KeyCode::KeyT
                ) {
                    return;
                }
                self.record_undo_snapshot();
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
        if self.is_playing() {
            return false;
        }
        let delta = match code {
            KeyCode::ArrowLeft => Some([-0.25, 0.0, 0.0]),
            KeyCode::ArrowRight => Some([0.25, 0.0, 0.0]),
            KeyCode::KeyA => Some([0.25, 0.0, 0.0]),
            KeyCode::KeyD => Some([-0.25, 0.0, 0.0]),
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
        if let Some(name) = self.selected_mesh.clone() {
            if let Some(draw) = self.resolved_mesh_draw_by_name(&name) {
                return self.gizmo_anchor_world(draw.center);
            }
        }
        self.editor.document().camera.target
    }

    fn gizmo_anchor_world(&self, mesh_center: [f32; 3]) -> [f32; 3] {
        match self.mesh_tool_mode {
            MeshToolMode::Pivot => self.selection_pivot_world.unwrap_or(mesh_center),
            MeshToolMode::Move | MeshToolMode::Deform => mesh_center,
        }
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
        let camera = self.editor.document().camera.clone();
        let mesh_names: Vec<String> = self.editor.document().meshes.keys().cloned().collect();
        for name in mesh_names {
            let Some(mesh) = self.editor.document().meshes.get(&name) else {
                continue;
            };
            if mesh.geometry.eq_ignore_ascii_case("plane") {
                continue;
            }
            let Some(draw) = self.resolved_mesh_draw_by_name(&name) else {
                continue;
            };
            let Some(screen) = project_world_to_screen(
                camera.position,
                camera.target,
                camera.fov_degrees,
                viewport,
                draw.center,
            ) else {
                continue;
            };
            let radius = projected_mesh_radius(
                camera.position,
                camera.target,
                camera.fov_degrees,
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

    fn pick_light_in_viewport(&mut self) -> Option<String> {
        let viewport = self.viewport_rect();
        let camera = self.editor.document().camera.clone();
        let mut best: Option<(String, f32)> = None;
        for (name, light) in &self.editor.document().lights {
            let Some(screen) = project_world_to_screen(
                camera.position,
                camera.target,
                camera.fov_degrees,
                viewport,
                light.position,
            ) else {
                continue;
            };
            let dx = self.mouse_pos[0] - screen[0];
            let dy = self.mouse_pos[1] - screen[1];
            let distance_sq = dx * dx + dy * dy;
            let radius = if self.selected_light.as_deref() == Some(name.as_str()) {
                18.0
            } else {
                14.0
            };
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
        if self.is_playing() {
            return;
        }
        if !self.point_in_viewport() {
            return;
        }
        match self.mode {
            EditorMode::Meshes => {
                if self.try_begin_gizmo_drag() {
                    return;
                }
                if let Some(name) = self.pick_mesh_in_viewport() {
                    self.set_selected_mesh(Some(name.clone()));
                    self.drag_state = DragState::None;
                    self.set_status(format!("SELECTED {}", name.to_uppercase()), C_OK);
                }
            }
            EditorMode::Lights => {
                if self.try_begin_light_drag() {
                    return;
                }
                if let Some(name) = self.pick_light_in_viewport() {
                    self.selected_light = Some(name.clone());
                    self.drag_state = DragState::None;
                    self.set_status(format!("SELECTED LIGHT {}", name.to_uppercase()), C_OK);
                }
            }
            _ => {}
        }
    }

    fn try_begin_gizmo_drag(&mut self) -> bool {
        let Some(name) = self.selected_mesh.clone() else {
            return false;
        };
        let viewport = self.viewport_rect();
        let camera = self.editor.document().camera.clone();
        let (start_position, start_scale) = {
            let Some(mesh) = self.editor.document().meshes.get(&name) else {
                return false;
            };
            (mesh.transform.position, mesh.transform.scale)
        };
        let Some(draw) = self.resolved_mesh_draw_by_name(&name) else {
            return false;
        };
        let pivot_center = self.gizmo_anchor_world(draw.center);
        let visuals = gizmo_axis_visuals(
            camera.position,
            camera.target,
            camera.fov_degrees,
            viewport,
            pivot_center,
            draw.size,
            draw.rotation_radians,
            self.gizmo_space,
        );
        let Some(visual) = pick_gizmo_axis(visuals, self.mouse_pos) else {
            return false;
        };
        self.record_undo_snapshot();
        self.drag_state = DragState::Gizmo {
            mesh_name: name.clone(),
            axis: visual.axis,
            mode: self.mesh_tool_mode,
            space: self.gizmo_space,
            start_mouse: self.mouse_pos,
            start_position,
            start_scale,
            start_rotation_radians: draw.rotation_radians,
            start_pivot_world: pivot_center,
            pixels_per_unit: visual.pixels_per_unit.max(1.0),
        };
        self.set_status(
            match self.mesh_tool_mode {
                MeshToolMode::Move => format!("MOVE {}", name.to_uppercase()),
                MeshToolMode::Deform => format!("DEFORM {}", name.to_uppercase()),
                MeshToolMode::Pivot => format!("PIVOT {}", name.to_uppercase()),
            },
            C_OK,
        );
        true
    }

    fn try_begin_light_drag(&mut self) -> bool {
        let Some(name) = self.selected_light.clone() else {
            return false;
        };
        let viewport = self.viewport_rect();
        let camera = self.editor.document().camera.clone();
        let Some(light) = self.editor.document().lights.get(&name).cloned() else {
            return false;
        };
        let visuals = gizmo_axis_visuals(
            camera.position,
            camera.target,
            camera.fov_degrees,
            viewport,
            light.position,
            [1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0],
            GizmoSpace::World,
        );
        let Some(visual) = pick_gizmo_axis(visuals, self.mouse_pos) else {
            return false;
        };
        self.record_undo_snapshot();
        self.drag_state = DragState::LightGizmo {
            light_name: name.clone(),
            axis: visual.axis,
            start_mouse: self.mouse_pos,
            start_position: light.position,
            pixels_per_unit: visual.pixels_per_unit.max(1.0),
        };
        self.set_status(format!("MOVE LIGHT {}", name.to_uppercase()), C_OK);
        true
    }

    fn update_drag(&mut self) {
        match self.drag_state {
            DragState::Gizmo {
                ref mesh_name,
                axis,
                mode,
                space,
                start_mouse,
                start_position,
                start_scale,
                start_rotation_radians,
                start_pivot_world,
                pixels_per_unit,
            } => {
                let mesh_name = mesh_name.clone();
                let screen_delta = [
                    self.mouse_pos[0] - start_mouse[0],
                    self.mouse_pos[1] - start_mouse[1],
                ];
                let camera = self.editor.document().camera.clone();
                if !self.editor.document().meshes.contains_key(&mesh_name) {
                    self.drag_state = DragState::None;
                    return;
                }
                let Some(draw) = self.resolved_mesh_draw_by_name(&mesh_name) else {
                    self.drag_state = DragState::None;
                    return;
                };
                let pivot_center = match mode {
                    MeshToolMode::Pivot => self.selection_pivot_world.unwrap_or(draw.center),
                    MeshToolMode::Move | MeshToolMode::Deform => draw.center,
                };
                let visuals = gizmo_axis_visuals(
                    camera.position,
                    camera.target,
                    camera.fov_degrees,
                    self.viewport_rect(),
                    pivot_center,
                    draw.size,
                    draw.rotation_radians,
                    space,
                );
                let Some(axis_visual) = visuals.into_iter().find(|visual| visual.axis == axis)
                else {
                    return;
                };
                let axis_screen = sub2(axis_visual.screen_end, axis_visual.screen_start);
                let axis_screen_dir = normalize2(axis_screen);
                let axis_units = dot2(screen_delta, axis_screen_dir) / pixels_per_unit.max(1.0);
                if matches!(mode, MeshToolMode::Pivot) {
                    let next_pivot = add3(
                        start_pivot_world,
                        scale3(axis_visual.world_direction, axis_units),
                    );
                    self.selection_pivot_world = Some(next_pivot);
                    if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&mesh_name) {
                        mesh.pivot_offset = sub3(next_pivot, draw.center);
                    }
                } else if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&mesh_name) {
                    apply_gizmo_delta(
                        mesh,
                        axis,
                        mode,
                        space,
                        start_position,
                        start_scale,
                        start_rotation_radians,
                        axis_visual.world_direction,
                        axis_units,
                    );
                }
                self.publish_transform_event_for_mesh(&mesh_name);
            }
            DragState::LightGizmo {
                ref light_name,
                axis,
                start_mouse,
                start_position,
                pixels_per_unit,
            } => {
                let light_name = light_name.clone();
                let screen_delta = [
                    self.mouse_pos[0] - start_mouse[0],
                    self.mouse_pos[1] - start_mouse[1],
                ];
                let camera = self.editor.document().camera.clone();
                let Some(light) = self.editor.document().lights.get(&light_name).cloned() else {
                    self.drag_state = DragState::None;
                    return;
                };
                let visuals = gizmo_axis_visuals(
                    camera.position,
                    camera.target,
                    camera.fov_degrees,
                    self.viewport_rect(),
                    light.position,
                    [1.0, 1.0, 1.0],
                    [0.0, 0.0, 0.0],
                    GizmoSpace::World,
                );
                let Some(axis_visual) = visuals.into_iter().find(|visual| visual.axis == axis)
                else {
                    return;
                };
                let axis_screen = sub2(axis_visual.screen_end, axis_visual.screen_start);
                let axis_screen_dir = normalize2(axis_screen);
                let axis_units = dot2(screen_delta, axis_screen_dir) / pixels_per_unit.max(1.0);
                if let Some(light) = self.editor.document_mut().lights.get_mut(&light_name) {
                    light.position = add3(
                        start_position,
                        scale3(axis_visual.world_direction, axis_units),
                    );
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
        let base_name = if geometry.eq_ignore_ascii_case("cube")
            && !self.editor.document().meshes.contains_key("car")
        {
            "car"
        } else {
            geometry
        };
        let name = unique_name(
            base_name,
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
        self.record_undo_snapshot();
        self.editor.upsert_mesh(
            &name,
            geometry,
            None,
            material,
            None,
            NuTransform::default(),
            None,
        );
        self.set_selected_mesh(Some(name.clone()));
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
        self.record_undo_snapshot();
        self.editor.upsert_mesh(
            &name,
            "obj",
            Some(path.clone()),
            material,
            None,
            NuTransform::default(),
            None,
        );
        self.set_selected_mesh(Some(name.clone()));
        self.mode = EditorMode::Meshes;
        self.set_status(format!("IMPORTED {}", name.to_uppercase()), C_OK);
    }

    fn add_light(&mut self) {
        let name = unique_name(
            "light",
            self.editor.document().lights.keys().map(String::as_str),
        );
        self.record_undo_snapshot();
        self.editor.upsert_light(
            &name,
            LightKind::Point,
            [3.0, 5.0, -3.0],
            [1.0, 1.0, 1.0],
            1.0,
            true,
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
        self.record_undo_snapshot();
        self.editor.upsert_material(
            &name,
            "lit.vert",
            "lit.frag",
            [1.0, 1.0, 1.0],
            0.5,
            0.0,
            None,
        );
        self.selected_material = Some(name.clone());
        self.mode = EditorMode::Materials;
        self.set_status(format!("ADDED {}", name.to_uppercase()), C_OK);
    }

    fn remove_selected(&mut self) {
        match self.mode {
            EditorMode::Meshes => {
                if let Some(name) = self.selected_mesh.clone() {
                    self.record_undo_snapshot();
                    self.editor.document_mut().meshes.remove(&name);
                    self.sync_editor_mesh_assets();
                    self.set_selected_mesh(None);
                    self.ensure_selection();
                    self.set_status(format!("REMOVED {}", name.to_uppercase()), C_WARN);
                }
            }
            EditorMode::Lights => {
                if let Some(name) = self.selected_light.clone() {
                    self.record_undo_snapshot();
                    self.editor.document_mut().lights.remove(&name);
                    self.selected_light = None;
                    self.ensure_selection();
                    self.set_status(format!("REMOVED {}", name.to_uppercase()), C_WARN);
                }
            }
            EditorMode::Materials => {
                if let Some(name) = self.selected_material.clone() {
                    self.record_undo_snapshot();
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
            ("SCREENSHOT", 122.0),
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
                    "SCREENSHOT" => self.queue_screenshot_capture(),
                    _ => {}
                }
            }
            x += rect.w + 10.0;
        }

        let play_rect = UiRect {
            x,
            y: 11.0,
            w: 86.0,
            h: BUTTON_H,
        };
        if self.button(
            canvas,
            play_rect,
            if self.is_playing() { "STOP" } else { "PLAY" },
            self.is_playing(),
        ) {
            self.toggle_play_mode();
        }
        x += play_rect.w + 10.0;

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
                        self.set_selected_mesh(Some(mesh_name));
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
                let environment = self.editor.document().environment.clone().unwrap_or(
                    crate::engine::NuEnvironmentSection {
                        ambient_color: [0.1, 0.1, 0.15],
                        ambient_intensity: 0.3,
                        shadow_mode: ShadowMode::Live,
                        shadow_max_distance: 32.0,
                        shadow_filter_radius: 1.5,
                    },
                );
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
                if self.button(
                    canvas,
                    UiRect {
                        x: rect.x + 14.0,
                        y,
                        w: rect.w - 28.0,
                        h: BUTTON_H,
                    },
                    &format!("SHADOW {}", shadow_mode_label(environment.shadow_mode)),
                    matches!(environment.shadow_mode, ShadowMode::Live),
                ) {
                    let next_mode = match environment.shadow_mode {
                        ShadowMode::Off => ShadowMode::Live,
                        ShadowMode::Live => ShadowMode::Off,
                    };
                    self.editor.set_environment_shadows(
                        next_mode,
                        environment.shadow_max_distance,
                        environment.shadow_filter_radius,
                    );
                }
                y += BUTTON_H + 8.0;
                if self.button(
                    canvas,
                    UiRect {
                        x: rect.x + 14.0,
                        y,
                        w: (rect.w - 40.0) * 0.5,
                        h: BUTTON_H,
                    },
                    &format!("DIST {:.0}-", environment.shadow_max_distance),
                    false,
                ) {
                    self.editor.set_environment_shadows(
                        environment.shadow_mode,
                        (environment.shadow_max_distance - 4.0).max(8.0),
                        environment.shadow_filter_radius,
                    );
                }
                if self.button(
                    canvas,
                    UiRect {
                        x: rect.x + 20.0 + (rect.w - 40.0) * 0.5,
                        y,
                        w: (rect.w - 40.0) * 0.5,
                        h: BUTTON_H,
                    },
                    &format!("DIST {:.0}+", environment.shadow_max_distance),
                    false,
                ) {
                    self.editor.set_environment_shadows(
                        environment.shadow_mode,
                        environment.shadow_max_distance + 4.0,
                        environment.shadow_filter_radius,
                    );
                }
                y += BUTTON_H + 8.0;
                if self.button(
                    canvas,
                    UiRect {
                        x: rect.x + 14.0,
                        y,
                        w: (rect.w - 40.0) * 0.5,
                        h: BUTTON_H,
                    },
                    &format!("SOFT {:.1}-", environment.shadow_filter_radius),
                    false,
                ) {
                    self.editor.set_environment_shadows(
                        environment.shadow_mode,
                        environment.shadow_max_distance,
                        (environment.shadow_filter_radius - 0.25).max(0.5),
                    );
                }
                if self.button(
                    canvas,
                    UiRect {
                        x: rect.x + 20.0 + (rect.w - 40.0) * 0.5,
                        y,
                        w: (rect.w - 40.0) * 0.5,
                        h: BUTTON_H,
                    },
                    &format!("SOFT {:.1}+", environment.shadow_filter_radius),
                    false,
                ) {
                    self.editor.set_environment_shadows(
                        environment.shadow_mode,
                        environment.shadow_max_distance,
                        environment.shadow_filter_radius + 0.25,
                    );
                }
                y += BUTTON_H + 8.0;
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
                    "F12 SCREENSHOT",
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
            if self.is_playing() {
                "PLAY MODE  WASD MOVE PLAYER  F8 STOP"
            } else {
                "LMB SELECT/DRAG  RMB ORBIT  WHEEL ZOOM  ARROWS/WASD CAMERA"
            },
            C_TEXT_MUTED,
            UI_LAYER_TEXT,
        );
        draw_text(
            canvas,
            [viewport.x + 14.0, viewport.y + 32.0],
            1.6,
            if self.show_physics_debug {
                "F6 PHYSICS DEBUG ON"
            } else {
                "F6 PHYSICS DEBUG OFF"
            },
            if self.show_physics_debug {
                [0.56, 0.88, 0.68, 1.0]
            } else {
                C_TEXT_MUTED
            },
            UI_LAYER_TEXT,
        );
        draw_text(
            canvas,
            [viewport.x + 14.0, viewport.y + 50.0],
            1.6,
            if self.live_shadows_enabled {
                "F7 LIVE SHADOWS ON"
            } else {
                "F7 LIVE SHADOWS OFF"
            },
            if self.live_shadows_enabled {
                [0.92, 0.82, 0.48, 1.0]
            } else {
                C_TEXT_MUTED
            },
            UI_LAYER_TEXT,
        );
        draw_text(
            canvas,
            [viewport.x + 14.0, viewport.y + 68.0],
            1.6,
            match self.mesh_tool_mode {
                MeshToolMode::Move => "G MOVE TOOL",
                MeshToolMode::Deform => "B DEFORM TOOL",
                MeshToolMode::Pivot => "P PIVOT TOOL",
            },
            C_TEXT_ACCENT,
            UI_LAYER_TEXT,
        );
        draw_text(
            canvas,
            [viewport.x + 14.0, viewport.y + 86.0],
            1.6,
            match self.gizmo_space {
                GizmoSpace::Local => "V LOCAL SPACE",
                GizmoSpace::World => "V WORLD SPACE",
            },
            C_TEXT_MUTED,
            UI_LAYER_TEXT,
        );

        self.draw_light_overlay(canvas, viewport);

        if self.mode == EditorMode::Meshes {
            if let Some(name) = self.selected_mesh.clone() {
                let camera = self.editor.document().camera.clone();
                if let Some(draw) = self.resolved_mesh_draw_by_name(&name) {
                    let pivot_center = self.gizmo_anchor_world(draw.center);
                    let visuals = gizmo_axis_visuals(
                        camera.position,
                        camera.target,
                        camera.fov_degrees,
                        viewport,
                        pivot_center,
                        draw.size,
                        draw.rotation_radians,
                        self.gizmo_space,
                    );
                    let hovered_axis =
                        pick_gizmo_axis(visuals.clone(), self.mouse_pos).map(|v| v.axis);
                    let active_axis = match self.drag_state {
                        DragState::Gizmo { axis, .. } => Some(axis),
                        _ => None,
                    };
                    if let Some((min, max)) = projected_mesh_screen_bounds(
                        camera.position,
                        camera.target,
                        camera.fov_degrees,
                        viewport,
                        draw.center,
                        draw.size,
                        draw.rotation_radians,
                    ) {
                        draw_mesh_gizmo(
                            canvas,
                            camera.position,
                            camera.target,
                            camera.fov_degrees,
                            viewport,
                            pivot_center,
                            draw.size,
                            draw.rotation_radians,
                            self.mesh_tool_mode,
                            hovered_axis,
                            active_axis,
                            visuals,
                        );
                        draw_text(
                            canvas,
                            [max[0] + 10.0, (min[1] - 18.0).max(viewport.y + 14.0)],
                            1.8,
                            &name,
                            C_TEXT_ACCENT,
                            UI_LAYER_TEXT,
                        );
                    }
                }
            }
        }
    }

    fn draw_light_overlay(&mut self, canvas: &mut Canvas2D<'_>, viewport: UiRect) {
        let camera = self.editor.document().camera.clone();
        let light_names: Vec<String> = self.editor.document().lights.keys().cloned().collect();
        for name in light_names {
            let Some(light) = self.editor.document().lights.get(&name).cloned() else {
                continue;
            };
            let Some(screen) = project_world_to_screen(
                camera.position,
                camera.target,
                camera.fov_degrees,
                viewport,
                light.position,
            ) else {
                continue;
            };
            let selected = self.selected_light.as_deref() == Some(name.as_str());
            let marker_color = match light.kind {
                LightKind::Point => [1.0, 0.88, 0.42, 1.0],
                LightKind::Directional => [0.62, 0.84, 1.0, 1.0],
            };
            let radius = if selected { 8.0 } else { 6.0 };
            canvas.fill_circle(
                screen,
                radius,
                [marker_color[0], marker_color[1], marker_color[2], 0.18],
                UI_LAYER_TEXT,
            );
            canvas.stroke_circle(
                screen,
                radius,
                marker_color,
                if selected { 2.0 } else { 1.4 },
                UI_LAYER_TEXT + 1,
            );
            draw_text(
                canvas,
                [screen[0] + 10.0, screen[1] - 14.0],
                1.5,
                &name,
                marker_color,
                UI_LAYER_TEXT + 2,
            );

            if selected && self.mode == EditorMode::Lights {
                let visuals = gizmo_axis_visuals(
                    camera.position,
                    camera.target,
                    camera.fov_degrees,
                    viewport,
                    light.position,
                    [1.0, 1.0, 1.0],
                    [0.0, 0.0, 0.0],
                    GizmoSpace::World,
                );
                let hovered_axis =
                    pick_gizmo_axis(visuals.clone(), self.mouse_pos).map(|visual| visual.axis);
                let active_axis = match self.drag_state {
                    DragState::LightGizmo { axis, .. } => Some(axis),
                    _ => None,
                };
                draw_mesh_gizmo(
                    canvas,
                    camera.position,
                    camera.target,
                    camera.fov_degrees,
                    viewport,
                    light.position,
                    [1.0, 1.0, 1.0],
                    [0.0, 0.0, 0.0],
                    MeshToolMode::Move,
                    hovered_axis,
                    active_axis,
                    visuals,
                );
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
        let script_snapshot = mesh_snapshot.script.clone();
        let mut toggle_geo = false;
        let mut next_mat = false;
        let mut pick_obj = false;
        let mut clear_obj = false;
        let mut toggle_player_camera = false;
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
        let physics_snapshot = mesh_snapshot.physics.clone();
        let physics_body_label = physics_snapshot
            .as_ref()
            .map(|physics| physics_body_label(physics.body))
            .unwrap_or("OFF");
        let physics_collider_label = physics_snapshot
            .as_ref()
            .map(|physics| physics_collider_label(physics.collider))
            .unwrap_or("AUTO");
        let mut toggle_physics = false;
        let mut cycle_body = false;
        let mut cycle_collider = false;
        let mass_down;
        let mass_up;
        if self.button(
            canvas,
            UiRect {
                x: rect.x + 14.0,
                y: *y,
                w: rect.w - 28.0,
                h: BUTTON_H,
            },
            &format!("PHYS {}", physics_body_label),
            physics_snapshot.is_some(),
        ) {
            toggle_physics = true;
        }
        *y += BUTTON_H + 8.0;
        if physics_snapshot.is_some() {
            if self.button(
                canvas,
                UiRect {
                    x: rect.x + 14.0,
                    y: *y,
                    w: rect.w - 28.0,
                    h: BUTTON_H,
                },
                &format!("BODY {}", physics_body_label),
                false,
            ) {
                cycle_body = true;
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
                &format!("COL {}", physics_collider_label),
                false,
            ) {
                cycle_collider = true;
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
            mass_down = self.button(canvas, down, "M-", false);
            mass_up = self.button(canvas, up, "M+", false);
            draw_text(
                canvas,
                [rect.x + 128.0, *y + 10.0],
                2.0,
                &format!(
                    "MASS {:.2}",
                    physics_snapshot
                        .as_ref()
                        .map(|physics| physics.mass)
                        .unwrap_or(1.0)
                ),
                C_TEXT_MUTED,
                UI_LAYER_TEXT,
            );
            *y += BUTTON_H + 12.0;
        } else {
            mass_down = false;
            mass_up = false;
        }
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
            &format!(
                "NA {}",
                truncate_middle(
                    &script_snapshot
                        .as_ref()
                        .and_then(|script| script.na_script.as_ref())
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "NONE".to_string()),
                    26
                )
            ),
            C_TEXT_MUTED,
            UI_LAYER_TEXT,
        );
        *y += 18.0;
        let na_pick_rect = UiRect {
            x: rect.x + 14.0,
            y: *y,
            w: (rect.w - 38.0) * 0.5,
            h: BUTTON_H,
        };
        let na_clear_rect = UiRect {
            x: na_pick_rect.x + na_pick_rect.w + 10.0,
            y: *y,
            w: na_pick_rect.w,
            h: BUTTON_H,
        };
        let pick_na_script = self.button(canvas, na_pick_rect, "PICK NA", false);
        let clear_na_script = self.button(canvas, na_clear_rect, "CLEAR NA", false);
        *y += BUTTON_H + 12.0;
        draw_text(
            canvas,
            [rect.x + 14.0, *y],
            2.0,
            &format!(
                "CPP {}",
                truncate_middle(
                    &script_snapshot
                        .as_ref()
                        .and_then(|script| script.cpp_script.as_ref())
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "NONE".to_string()),
                    25
                )
            ),
            C_TEXT_MUTED,
            UI_LAYER_TEXT,
        );
        *y += 18.0;
        let cpp_pick_rect = UiRect {
            x: rect.x + 14.0,
            y: *y,
            w: (rect.w - 38.0) * 0.5,
            h: BUTTON_H,
        };
        let cpp_clear_rect = UiRect {
            x: cpp_pick_rect.x + cpp_pick_rect.w + 10.0,
            y: *y,
            w: cpp_pick_rect.w,
            h: BUTTON_H,
        };
        let pick_cpp_script = self.button(canvas, cpp_pick_rect, "PICK CPP", false);
        let clear_cpp_script = self.button(canvas, cpp_clear_rect, "CLEAR CPP", false);
        *y += BUTTON_H + 8.0;
        if self.button(
            canvas,
            UiRect {
                x: rect.x + 14.0,
                y: *y,
                w: rect.w - 28.0,
                h: BUTTON_H,
            },
            if script_snapshot
                .as_ref()
                .is_some_and(|script| script.player_camera)
            {
                "PLAYER CAMERA ON"
            } else {
                "PLAYER CAMERA OFF"
            },
            script_snapshot
                .as_ref()
                .is_some_and(|script| script.player_camera),
        ) {
            toggle_player_camera = true;
        }
        *y += BUTTON_H + 12.0;
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
        if toggle_geo
            || toggle_physics
            || cycle_body
            || cycle_collider
            || mass_down
            || mass_up
            || clear_obj
            || clear_na_script
            || clear_cpp_script
            || toggle_player_camera
            || (next_mat && !material_names.is_empty())
        {
            self.record_undo_snapshot();
        }
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
        if toggle_physics {
            let next = if physics_snapshot.is_some() {
                None
            } else {
                Some(default_mesh_physics(&mesh_snapshot))
            };
            let _ = self.editor.set_mesh_physics(&name, next);
        }
        if cycle_body {
            if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&name) {
                if let Some(physics) = &mut mesh.physics {
                    physics.body = match physics.body {
                        NuPhysicsBodyKind::Static => NuPhysicsBodyKind::Dynamic,
                        NuPhysicsBodyKind::Dynamic => NuPhysicsBodyKind::Kinematic,
                        NuPhysicsBodyKind::Kinematic => NuPhysicsBodyKind::Static,
                    };
                }
            }
        }
        if cycle_collider {
            if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&name) {
                if let Some(physics) = &mut mesh.physics {
                    physics.collider = match physics.collider {
                        NuPhysicsColliderKind::Auto => NuPhysicsColliderKind::Cuboid,
                        NuPhysicsColliderKind::Cuboid => NuPhysicsColliderKind::Sphere,
                        NuPhysicsColliderKind::Sphere => NuPhysicsColliderKind::Plane,
                        NuPhysicsColliderKind::Plane => NuPhysicsColliderKind::Auto,
                    };
                    if matches!(physics.collider, NuPhysicsColliderKind::Plane)
                        && !mesh.geometry.eq_ignore_ascii_case("plane")
                    {
                        physics.collider = NuPhysicsColliderKind::Auto;
                    }
                }
            }
        }
        if mass_down {
            if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&name) {
                if let Some(physics) = &mut mesh.physics {
                    physics.mass = (physics.mass - 0.1).max(0.1);
                }
            }
        }
        if mass_up {
            if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&name) {
                if let Some(physics) = &mut mesh.physics {
                    physics.mass += 0.1;
                }
            }
        }
        if pick_obj {
            if let Some(path) = FileDialog::new()
                .add_filter("wavefront obj", &["obj"])
                .pick_file()
            {
                self.record_undo_snapshot();
                if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&name) {
                    mesh.geometry = "obj".to_string();
                    mesh.source = Some(path.clone());
                }
                self.sync_editor_mesh_assets();
                self.set_status(format!("LINKED {}", name.to_uppercase()), C_OK);
            }
        }
        if clear_obj {
            if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&name) {
                let _ = mesh.source.take();
                mesh.geometry = "cube".to_string();
            }
            self.sync_editor_mesh_assets();
            self.set_status(format!("UNLINKED {}", name.to_uppercase()), C_WARN);
        }
        if next_mat && !material_names.is_empty() {
            if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&name) {
                mesh.material = cycle_name(&material_names, &mesh.material, 1);
            }
        }
        if pick_na_script {
            if let Some(path) = FileDialog::new()
                .add_filter("nascript", &["na"])
                .pick_file()
            {
                self.record_undo_snapshot();
                if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&name) {
                    let script = mesh.script.get_or_insert(NuMeshScriptSection {
                        na_script: None,
                        cpp_script: None,
                        player_camera: false,
                    });
                    script.na_script = Some(path);
                }
                self.set_status(format!("NA SCRIPT {}", name.to_uppercase()), C_OK);
            }
        }
        if clear_na_script {
            if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&name) {
                if let Some(script) = &mut mesh.script {
                    script.na_script = None;
                    if script.cpp_script.is_none() && !script.player_camera {
                        mesh.script = None;
                    }
                }
            }
        }
        if pick_cpp_script {
            if let Some(path) = FileDialog::new()
                .add_filter("c++ source", &["cpp", "cc", "cxx", "hpp", "h"])
                .pick_file()
            {
                self.record_undo_snapshot();
                if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&name) {
                    let script = mesh.script.get_or_insert(NuMeshScriptSection {
                        na_script: None,
                        cpp_script: None,
                        player_camera: false,
                    });
                    script.cpp_script = Some(path);
                }
                self.set_status(format!("CPP SCRIPT {}", name.to_uppercase()), C_OK);
            }
        }
        if clear_cpp_script {
            if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&name) {
                if let Some(script) = &mut mesh.script {
                    script.cpp_script = None;
                    if script.na_script.is_none() && !script.player_camera {
                        mesh.script = None;
                    }
                }
            }
        }
        if toggle_player_camera {
            if let Some(mesh) = self.editor.document_mut().meshes.get_mut(&name) {
                let script = mesh.script.get_or_insert(NuMeshScriptSection {
                    na_script: None,
                    cpp_script: None,
                    player_camera: false,
                });
                script.player_camera = !script.player_camera;
                if script.na_script.is_none()
                    && script.cpp_script.is_none()
                    && !script.player_camera
                {
                    mesh.script = None;
                }
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
        let mut toggle_casts_shadow = false;
        let mut toggle_preview_selected = false;
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
        if self.button(
            canvas,
            UiRect {
                x: rect.x + 14.0,
                y: *y,
                w: rect.w - 28.0,
                h: BUTTON_H,
            },
            if light_snapshot.casts_shadow {
                "CASTS SHADOW ON"
            } else {
                "CASTS SHADOW OFF"
            },
            light_snapshot.casts_shadow,
        ) {
            toggle_casts_shadow = true;
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
            if self.preview_selected_light_only {
                "PREVIEW SELECTED ONLY"
            } else {
                "PREVIEW ALL LIGHTS"
            },
            self.preview_selected_light_only,
        ) {
            toggle_preview_selected = true;
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
            self.record_undo_snapshot();
            if let Some(light) = self.editor.document_mut().lights.get_mut(&name) {
                light.kind = match light.kind {
                    LightKind::Point => LightKind::Directional,
                    LightKind::Directional => LightKind::Point,
                };
            }
        }
        if toggle_casts_shadow {
            self.record_undo_snapshot();
            if let Some(light) = self.editor.document_mut().lights.get_mut(&name) {
                light.casts_shadow = !light.casts_shadow;
            }
        }
        if toggle_preview_selected {
            self.preview_selected_light_only = !self.preview_selected_light_only;
            self.set_status(
                if self.preview_selected_light_only {
                    "PREVIEWING SELECTED LIGHT"
                } else {
                    "PREVIEWING ALL LIGHTS"
                },
                C_OK,
            );
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
        let metallic_down;
        let metallic_up;
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
        metallic_down = self.button(canvas, down, "M-", false);
        metallic_up = self.button(canvas, up, "M+", false);
        draw_text(
            canvas,
            [rect.x + 128.0, *y + 10.0],
            2.0,
            &format!("METAL {:.2}", material_snapshot.metallic),
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
        if rough_down || rough_up || metallic_down || metallic_up || clear_tex {
            self.record_undo_snapshot();
        }
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
        if metallic_down {
            if let Some(material) = self.editor.document_mut().materials.get_mut(&name) {
                material.metallic = (material.metallic - 0.05).max(0.0);
            }
        }
        if metallic_up {
            if let Some(material) = self.editor.document_mut().materials.get_mut(&name) {
                material.metallic = (material.metallic + 0.05).min(1.0);
            }
        }
        if pick_vert {
            if let Some(path) = FileDialog::new()
                .add_filter("vertex shader", &["vert", "glsl", "spv"])
                .pick_file()
            {
                self.record_undo_snapshot();
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
                self.record_undo_snapshot();
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
                self.record_undo_snapshot();
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
        let mut lighting = LightingConfig::default();
        let screenshot_shadow_boost_active = self.screenshot_shadow_boost_active();
        let camera_3d = if let Some(world) = self.compiled_world() {
            let has_explicit_lights = !world.light_entities().is_empty();
            let mut has_point_light = false;
            let mut has_shadow_casting_directional_light = false;
            let preview_selected_light_only = self.preview_selected_light_only;
            let selected_light_entity = self
                .selected_light
                .as_deref()
                .and_then(|name| world.light_entity(name));
            if has_explicit_lights {
                lighting.clear_point_lights();
                lighting.fill_light.intensity = 0.0;
                lighting.shadows.mode = ShadowMode::Off;
            }
            if let Some(environment) = world.environment() {
                lighting.ambient_color = environment.ambient_color;
                lighting.ambient_intensity = environment.ambient_intensity;
                lighting.shadows.mode = environment.shadow_mode;
                lighting.shadows.live.max_distance = environment.shadow_max_distance;
                lighting.shadows.live.filter_radius = environment.shadow_filter_radius;
            }
            for entity in world.light_entities().values().copied() {
                let Some(light) = world.light(entity) else {
                    continue;
                };
                if preview_selected_light_only
                    && selected_light_entity.is_some()
                    && selected_light_entity != Some(entity)
                {
                    continue;
                }
                match light.kind {
                    LightKind::Point => {
                        has_point_light = true;
                        let _ = lighting.push_point_light(
                            PointLight {
                                position: light.position,
                                color: light.color,
                                intensity: light.intensity,
                                range: 18.0,
                            },
                            light.casts_shadow,
                        );
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
                        if light.casts_shadow {
                            has_shadow_casting_directional_light = true;
                            if let Some(environment) = world.environment() {
                                lighting.shadows.mode = environment.shadow_mode;
                            } else {
                                lighting.shadows.mode = ShadowMode::Live;
                            }
                        }
                    }
                }
            }
            if has_explicit_lights && !has_shadow_casting_directional_light {
                lighting.shadows.mode = ShadowMode::Off;
            }
            if has_explicit_lights && !has_point_light {
                lighting.clear_point_lights();
            }
            world
                .primary_camera()
                .map_or(Camera3D::default(), |camera| Camera3D {
                    position: camera.position,
                    target: camera.target,
                    up: [0.0, 1.0, 0.0],
                    fov_y_degrees: camera.fov_degrees,
                    near_clip: 0.1,
                    far_clip: 200.0,
                })
        } else {
            let document = self.editor.document();
            if let Some(environment) = &document.environment {
                lighting.ambient_color = environment.ambient_color;
                lighting.ambient_intensity = environment.ambient_intensity;
                lighting.shadows.mode = environment.shadow_mode;
                lighting.shadows.live.max_distance = environment.shadow_max_distance;
                lighting.shadows.live.filter_radius = environment.shadow_filter_radius;
            }
            Camera3D {
                position: document.camera.position,
                target: document.camera.target,
                up: [0.0, 1.0, 0.0],
                fov_y_degrees: document.camera.fov_degrees,
                near_clip: 0.1,
                far_clip: 200.0,
            }
        };
        if screenshot_shadow_boost_active {
            if lighting.fill_light.intensity > 0.0 {
                lighting.shadows.mode = ShadowMode::Live;
            }
            lighting.shadows.minimum_visibility = lighting.shadows.minimum_visibility.min(0.06);
            lighting.shadows.bias = lighting.shadows.bias.min(0.0015);
            lighting.shadows.live.filter_radius = lighting.shadows.live.filter_radius.max(2.5);
            lighting.shadows.live.max_distance = lighting.shadows.live.max_distance.min(24.0);
        }
        lighting.shadows.mode = if self.live_shadows_enabled || screenshot_shadow_boost_active {
            lighting.shadows.mode
        } else {
            ShadowMode::Off
        };
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
            camera_3d,
            lighting,
            screenshot_path: self.pending_screenshot_path.borrow_mut().take(),
            screenshot_accumulation_samples: 1,
            screenshot_resolution: crate::scene::ScreenshotResolution::K4,
            capture_cursor: false,
        }
    }

    fn update(&mut self, delta_time_seconds: f32) {
        if self.screenshot_shadow_boost_frames > 0 {
            self.screenshot_shadow_boost_frames -= 1;
        }
        if self.is_playing() {
            self.update_play_mode(delta_time_seconds);
        }
        if let Some(hot_reload) = &mut self.hot_reload {
            match hot_reload
                .poll_changes_with_events(&mut self.event_bus, EventDeliveryMode::Queued)
            {
                Ok(Some(batch)) => {
                    if let Some(scene) = &batch.scene {
                        self.editor.replace_document(scene.clone());
                        self.clear_history();
                        self.play_session = None;
                        self.player_input = PlayerInputState::default();
                    }
                    self.sync_editor_mesh_assets();
                    self.last_reload = Some(batch.clone());
                    self.set_selected_mesh(None);
                    self.selected_light = None;
                    self.selected_material = None;
                    self.ensure_selection();
                    self.set_status(
                        format!(
                            "AUTO RELOAD {} SH / {} TX",
                            batch.shaders.len(),
                            batch.textures.len()
                        ),
                        C_OK,
                    );
                }
                Ok(None) => {}
                Err(error) => self.set_status(format!("WATCH FAILED: {error}"), C_WARN),
            }
        }
        self.event_bus.process_queued();
    }

    fn window_event(&mut self, _window: &winit::window::Window, event: &WindowEvent) {
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
                    if matches!(
                        self.drag_state,
                        DragState::Gizmo { .. } | DragState::LightGizmo { .. }
                    ) {
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
                    if !self.is_playing() && self.point_in_viewport() {
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
                if !self.is_playing()
                    && self.point_in_viewport()
                    && matches!(self.drag_state, DragState::None)
                    && !self.left_mouse_down
                    && !self.right_mouse_down
                {
                    let amount = match delta {
                        MouseScrollDelta::LineDelta(_, y) => *y * 0.6,
                        MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.02,
                    };
                    if amount.abs() > f32::EPSILON {
                        zoom_document_camera(self.editor.document_mut(), amount);
                    }
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers_state = modifiers.state();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    let pressed = event.state == ElementState::Pressed;
                    let _ = self.handle_play_input_key(code, pressed);
                    if pressed {
                        if code == KeyCode::PrintScreen {
                            self.trigger_screenshot_shadow_boost();
                        }
                        if code == KeyCode::F12 {
                            self.queue_screenshot_capture();
                        }
                        if code == KeyCode::F8 {
                            self.toggle_play_mode();
                            return;
                        }
                        if self.is_playing() {
                            return;
                        }
                        if self.handle_history_shortcut(code) {
                            return;
                        }
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
        let viewport = self.viewport_rect();
        if let Some(world) = self.compiled_world() {
            populate_scene_preview_from_world(
                frame,
                &world,
                self.selected_mesh.as_deref(),
                self.show_physics_debug,
                viewport,
                scene_base_dir.as_deref(),
                &mut self.obj_mesh_cache,
            );
        } else {
            populate_scene_preview(
                frame,
                self.editor.document(),
                self.selected_mesh.as_deref(),
                self.show_physics_debug,
                viewport,
                scene_base_dir.as_deref(),
                &mut self.obj_mesh_cache,
            );
        }
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
    show_physics_debug: bool,
    viewport: UiRect,
    scene_base_dir: Option<&Path>,
    obj_mesh_cache: &mut HashMap<PathBuf, Result<Arc<MeshAsset3D>, String>>,
) {
    draw_editor_grid_overlay(frame, document, viewport);
    for (mesh_name, mesh) in &document.meshes {
        let mut draw = resolve_mesh_draw(document, mesh, 0, scene_base_dir, obj_mesh_cache);
        if selected_mesh == Some(mesh_name.as_str()) {
            draw.color = brighten_color(draw.color, 0.18);
        }
        frame.draw_mesh_3d(draw);
    }
    if show_physics_debug {
        draw_physics_debug_overlay(frame, document, viewport, scene_base_dir, obj_mesh_cache);
    }
}

fn populate_scene_preview_from_world(
    frame: &mut SceneFrame,
    world: &NuSceneWorld,
    selected_mesh: Option<&str>,
    show_physics_debug: bool,
    viewport: UiRect,
    scene_base_dir: Option<&Path>,
    obj_mesh_cache: &mut HashMap<PathBuf, Result<Arc<MeshAsset3D>, String>>,
) {
    draw_editor_grid_overlay_world(frame, world, viewport);
    for (mesh_name, entity) in world.mesh_entities().iter() {
        let mut draw = resolve_mesh_draw_from_world(world, *entity, scene_base_dir, obj_mesh_cache);
        if selected_mesh == Some(mesh_name.as_str()) {
            draw.color = brighten_color(draw.color, 0.18);
        }
        frame.draw_mesh_3d(draw);
    }
    if show_physics_debug {
        draw_physics_debug_overlay_world(frame, world, viewport, scene_base_dir, obj_mesh_cache);
    }
}

fn draw_editor_grid_overlay(frame: &mut SceneFrame, document: &NuSceneDocument, viewport: UiRect) {
    let _ = (document, viewport);
    draw_editor_grid_meshes(frame);
}

fn draw_editor_grid_overlay_world(frame: &mut SceneFrame, world: &NuSceneWorld, viewport: UiRect) {
    let _ = (world, viewport);
    draw_editor_grid_meshes(frame);
}

fn draw_editor_grid_meshes(frame: &mut SceneFrame) {
    const GRID_EXTENT: i32 = 12;
    const GRID_STEP: f32 = 1.0;
    const GRID_Y: f32 = 0.002;
    const GRID_THICKNESS_MINOR: f32 = 0.028;
    const GRID_THICKNESS_MAJOR: f32 = 0.042;
    let full_span = (GRID_EXTENT * 2) as f32 * GRID_STEP;

    for index in -GRID_EXTENT..=GRID_EXTENT {
        let position = index as f32 * GRID_STEP;
        let major = index == 0 || index % 5 == 0;
        let color = if index == 0 {
            [0.30, 0.34, 0.44, 1.0]
        } else if major {
            [0.18, 0.20, 0.26, 1.0]
        } else {
            [0.11, 0.12, 0.16, 1.0]
        };
        let z_color = if index == 0 {
            [0.26, 0.32, 0.46, 1.0]
        } else {
            color
        };
        let thickness = if major {
            GRID_THICKNESS_MAJOR
        } else {
            GRID_THICKNESS_MINOR
        };
        frame.draw_mesh_3d(MeshDraw3D {
            mesh: Mesh3D::Plane,
            center: [position, GRID_Y, 0.0],
            size: [thickness, 1.0, full_span],
            rotation_radians: [0.0, 0.0, 0.0],
            color,
            material: MeshMaterial3D::default(),
        });
        frame.draw_mesh_3d(MeshDraw3D {
            mesh: Mesh3D::Plane,
            center: [0.0, GRID_Y, position],
            size: [full_span, 1.0, thickness],
            rotation_radians: [0.0, 0.0, 0.0],
            color: z_color,
            material: MeshMaterial3D::default(),
        });
    }
}

fn draw_physics_debug_overlay(
    frame: &mut SceneFrame,
    document: &NuSceneDocument,
    viewport: UiRect,
    scene_base_dir: Option<&Path>,
    obj_mesh_cache: &mut HashMap<PathBuf, Result<Arc<MeshAsset3D>, String>>,
) {
    let mut ui = frame.ui_canvas();
    for mesh in document.meshes.values() {
        let Some(physics) = &mesh.physics else {
            continue;
        };
        let draw = resolve_mesh_draw(document, mesh, 0, scene_base_dir, obj_mesh_cache);
        match physics.collider {
            NuPhysicsColliderKind::Plane => {}
            NuPhysicsColliderKind::Sphere => {
                if let Some(screen) = project_world_to_screen(
                    document.camera.position,
                    document.camera.target,
                    document.camera.fov_degrees,
                    viewport,
                    draw.center,
                ) {
                    let radius = projected_mesh_radius(
                        document.camera.position,
                        document.camera.target,
                        document.camera.fov_degrees,
                        viewport,
                        draw.center,
                        draw.size,
                    );
                    ui.stroke_circle(
                        screen,
                        radius.max(12.0),
                        [0.48, 0.86, 1.0, 0.95],
                        1.5,
                        UI_LAYER_TEXT,
                    );
                }
            }
            NuPhysicsColliderKind::Auto | NuPhysicsColliderKind::Cuboid => {
                draw_cuboid_debug_outline(
                    &mut ui,
                    document,
                    viewport,
                    draw.center,
                    draw.size,
                    draw.rotation_radians,
                    [0.48, 0.86, 1.0, 0.95],
                );
            }
        }
    }
}

fn draw_physics_debug_overlay_world(
    frame: &mut SceneFrame,
    world: &NuSceneWorld,
    viewport: UiRect,
    scene_base_dir: Option<&Path>,
    obj_mesh_cache: &mut HashMap<PathBuf, Result<Arc<MeshAsset3D>, String>>,
) {
    let Some(camera) = world.primary_camera() else {
        return;
    };
    let mut ui = frame.ui_canvas();
    for entity in world.mesh_entities().values().copied() {
        let Some(physics) = world.physics_body(entity) else {
            continue;
        };
        let draw = resolve_mesh_draw_from_world(world, entity, scene_base_dir, obj_mesh_cache);
        match physics.body.collider {
            NuPhysicsColliderKind::Plane => {}
            NuPhysicsColliderKind::Sphere => {
                if let Some(screen) = project_world_to_screen(
                    camera.position,
                    camera.target,
                    camera.fov_degrees,
                    viewport,
                    draw.center,
                ) {
                    let radius = projected_mesh_radius(
                        camera.position,
                        camera.target,
                        camera.fov_degrees,
                        viewport,
                        draw.center,
                        draw.size,
                    );
                    ui.stroke_circle(
                        screen,
                        radius.max(12.0),
                        [0.48, 0.86, 1.0, 0.95],
                        1.5,
                        UI_LAYER_TEXT,
                    );
                }
            }
            NuPhysicsColliderKind::Auto | NuPhysicsColliderKind::Cuboid => {
                draw_cuboid_debug_outline_camera(
                    &mut ui,
                    camera.position,
                    camera.target,
                    camera.fov_degrees,
                    viewport,
                    draw.center,
                    draw.size,
                    draw.rotation_radians,
                    [0.48, 0.86, 1.0, 0.95],
                );
            }
        }
    }
}

fn draw_cuboid_debug_outline(
    canvas: &mut Canvas2D<'_>,
    document: &NuSceneDocument,
    viewport: UiRect,
    center: [f32; 3],
    size: [f32; 3],
    rotation_radians: [f32; 3],
    color: [f32; 4],
) {
    draw_cuboid_debug_outline_camera(
        canvas,
        document.camera.position,
        document.camera.target,
        document.camera.fov_degrees,
        viewport,
        center,
        size,
        rotation_radians,
        color,
    );
}

fn draw_cuboid_debug_outline_camera(
    canvas: &mut Canvas2D<'_>,
    camera_position: [f32; 3],
    camera_target: [f32; 3],
    camera_fov_degrees: f32,
    viewport: UiRect,
    center: [f32; 3],
    size: [f32; 3],
    rotation_radians: [f32; 3],
    color: [f32; 4],
) {
    let half = [size[0] * 0.5, size[1] * 0.5, size[2] * 0.5];
    let local = [
        [-half[0], -half[1], -half[2]],
        [half[0], -half[1], -half[2]],
        [half[0], half[1], -half[2]],
        [-half[0], half[1], -half[2]],
        [-half[0], -half[1], half[2]],
        [half[0], -half[1], half[2]],
        [half[0], half[1], half[2]],
        [-half[0], half[1], half[2]],
    ];
    let corners = local.map(|point| add3(rotate_vector_3d(point, rotation_radians), center));
    let mut projected = [[0.0; 2]; 8];
    for (index, corner) in corners.into_iter().enumerate() {
        let Some(screen) = project_world_to_screen(
            camera_position,
            camera_target,
            camera_fov_degrees,
            viewport,
            corner,
        ) else {
            return;
        };
        projected[index] = screen;
    }
    let edges = [
        (0usize, 1usize),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    for (a, b) in edges {
        canvas.line(projected[a], projected[b], 1.3, color, UI_LAYER_TEXT);
    }
}

fn resolve_mesh_draw_from_world(
    world: &NuSceneWorld,
    entity: u64,
    scene_base_dir: Option<&Path>,
    obj_mesh_cache: &mut HashMap<PathBuf, Result<Arc<MeshAsset3D>, String>>,
) -> MeshDraw3D {
    let mesh = world
        .mesh_renderer(entity)
        .expect("world mesh entity should have a mesh renderer");
    let transform = world
        .resolved_transform(entity)
        .expect("world mesh entity should have a resolved transform");
    let mut scale = transform.scale;
    let rotation = [
        transform.rotation_degrees[0].to_radians(),
        transform.rotation_degrees[1].to_radians(),
        transform.rotation_degrees[2].to_radians(),
    ];
    let material = world.materials().get(&mesh.material);
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
        center: transform.position,
        size: scale,
        rotation_radians: rotation,
        color,
        material: MeshMaterial3D {
            albedo_texture: None,
            roughness: material.map_or(0.5, |material| material.roughness),
            metallic: material.map_or(0.0, |material| material.metallic),
        },
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
        material: MeshMaterial3D {
            albedo_texture: None,
            roughness: material.map_or(0.5, |material| material.roughness),
            metallic: material.map_or(0.0, |material| material.metallic),
        },
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

fn shadow_mode_label(mode: ShadowMode) -> &'static str {
    match mode {
        ShadowMode::Off => "OFF",
        ShadowMode::Live => "LIVE",
    }
}

fn physics_body_label(kind: NuPhysicsBodyKind) -> &'static str {
    match kind {
        NuPhysicsBodyKind::Static => "STATIC",
        NuPhysicsBodyKind::Dynamic => "DYNAMIC",
        NuPhysicsBodyKind::Kinematic => "KINEMATIC",
    }
}

fn physics_collider_label(kind: NuPhysicsColliderKind) -> &'static str {
    match kind {
        NuPhysicsColliderKind::Auto => "AUTO",
        NuPhysicsColliderKind::Cuboid => "CUBOID",
        NuPhysicsColliderKind::Sphere => "SPHERE",
        NuPhysicsColliderKind::Plane => "PLANE",
    }
}

fn default_mesh_physics(mesh: &NuMeshSection) -> NuPhysicsSection {
    NuPhysicsSection {
        body: if mesh.geometry.eq_ignore_ascii_case("plane") {
            NuPhysicsBodyKind::Static
        } else {
            NuPhysicsBodyKind::Dynamic
        },
        collider: if mesh.geometry.eq_ignore_ascii_case("sphere") {
            NuPhysicsColliderKind::Sphere
        } else if mesh.geometry.eq_ignore_ascii_case("plane") {
            NuPhysicsColliderKind::Plane
        } else {
            NuPhysicsColliderKind::Auto
        },
        mass: 1.0,
    }
}

fn play_movement_delta(
    program: Option<&NaScriptProgram>,
    input: PlayerInputState,
    delta_time_seconds: f32,
    yaw_radians: f32,
) -> [f32; 3] {
    let forward = [yaw_radians.sin(), 0.0, yaw_radians.cos()];
    let right = [forward[2], 0.0, -forward[0]];
    let mut movement = [0.0, 0.0, 0.0];

    if let Some(program) = program {
        for binding in &program.move_bindings {
            let pressed = match binding.key.as_str() {
                "W" => input.forward,
                "S" => input.backward,
                "A" => input.left,
                "D" => input.right,
                _ => false,
            };
            if !pressed {
                continue;
            }
            let direction = match binding.direction {
                NaMoveDirection::Forward => forward,
                NaMoveDirection::Backward => scale3(forward, -1.0),
                NaMoveDirection::Left => scale3(right, -1.0),
                NaMoveDirection::Right => right,
            };
            movement = add3(
                movement,
                scale3(direction, binding.speed * delta_time_seconds),
            );
        }
        return movement;
    }

    if input.forward {
        movement = add3(movement, forward);
    }
    if input.backward {
        movement = sub3(movement, forward);
    }
    if input.right {
        movement = add3(movement, right);
    }
    if input.left {
        movement = sub3(movement, right);
    }
    if length3(movement) > 0.0001 {
        return scale3(normalize3(movement), 4.5 * delta_time_seconds);
    }
    [0.0, 0.0, 0.0]
}

fn project_world_to_screen(
    camera_position: [f32; 3],
    camera_target: [f32; 3],
    fov_degrees: f32,
    viewport: UiRect,
    point: [f32; 3],
) -> Option<[f32; 2]> {
    let aspect = (viewport.w / viewport.h.max(1.0)).max(0.0001);
    let view_projection = mul_mat4_editor(
        perspective_lh_editor(fov_degrees.to_radians(), aspect, 0.1, 200.0),
        look_at_lh_editor(camera_position, camera_target, [0.0, 1.0, 0.0]),
    );
    let clip = transform_point4_mat4_editor(view_projection, point);
    if clip[3] <= 0.01 {
        return None;
    }
    let inv_w = 1.0 / clip[3];
    let ndc_x = clip[0] * inv_w;
    let ndc_y = clip[1] * inv_w;
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

fn perspective_lh_editor(fov_y_radians: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let f = 1.0 / (fov_y_radians * 0.5).tan().max(0.0001);
    let range = (far - near).max(0.0001);
    [
        [f / aspect.max(0.0001), 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, far / range, (-near * far) / range],
        [0.0, 0.0, 1.0, 0.0],
    ]
}

fn look_at_lh_editor(eye: [f32; 3], center: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
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

fn mul_mat4_editor(left: [[f32; 4]; 4], right: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
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

fn transform_point4_mat4_editor(matrix: [[f32; 4]; 4], point: [f32; 3]) -> [f32; 4] {
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

fn projected_mesh_screen_bounds(
    camera_position: [f32; 3],
    camera_target: [f32; 3],
    fov_degrees: f32,
    viewport: UiRect,
    center: [f32; 3],
    size: [f32; 3],
    rotation_radians: [f32; 3],
) -> Option<([f32; 2], [f32; 2])> {
    let half = [size[0] * 0.5, size[1] * 0.5, size[2] * 0.5];
    let local = [
        [-half[0], -half[1], -half[2]],
        [half[0], -half[1], -half[2]],
        [half[0], half[1], -half[2]],
        [-half[0], half[1], -half[2]],
        [-half[0], -half[1], half[2]],
        [half[0], -half[1], half[2]],
        [half[0], half[1], half[2]],
        [-half[0], half[1], half[2]],
    ];
    let mut min = [f32::INFINITY, f32::INFINITY];
    let mut max = [f32::NEG_INFINITY, f32::NEG_INFINITY];
    let mut projected_any = false;
    for point in local {
        let world = add3(rotate_vector_3d(point, rotation_radians), center);
        let Some(screen) =
            project_world_to_screen(camera_position, camera_target, fov_degrees, viewport, world)
        else {
            continue;
        };
        projected_any = true;
        min[0] = min[0].min(screen[0]);
        min[1] = min[1].min(screen[1]);
        max[0] = max[0].max(screen[0]);
        max[1] = max[1].max(screen[1]);
    }
    if projected_any {
        Some((min, max))
    } else {
        None
    }
}

fn gizmo_screen_origin(
    camera_position: [f32; 3],
    camera_target: [f32; 3],
    camera_fov_degrees: f32,
    viewport: UiRect,
    center: [f32; 3],
) -> Option<[f32; 2]> {
    project_world_to_screen(
        camera_position,
        camera_target,
        camera_fov_degrees,
        viewport,
        center,
    )
}

fn draw_mesh_gizmo(
    canvas: &mut Canvas2D<'_>,
    _camera_position: [f32; 3],
    _camera_target: [f32; 3],
    _camera_fov_degrees: f32,
    _viewport: UiRect,
    _center: [f32; 3],
    _size: [f32; 3],
    _rotation_radians: [f32; 3],
    tool_mode: MeshToolMode,
    hovered_axis: Option<GizmoAxis>,
    active_axis: Option<GizmoAxis>,
    visuals: Vec<GizmoAxisVisual>,
) {
    for visual in visuals {
        let active = active_axis == Some(visual.axis);
        let hovered = hovered_axis == Some(visual.axis);
        let line_width = if active {
            3.6
        } else if hovered {
            3.0
        } else {
            2.4
        };
        let color = if active {
            brighten_color(visual.color, 0.35)
        } else if hovered {
            brighten_color(visual.color, 0.18)
        } else {
            visual.color
        };
        canvas.line(
            visual.screen_start,
            visual.screen_end,
            line_width,
            color,
            UI_LAYER_TEXT,
        );
        let fill = match tool_mode {
            MeshToolMode::Move => color,
            MeshToolMode::Deform => [
                color[0],
                color[1],
                color[2],
                if active { 0.40 } else { 0.24 },
            ],
            MeshToolMode::Pivot => [
                color[0],
                color[1],
                color[2],
                if active { 0.55 } else { 0.32 },
            ],
        };
        let handle_size = if active {
            [15.0, 15.0]
        } else if hovered {
            [13.0, 13.0]
        } else {
            [12.0, 12.0]
        };
        canvas.fill_rect(visual.screen_end, handle_size, 0.0, fill, UI_LAYER_TEXT + 1);
        canvas.stroke_rect(
            visual.screen_end,
            handle_size,
            0.0,
            color,
            if active { 1.8 } else { 1.3 },
            UI_LAYER_TEXT + 2,
        );
    }
}

fn gizmo_axis_visuals(
    camera_position: [f32; 3],
    camera_target: [f32; 3],
    camera_fov_degrees: f32,
    viewport: UiRect,
    center: [f32; 3],
    size: [f32; 3],
    rotation_radians: [f32; 3],
    space: GizmoSpace,
) -> Vec<GizmoAxisVisual> {
    const GIZMO_AXIS_LENGTH_PX: f32 = 72.0;
    let Some(screen_center) = gizmo_screen_origin(
        camera_position,
        camera_target,
        camera_fov_degrees,
        viewport,
        center,
    ) else {
        return Vec::new();
    };
    let extent = size[0].max(size[1]).max(size[2]).max(1.0);
    let defs = [
        (GizmoAxis::X, [1.0, 0.0, 0.0], [0.96, 0.30, 0.30, 1.0]),
        (GizmoAxis::Y, [0.0, 1.0, 0.0], [0.38, 0.90, 0.46, 1.0]),
        (GizmoAxis::Z, [0.0, 0.0, 1.0], [0.36, 0.62, 1.0, 1.0]),
    ];
    let mut visuals = Vec::new();
    for (axis, axis_direction, color) in defs {
        let world_direction = axis_direction_for_space(axis_direction, rotation_radians, space);
        let end_world = add3(center, scale3(world_direction, extent));
        let Some(screen_end) = project_world_to_screen(
            camera_position,
            camera_target,
            camera_fov_degrees,
            viewport,
            end_world,
        ) else {
            continue;
        };
        let axis_screen = sub2(screen_end, screen_center);
        let axis_screen_len = length2(axis_screen);
        if axis_screen_len <= 0.0001 {
            continue;
        }
        let axis_screen_dir = [
            axis_screen[0] / axis_screen_len,
            axis_screen[1] / axis_screen_len,
        ];
        let pixels_per_unit = axis_screen_len / extent.max(0.001);
        visuals.push(GizmoAxisVisual {
            axis,
            screen_start: screen_center,
            screen_end: [
                screen_center[0] + axis_screen_dir[0] * GIZMO_AXIS_LENGTH_PX,
                screen_center[1] + axis_screen_dir[1] * GIZMO_AXIS_LENGTH_PX,
            ],
            world_direction,
            color,
            pixels_per_unit,
        });
    }
    visuals
}

fn pick_gizmo_axis(visuals: Vec<GizmoAxisVisual>, mouse: [f32; 2]) -> Option<GizmoAxisVisual> {
    let mut best: Option<(GizmoAxisVisual, f32)> = None;
    for visual in visuals {
        let distance = point_segment_distance(mouse, visual.screen_start, visual.screen_end);
        let handle_distance = length2(sub2(mouse, visual.screen_end));
        let score = distance.min(handle_distance);
        if score > 14.0 {
            continue;
        }
        match best {
            Some((_, best_score)) if score >= best_score => {}
            _ => best = Some((visual, score)),
        }
    }
    best.map(|(visual, _)| visual)
}

fn apply_gizmo_delta(
    mesh: &mut NuMeshSection,
    axis: GizmoAxis,
    mode: MeshToolMode,
    space: GizmoSpace,
    start_position: [f32; 3],
    start_scale: [f32; 3],
    start_rotation_radians: [f32; 3],
    axis_world_direction: [f32; 3],
    delta_units: f32,
) {
    let axis_index = match axis {
        GizmoAxis::X => 0,
        GizmoAxis::Y => 1,
        GizmoAxis::Z => 2,
    };
    match mode {
        MeshToolMode::Move => {
            mesh.transform.position = start_position;
            let delta = match space {
                GizmoSpace::World => {
                    let mut axis_delta = [0.0, 0.0, 0.0];
                    axis_delta[axis_index] = delta_units;
                    axis_delta
                }
                GizmoSpace::Local => scale3(axis_world_direction, delta_units),
            };
            mesh.transform.position = add3(start_position, delta);
        }
        MeshToolMode::Deform => {
            mesh.transform.position = start_position;
            mesh.transform.scale = start_scale;
            let next_scale = (start_scale[axis_index] + delta_units).max(0.1);
            let applied_delta = next_scale - start_scale[axis_index];
            mesh.transform.scale[axis_index] = next_scale;
            let offset_direction = match space {
                GizmoSpace::World => {
                    let mut axis_delta = [0.0, 0.0, 0.0];
                    axis_delta[axis_index] = 1.0;
                    axis_delta
                }
                GizmoSpace::Local => axis_direction_for_space(
                    match axis {
                        GizmoAxis::X => [1.0, 0.0, 0.0],
                        GizmoAxis::Y => [0.0, 1.0, 0.0],
                        GizmoAxis::Z => [0.0, 0.0, 1.0],
                    },
                    start_rotation_radians,
                    GizmoSpace::Local,
                ),
            };
            mesh.transform.position = add3(
                start_position,
                scale3(offset_direction, applied_delta * 0.5),
            );
        }
        MeshToolMode::Pivot => {}
    }
}

fn axis_direction_for_space(
    axis_direction: [f32; 3],
    rotation_radians: [f32; 3],
    space: GizmoSpace,
) -> [f32; 3] {
    match space {
        GizmoSpace::World => axis_direction,
        GizmoSpace::Local => normalize3(rotate_vector_3d(axis_direction, rotation_radians)),
    }
}

fn sub2(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn length2(v: [f32; 2]) -> f32 {
    (v[0] * v[0] + v[1] * v[1]).sqrt()
}

fn normalize2(v: [f32; 2]) -> [f32; 2] {
    let len = length2(v);
    if len <= f32::EPSILON {
        [1.0, 0.0]
    } else {
        [v[0] / len, v[1] / len]
    }
}

fn dot2(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}

fn point_segment_distance(point: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let ab = sub2(b, a);
    let ab_len_sq = dot2(ab, ab).max(0.0001);
    let ap = sub2(point, a);
    let t = (dot2(ap, ab) / ab_len_sq).clamp(0.0, 1.0);
    let closest = [a[0] + ab[0] * t, a[1] + ab[1] * t];
    length2(sub2(point, closest))
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

fn orbit_document_camera(document: &mut NuSceneDocument, pivot: [f32; 3], delta: [f32; 2]) {
    let offset = sub3(document.camera.position, pivot);
    let radius = length3(offset).max(0.25);
    let mut yaw = offset[0].atan2(offset[2]);
    let mut pitch = (offset[1] / radius).asin();
    yaw += delta[0] * 0.01;
    pitch = (pitch + delta[1] * 0.01).clamp(-1.3, 1.3);
    let horizontal = radius * pitch.cos();
    document.camera.target = pivot;
    document.camera.position = [
        pivot[0] + horizontal * yaw.sin(),
        pivot[1] + radius * pitch.sin(),
        pivot[2] + horizontal * yaw.cos(),
    ];
}

fn zoom_document_camera(document: &mut NuSceneDocument, amount: f32) {
    let view_forward = normalize3(sub3(document.camera.target, document.camera.position));
    let step = amount * 0.8;
    let delta = scale3(view_forward, step);
    document.camera.position = add3(document.camera.position, delta);
    document.camera.target = add3(document.camera.target, delta);
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

fn rotate_vector_3d(vector: [f32; 3], rotation_radians: [f32; 3]) -> [f32; 3] {
    let (sx, cx) = rotation_radians[0].sin_cos();
    let (sy, cy) = rotation_radians[1].sin_cos();
    let (sz, cz) = rotation_radians[2].sin_cos();

    let mut v = vector;
    v = [v[0], v[1] * cx - v[2] * sx, v[1] * sx + v[2] * cx];
    v = [v[0] * cy + v[2] * sy, v[1], -v[0] * sy + v[2] * cy];
    [v[0] * cz - v[1] * sz, v[0] * sz + v[1] * cz, v[2]]
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
