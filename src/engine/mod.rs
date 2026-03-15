pub mod world;

use crate::event::{EngineEvent, EventBus, EventDeliveryMode, ResourceEventKind};
use crate::lighting::ShadowMode;
use crate::physics::{BodyHandle, ColliderShape, PhysicsConfig, PhysicsWorld, RigidBody};
use crate::resource::{AssetHandle, AssetKind, AssetManager, AssetState};
use crate::scene::{MeshAsset3D, MeshVertex3D};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use shaderc::{CompileOptions, Compiler, EnvVersion, OptimizationLevel, ShaderKind, TargetEnv};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use tobj::LoadOptions;
use world::NuSceneWorld;

pub const NU_SCENE_EXTENSION: &str = "nuscene";
pub const NU_SCENE_FORMAT_HEADER: &str = "# nu scene format v1";
pub const EXPLICIT_NUSCENE_TEMPLATE: &str = r#"# nu scene format v1
# .nuscene

[meta]
engine = "nu"
format = "nu_scene_v1"
extension = ".nuscene"

[scene]
name = "test_scene"
syntax = opengl
# syntax selects the source profile translated into Vulkan in nu_scene_v1.

[camera]
position = 0.0, 5.0, 10.0
target = 0.0, 0.0, 0.0
fov = 60.0

[light.key]
type = point
position = 5.0, 8.0, 3.0
color = 1.0, 1.0, 1.0
intensity = 1.0

[light.fill]
type = point
position = -3.0, 4.0, -2.0
color = 0.45, 0.48, 0.60
intensity = 0.35

[mesh.car]
geometry = cube
material = red_material
transform.position = 0.0, 1.0, 0.0
transform.rotation_degrees = 45.0, 0.0, 0.0
transform.scale = 1.0, 1.0, 1.0

# Built-in geometry currently supported by nu:
# cube
# plane
# sphere
#
# Example imported mesh:
# [mesh.crate]
# geometry = obj
# source = meshes/crate.obj
# material = red_material
# transform.position = 2.0, 1.0, 0.0
# transform.rotation_degrees = 0.0, 0.0, 0.0
# transform.scale = 1.0, 1.0, 1.0
# script.na = scripts/player_controller.na
# script.cpp = scripts/player_controller.cpp
# script.player_camera = true
#
# Example parented mesh:
# [mesh.wheel]
# geometry = cube
# material = wheel_material
# parent = mesh.car
# transform.position = 1.0, -0.5, 0.0
# transform.rotation_degrees = 0.0, 0.0, 90.0
# transform.scale = 0.5, 0.5, 0.2

[environment]
ambient_color = 0.1, 0.1, 0.15
ambient_intensity = 0.3
shadow_mode = live
shadow_max_distance = 32.0
shadow_filter_radius = 1.5

[material.red_material]
shader.vertex = lit.vert
shader.fragment = lit.frag
color = 1.0, 0.0, 0.0
roughness = 0.5
metallic = 0.0
albedo_texture = crate.png

# [material.wheel_material]
# shader.vertex = lit.vert
# shader.fragment = lit.frag
# color = 0.15, 0.15, 0.15
# roughness = 0.7
# metallic = 0.0
"#;

#[derive(Debug)]
pub enum EngineError {
    Io {
        path: Option<PathBuf>,
        reason: String,
    },
    Parse {
        line: usize,
        reason: String,
    },
    InvalidScene {
        reason: String,
    },
    Notify {
        reason: String,
    },
    ShaderCompile {
        path: PathBuf,
        reason: String,
    },
    MeshLoad {
        path: PathBuf,
        reason: String,
    },
}

impl Display for EngineError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, reason } => match path {
                Some(path) => write!(f, "io error at {}: {reason}", path.display()),
                None => write!(f, "io error: {reason}"),
            },
            Self::Parse { line, reason } => write!(f, "scene parse error on line {line}: {reason}"),
            Self::InvalidScene { reason } => write!(f, "invalid nu scene: {reason}"),
            Self::Notify { reason } => write!(f, "hot reload notify error: {reason}"),
            Self::ShaderCompile { path, reason } => {
                write!(f, "shader compile failed for {}: {reason}", path.display())
            }
            Self::MeshLoad { path, reason } => {
                write!(f, "mesh load failed for {}: {reason}", path.display())
            }
        }
    }
}

impl Error for EngineError {}

