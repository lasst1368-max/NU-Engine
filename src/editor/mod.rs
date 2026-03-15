use crate::engine::{
    EngineError, HotReloadManager, LightKind, NuCameraSection, NuEnvironmentSection,
    NuLightSection, NuMaterialSection, NuMeshSection, NuPhysicsSection, NuSceneDocument,
    NuSceneMetadata, NuSceneSection, NuTransform, ReloadBatch, SceneSyntax, load_scene_file,
};
use crate::lighting::ShadowMode;
use std::fs;
use std::path::{Path, PathBuf};

pub mod ui;

#[derive(Debug, Clone)]
pub struct SceneEditor {
    scene_path: Option<PathBuf>,
    document: NuSceneDocument,
}

impl SceneEditor {
    pub fn open(scene_path: impl AsRef<Path>) -> Result<Self, EngineError> {
        let scene_path = scene_path.as_ref().to_path_buf();
        let document = load_scene_file(&scene_path)?;
        Ok(Self {
            scene_path: Some(scene_path),
            document,
        })
    }

    pub fn new_empty(name: impl Into<String>) -> Self {
        Self {
            scene_path: None,
            document: NuSceneDocument {
                metadata: NuSceneMetadata::default(),
                scene: NuSceneSection {
                    name: name.into(),
                    syntax: SceneSyntax::OpenGl,
                },
                camera: NuCameraSection {
                    position: [0.0, 5.0, 10.0],
                    target: [0.0, 0.0, 0.0],
                    fov_degrees: 60.0,
                },
                lights: Default::default(),
                environment: Some(NuEnvironmentSection {
                    ambient_color: [0.1, 0.1, 0.15],
                    ambient_intensity: 0.3,
                    shadow_mode: ShadowMode::Live,
                    shadow_max_distance: 32.0,
                    shadow_filter_radius: 1.5,
                }),
                meshes: Default::default(),
                materials: Default::default(),
            },
        }
    }

    pub fn from_document(document: NuSceneDocument) -> Self {
        Self {
            scene_path: None,
            document,
        }
    }

    pub fn scene_path(&self) -> Option<&Path> {
        self.scene_path.as_deref()
    }

    pub fn document(&self) -> &NuSceneDocument {
        &self.document
    }

    pub fn document_mut(&mut self) -> &mut NuSceneDocument {
        &mut self.document
    }

    pub fn replace_document(&mut self, document: NuSceneDocument) {
        self.document = document;
    }

    pub fn set_scene_path(&mut self, scene_path: impl AsRef<Path>) {
        self.scene_path = Some(scene_path.as_ref().to_path_buf());
    }

    pub fn set_scene_name(&mut self, name: impl Into<String>) {
        self.document.scene.name = name.into();
    }

    pub fn set_syntax(&mut self, syntax: SceneSyntax) {
        self.document.scene.syntax = syntax;
    }

    pub fn set_backend(&mut self, backend: SceneSyntax) {
        self.set_syntax(backend);
    }

    pub fn set_camera(&mut self, position: [f32; 3], target: [f32; 3], fov_degrees: f32) {
        self.document.camera = NuCameraSection {
            position,
            target,
            fov_degrees,
        };
    }

    pub fn set_environment(&mut self, ambient_color: [f32; 3], ambient_intensity: f32) {
        let mut environment = self
            .document
            .environment
            .clone()
            .unwrap_or(default_environment());
        environment.ambient_color = ambient_color;
        environment.ambient_intensity = ambient_intensity;
        self.document.environment = Some(environment);
    }

    pub fn set_environment_shadows(
        &mut self,
        shadow_mode: ShadowMode,
        shadow_max_distance: f32,
        shadow_filter_radius: f32,
    ) {
        let mut environment = self
            .document
            .environment
            .clone()
            .unwrap_or(default_environment());
        environment.shadow_mode = shadow_mode;
        environment.shadow_max_distance = shadow_max_distance.max(1.0);
        environment.shadow_filter_radius = shadow_filter_radius.max(0.5);
        self.document.environment = Some(environment);
    }

