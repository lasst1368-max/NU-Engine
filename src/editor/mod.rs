use crate::engine::{
    EngineError, HotReloadManager, LightKind, NuCameraSection, NuEnvironmentSection,
    NuLightSection, NuMaterialSection, NuMeshSection, NuSceneDocument, NuSceneMetadata,
    NuSceneSection, NuTransform, ReloadBatch, SceneSyntax, load_scene_file,
};
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
        self.document.environment = Some(NuEnvironmentSection {
            ambient_color,
            ambient_intensity,
        });
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
            },
        );
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
        out.push_str(&format!("intensity = {}\n\n", format_f32(light.intensity)));
    }

    if let Some(environment) = &document.environment {
        out.push_str("[environment]\n");
        out.push_str(&format!(
            "ambient_color = {}\n",
            format_vec3(environment.ambient_color)
        ));
        out.push_str(&format!(
            "ambient_intensity = {}\n\n",
            format_f32(environment.ambient_intensity)
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
    use crate::engine::parse_scene_str;
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
        );
        editor.upsert_material(
            "red_material",
            "lit.vert",
            "lit.frag",
            [1.0, 0.0, 0.0],
            0.5,
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
        );

        let scene_text = editor.to_nuscene_string();
        assert!(scene_text.contains("transform.rotation_radians = 1.570796"));
        assert!(!scene_text.contains("transform.rotation_degrees"));
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
        );
        editor.upsert_material(
            "red_material",
            "lit.vert",
            "lit.frag",
            [1.0, 0.0, 0.0],
            0.5,
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
}
