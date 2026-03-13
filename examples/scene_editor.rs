use nu::{LightKind, NuTransform, SceneEditor, SceneSyntax};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from("scenes/editor_generated.nuscene");

    let mut editor = SceneEditor::new_empty("editor_generated");
    editor.set_syntax(SceneSyntax::Vulkan);
    editor.set_camera([0.0, 5.0, 10.0], [0.0, 0.0, 0.0], 60.0);
    editor.set_environment([0.10, 0.10, 0.15], 0.3);
    editor.upsert_light(
        "key",
        LightKind::Point,
        [5.0, 8.0, 3.0],
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
        "red_material",
        None,
        NuTransform {
            position: [0.0, 1.0, 0.0],
            rotation_degrees: [45.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        },
    );
    editor.save_as(&output)?;

    println!("saved {}", output.display());
    Ok(())
}