    pub fn clear_environment(&mut self) {
        self.document.environment = None;
    }

    pub fn upsert_light(
        &mut self,
        name: impl Into<String>,
        kind: LightKind,
        position: [f32; 3],
        color: [f32; 3],
        intensity: f32,
        casts_shadow: bool,
    ) {
        let name = name.into();
        self.document.lights.insert(
            name.clone(),
            NuLightSection {
                name,
                kind,
                position,
                color,
                intensity,
                casts_shadow,
            },
        );
    }

    pub fn remove_light(&mut self, name: &str) -> Option<NuLightSection> {
        self.document.lights.remove(name)
    }

    pub fn upsert_mesh(
        &mut self,
        name: impl Into<String>,
        geometry: impl Into<String>,
        source: Option<PathBuf>,
        material: impl Into<String>,
        parent: Option<String>,
        transform: NuTransform,
        physics: Option<NuPhysicsSection>,
    ) {
        let name = name.into();
        self.document.meshes.insert(
            name.clone(),
            NuMeshSection {
                name,
                geometry: geometry.into(),
                source,
                material: material.into(),
                parent,
                transform,
                pivot_offset: [0.0, 0.0, 0.0],
                physics,
                script: None,
            },
        );
    }

    pub fn set_mesh_physics(
        &mut self,
        name: &str,
        physics: Option<NuPhysicsSection>,
    ) -> Result<(), EngineError> {
        let mesh = self
            .document
            .meshes
            .get_mut(name)
            .ok_or_else(|| EngineError::InvalidScene {
                reason: format!("mesh `{name}` does not exist"),
            })?;
        mesh.physics = physics;
        Ok(())
    }

    pub fn set_mesh_transform(
        &mut self,
        name: &str,
        position: [f32; 3],
        rotation_degrees: [f32; 3],
        scale: [f32; 3],
    ) -> Result<(), EngineError> {
        let mesh = self
            .document
            .meshes
            .get_mut(name)
            .ok_or_else(|| EngineError::InvalidScene {
                reason: format!("mesh `{name}` does not exist"),
            })?;
        mesh.transform = NuTransform {
            position,
            rotation_degrees,
            scale,
        };
        Ok(())
    }

    pub fn set_mesh_parent(
        &mut self,
        name: &str,
        parent: Option<String>,
    ) -> Result<(), EngineError> {
        let mesh = self
            .document
            .meshes
            .get_mut(name)
            .ok_or_else(|| EngineError::InvalidScene {
                reason: format!("mesh `{name}` does not exist"),
            })?;
        mesh.parent = parent;
        Ok(())
    }

    pub fn upsert_material(
        &mut self,
        name: impl Into<String>,
        shader_vertex: impl Into<PathBuf>,
        shader_fragment: impl Into<PathBuf>,
        color: [f32; 3],
        roughness: f32,
        metallic: f32,
        albedo_texture: Option<PathBuf>,
    ) {
        let name = name.into();
        self.document.materials.insert(
            name.clone(),
            NuMaterialSection {
                name,
                shader_vertex: shader_vertex.into(),
                shader_fragment: shader_fragment.into(),
                color,
                roughness,
                metallic,
                albedo_texture,
            },
        );
    }

    pub fn to_nuscene_string(&self) -> String {
        serialize_document(&self.document)
    }

    pub fn save(&mut self) -> Result<(), EngineError> {
        let Some(path) = self.scene_path.clone() else {
            return Err(EngineError::InvalidScene {
                reason: "scene editor has no source path; use save_as".into(),
            });
        };
        self.save_as(path)
    }

    pub fn save_as(&mut self, scene_path: impl AsRef<Path>) -> Result<(), EngineError> {
        let scene_path = scene_path.as_ref().to_path_buf();
        let serialized = self.to_nuscene_string();
        if let Some(parent) = scene_path.parent() {
            fs::create_dir_all(parent).map_err(|err| EngineError::Io {
                path: Some(parent.to_path_buf()),
                reason: err.to_string(),
            })?;
        }
        fs::write(&scene_path, serialized).map_err(|err| EngineError::Io {
            path: Some(scene_path.clone()),
            reason: err.to_string(),
        })?;
        self.scene_path = Some(scene_path);
        Ok(())
    }

