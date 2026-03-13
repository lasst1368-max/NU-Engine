use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir missing"));
    let shader_dir = manifest_dir.join("shaders");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR missing"));

    println!("cargo:rerun-if-changed={}", shader_dir.display());

    let mut compiler = shaderc::Compiler::new().expect("failed to create shader compiler");
    let mut options = shaderc::CompileOptions::new().expect("failed to create compile options");
    options.set_target_env(
        shaderc::TargetEnv::Vulkan,
        shaderc::EnvVersion::Vulkan1_0 as u32,
    );
    options.set_optimization_level(shaderc::OptimizationLevel::Performance);
    options.set_generate_debug_info();

    compile_shader(
        &mut compiler,
        &options,
        &shader_dir.join("primitive_2d.vert"),
        shaderc::ShaderKind::Vertex,
        &out_dir.join("primitive_2d.vert.spv"),
    );
    compile_shader(
        &mut compiler,
        &options,
        &shader_dir.join("primitive_2d.frag"),
        shaderc::ShaderKind::Fragment,
        &out_dir.join("primitive_2d.frag.spv"),
    );
    compile_shader(
        &mut compiler,
        &options,
        &shader_dir.join("cube_3d.vert"),
        shaderc::ShaderKind::Vertex,
        &out_dir.join("cube_3d.vert.spv"),
    );
    compile_shader(
        &mut compiler,
        &options,
        &shader_dir.join("cube_3d.frag"),
        shaderc::ShaderKind::Fragment,
        &out_dir.join("cube_3d.frag.spv"),
    );
    compile_shader(
        &mut compiler,
        &options,
        &shader_dir.join("text_2d.vert"),
        shaderc::ShaderKind::Vertex,
        &out_dir.join("text_2d.vert.spv"),
    );
    compile_shader(
        &mut compiler,
        &options,
        &shader_dir.join("text_2d.frag"),
        shaderc::ShaderKind::Fragment,
        &out_dir.join("text_2d.frag.spv"),
    );
}

fn compile_shader(
    compiler: &mut shaderc::Compiler,
    options: &shaderc::CompileOptions<'_>,
    source_path: &Path,
    kind: shaderc::ShaderKind,
    output_path: &Path,
) {
    let source = fs::read_to_string(source_path)
        .unwrap_or_else(|err| panic!("failed reading {}: {err}", source_path.display()));
    let source_name = source_path.to_string_lossy();
    let binary = compiler
        .compile_into_spirv(&source, kind, &source_name, "main", Some(options))
        .unwrap_or_else(|err| panic!("failed compiling {}: {err}", source_path.display()));

    fs::write(output_path, binary.as_binary_u8())
        .unwrap_or_else(|err| panic!("failed writing {}: {err}", output_path.display()));
}
