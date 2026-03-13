use nu::run_basic_scene_editor;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scene_path = std::env::args().nth(1).map(PathBuf::from);
    run_basic_scene_editor(scene_path.as_deref())?;
    Ok(())
}