    pub fn save_and_reload(
        &mut self,
        hot_reload: &mut HotReloadManager,
    ) -> Result<ReloadBatch, EngineError> {
        let Some(path) = self.scene_path.clone() else {
            return Err(EngineError::InvalidScene {
                reason: "scene editor has no source path; use save_as before hot reload".into(),
            });
        };
        self.save_as(path)?;
        hot_reload.reload_now()
    }
}

fn serialize_document(document: &NuSceneDocument) -> String {
    let mut out = String::new();
    out.push_str("# nu scene format v1\n");
    out.push_str("# .nuscene\n\n");

    out.push_str("[meta]\n");
    out.push_str(&format!(
        "engine = \"{}\"\n",
        escape_string(&document.metadata.engine_name)
    ));
    out.push_str(&format!(
        "format = \"{}\"\n",
        escape_string(&document.metadata.format_name)
    ));
    out.push_str(&format!(
        "extension = \".{}\"\n\n",
        escape_string(&document.metadata.extension)
    ));

    out.push_str("[scene]\n");
    out.push_str(&format!(
        "name = \"{}\"\n",
        escape_string(&document.scene.name)
    ));
    out.push_str(&format!(
        "syntax = {}\n",
        serialize_syntax(document.scene.syntax)
    ));
    out.push_str("# syntax selects the source profile translated into Vulkan in nu_scene_v1.\n\n");

    out.push_str("[camera]\n");
    out.push_str(&format!(
        "position = {}\n",
        format_vec3(document.camera.position)
    ));
    out.push_str(&format!(
        "target = {}\n",
        format_vec3(document.camera.target)
    ));
    out.push_str(&format!(
        "fov = {}\n\n",
        format_f32(document.camera.fov_degrees)
    ));

    for light in document.lights.values() {
        out.push_str(&format!("[light.{}]\n", light.name));
        out.push_str(&format!("type = {}\n", serialize_light_kind(light.kind)));
        out.push_str(&format!("position = {}\n", format_vec3(light.position)));
        out.push_str(&format!("color = {}\n", format_vec3(light.color)));
        out.push_str(&format!("intensity = {}\n", format_f32(light.intensity)));
        if !light.casts_shadow {
            out.push_str("casts_shadow = false\n");
        }
        out.push('\n');
    }

    if let Some(environment) = &document.environment {
        out.push_str("[environment]\n");
        out.push_str(&format!(
            "ambient_color = {}\n",
            format_vec3(environment.ambient_color)
        ));
        out.push_str(&format!(
            "ambient_intensity = {}\n",
            format_f32(environment.ambient_intensity)
        ));
        out.push_str(&format!(
            "shadow_mode = {}\n",
            serialize_shadow_mode(environment.shadow_mode)
        ));
        out.push_str(&format!(
            "shadow_max_distance = {}\n",
            format_f32(environment.shadow_max_distance)
        ));
        out.push_str(&format!(
            "shadow_filter_radius = {}\n\n",
            format_f32(environment.shadow_filter_radius)
        ));
    }

    for mesh in document.meshes.values() {
        out.push_str(&format!("[mesh.{}]\n", mesh.name));
        out.push_str(&format!("geometry = {}\n", mesh.geometry));
        if let Some(source) = &mesh.source {
            out.push_str(&format!("source = {}\n", format_path(source)));
        }
        out.push_str(&format!("material = {}\n", mesh.material));
        if let Some(parent) = &mesh.parent {
            out.push_str(&format!("parent = {}\n", parent));
        }
        out.push_str(&format!(
            "transform.position = {}\n",
            format_vec3(mesh.transform.position)
        ));
        out.push_str(&format!(
            "transform.rotation_radians = {}\n",
            format_vec3([
                mesh.transform.rotation_degrees[0].to_radians(),
                mesh.transform.rotation_degrees[1].to_radians(),
                mesh.transform.rotation_degrees[2].to_radians(),
            ])
        ));
        out.push_str(&format!(
            "transform.scale = {}\n\n",
            format_vec3(mesh.transform.scale)
        ));
        if mesh.pivot_offset != [0.0, 0.0, 0.0] {
            out.push_str(&format!(
                "pivot.offset = {}\n",
                format_vec3(mesh.pivot_offset)
            ));
        }
        if let Some(physics) = &mesh.physics {
            out.push_str(&format!(
                "physics.body = {}\n",
                serialize_physics_body(physics.body)
            ));
            out.push_str(&format!(
                "physics.collider = {}\n",
                serialize_physics_collider(physics.collider)
            ));
            out.push_str(&format!("physics.mass = {}\n\n", format_f32(physics.mass)));
        }
        if let Some(script) = &mesh.script {
            if let Some(path) = &script.na_script {
                out.push_str(&format!("script.na = {}\n", format_path(path)));
            }
            if let Some(path) = &script.cpp_script {
                out.push_str(&format!("script.cpp = {}\n", format_path(path)));
            }
            if script.player_camera {
                out.push_str("script.player_camera = true\n");
            }
            out.push('\n');
        }
    }

    for material in document.materials.values() {
        out.push_str(&format!("[material.{}]\n", material.name));
        out.push_str(&format!(
            "shader.vertex = {}\n",
            format_path(&material.shader_vertex)
        ));
        out.push_str(&format!(
            "shader.fragment = {}\n",
            format_path(&material.shader_fragment)
        ));
        out.push_str(&format!("color = {}\n", format_vec3(material.color)));
        out.push_str(&format!("roughness = {}\n", format_f32(material.roughness)));
        out.push_str(&format!("metallic = {}\n", format_f32(material.metallic)));
        if let Some(texture) = &material.albedo_texture {
            out.push_str(&format!("albedo_texture = {}\n", format_path(texture)));
        }
        out.push('\n');
    }

    out
}

