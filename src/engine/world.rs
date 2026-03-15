use super::{
    EngineError, LightKind, NuCameraSection, NuEnvironmentSection, NuLightSection,
    NuMaterialSection, NuMeshScriptSection, NuMeshSection, NuPhysicsSection, NuSceneDocument,
    NuSceneMetadata, NuSceneSection, NuTransform,
};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub type EntityId = u64;

#[derive(Debug, Clone, PartialEq)]
pub struct TransformComponent {
    pub local: NuTransform,
    pub parent: Option<EntityId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeshRendererComponent {
    pub geometry: String,
    pub source: Option<PathBuf>,
    pub material: String,
    pub script: Option<NuMeshScriptSection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraComponent {
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub fov_degrees: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LightComponent {
    pub kind: LightKind,
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
    pub casts_shadow: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhysicsBodyComponent {
    pub body: NuPhysicsSection,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneEntity {
    pub id: EntityId,
    pub name: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NuSceneWorld {
    metadata: NuSceneMetadata,
    scene: NuSceneSection,
    environment: Option<NuEnvironmentSection>,
    next_entity_id: EntityId,
    entities: BTreeMap<EntityId, SceneEntity>,
    mesh_name_to_entity: BTreeMap<String, EntityId>,
    light_name_to_entity: BTreeMap<String, EntityId>,
    camera_entity: EntityId,
    transforms: BTreeMap<EntityId, TransformComponent>,
    meshes: BTreeMap<EntityId, MeshRendererComponent>,
    lights: BTreeMap<EntityId, LightComponent>,
    cameras: BTreeMap<EntityId, CameraComponent>,
    physics_bodies: BTreeMap<EntityId, PhysicsBodyComponent>,
    materials: BTreeMap<String, NuMaterialSection>,
}

impl NuSceneWorld {
    pub fn from_document(document: &NuSceneDocument) -> Result<Self, EngineError> {
        let mut world = Self {
            metadata: document.metadata.clone(),
            scene: document.scene.clone(),
            environment: document.environment.clone(),
            next_entity_id: 1,
            entities: BTreeMap::new(),
            mesh_name_to_entity: BTreeMap::new(),
            light_name_to_entity: BTreeMap::new(),
            camera_entity: 0,
            transforms: BTreeMap::new(),
            meshes: BTreeMap::new(),
            lights: BTreeMap::new(),
            cameras: BTreeMap::new(),
            physics_bodies: BTreeMap::new(),
            materials: document.materials.clone(),
        };

        let camera_entity = world.insert_entity("camera");
        world.camera_entity = camera_entity;
        world.cameras.insert(
            camera_entity,
            CameraComponent {
                position: document.camera.position,
                target: document.camera.target,
                fov_degrees: document.camera.fov_degrees,
            },
        );

        for light in document.lights.values() {
            let entity = world.insert_entity(&format!("light:{}", light.name));
            world
                .light_name_to_entity
                .insert(light.name.clone(), entity);
            world.lights.insert(
                entity,
                LightComponent {
                    kind: light.kind,
                    position: light.position,
                    color: light.color,
                    intensity: light.intensity,
                    casts_shadow: light.casts_shadow,
                },
            );
        }

        for mesh in document.meshes.values() {
            let entity = world.insert_entity(&mesh.name);
            world.mesh_name_to_entity.insert(mesh.name.clone(), entity);
        }

        for mesh in document.meshes.values() {
            let entity = world.mesh_name_to_entity[&mesh.name];
            let parent = match &mesh.parent {
                Some(parent) => {
                    let Some(parent_name) = parent.strip_prefix("mesh.") else {
                        return Err(EngineError::InvalidScene {
                            reason: format!(
                                "mesh `{}` has invalid parent `{parent}`; expected `mesh.<name>`",
                                mesh.name
                            ),
                        });
                    };
                    Some(*world.mesh_name_to_entity.get(parent_name).ok_or_else(|| {
                        EngineError::InvalidScene {
                            reason: format!(
                                "mesh `{}` references missing parent `mesh.{parent_name}`",
                                mesh.name
                            ),
                        }
                    })?)
                }
                None => None,
            };
            world.transforms.insert(
                entity,
                TransformComponent {
                    local: mesh.transform.clone(),
                    parent,
                },
            );
            world.meshes.insert(
                entity,
                MeshRendererComponent {
                    geometry: mesh.geometry.clone(),
                    source: mesh.source.clone(),
                    material: mesh.material.clone(),
                    script: mesh.script.clone(),
                },
            );
            if let Some(physics) = &mesh.physics {
                world.physics_bodies.insert(
                    entity,
                    PhysicsBodyComponent {
                        body: physics.clone(),
                    },
                );
            }
        }

        Ok(world)
    }

    pub fn to_document(&self) -> Result<NuSceneDocument, EngineError> {
        let camera =
            self.cameras
                .get(&self.camera_entity)
                .ok_or_else(|| EngineError::InvalidScene {
                    reason: "scene world is missing a camera component".to_string(),
                })?;

        let mut lights = BTreeMap::new();
        for (name, entity) in &self.light_name_to_entity {
            let Some(light) = self.lights.get(entity) else {
                continue;
            };
            lights.insert(
                name.clone(),
                NuLightSection {
                    name: name.clone(),
                    kind: light.kind,
                    position: light.position,
                    color: light.color,
                    intensity: light.intensity,
                    casts_shadow: light.casts_shadow,
                },
            );
        }

        let mut meshes = BTreeMap::new();
        for (name, entity) in &self.mesh_name_to_entity {
            let transform =
                self.transforms
                    .get(entity)
                    .ok_or_else(|| EngineError::InvalidScene {
                        reason: format!("mesh entity `{name}` is missing a transform component"),
                    })?;
            let mesh = self
                .meshes
                .get(entity)
                .ok_or_else(|| EngineError::InvalidScene {
                    reason: format!("mesh entity `{name}` is missing a mesh renderer component"),
                })?;
            let parent = transform.parent.map(|parent_id| {
                let parent_name = self
                    .entities
                    .get(&parent_id)
                    .map(|entity| entity.name.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                format!("mesh.{parent_name}")
            });
            meshes.insert(
                name.clone(),
                NuMeshSection {
                    name: name.clone(),
                    geometry: mesh.geometry.clone(),
                    source: mesh.source.clone(),
                    material: mesh.material.clone(),
                    parent,
                    transform: transform.local.clone(),
                    pivot_offset: [0.0, 0.0, 0.0],
                    physics: self
                        .physics_bodies
                        .get(entity)
                        .map(|body| body.body.clone()),
                    script: mesh.script.clone(),
                },
            );
        }

        Ok(NuSceneDocument {
            metadata: self.metadata.clone(),
            scene: self.scene.clone(),
            camera: NuCameraSection {
                position: camera.position,
                target: camera.target,
                fov_degrees: camera.fov_degrees,
            },
            lights,
            environment: self.environment.clone(),
            meshes,
            materials: self.materials.clone(),
        })
    }

    pub fn camera_entity(&self) -> EntityId {
        self.camera_entity
    }

    pub fn scene_section(&self) -> &NuSceneSection {
        &self.scene
    }

    pub fn environment(&self) -> Option<&NuEnvironmentSection> {
        self.environment.as_ref()
    }

    pub fn materials(&self) -> &BTreeMap<String, NuMaterialSection> {
        &self.materials
    }

    pub fn mesh_entities(&self) -> &BTreeMap<String, EntityId> {
        &self.mesh_name_to_entity
    }

    pub fn light_entities(&self) -> &BTreeMap<String, EntityId> {
        &self.light_name_to_entity
    }

    pub fn mesh_entity(&self, name: &str) -> Option<EntityId> {
        self.mesh_name_to_entity.get(name).copied()
    }

    pub fn light_entity(&self, name: &str) -> Option<EntityId> {
        self.light_name_to_entity.get(name).copied()
    }

    pub fn transform(&self, entity: EntityId) -> Option<&TransformComponent> {
        self.transforms.get(&entity)
    }

    pub fn transform_mut(&mut self, entity: EntityId) -> Option<&mut TransformComponent> {
        self.transforms.get_mut(&entity)
    }

    pub fn mesh_renderer(&self, entity: EntityId) -> Option<&MeshRendererComponent> {
        self.meshes.get(&entity)
    }

    pub fn camera(&self, entity: EntityId) -> Option<&CameraComponent> {
        self.cameras.get(&entity)
    }

    pub fn primary_camera(&self) -> Option<&CameraComponent> {
        self.camera(self.camera_entity)
    }

    pub fn light(&self, entity: EntityId) -> Option<&LightComponent> {
        self.lights.get(&entity)
    }

    pub fn physics_body(&self, entity: EntityId) -> Option<&PhysicsBodyComponent> {
        self.physics_bodies.get(&entity)
    }

    pub fn resolved_transform(&self, entity: EntityId) -> Option<NuTransform> {
        let transform = self.transforms.get(&entity)?;
        Some(match transform.parent {
            Some(parent) => {
                let parent_transform = self.resolved_transform(parent)?;
                NuTransform {
                    position: add3(transform.local.position, parent_transform.position),
                    rotation_degrees: add3(
                        transform.local.rotation_degrees,
                        parent_transform.rotation_degrees,
                    ),
                    scale: [
                        transform.local.scale[0] * parent_transform.scale[0],
                        transform.local.scale[1] * parent_transform.scale[1],
                        transform.local.scale[2] * parent_transform.scale[2],
                    ],
                }
            }
            None => transform.local.clone(),
        })
    }

    fn insert_entity(&mut self, name: &str) -> EntityId {
        let id = self.next_entity_id;
        self.next_entity_id = self.next_entity_id.wrapping_add(1);
        self.entities.insert(
            id,
            SceneEntity {
                id,
                name: name.to_string(),
                active: true,
            },
        );
        id
    }
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{SceneSyntax, parse_scene_str};

    #[test]
    fn scene_world_compiles_document_into_runtime_components() {
        let document = parse_scene_str(
            r#"# nu scene format v1
# .nuscene

[scene]
name = "world_test"
syntax = opengl

[camera]
position = 0.0, 5.0, 10.0
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
transform.position = 1.0, 2.0, 3.0

[mesh.child]
geometry = sphere
material = red_material
parent = mesh.root
transform.position = 1.0, 0.0, 0.0

[material.red_material]
shader.vertex = lit.vert
shader.fragment = lit.frag
"#,
        )
        .expect("scene should parse");

        let world = NuSceneWorld::from_document(&document).expect("world should compile");
        let root = world.mesh_entity("root").expect("root entity");
        let child = world.mesh_entity("child").expect("child entity");
        assert_eq!(world.camera_entity(), 1);
        assert_eq!(
            world
                .mesh_renderer(child)
                .expect("mesh renderer")
                .geometry
                .as_str(),
            "sphere"
        );
        assert_eq!(
            world
                .resolved_transform(root)
                .expect("root transform")
                .position,
            [1.0, 2.0, 3.0]
        );
        assert_eq!(
            world
                .resolved_transform(child)
                .expect("child transform")
                .position,
            [2.0, 2.0, 3.0]
        );
    }

    #[test]
    fn scene_world_round_trips_back_to_document() {
        let document = NuSceneDocument {
            metadata: Default::default(),
            scene: super::super::NuSceneSection {
                name: "roundtrip".to_string(),
                syntax: SceneSyntax::Vulkan,
            },
            camera: NuCameraSection {
                position: [0.0, 4.0, 8.0],
                target: [0.0, 0.0, 0.0],
                fov_degrees: 60.0,
            },
            lights: BTreeMap::from([(
                "key".to_string(),
                NuLightSection {
                    name: "key".to_string(),
                    kind: LightKind::Point,
                    position: [1.0, 2.0, 3.0],
                    color: [1.0, 1.0, 1.0],
                    intensity: 1.0,
                    casts_shadow: true,
                },
            )]),
            environment: None,
            meshes: BTreeMap::from([(
                "cube".to_string(),
                NuMeshSection {
                    name: "cube".to_string(),
                    geometry: "cube".to_string(),
                    source: None,
                    material: "red_material".to_string(),
                    parent: None,
                    transform: NuTransform::default(),
                    pivot_offset: [0.0, 0.0, 0.0],
                    physics: None,
                    script: None,
                },
            )]),
            materials: BTreeMap::from([(
                "red_material".to_string(),
                NuMaterialSection {
                    name: "red_material".to_string(),
                    shader_vertex: PathBuf::from("lit.vert"),
                    shader_fragment: PathBuf::from("lit.frag"),
                    color: [1.0, 0.0, 0.0],
                    roughness: 0.5,
                    metallic: 0.0,
                    albedo_texture: None,
                },
            )]),
        };

        let world = NuSceneWorld::from_document(&document).expect("world should compile");
        let roundtrip = world.to_document().expect("world should serialize");
        assert_eq!(roundtrip.scene.name, "roundtrip");
        assert_eq!(roundtrip.scene.syntax, SceneSyntax::Vulkan);
        assert!(roundtrip.meshes.contains_key("cube"));
        assert!(roundtrip.materials.contains_key("red_material"));
        assert!(roundtrip.lights.contains_key("key"));
    }
}