impl From<notify::Error> for EngineError {
    fn from(value: notify::Error) -> Self {
        Self::Notify {
            reason: value.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneSyntax {
    OpenGl,
    Vulkan,
    Raw,
}

impl SceneSyntax {
    fn parse(value: &str) -> Result<Self, EngineError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "opengl" => Ok(Self::OpenGl),
            "vulkan" => Ok(Self::Vulkan),
            "raw" => Ok(Self::Raw),
            other => Err(EngineError::InvalidScene {
                reason: format!("unsupported syntax `{other}`"),
            }),
        }
    }
}

pub type SceneBackend = SceneSyntax;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightKind {
    Point,
    Directional,
}

impl LightKind {
    fn parse(value: &str) -> Result<Self, EngineError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "point" => Ok(Self::Point),
            "directional" => Ok(Self::Directional),
            other => Err(EngineError::InvalidScene {
                reason: format!("unsupported light type `{other}`"),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NuSceneMetadata {
    pub format_version: u32,
    pub engine_name: String,
    pub format_name: String,
    pub extension: String,
}

impl Default for NuSceneMetadata {
    fn default() -> Self {
        Self {
            format_version: 1,
            engine_name: "nu".to_string(),
            format_name: "nu_scene_v1".to_string(),
            extension: NU_SCENE_EXTENSION.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NuSceneSection {
    pub name: String,
    pub syntax: SceneSyntax,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NuCameraSection {
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub fov_degrees: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NuLightSection {
    pub name: String,
    pub kind: LightKind,
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
    pub casts_shadow: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NuTransform {
    pub position: [f32; 3],
    pub rotation_degrees: [f32; 3],
    pub scale: [f32; 3],
}

impl Default for NuTransform {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NuPhysicsBodyKind {
    Static,
    Dynamic,
    Kinematic,
}

impl NuPhysicsBodyKind {
    fn parse(value: &str) -> Result<Self, EngineError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "static" => Ok(Self::Static),
            "dynamic" => Ok(Self::Dynamic),
            "kinematic" => Ok(Self::Kinematic),
            other => Err(EngineError::InvalidScene {
                reason: format!("unsupported physics body `{other}`"),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NuPhysicsColliderKind {
    Auto,
    Cuboid,
    Sphere,
    Plane,
}

impl NuPhysicsColliderKind {
    fn parse(value: &str) -> Result<Self, EngineError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "cuboid" => Ok(Self::Cuboid),
            "sphere" => Ok(Self::Sphere),
            "plane" => Ok(Self::Plane),
            other => Err(EngineError::InvalidScene {
                reason: format!("unsupported physics collider `{other}`"),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NuPhysicsSection {
    pub body: NuPhysicsBodyKind,
    pub collider: NuPhysicsColliderKind,
    pub mass: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NuMeshScriptSection {
    pub na_script: Option<PathBuf>,
    pub cpp_script: Option<PathBuf>,
    pub player_camera: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NuMeshSection {
    pub name: String,
    pub geometry: String,
    pub source: Option<PathBuf>,
    pub material: String,
    pub parent: Option<String>,
    pub transform: NuTransform,
    pub pivot_offset: [f32; 3],
    pub physics: Option<NuPhysicsSection>,
    pub script: Option<NuMeshScriptSection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NuMaterialSection {
    pub name: String,
    pub shader_vertex: PathBuf,
    pub shader_fragment: PathBuf,
    pub color: [f32; 3],
    pub roughness: f32,
    pub metallic: f32,
    pub albedo_texture: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NuEnvironmentSection {
    pub ambient_color: [f32; 3],
    pub ambient_intensity: f32,
    pub shadow_mode: ShadowMode,
    pub shadow_max_distance: f32,
    pub shadow_filter_radius: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NuSceneDocument {
    pub metadata: NuSceneMetadata,
    pub scene: NuSceneSection,
    pub camera: NuCameraSection,
    pub lights: BTreeMap<String, NuLightSection>,
    pub environment: Option<NuEnvironmentSection>,
    pub meshes: BTreeMap<String, NuMeshSection>,
    pub materials: BTreeMap<String, NuMaterialSection>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct NuResolvedTransform {
    position: [f32; 3],
    rotation_degrees: [f32; 3],
    scale: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NuScenePhysicsBinding {
    pub mesh_name: String,
    pub body_handle: BodyHandle,
}

#[derive(Debug)]
pub struct NuScenePhysicsRuntime {
    world: PhysicsWorld,
    bindings: Vec<NuScenePhysicsBinding>,
}

impl NuScenePhysicsRuntime {
    pub fn from_scene(scene: &NuSceneDocument) -> Result<Self, EngineError> {
        Self::from_scene_with_config(scene, PhysicsConfig::default())
    }

    pub fn from_scene_with_config(
        scene: &NuSceneDocument,
        config: PhysicsConfig,
    ) -> Result<Self, EngineError> {
        let mut world = PhysicsWorld::new(config);
        let mut bindings = Vec::new();

        for (mesh_name, mesh) in &scene.meshes {
            let Some(physics) = &mesh.physics else {
                continue;
            };
            let resolved = resolve_mesh_world_transform(scene, mesh_name, 0)?;
            let collider = collider_from_mesh(mesh_name, mesh, physics, resolved)?;
            let body = rigid_body_from_mesh(mesh, physics, resolved, collider);
            let body_handle = world.insert_body(body);
            bindings.push(NuScenePhysicsBinding {
                mesh_name: mesh_name.clone(),
                body_handle,
            });
        }

        Ok(Self { world, bindings })
    }

    pub fn world(&self) -> &PhysicsWorld {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut PhysicsWorld {
        &mut self.world
    }

    pub fn bindings(&self) -> &[NuScenePhysicsBinding] {
        &self.bindings
    }

    pub fn body_for_mesh(&self, mesh_name: &str) -> Option<(BodyHandle, &RigidBody)> {
        let binding = self
            .bindings
            .iter()
            .find(|binding| binding.mesh_name.eq_ignore_ascii_case(mesh_name))?;
        Some((binding.body_handle, self.world.body(binding.body_handle)?))
    }

    pub fn step(&mut self, delta_time_seconds: f32) {
        self.world.step(delta_time_seconds);
    }

    pub fn sync_to_scene(&self, scene: &mut NuSceneDocument) -> Result<(), EngineError> {
        let updates = self.collect_scene_updates(scene)?;
        for (mesh_name, position, rotation_degrees) in updates {
            let Some(mesh) = scene.meshes.get_mut(&mesh_name) else {
                continue;
            };
            mesh.transform.position = position;
            mesh.transform.rotation_degrees = rotation_degrees;
        }
        Ok(())
    }

    fn collect_scene_updates(
        &self,
        scene: &NuSceneDocument,
    ) -> Result<Vec<(String, [f32; 3], [f32; 3])>, EngineError> {
        let mut updates = Vec::with_capacity(self.bindings.len());
        for binding in &self.bindings {
            let Some(body) = self.world.body(binding.body_handle) else {
                continue;
            };
            let Some(mesh) = scene.meshes.get(&binding.mesh_name) else {
                continue;
            };
            let parent_transform = match &mesh.parent {
                Some(parent) => {
                    let Some(parent_name) = parent.strip_prefix("mesh.") else {
                        return Err(EngineError::InvalidScene {
                            reason: format!(
                                "mesh `{}` has invalid parent `{parent}`; expected `mesh.<name>`",
                                mesh.name
                            ),
                        });
                    };
                    Some(resolve_mesh_world_transform(scene, parent_name, 0)?)
                }
                None => None,
            };
            let local_position = parent_transform
                .map_or(body.position, |parent| sub3(body.position, parent.position));
            let local_rotation_degrees =
                parent_transform.map_or(radians3_to_degrees(body.rotation_radians), |parent| {
                    sub3(
                        radians3_to_degrees(body.rotation_radians),
                        parent.rotation_degrees,
                    )
                });
            updates.push((
                binding.mesh_name.clone(),
                local_position,
                local_rotation_degrees,
            ));
        }
        Ok(updates)
    }
}

pub fn build_scene_physics_runtime(
    scene: &NuSceneDocument,
) -> Result<NuScenePhysicsRuntime, EngineError> {
    NuScenePhysicsRuntime::from_scene(scene)
}

pub fn build_scene_physics_runtime_with_config(
    scene: &NuSceneDocument,
    config: PhysicsConfig,
) -> Result<NuScenePhysicsRuntime, EngineError> {
    NuScenePhysicsRuntime::from_scene_with_config(scene, config)
}

pub fn build_scene_world(scene: &NuSceneDocument) -> Result<NuSceneWorld, EngineError> {
    NuSceneWorld::from_document(scene)
}

pub fn register_scene_assets(
    scene: &NuSceneDocument,
    scene_path: impl AsRef<Path>,
    asset_manager: &mut AssetManager,
) -> SceneAssetBindings {
    let references = scene.asset_references(scene_path);
    register_scene_assets_from_references(&references, asset_manager)
}

impl NuSceneDocument {
    pub fn compile_world(&self) -> Result<NuSceneWorld, EngineError> {
        NuSceneWorld::from_document(self)
    }

    pub fn asset_references(&self, scene_path: impl AsRef<Path>) -> SceneAssetReferences {
        let scene_path = scene_path.as_ref();
        let base_dir = scene_path.parent().unwrap_or_else(|| Path::new("."));
        let mut shader_programs = BTreeMap::new();
        let mut textures = BTreeMap::new();
        let mut meshes = BTreeMap::new();

        for (material_name, material) in &self.materials {
            shader_programs.insert(
                material_name.clone(),
                ShaderProgramPaths {
                    material_name: material_name.clone(),
                    vertex: normalize_asset_path(base_dir.join(&material.shader_vertex)),
                    fragment: normalize_asset_path(base_dir.join(&material.shader_fragment)),
                },
            );
            if let Some(texture) = &material.albedo_texture {
                textures.insert(
                    material_name.clone(),
                    normalize_asset_path(base_dir.join(texture)),
                );
            }
        }
        for (mesh_name, mesh) in &self.meshes {
            if let Some(source) = &mesh.source {
                meshes.insert(
                    mesh_name.clone(),
                    normalize_asset_path(base_dir.join(source)),
                );
            }
        }

        SceneAssetReferences {
            scene_path: normalize_asset_path(scene_path.to_path_buf()),
            shader_programs,
            textures,
            meshes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShaderProgramPaths {
    pub material_name: String,
    pub vertex: PathBuf,
    pub fragment: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneAssetReferences {
    pub scene_path: PathBuf,
    pub shader_programs: BTreeMap<String, ShaderProgramPaths>,
    pub textures: BTreeMap<String, PathBuf>,
    pub meshes: BTreeMap<String, PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShaderProgramAssetHandles {
    pub vertex: AssetHandle,
    pub fragment: AssetHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneAssetBindings {
    pub scene: AssetHandle,
    pub shader_programs: BTreeMap<String, ShaderProgramAssetHandles>,
    pub textures: BTreeMap<String, AssetHandle>,
    pub meshes: BTreeMap<String, AssetHandle>,
}

impl SceneAssetBindings {
    fn unique_handles(&self) -> HashSet<AssetHandle> {
        let mut handles = HashSet::new();
        handles.insert(self.scene);
        for shader in self.shader_programs.values() {
            handles.insert(shader.vertex);
            handles.insert(shader.fragment);
        }
        for handle in self.textures.values() {
            handles.insert(*handle);
        }
        for handle in self.meshes.values() {
            handles.insert(*handle);
        }
        handles
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
}

#[derive(Debug, Clone)]
pub struct ReloadedShader {
    pub material_name: String,
    pub path: PathBuf,
    pub stage: ShaderStage,
    pub spirv_words: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct ReloadedTexture {
    pub material_name: String,
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ReloadBatch {
    pub changed_paths: Vec<PathBuf>,
    pub scene: Option<NuSceneDocument>,
    pub shaders: Vec<ReloadedShader>,
    pub textures: Vec<ReloadedTexture>,
}

pub struct HotReloadManager {
    scene_path: PathBuf,
    scene: NuSceneDocument,
    assets: SceneAssetReferences,
    asset_manager: AssetManager,
    asset_bindings: SceneAssetBindings,
    watched_paths: HashSet<PathBuf>,
    watcher: RecommendedWatcher,
    events: Receiver<Result<Event, notify::Error>>,
}

impl HotReloadManager {
    pub fn open(scene_path: impl AsRef<Path>) -> Result<Self, EngineError> {
        let scene_path = normalize_asset_path(scene_path.as_ref().to_path_buf());
        let scene = load_scene_file(&scene_path)?;
        let assets = scene.asset_references(&scene_path);
        let mut asset_manager = AssetManager::default();
        let asset_bindings = register_scene_assets_from_references(&assets, &mut asset_manager);
        let (tx, rx) = mpsc::channel();
        let watcher = notify::recommended_watcher(move |event| {
            let _ = tx.send(event);
        })?;
        let mut manager = Self {
            scene_path,
            scene,
            assets,
            asset_manager,
            asset_bindings,
            watched_paths: HashSet::new(),
            watcher,
            events: rx,
        };
        manager.sync_watched_paths()?;
        Ok(manager)
    }

    pub fn scene(&self) -> &NuSceneDocument {
        &self.scene
    }

    pub fn asset_references(&self) -> &SceneAssetReferences {
        &self.assets
    }

    pub fn asset_manager(&self) -> &AssetManager {
        &self.asset_manager
    }

    pub fn asset_bindings(&self) -> &SceneAssetBindings {
        &self.asset_bindings
    }

    pub fn reload_now(&mut self) -> Result<ReloadBatch, EngineError> {
        let scene = load_scene_file(&self.scene_path)?;
        let references = scene.asset_references(&self.scene_path);
        self.asset_bindings = sync_scene_assets(
            &mut self.asset_manager,
            Some(&self.asset_bindings),
            &references,
        );
        self.assets = references;
        self.scene = scene.clone();
        self.sync_watched_paths()?;

        let mut changed_paths = vec![self.assets.scene_path.clone()];
        let mut shaders = Vec::new();
        let mut textures = Vec::new();
        for program in self.assets.shader_programs.values() {
            changed_paths.push(program.vertex.clone());
            changed_paths.push(program.fragment.clone());
            shaders.push(compile_shader_file(
                &program.material_name,
                &program.vertex,
            )?);
            shaders.push(compile_shader_file(
                &program.material_name,
                &program.fragment,
            )?);
        }
        for (material_name, texture_path) in &self.assets.textures {
            changed_paths.push(texture_path.clone());
            textures.push(ReloadedTexture {
                material_name: material_name.clone(),
                path: texture_path.clone(),
                bytes: fs::read(texture_path).map_err(|err| EngineError::Io {
                    path: Some(texture_path.clone()),
                    reason: err.to_string(),
                })?,
            });
        }

        changed_paths.sort();
        changed_paths.dedup();

        Ok(ReloadBatch {
            changed_paths,
            scene: Some(scene),
            shaders,
            textures,
        })
    }

    pub fn reload_now_with_events(
        &mut self,
        event_bus: &mut EventBus,
        delivery: EventDeliveryMode,
    ) -> Result<ReloadBatch, EngineError> {
        let batch = self.reload_now()?;
        publish_reload_batch_events(&batch, event_bus, delivery);
        Ok(batch)
    }

    pub fn poll_changes(&mut self) -> Result<Option<ReloadBatch>, EngineError> {
        let mut changed = BTreeSet::new();
        while let Ok(event) = self.events.try_recv() {
            let event = event?;
            for path in event.paths {
                changed.insert(normalize_asset_path(path));
            }
        }

        if changed.is_empty() {
            return Ok(None);
        }

        let changed_paths: Vec<PathBuf> = changed.into_iter().collect();
        let scene_changed = changed_paths
            .iter()
            .any(|path| *path == self.assets.scene_path);

        let mut updated_scene = None;
        if scene_changed {
            let scene = load_scene_file(&self.scene_path)?;
            let references = scene.asset_references(&self.scene_path);
            self.asset_bindings = sync_scene_assets(
                &mut self.asset_manager,
                Some(&self.asset_bindings),
                &references,
            );
            self.assets = references;
            self.scene = scene.clone();
            self.sync_watched_paths()?;
            updated_scene = Some(scene);
        }

        let mut shaders = Vec::new();
        let mut textures = Vec::new();
        let referenced_paths = self.assets.clone();
        for program in referenced_paths.shader_programs.values() {
            let should_reload = scene_changed
                || changed_paths
                    .iter()
                    .any(|path| *path == program.vertex || *path == program.fragment);
            if should_reload {
                shaders.push(compile_shader_file(
                    &program.material_name,
                    &program.vertex,
                )?);
                shaders.push(compile_shader_file(
                    &program.material_name,
                    &program.fragment,
                )?);
            }
        }
        for (material_name, texture_path) in &referenced_paths.textures {
            let should_reload =
                scene_changed || changed_paths.iter().any(|path| *path == *texture_path);
            if should_reload {
                textures.push(ReloadedTexture {
                    material_name: material_name.clone(),
                    path: texture_path.clone(),
                    bytes: fs::read(texture_path).map_err(|err| EngineError::Io {
                        path: Some(texture_path.clone()),
                        reason: err.to_string(),
                    })?,
                });
            }
        }

        Ok(Some(ReloadBatch {
            changed_paths,
            scene: updated_scene,
            shaders,
            textures,
        }))
    }

    pub fn poll_changes_with_events(
        &mut self,
        event_bus: &mut EventBus,
        delivery: EventDeliveryMode,
    ) -> Result<Option<ReloadBatch>, EngineError> {
        let batch = self.poll_changes()?;
        if let Some(batch) = &batch {
            publish_reload_batch_events(batch, event_bus, delivery);
        }
        Ok(batch)
    }

    fn sync_watched_paths(&mut self) -> Result<(), EngineError> {
        let mut desired = HashSet::new();
        desired.insert(self.assets.scene_path.clone());
        for program in self.assets.shader_programs.values() {
            desired.insert(program.vertex.clone());
            desired.insert(program.fragment.clone());
        }
        for texture in self.assets.textures.values() {
            desired.insert(texture.clone());
        }

        let current = self.watched_paths.clone();
        let to_remove: Vec<PathBuf> = current.difference(&desired).cloned().collect();
        let to_add: Vec<PathBuf> = desired.difference(&current).cloned().collect();
        for path in to_remove {
            let _ = self.watcher.unwatch(&path);
            self.watched_paths.remove(&path);
        }
        for path in to_add {
            if path.exists() {
                self.watcher.watch(&path, RecursiveMode::NonRecursive)?;
                self.watched_paths.insert(path);
            }
        }
        Ok(())
    }
}

pub fn publish_reload_batch_events(
    batch: &ReloadBatch,
    event_bus: &mut EventBus,
    delivery: EventDeliveryMode,
) {
    if let Some(scene) = &batch.scene {
        event_bus.publish(
            EngineEvent::SceneLoaded {
                scene_name: scene.scene.name.clone(),
            },
            delivery,
        );
        event_bus.publish(
            EngineEvent::ResourceReloaded {
                kind: ResourceEventKind::Scene,
                resource_id: scene.scene.name.clone(),
            },
            delivery,
        );
    }
    for shader in &batch.shaders {
        event_bus.publish(
            EngineEvent::ResourceReloaded {
                kind: ResourceEventKind::Shader,
                resource_id: shader.path.display().to_string(),
            },
            delivery,
        );
    }
    for texture in &batch.textures {
        event_bus.publish(
            EngineEvent::ResourceReloaded {
                kind: ResourceEventKind::Texture,
                resource_id: texture.path.display().to_string(),
            },
            delivery,
        );
    }
}

pub fn load_scene_file(path: impl AsRef<Path>) -> Result<NuSceneDocument, EngineError> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|err| EngineError::Io {
        path: Some(path.to_path_buf()),
        reason: err.to_string(),
    })?;
    parse_scene_str(&source)
}

pub fn parse_scene_str(source: &str) -> Result<NuSceneDocument, EngineError> {
    let metadata = parse_metadata(source);
    let sections = parse_sections(source)?;
    let metadata = if let Some(meta_table) = sections.get("meta") {
        NuSceneMetadata {
            format_version: metadata.format_version,
            engine_name: optional_string(meta_table, "engine")
                .unwrap_or_else(|| metadata.engine_name.clone()),
            format_name: optional_string(meta_table, "format")
                .unwrap_or_else(|| metadata.format_name.clone()),
            extension: optional_string(meta_table, "extension")
                .unwrap_or_else(|| metadata.extension.clone()),
        }
    } else {
        metadata
    };

    let scene_table = sections
        .get("scene")
        .ok_or_else(|| EngineError::InvalidScene {
            reason: "missing [scene] section".into(),
        })?;
    let scene = NuSceneSection {
        name: required_string(scene_table, "name")?,
        syntax: SceneSyntax::parse(&required_string_either(scene_table, "syntax", "backend")?)?,
    };

    let camera_table = sections
        .get("camera")
        .ok_or_else(|| EngineError::InvalidScene {
            reason: "missing [camera] section".into(),
        })?;
    let camera = NuCameraSection {
        position: required_vec3(camera_table, "position")?,
        target: required_vec3(camera_table, "target")?,
        fov_degrees: required_number(camera_table, "fov")?,
    };

    let mut lights = BTreeMap::new();
    for (section_name, table) in &sections {
        if section_name == "light" || section_name.starts_with("light.") {
            let name = section_name
                .split_once('.')
                .map(|(_, name)| name.to_string())
                .unwrap_or_else(|| "default".to_string());
            lights.insert(
                name.clone(),
                NuLightSection {
                    name,
                    kind: LightKind::parse(&required_string(table, "type")?)?,
                    position: required_vec3(table, "position")?,
                    color: required_vec3(table, "color")?,
                    intensity: required_number(table, "intensity")?,
                    casts_shadow: optional_bool(table, "casts_shadow").unwrap_or(true),
                },
            );
        }
    }
    if lights.is_empty() {
        return Err(EngineError::InvalidScene {
            reason: "at least one [light] section is required".into(),
        });
    }

    let mut meshes = BTreeMap::new();
    for (section_name, table) in &sections {
        if let Some((prefix, name)) = section_name.split_once('.') {
            if prefix != "mesh" {
                continue;
            }
            meshes.insert(
                name.to_string(),
                NuMeshSection {
                    name: name.to_string(),
                    geometry: required_string(table, "geometry")?,
                    source: optional_string(table, "source").map(PathBuf::from),
                    material: required_string(table, "material")?,
                    parent: optional_string(table, "parent"),
                    transform: NuTransform {
                        position: optional_vec3(table, "transform.position")
                            .unwrap_or([0.0, 0.0, 0.0]),
                        rotation_degrees: parse_rotation_degrees(table),
                        scale: optional_vec3(table, "transform.scale").unwrap_or([1.0, 1.0, 1.0]),
                    },
                    pivot_offset: optional_vec3(table, "pivot.offset").unwrap_or([0.0, 0.0, 0.0]),
                    physics: parse_optional_physics(table)?,
                    script: parse_optional_script(table),
                },
            );
        }
    }

    let mut materials = BTreeMap::new();
    for (section_name, table) in &sections {
        if let Some((prefix, name)) = section_name.split_once('.') {
            if prefix != "material" {
                continue;
            }
            let shader_entries = optional_string_list(table, "shader").unwrap_or_default();
            let (shader_vertex, shader_fragment) =
                material_shader_paths(table, name, shader_entries)?;
            materials.insert(
                name.to_string(),
                NuMaterialSection {
                    name: name.to_string(),
                    shader_vertex,
                    shader_fragment,
                    color: optional_vec3(table, "color").unwrap_or([1.0, 1.0, 1.0]),
                    roughness: optional_number(table, "roughness").unwrap_or(0.5),
                    metallic: optional_number(table, "metallic").unwrap_or(0.0),
                    albedo_texture: optional_string(table, "albedo_texture").map(PathBuf::from),
                },
            );
        }
    }

    let environment = if let Some(table) = sections.get("environment") {
        Some(NuEnvironmentSection {
            ambient_color: optional_vec3(table, "ambient_color").unwrap_or([0.05, 0.05, 0.07]),
            ambient_intensity: optional_number(table, "ambient_intensity").unwrap_or(1.0),
            shadow_mode: optional_string(table, "shadow_mode")
                .as_deref()
                .map(parse_shadow_mode)
                .transpose()?
                .unwrap_or(ShadowMode::Live),
            shadow_max_distance: optional_number(table, "shadow_max_distance").unwrap_or(32.0),
            shadow_filter_radius: optional_number(table, "shadow_filter_radius").unwrap_or(1.5),
        })
    } else {
        None
    };

    validate_mesh_sections(&meshes)?;

    Ok(NuSceneDocument {
        metadata,
        scene,
        camera,
        lights,
        environment,
        meshes,
        materials,
    })
}

pub fn load_obj_mesh_asset(path: impl AsRef<Path>) -> Result<Arc<MeshAsset3D>, EngineError> {
    let path = path.as_ref();
    let (models, _) = tobj::load_obj(
        path,
        &LoadOptions {
            triangulate: true,
            single_index: true,
            ..LoadOptions::default()
        },
    )
    .map_err(|err| EngineError::MeshLoad {
        path: path.to_path_buf(),
        reason: err.to_string(),
    })?;

    let mut positions = Vec::<[f32; 3]>::new();
    let mut normals = Vec::<[f32; 3]>::new();
    let mut uvs = Vec::<[f32; 2]>::new();
    let mut bounds_min = [f32::INFINITY; 3];
    let mut bounds_max = [f32::NEG_INFINITY; 3];

    for model in &models {
        let mesh = &model.mesh;
        if mesh.indices.len() % 3 != 0 {
            return Err(EngineError::MeshLoad {
                path: path.to_path_buf(),
                reason: format!("mesh `{}` is not triangulated after load", model.name),
            });
        }

        for triangle in mesh.indices.chunks_exact(3) {
            let p = [
                read_position(mesh, triangle[0] as usize),
                read_position(mesh, triangle[1] as usize),
                read_position(mesh, triangle[2] as usize),
            ];
            let face_normal = normalize_face_normal(p[0], p[1], p[2]);
            for &index in triangle {
                let vertex_index = index as usize;
                let position = read_position(mesh, vertex_index);
                let normal = if mesh.normals.len() >= ((vertex_index + 1) * 3) {
                    normalize3([
                        mesh.normals[vertex_index * 3],
                        mesh.normals[vertex_index * 3 + 1],
                        mesh.normals[vertex_index * 3 + 2],
                    ])
                } else {
                    face_normal
                };
                let uv = if mesh.texcoords.len() >= ((vertex_index + 1) * 2) {
                    [
                        mesh.texcoords[vertex_index * 2],
                        1.0 - mesh.texcoords[vertex_index * 2 + 1],
                    ]
                } else {
                    [0.0, 0.0]
                };
                bounds_min[0] = bounds_min[0].min(position[0]);
                bounds_min[1] = bounds_min[1].min(position[1]);
                bounds_min[2] = bounds_min[2].min(position[2]);
                bounds_max[0] = bounds_max[0].max(position[0]);
                bounds_max[1] = bounds_max[1].max(position[1]);
                bounds_max[2] = bounds_max[2].max(position[2]);
                positions.push(position);
                normals.push(normal);
                uvs.push(uv);
            }
        }
    }

    if positions.is_empty() {
        return Err(EngineError::MeshLoad {
            path: path.to_path_buf(),
            reason: "obj contained no triangle vertices".into(),
        });
    }

    let center = [
        (bounds_min[0] + bounds_max[0]) * 0.5,
        (bounds_min[1] + bounds_max[1]) * 0.5,
        (bounds_min[2] + bounds_max[2]) * 0.5,
    ];
    let base_size = [
        (bounds_max[0] - bounds_min[0]).max(0.0001),
        (bounds_max[1] - bounds_min[1]).max(0.0001),
        (bounds_max[2] - bounds_min[2]).max(0.0001),
    ];
    let half = [base_size[0] * 0.5, base_size[1] * 0.5, base_size[2] * 0.5];
    let vertices = positions
        .into_iter()
        .zip(normals)
        .zip(uvs)
        .map(|((position, normal), uv)| MeshVertex3D {
            position: [
                (position[0] - center[0]) / half[0],
                (position[1] - center[1]) / half[1],
                (position[2] - center[2]) / half[2],
            ],
            normal,
            uv,
        })
        .collect::<Vec<_>>();

    Ok(Arc::new(MeshAsset3D {
        name: path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("obj_mesh")
            .to_string(),
        vertices: Arc::<[MeshVertex3D]>::from(vertices),
        base_size,
    }))
}

fn compile_shader_file(material_name: &str, path: &Path) -> Result<ReloadedShader, EngineError> {
    let source = fs::read_to_string(path).map_err(|err| EngineError::Io {
        path: Some(path.to_path_buf()),
        reason: err.to_string(),
    })?;
    let stage = shader_stage_from_path(path)?;
    let kind = match stage {
        ShaderStage::Vertex => ShaderKind::Vertex,
        ShaderStage::Fragment => ShaderKind::Fragment,
        ShaderStage::Compute => ShaderKind::Compute,
    };
    let compiler = Compiler::new().map_err(|_| EngineError::ShaderCompile {
        path: path.to_path_buf(),
        reason: "failed to construct shader compiler".into(),
    })?;
    let mut options = CompileOptions::new().map_err(|_| EngineError::ShaderCompile {
        path: path.to_path_buf(),
        reason: "failed to construct shader compile options".into(),
    })?;
    options.set_target_env(TargetEnv::Vulkan, EnvVersion::Vulkan1_0 as u32);
    options.set_optimization_level(OptimizationLevel::Performance);
    let artifact = compiler
        .compile_into_spirv(
            &source,
            kind,
            &path.to_string_lossy(),
            "main",
            Some(&options),
        )
        .map_err(|err| EngineError::ShaderCompile {
            path: path.to_path_buf(),
            reason: err.to_string(),
        })?;
    Ok(ReloadedShader {
        material_name: material_name.to_string(),
        path: path.to_path_buf(),
        stage,
        spirv_words: artifact.as_binary().to_vec(),
    })
}

fn shader_stage_from_path(path: &Path) -> Result<ShaderStage, EngineError> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| EngineError::InvalidScene {
            reason: format!("shader path `{}` has no valid file name", path.display()),
        })?
        .to_ascii_lowercase();
    if name.contains(".vert") {
        Ok(ShaderStage::Vertex)
    } else if name.contains(".frag") {
        Ok(ShaderStage::Fragment)
    } else if name.contains(".comp") {
        Ok(ShaderStage::Compute)
    } else {
        Err(EngineError::InvalidScene {
            reason: format!(
                "shader `{}` must contain .vert, .frag, or .comp in the file name",
                path.display()
            ),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
enum SceneValue {
    String(String),
    Number(f32),
    List(Vec<SceneValue>),
}

type SectionMap = BTreeMap<String, BTreeMap<String, SceneValue>>;

fn parse_metadata(source: &str) -> NuSceneMetadata {
    let mut metadata = NuSceneMetadata::default();
    for line in source.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('#') {
            continue;
        }
        if trimmed.eq_ignore_ascii_case(NU_SCENE_FORMAT_HEADER) {
            metadata.format_version = 1;
        } else if trimmed.eq_ignore_ascii_case("# .nuscene") {
            metadata.extension = NU_SCENE_EXTENSION.to_string();
        }
    }
    if let Ok(sections) = parse_sections(source) {
        if let Some(meta) = sections.get("meta") {
            if let Some(engine) = optional_string(meta, "engine") {
                metadata.engine_name = engine;
            }
            if let Some(format_name) = optional_string(meta, "format") {
                metadata.format_name = format_name;
            }
            if let Some(extension) = optional_string(meta, "extension") {
                metadata.extension = extension.trim_start_matches('.').to_string();
            }
        }
    }
    metadata
}

fn parse_sections(source: &str) -> Result<SectionMap, EngineError> {
    let mut current_section: Option<String> = None;
    let mut sections: SectionMap = BTreeMap::new();

    for (index, raw_line) in source.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            if !line.ends_with(']') {
                return Err(EngineError::Parse {
                    line: line_number,
                    reason: "section header must end with `]`".into(),
                });
            }
            let section_name = line[1..line.len() - 1].trim();
            if section_name.is_empty() {
                return Err(EngineError::Parse {
                    line: line_number,
                    reason: "section name cannot be empty".into(),
                });
            }
            if sections.contains_key(section_name) {
                return Err(EngineError::Parse {
                    line: line_number,
                    reason: format!("duplicate section header `[{}]`", section_name),
                });
            }
            current_section = Some(section_name.to_string());
            sections.entry(section_name.to_string()).or_default();
            continue;
        }

        let Some(section_name) = current_section.clone() else {
            return Err(EngineError::Parse {
                line: line_number,
                reason: "key/value entry found before any section header".into(),
            });
        };
        let Some((key, value)) = line.split_once('=') else {
            return Err(EngineError::Parse {
                line: line_number,
                reason: "expected `key = value`".into(),
            });
        };
        let key = key.trim();
        if key.is_empty() {
            return Err(EngineError::Parse {
                line: line_number,
                reason: "key cannot be empty".into(),
            });
        }
        let value = parse_value(value.trim());
        let table = sections.entry(section_name.clone()).or_default();
        if table.contains_key(key) {
            return Err(EngineError::Parse {
                line: line_number,
                reason: format!("duplicate key `{key}` in section `[{}]`", section_name),
            });
        }
        table.insert(key.to_string(), value);
    }

    Ok(sections)
}

fn parse_value(value: &str) -> SceneValue {
    if value.contains(',') {
        return SceneValue::List(
            value
                .split(',')
                .map(|entry| parse_scalar(entry.trim()))
                .collect(),
        );
    }
    parse_scalar(value)
}

fn parse_scalar(value: &str) -> SceneValue {
    let trimmed = value.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        return SceneValue::String(trimmed[1..trimmed.len() - 1].to_string());
    }
    if let Ok(number) = trimmed.parse::<f32>() {
        return SceneValue::Number(number);
    }
    SceneValue::String(trimmed.to_string())
}

fn required_string(table: &BTreeMap<String, SceneValue>, key: &str) -> Result<String, EngineError> {
    optional_string(table, key).ok_or_else(|| EngineError::InvalidScene {
        reason: format!("missing string field `{key}`"),
    })
}

fn required_string_either(
    table: &BTreeMap<String, SceneValue>,
    primary_key: &str,
    fallback_key: &str,
) -> Result<String, EngineError> {
    optional_string(table, primary_key)
        .or_else(|| optional_string(table, fallback_key))
        .ok_or_else(|| EngineError::InvalidScene {
            reason: format!(
                "missing string field `{primary_key}` (legacy `{fallback_key}` also accepted)"
            ),
        })
}

fn optional_string(table: &BTreeMap<String, SceneValue>, key: &str) -> Option<String> {
    match table.get(key) {
        Some(SceneValue::String(value)) => Some(value.clone()),
        _ => None,
    }
}

fn required_number(table: &BTreeMap<String, SceneValue>, key: &str) -> Result<f32, EngineError> {
    optional_number(table, key).ok_or_else(|| EngineError::InvalidScene {
        reason: format!("missing numeric field `{key}`"),
    })
}

fn optional_number(table: &BTreeMap<String, SceneValue>, key: &str) -> Option<f32> {
    match table.get(key) {
        Some(SceneValue::Number(value)) => Some(*value),
        _ => None,
    }
}

fn required_vec3(table: &BTreeMap<String, SceneValue>, key: &str) -> Result<[f32; 3], EngineError> {
    optional_vec3(table, key).ok_or_else(|| EngineError::InvalidScene {
        reason: format!("missing vec3 field `{key}`"),
    })
}

fn optional_vec3(table: &BTreeMap<String, SceneValue>, key: &str) -> Option<[f32; 3]> {
    match table.get(key) {
        Some(SceneValue::List(values)) if values.len() == 3 => Some([
            scalar_as_number(&values[0])?,
            scalar_as_number(&values[1])?,
            scalar_as_number(&values[2])?,
        ]),
        _ => None,
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    }
}

fn optional_bool(table: &BTreeMap<String, SceneValue>, key: &str) -> Option<bool> {
    optional_string(table, key).and_then(|value| parse_bool(&value))
}

fn parse_rotation_degrees(table: &BTreeMap<String, SceneValue>) -> [f32; 3] {
    if let Some(value) = optional_vec3(table, "transform.rotation_degrees") {
        return value;
    }
    if let Some(value) = optional_vec3(table, "transform.rotation_radians") {
        return [
            value[0].to_degrees(),
            value[1].to_degrees(),
            value[2].to_degrees(),
        ];
    }
    optional_vec3(table, "transform.rotation").unwrap_or([0.0, 0.0, 0.0])
}

fn parse_optional_physics(
    table: &BTreeMap<String, SceneValue>,
) -> Result<Option<NuPhysicsSection>, EngineError> {
    let body = optional_string(table, "physics.body");
    let collider = optional_string(table, "physics.collider");
    let mass = optional_number(table, "physics.mass");
    if body.is_none() && collider.is_none() && mass.is_none() {
        return Ok(None);
    }
    Ok(Some(NuPhysicsSection {
        body: body
            .as_deref()
            .map(NuPhysicsBodyKind::parse)
            .transpose()?
            .unwrap_or(NuPhysicsBodyKind::Static),
        collider: collider
            .as_deref()
            .map(NuPhysicsColliderKind::parse)
            .transpose()?
            .unwrap_or(NuPhysicsColliderKind::Auto),
        mass: mass.unwrap_or(1.0).max(0.0001),
    }))
}

fn parse_optional_script(table: &BTreeMap<String, SceneValue>) -> Option<NuMeshScriptSection> {
    let na_script = optional_string(table, "script.na").map(PathBuf::from);
    let cpp_script = optional_string(table, "script.cpp").map(PathBuf::from);
    let player_camera = optional_bool(table, "script.player_camera").unwrap_or(false);
    if na_script.is_none() && cpp_script.is_none() && !player_camera {
        None
    } else {
        Some(NuMeshScriptSection {
            na_script,
            cpp_script,
            player_camera,
        })
    }
}

fn parse_shadow_mode(value: &str) -> Result<ShadowMode, EngineError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "off" => Ok(ShadowMode::Off),
        "live" => Ok(ShadowMode::Live),
        other => Err(EngineError::InvalidScene {
            reason: format!("unsupported shadow mode `{other}`"),
        }),
    }
}

fn optional_string_list(table: &BTreeMap<String, SceneValue>, key: &str) -> Option<Vec<String>> {
    match table.get(key) {
        Some(SceneValue::List(values)) => values.iter().map(scalar_as_string).collect(),
        Some(SceneValue::String(value)) => Some(vec![value.clone()]),
        _ => None,
    }
}

fn material_shader_paths(
    table: &BTreeMap<String, SceneValue>,
    material_name: &str,
    shader_entries: Vec<String>,
) -> Result<(PathBuf, PathBuf), EngineError> {
    let explicit_vertex = optional_string(table, "shader.vertex").map(PathBuf::from);
    let explicit_fragment = optional_string(table, "shader.fragment").map(PathBuf::from);
    match (explicit_vertex, explicit_fragment) {
        (Some(vertex), Some(fragment)) => Ok((vertex, fragment)),
        (None, None) => {
            if shader_entries.len() != 2 {
                return Err(EngineError::InvalidScene {
                    reason: format!(
                        "[material.{material_name}] shader must contain exactly two entries or explicit shader.vertex/shader.fragment keys"
                    ),
                });
            }
            Ok((
                PathBuf::from(&shader_entries[0]),
                PathBuf::from(&shader_entries[1]),
            ))
        }
        _ => Err(EngineError::InvalidScene {
            reason: format!(
                "[material.{material_name}] shader.vertex and shader.fragment must both be set"
            ),
        }),
    }
}

fn read_position(mesh: &tobj::Mesh, index: usize) -> [f32; 3] {
    [
        mesh.positions[index * 3],
        mesh.positions[index * 3 + 1],
        mesh.positions[index * 3 + 2],
    ]
}

fn normalize_face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    normalize3([
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ])
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if length <= 0.0001 {
        [0.0, 1.0, 0.0]
    } else {
        [v[0] / length, v[1] / length, v[2] / length]
    }
}

fn scalar_as_number(value: &SceneValue) -> Option<f32> {
    match value {
        SceneValue::Number(value) => Some(*value),
        _ => None,
    }
}

fn scalar_as_string(value: &SceneValue) -> Option<String> {
    match value {
        SceneValue::String(value) => Some(value.clone()),
        _ => None,
    }
}

fn normalize_asset_path(path: PathBuf) -> PathBuf {
    fs::canonicalize(&path).unwrap_or(path)
}

fn resolve_mesh_world_transform(
    scene: &NuSceneDocument,
    mesh_name: &str,
    depth: usize,
) -> Result<NuResolvedTransform, EngineError> {
    if depth >= 32 {
        return Err(EngineError::InvalidScene {
            reason: format!("mesh `{mesh_name}` parent chain is too deep"),
        });
    }
    let mesh = scene
        .meshes
        .get(mesh_name)
        .ok_or_else(|| EngineError::InvalidScene {
            reason: format!("mesh `{mesh_name}` is missing from scene"),
        })?;
    let mut resolved = NuResolvedTransform {
        position: mesh.transform.position,
        rotation_degrees: mesh.transform.rotation_degrees,
        scale: mesh.transform.scale,
    };
    if let Some(parent) = &mesh.parent {
        let Some(parent_name) = parent.strip_prefix("mesh.") else {
            return Err(EngineError::InvalidScene {
                reason: format!(
                    "mesh `{mesh_name}` has invalid parent `{parent}`; expected `mesh.<name>`"
                ),
            });
        };
        let parent = resolve_mesh_world_transform(scene, parent_name, depth + 1)?;
        resolved.position = add3(parent.position, resolved.position);
        resolved.rotation_degrees = add3(parent.rotation_degrees, resolved.rotation_degrees);
        resolved.scale = mul3(parent.scale, resolved.scale);
    }
    Ok(resolved)
}

fn collider_from_mesh(
    _mesh_name: &str,
    mesh: &NuMeshSection,
    physics: &NuPhysicsSection,
    resolved: NuResolvedTransform,
) -> Result<ColliderShape, EngineError> {
    let geometry = mesh.geometry.trim().to_ascii_lowercase();
    let collider_kind = match physics.collider {
        NuPhysicsColliderKind::Auto => match geometry.as_str() {
            "sphere" => NuPhysicsColliderKind::Sphere,
            "plane" => NuPhysicsColliderKind::Plane,
            _ => NuPhysicsColliderKind::Cuboid,
        },
        kind => kind,
    };
    match collider_kind {
        NuPhysicsColliderKind::Auto => unreachable!(),
        NuPhysicsColliderKind::Cuboid => Ok(ColliderShape::Cuboid {
            half_extents: [
                resolved.scale[0].abs() * 0.5,
                resolved.scale[1].abs() * 0.5,
                resolved.scale[2].abs() * 0.5,
            ],
        }),
        NuPhysicsColliderKind::Sphere => {
            let radius = resolved.scale[0]
                .abs()
                .max(resolved.scale[1].abs())
                .max(resolved.scale[2].abs())
                * 0.5;
            Ok(ColliderShape::Sphere { radius })
        }
        NuPhysicsColliderKind::Plane => Ok(ColliderShape::Plane {
            normal: [0.0, 1.0, 0.0],
            offset: resolved.position[1],
        }),
    }
}

fn rigid_body_from_mesh(
    mesh: &NuMeshSection,
    physics: &NuPhysicsSection,
    resolved: NuResolvedTransform,
    collider: ColliderShape,
) -> RigidBody {
    let rotation_radians = degrees3_to_radians(resolved.rotation_degrees);
    let mut body = match physics.body {
        NuPhysicsBodyKind::Static => RigidBody::static_body(resolved.position, collider),
        NuPhysicsBodyKind::Dynamic => {
            RigidBody::dynamic_body(resolved.position, physics.mass.max(0.0001), collider)
        }
        NuPhysicsBodyKind::Kinematic => RigidBody::kinematic_body(resolved.position, collider),
    };
    body.rotation_radians = rotation_radians;
    if mesh.geometry.eq_ignore_ascii_case("plane") {
        body.angular_velocity = [0.0, 0.0, 0.0];
    }
    body
}

fn degrees3_to_radians(value: [f32; 3]) -> [f32; 3] {
    [
        value[0].to_radians(),
        value[1].to_radians(),
        value[2].to_radians(),
    ]
}

fn radians3_to_degrees(value: [f32; 3]) -> [f32; 3] {
    [
        value[0].to_degrees(),
        value[1].to_degrees(),
        value[2].to_degrees(),
    ]
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn mul3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}

fn validate_mesh_sections(meshes: &BTreeMap<String, NuMeshSection>) -> Result<(), EngineError> {
    for (name, mesh) in meshes {
        validate_geometry_identifier(name, &mesh.geometry)?;
        if mesh.geometry.eq_ignore_ascii_case("obj") && mesh.source.is_none() {
            return Err(EngineError::InvalidScene {
                reason: format!("mesh `{name}` uses geometry `obj` but has no `source` path"),
            });
        }
        if let Some(physics) = &mesh.physics {
            validate_mesh_physics(name, mesh, physics)?;
        }
        if let Some(parent) = &mesh.parent {
            let Some(parent_name) = parent.strip_prefix("mesh.") else {
                return Err(EngineError::InvalidScene {
                    reason: format!(
                        "mesh `{name}` has invalid parent `{parent}`; expected `mesh.<name>`"
                    ),
                });
            };
            if parent_name == name {
                return Err(EngineError::InvalidScene {
                    reason: format!("mesh `{name}` cannot parent itself"),
                });
            }
            if !meshes.contains_key(parent_name) {
                return Err(EngineError::InvalidScene {
                    reason: format!("mesh `{name}` references missing parent `{parent}`"),
                });
            }
        }
    }
    Ok(())
}

fn validate_mesh_physics(
    mesh_name: &str,
    mesh: &NuMeshSection,
    physics: &NuPhysicsSection,
) -> Result<(), EngineError> {
    if matches!(physics.collider, NuPhysicsColliderKind::Plane)
        && !mesh.geometry.eq_ignore_ascii_case("plane")
    {
        return Err(EngineError::InvalidScene {
            reason: format!(
                "mesh `{mesh_name}` uses physics collider `plane` but geometry is not `plane`"
            ),
        });
    }
    if matches!(physics.body, NuPhysicsBodyKind::Dynamic) && physics.mass <= 0.0 {
        return Err(EngineError::InvalidScene {
            reason: format!("mesh `{mesh_name}` uses dynamic physics but mass is not positive"),
        });
    }
    Ok(())
}

fn validate_geometry_identifier(mesh_name: &str, geometry: &str) -> Result<(), EngineError> {
    match geometry.trim().to_ascii_lowercase().as_str() {
        "cube" | "plane" | "sphere" => Ok(()),
        "obj" => Ok(()),
        other => Err(EngineError::InvalidScene {
            reason: format!(
                "mesh `{mesh_name}` uses unsupported geometry `{other}`; supported geometry: cube, plane, sphere, obj"
            ),
        }),
    }
}

fn register_scene_assets_from_references(
    references: &SceneAssetReferences,
    asset_manager: &mut AssetManager,
) -> SceneAssetBindings {
    let scene = register_asset_path(
        asset_manager,
        AssetKind::Scene,
        &references.scene_path,
        AssetState::Loaded,
    );
    let shader_programs = references
        .shader_programs
        .iter()
        .map(|(material_name, paths)| {
            let vertex = register_asset_path(
                asset_manager,
                AssetKind::Shader,
                &paths.vertex,
                AssetState::Loaded,
            );
            let fragment = register_asset_path(
                asset_manager,
                AssetKind::Shader,
                &paths.fragment,
                AssetState::Loaded,
            );
            (
                material_name.clone(),
                ShaderProgramAssetHandles { vertex, fragment },
            )
        })
        .collect();
    let textures = references
        .textures
        .iter()
        .map(|(material_name, path)| {
            (
                material_name.clone(),
                register_asset_path(asset_manager, AssetKind::Texture, path, AssetState::Loaded),
            )
        })
        .collect();
    let meshes = references
        .meshes
        .iter()
        .map(|(mesh_name, path)| {
            (
                mesh_name.clone(),
                register_asset_path(asset_manager, AssetKind::Mesh, path, AssetState::Loaded),
            )
        })
        .collect();

    SceneAssetBindings {
        scene,
        shader_programs,
        textures,
        meshes,
    }
}

fn sync_scene_assets(
    asset_manager: &mut AssetManager,
    previous: Option<&SceneAssetBindings>,
    next_references: &SceneAssetReferences,
) -> SceneAssetBindings {
    let next_bindings = register_scene_assets_from_references(next_references, asset_manager);
    if let Some(previous) = previous {
        for handle in previous.unique_handles() {
            let _ = asset_manager.release(handle);
        }
    }
    next_bindings
}

fn register_asset_path(
    asset_manager: &mut AssetManager,
    kind: AssetKind,
    path: &Path,
    state: AssetState,
) -> AssetHandle {
    let normalized = normalize_asset_path(path.to_path_buf());
    let handle = asset_manager.register(
        kind,
        normalized.to_string_lossy().into_owned(),
        Some(normalized),
    );
    let _ = asset_manager.mark_state(handle, state);
    handle
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EngineEvent, EventBus, EventDeliveryMode, ResourceEventKind};

    const SAMPLE_SCENE: &str = r#"# nu scene format v1
# .nuscene

[scene]
name = "test_scene"
syntax = opengl

[camera]
position = 0.0, 5.0, 10.0
target = 0.0, 0.0, 0.0
fov = 60.0

[light]
type = point
position = 5.0, 8.0, 3.0
color = 1.0, 1.0, 1.0
intensity = 1.0

[mesh.cube]
geometry = cube
material = red_material
transform.position = 0.0, 1.0, 0.0
transform.rotation = 45.0, 0.0, 0.0

[material.red_material]
shader = lit.vert, lit.frag
color = 1.0, 0.0, 0.0
roughness = 0.5
albedo_texture = crate.png
"#;

    #[test]
    fn parser_accepts_nu_scene_sample() {
        let document = parse_scene_str(SAMPLE_SCENE).expect("sample scene should parse");

        assert_eq!(document.metadata.format_version, 1);
        assert_eq!(document.metadata.engine_name, "nu");
        assert_eq!(document.metadata.format_name, "nu_scene_v1");
        assert_eq!(document.metadata.extension, "nuscene");
        assert_eq!(document.scene.name, "test_scene");
        assert_eq!(document.scene.syntax, SceneSyntax::OpenGl);
        assert_eq!(document.camera.position, [0.0, 5.0, 10.0]);
        assert_eq!(document.camera.target, [0.0, 0.0, 0.0]);
        assert_eq!(document.camera.fov_degrees, 60.0);
        assert_eq!(document.meshes["cube"].material, "red_material");
        assert_eq!(
            document.meshes["cube"].transform.rotation_degrees,
            [45.0, 0.0, 0.0]
        );
        assert_eq!(
            document.materials["red_material"].shader_vertex,
            PathBuf::from("lit.vert")
        );
        assert_eq!(
            document.materials["red_material"].shader_fragment,
            PathBuf::from("lit.frag")
        );
        assert_eq!(
            document.materials["red_material"].albedo_texture,
            Some(PathBuf::from("crate.png"))
        );
    }

    #[test]
    fn asset_reference_graph_collects_scene_shader_and_texture_paths() {
        let document = parse_scene_str(SAMPLE_SCENE).expect("sample scene should parse");
        let references =
            document.asset_references(Path::new("D:/3D/project/scenes/test_scene.nuscene"));

        assert!(
            references.scene_path.ends_with("test_scene.nuscene"),
            "scene path should include the scene file"
        );
        assert!(
            references.shader_programs["red_material"]
                .vertex
                .ends_with("scenes/lit.vert")
        );
        assert!(
            references.shader_programs["red_material"]
                .fragment
                .ends_with("scenes/lit.frag")
        );
        assert!(
            references.textures["red_material"].ends_with("scenes/crate.png"),
            "texture path should resolve relative to the scene file"
        );
        assert!(references.meshes.is_empty());
    }

    #[test]
    fn asset_reference_graph_collects_obj_mesh_paths() {
        let document = parse_scene_str(
            r#"# nu scene format v1
# .nuscene

[scene]
name = "obj_scene"
syntax = vulkan

[camera]
position = 0.0, 0.0, 5.0
target = 0.0, 0.0, 0.0
fov = 60.0

[light.key]
type = point
position = 0.0, 4.0, 4.0
color = 1.0, 1.0, 1.0
intensity = 1.0

[mesh.crate]
geometry = obj
source = meshes/crate.obj
material = red_material

[material.red_material]
shader.vertex = lit.vert
shader.fragment = lit.frag
"#,
        )
        .expect("obj scene should parse");

        let references =
            document.asset_references(Path::new("D:/3D/project/scenes/obj_scene.nuscene"));

        assert!(
            references.meshes["crate"].ends_with("scenes/meshes/crate.obj"),
            "mesh path should resolve relative to the scene file"
        );
    }

    #[test]
    fn explicit_template_is_parseable() {
        let document =
            parse_scene_str(EXPLICIT_NUSCENE_TEMPLATE).expect("explicit template should parse");
        assert_eq!(document.metadata.engine_name, "nu");
        assert_eq!(document.scene.name, "test_scene");
        assert!(document.lights.contains_key("key"));
        assert!(document.lights.contains_key("fill"));
        assert_eq!(
            document.environment,
            Some(NuEnvironmentSection {
                ambient_color: [0.1, 0.1, 0.15],
                ambient_intensity: 0.3,
                shadow_mode: ShadowMode::Live,
                shadow_max_distance: 32.0,
                shadow_filter_radius: 1.5,
            })
        );
        assert_eq!(
            document.meshes["car"].transform.rotation_degrees,
            [45.0, 0.0, 0.0]
        );
        assert_eq!(
            document.materials["red_material"].shader_vertex,
            PathBuf::from("lit.vert")
        );
    }

    #[test]
    fn rotation_radians_is_converted_to_degrees() {
        let document = parse_scene_str(
            r#"# nu scene format v1
# .nuscene

[scene]
name = "rotation_test"
syntax = vulkan

[camera]
position = 0.0, 0.0, 5.0
target = 0.0, 0.0, 0.0
fov = 60.0

[light.key]
type = point
position = 0.0, 4.0, 4.0
color = 1.0, 1.0, 1.0
intensity = 1.0

[mesh.cube]
geometry = cube
material = red_material
transform.rotation_radians = 1.5707964, 0.0, 0.0

[material.red_material]
shader.vertex = lit.vert
shader.fragment = lit.frag
"#,
        )
        .expect("scene with rotation_radians should parse");

        assert!((document.meshes["cube"].transform.rotation_degrees[0] - 90.0).abs() < 0.01);
        assert_eq!(document.meshes["cube"].transform.rotation_degrees[1], 0.0);
        assert_eq!(document.meshes["cube"].transform.rotation_degrees[2], 0.0);
    }

    #[test]
    fn duplicate_sections_are_rejected() {
        let error = parse_scene_str(
            r#"# nu scene format v1
# .nuscene

[scene]
name = "dup"
syntax = opengl

[scene]
name = "dup_again"
syntax = opengl
"#,
        )
        .expect_err("duplicate sections should fail");

        assert!(error.to_string().contains("duplicate section header"));
    }

    #[test]
    fn missing_mesh_parent_is_rejected() {
        let error = parse_scene_str(
            r#"# nu scene format v1
# .nuscene

[scene]
name = "missing_parent"
syntax = opengl

[camera]
position = 0.0, 0.0, 5.0
target = 0.0, 0.0, 0.0
fov = 60.0

[light.key]
type = point
position = 0.0, 4.0, 4.0
color = 1.0, 1.0, 1.0
intensity = 1.0

[mesh.wheel]
geometry = cube
material = red_material
parent = mesh.car

[material.red_material]
shader.vertex = lit.vert
shader.fragment = lit.frag
"#,
        )
        .expect_err("missing parent should fail");

        assert!(error.to_string().contains("references missing parent"));
    }

    #[test]
    fn unsupported_geometry_is_rejected() {
        let error = parse_scene_str(
            r#"# nu scene format v1
# .nuscene

[scene]
name = "bad_geometry"
syntax = opengl

[camera]
position = 0.0, 0.0, 5.0
target = 0.0, 0.0, 0.0
fov = 60.0

[light.key]
type = point
position = 0.0, 4.0, 4.0
color = 1.0, 1.0, 1.0
intensity = 1.0

[mesh.torus]
geometry = torus
material = red_material

[material.red_material]
shader.vertex = lit.vert
shader.fragment = lit.frag
"#,
        )
        .expect_err("unsupported geometry should fail");

        assert!(error.to_string().contains("unsupported geometry"));
    }

    #[test]
    fn legacy_backend_key_is_still_accepted() {
        let document = parse_scene_str(
            r#"# nu scene format v1
# .nuscene

[scene]
name = "legacy_scene"
backend = raw

[camera]
position = 0.0, 0.0, 5.0
target = 0.0, 0.0, 0.0
fov = 60.0

[light.key]
type = point
position = 0.0, 4.0, 4.0
color = 1.0, 1.0, 1.0
intensity = 1.0

[mesh.cube]
geometry = cube
material = red_material

[material.red_material]
shader.vertex = lit.vert
shader.fragment = lit.frag
"#,
        )
        .expect("legacy backend key should still parse");

        assert_eq!(document.scene.syntax, SceneSyntax::Raw);
    }

    #[test]
    fn scene_physics_runtime_builds_bodies_from_mesh_sections() {
        let document = parse_scene_str(
            r#"# nu scene format v1
# .nuscene

[scene]
name = "physics_scene"
syntax = opengl

[camera]
position = 0.0, 4.0, 8.0
target = 0.0, 0.0, 0.0
fov = 60.0

[light.key]
type = point
position = 1.0, 4.0, 2.0
color = 1.0, 1.0, 1.0
intensity = 1.0

[mesh.floor]
geometry = plane
material = red_material
transform.position = 0.0, 0.0, 0.0
transform.scale = 8.0, 1.0, 8.0
physics.body = static
physics.collider = plane
physics.mass = 1.0

[mesh.ball]
geometry = sphere
material = red_material
transform.position = 0.0, 3.0, 0.0
transform.scale = 2.0, 2.0, 2.0
physics.body = dynamic
physics.collider = auto
physics.mass = 1.5

[material.red_material]
shader.vertex = lit.vert
shader.fragment = lit.frag
"#,
        )
        .expect("scene should parse");

        let runtime = build_scene_physics_runtime(&document).expect("physics runtime should build");

        assert_eq!(runtime.bindings().len(), 2);
        let (_, floor) = runtime
            .body_for_mesh("floor")
            .expect("floor body should exist");
        assert_eq!(floor.body_type, crate::physics::BodyType::Static);
        assert_eq!(
            floor.collider,
            ColliderShape::Plane {
                normal: [0.0, 1.0, 0.0],
                offset: 0.0,
            }
        );

        let (_, ball) = runtime
            .body_for_mesh("ball")
            .expect("ball body should exist");
        assert_eq!(ball.body_type, crate::physics::BodyType::Dynamic);
        assert_eq!(ball.position, [0.0, 3.0, 0.0]);
        assert_eq!(ball.collider, ColliderShape::Sphere { radius: 1.0 });
        assert!((ball.mass - 1.5).abs() < 0.0001);
    }

    #[test]
    fn scene_physics_runtime_syncs_dynamic_body_back_into_scene_mesh() {
        let mut document = parse_scene_str(
            r#"# nu scene format v1
# .nuscene

[scene]
name = "sync_scene"
syntax = opengl

[camera]
position = 0.0, 4.0, 8.0
target = 0.0, 0.0, 0.0
fov = 60.0

[light.key]
type = point
position = 1.0, 4.0, 2.0
color = 1.0, 1.0, 1.0
intensity = 1.0

[mesh.root]
geometry = cube
material = red_material
transform.position = 10.0, 0.0, 0.0
transform.rotation_degrees = 10.0, 20.0, 30.0

[mesh.child]
geometry = cube
material = red_material
parent = mesh.root
transform.position = 1.0, 2.0, 3.0
transform.rotation_degrees = 5.0, 6.0, 7.0
physics.body = kinematic
physics.collider = cuboid
physics.mass = 1.0

[material.red_material]
shader.vertex = lit.vert
shader.fragment = lit.frag
"#,
        )
        .expect("scene should parse");

        let mut runtime =
            build_scene_physics_runtime(&document).expect("physics runtime should build");
        let (body_handle, _) = runtime
            .body_for_mesh("child")
            .expect("child body should exist");
        let body = runtime
            .world_mut()
            .body_mut(body_handle)
            .expect("body should still exist");
        body.position = [15.0, 9.0, 5.0];
        body.rotation_radians = [
            45.0f32.to_radians(),
            35.0f32.to_radians(),
            15.0f32.to_radians(),
        ];

        runtime
            .sync_to_scene(&mut document)
            .expect("scene sync should succeed");

        let child = &document.meshes["child"];
        assert_eq!(child.transform.position, [5.0, 9.0, 5.0]);
        assert!((child.transform.rotation_degrees[0] - 35.0).abs() < 0.01);
        assert!((child.transform.rotation_degrees[1] - 15.0).abs() < 0.01);
        assert!((child.transform.rotation_degrees[2] + 15.0).abs() < 0.01);
    }

    #[test]
    fn shader_stage_is_inferred_from_filename_suffix() {
        assert_eq!(
            shader_stage_from_path(Path::new("basic.vert")).unwrap(),
            ShaderStage::Vertex
        );
        assert_eq!(
            shader_stage_from_path(Path::new("basic.frag.glsl")).unwrap(),
            ShaderStage::Fragment
        );
    }

    #[test]
    fn reload_batch_emits_scene_shader_and_texture_events() {
        let scene = parse_scene_str(EXPLICIT_NUSCENE_TEMPLATE).expect("template should parse");
        let batch = ReloadBatch {
            changed_paths: vec![],
            scene: Some(scene),
            shaders: vec![ReloadedShader {
                material_name: "red_material".to_string(),
                path: PathBuf::from("lit.vert"),
                stage: ShaderStage::Vertex,
                spirv_words: vec![1, 2, 3],
            }],
            textures: vec![ReloadedTexture {
                material_name: "red_material".to_string(),
                path: PathBuf::from("crate.png"),
                bytes: vec![1, 2, 3],
            }],
        };
        let mut bus = EventBus::default();

        publish_reload_batch_events(&batch, &mut bus, EventDeliveryMode::Queued);

        assert_eq!(bus.queued_len(), 4);
        let trace = bus.trace();
        assert!(matches!(trace[0], EngineEvent::SceneLoaded { .. }));
        assert!(matches!(
            trace[1],
            EngineEvent::ResourceReloaded {
                kind: ResourceEventKind::Scene,
                ..
            }
        ));
        assert!(matches!(
            trace[2],
            EngineEvent::ResourceReloaded {
                kind: ResourceEventKind::Shader,
                ..
            }
        ));
        assert!(matches!(
            trace[3],
            EngineEvent::ResourceReloaded {
                kind: ResourceEventKind::Texture,
                ..
            }
        ));
    }

    #[test]
    fn register_scene_assets_tracks_scene_shader_texture_and_mesh_handles() {
        let document = parse_scene_str(
            r#"# nu scene format v1
# .nuscene

[scene]
name = "asset_scene"
syntax = vulkan

[camera]
position = 0.0, 0.0, 5.0
target = 0.0, 0.0, 0.0
fov = 60.0

[light.key]
type = point
position = 0.0, 4.0, 4.0
color = 1.0, 1.0, 1.0
intensity = 1.0

[mesh.crate]
geometry = obj
source = meshes/crate.obj
material = red_material

[material.red_material]
shader.vertex = lit.vert
shader.fragment = lit.frag
albedo_texture = crate.png
"#,
        )
        .expect("scene should parse");
        let mut asset_manager = AssetManager::default();
        let bindings = register_scene_assets(
            &document,
            Path::new("D:/3D/project/scenes/asset_scene.nuscene"),
            &mut asset_manager,
        );

        assert_eq!(
            asset_manager.get(bindings.scene).expect("scene asset").kind,
            AssetKind::Scene
        );
        assert_eq!(
            asset_manager
                .get(bindings.shader_programs["red_material"].vertex)
                .expect("vertex shader")
                .kind,
            AssetKind::Shader
        );
        assert_eq!(
            asset_manager
                .get(bindings.textures["red_material"])
                .expect("texture")
                .kind,
            AssetKind::Texture
        );
        assert_eq!(
            asset_manager
                .get(bindings.meshes["crate"])
                .expect("mesh")
                .kind,
            AssetKind::Mesh
        );
    }
}