fn serialize_syntax(syntax: SceneSyntax) -> &'static str {
    match syntax {
        SceneSyntax::OpenGl => "opengl",
        SceneSyntax::Vulkan => "vulkan",
        SceneSyntax::Raw => "raw",
    }
}

fn serialize_light_kind(kind: LightKind) -> &'static str {
    match kind {
        LightKind::Point => "point",
        LightKind::Directional => "directional",
    }
}

fn serialize_physics_body(kind: crate::engine::NuPhysicsBodyKind) -> &'static str {
    match kind {
        crate::engine::NuPhysicsBodyKind::Static => "static",
        crate::engine::NuPhysicsBodyKind::Dynamic => "dynamic",
        crate::engine::NuPhysicsBodyKind::Kinematic => "kinematic",
    }
}

fn serialize_physics_collider(kind: crate::engine::NuPhysicsColliderKind) -> &'static str {
    match kind {
        crate::engine::NuPhysicsColliderKind::Auto => "auto",
        crate::engine::NuPhysicsColliderKind::Cuboid => "cuboid",
        crate::engine::NuPhysicsColliderKind::Sphere => "sphere",
        crate::engine::NuPhysicsColliderKind::Plane => "plane",
    }
}

fn serialize_shadow_mode(mode: ShadowMode) -> &'static str {
    match mode {
        ShadowMode::Off => "off",
        ShadowMode::Live => "live",
    }
}

fn default_environment() -> NuEnvironmentSection {
    NuEnvironmentSection {
        ambient_color: [0.1, 0.1, 0.15],
        ambient_intensity: 0.3,
        shadow_mode: ShadowMode::Live,
        shadow_max_distance: 32.0,
        shadow_filter_radius: 1.5,
    }
}

fn escape_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn format_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn format_vec3(value: [f32; 3]) -> String {
    format!(
        "{}, {}, {}",
        format_f32(value[0]),
        format_f32(value[1]),
        format_f32(value[2])
    )
}

fn format_f32(value: f32) -> String {
    let mut rendered = format!("{value:.6}");
    while rendered.contains('.') && rendered.ends_with('0') {
        rendered.pop();
    }
    if rendered.ends_with('.') {
        rendered.push('0');
    }
    if !rendered.contains('.') {
        rendered.push_str(".0");
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{NuMeshScriptSection, parse_scene_str};
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn editor_serializes_rotation_as_radians() {
        let mut editor = SceneEditor::new_empty("editor_test");
        editor.upsert_light(
            "key",
            LightKind::Point,
            [1.0, 2.0, 3.0],
            [1.0, 1.0, 1.0],
            1.0,
            true,
        );
        editor.upsert_material(
            "red_material",
            "lit.vert",
            "lit.frag",
            [1.0, 0.0, 0.0],
            0.5,
            0.0,
            Some(PathBuf::from("crate.png")),
        );
        editor.upsert_mesh(
            "cube",
            "cube",
            None,
            "red_material",
            None,
            NuTransform {
                position: [0.0, 1.0, 0.0],
                rotation_degrees: [90.0, 0.0, 0.0],
                scale: [1.0, 1.0, 1.0],
            },
            None,
        );

        let scene_text = editor.to_nuscene_string();
        assert!(scene_text.contains("transform.rotation_radians = 1.570796"));
        assert!(!scene_text.contains("transform.rotation_degrees"));
        assert!(scene_text.contains("shadow_mode = live"));
        assert!(scene_text.contains("shadow_max_distance = 32.0"));
        assert!(scene_text.contains("shadow_filter_radius = 1.5"));
    }

    #[test]
    fn editor_save_round_trips_rotation_units() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos();
        let temp_path = std::env::temp_dir().join(format!("nu_editor_roundtrip_{unique}.nuscene"));

        let mut editor = SceneEditor::new_empty("roundtrip");
        editor.set_syntax(SceneSyntax::Vulkan);
        editor.upsert_light(
            "key",
            LightKind::Point,
            [0.0, 4.0, 4.0],
            [1.0, 1.0, 1.0],
            1.0,
            true,
        );
        editor.upsert_material(
            "red_material",
            "lit.vert",
            "lit.frag",
            [1.0, 0.0, 0.0],
            0.5,
            0.0,
            None,
        );
        editor.upsert_mesh(
            "cube",
            "cube",
            None,
            "red_material",
            None,
            NuTransform {
                position: [0.0, 1.0, 0.0],
                rotation_degrees: [45.0, 30.0, 15.0],
                scale: [1.0, 1.0, 1.0],
            },
            None,
        );
        editor
            .save_as(&temp_path)
            .expect("editor should save a scene file");

        let reloaded = load_scene_file(&temp_path).expect("saved scene should parse");
        assert_eq!(reloaded.scene.syntax, SceneSyntax::Vulkan);
        assert!((reloaded.meshes["cube"].transform.rotation_degrees[0] - 45.0).abs() < 0.01);
        assert!((reloaded.meshes["cube"].transform.rotation_degrees[1] - 30.0).abs() < 0.01);
        assert!((reloaded.meshes["cube"].transform.rotation_degrees[2] - 15.0).abs() < 0.01);

        let serialized = fs::read_to_string(&temp_path).expect("saved file should exist");
        assert!(serialized.contains("transform.rotation_radians = 0.785398, 0.523599, 0.261799"));
        assert!(parse_scene_str(&serialized).is_ok());

        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn editor_save_round_trips_mesh_pivot_offset() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos();
        let temp_path =
            std::env::temp_dir().join(format!("nu_editor_pivot_roundtrip_{unique}.nuscene"));

        let mut editor = SceneEditor::new_empty("pivot_roundtrip");
        editor.upsert_light(
            "key",
            LightKind::Point,
            [0.0, 4.0, 4.0],
            [1.0, 1.0, 1.0],
            1.0,
            true,
        );
        editor.upsert_material(
            "red_material",
            "lit.vert",
            "lit.frag",
            [1.0, 0.0, 0.0],
            0.5,
            0.0,
            None,
        );
        editor.upsert_mesh(
            "cube",
            "cube",
            None,
            "red_material",
            None,
            NuTransform::default(),
            None,
        );
        editor
            .document_mut()
            .meshes
            .get_mut("cube")
            .expect("cube should exist")
            .pivot_offset = [1.25, -0.5, 0.75];
        editor
            .save_as(&temp_path)
            .expect("editor should save a scene file");

        let reloaded = load_scene_file(&temp_path).expect("saved scene should parse");
        assert_eq!(reloaded.meshes["cube"].pivot_offset, [1.25, -0.5, 0.75]);

        let serialized = fs::read_to_string(&temp_path).expect("saved file should exist");
        assert!(serialized.contains("pivot.offset = 1.25, -0.5, 0.75"));
        assert!(parse_scene_str(&serialized).is_ok());

        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn editor_save_round_trips_mesh_scripts() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos();
        let temp_path =
            std::env::temp_dir().join(format!("nu_editor_script_roundtrip_{unique}.nuscene"));

        let mut editor = SceneEditor::new_empty("script_roundtrip");
        editor.upsert_light(
            "key",
            LightKind::Point,
            [0.0, 4.0, 4.0],
            [1.0, 1.0, 1.0],
            1.0,
            true,
        );
        editor.upsert_material(
            "red_material",
            "lit.vert",
            "lit.frag",
            [1.0, 0.0, 0.0],
            0.5,
            0.0,
            None,
        );
        editor.upsert_mesh(
            "car",
            "cube",
            None,
            "red_material",
            None,
            NuTransform::default(),
            None,
        );
        editor
            .document_mut()
            .meshes
            .get_mut("car")
            .expect("car should exist")
            .script = Some(NuMeshScriptSection {
            na_script: Some(PathBuf::from("scripts/player_controller.na")),
            cpp_script: Some(PathBuf::from("scripts/player_controller.cpp")),
            player_camera: true,
        });
        editor
            .save_as(&temp_path)
            .expect("editor should save a scene file");

        let reloaded = load_scene_file(&temp_path).expect("saved scene should parse");
        let script = reloaded.meshes["car"]
            .script
            .as_ref()
            .expect("script should persist");
        assert_eq!(
            script.na_script.as_deref(),
            Some(Path::new("scripts/player_controller.na"))
        );
        assert_eq!(
            script.cpp_script.as_deref(),
            Some(Path::new("scripts/player_controller.cpp"))
        );
        assert!(script.player_camera);

        let serialized = fs::read_to_string(&temp_path).expect("saved file should exist");
        assert!(serialized.contains("script.na = scripts/player_controller.na"));
        assert!(serialized.contains("script.cpp = scripts/player_controller.cpp"));
        assert!(serialized.contains("script.player_camera = true"));

        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn editor_save_round_trips_light_shadow_toggle() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos();
        let temp_path =
            std::env::temp_dir().join(format!("nu_editor_light_shadow_roundtrip_{unique}.nuscene"));

        let mut editor = SceneEditor::new_empty("light_shadow_roundtrip");
        editor.upsert_light(
            "key",
            LightKind::Directional,
            [0.0, 4.0, 4.0],
            [1.0, 0.95, 0.9],
            1.0,
            false,
        );
        editor
            .save_as(&temp_path)
            .expect("editor should save a scene file");

        let reloaded = load_scene_file(&temp_path).expect("saved scene should parse");
        assert!(!reloaded.lights["key"].casts_shadow);

        let serialized = fs::read_to_string(&temp_path).expect("saved file should exist");
        assert!(serialized.contains("casts_shadow = false"));
        assert!(parse_scene_str(&serialized).is_ok());

        let _ = fs::remove_file(temp_path);
    }
}
