# API Map

This document maps the main engine API surfaces to the files and lines where they live.

It is intentionally practical:
- what the main entry points are
- how data flows through the engine
- where the editor, runtime, FFI, events, assets, and scripts live
- where the current PBR material path is implemented

## High-Level Layout

The crate root re-exports the main subsystems from [src/lib.rs](D:/3D/API/src/lib.rs):
- FFI module: [src/lib.rs:8](D:/3D/API/src/lib.rs:8)
- engine world exports: [src/lib.rs:34](D:/3D/API/src/lib.rs:34)
- event exports: [src/lib.rs:50](D:/3D/API/src/lib.rs:50)
- resource exports: [src/lib.rs:71](D:/3D/API/src/lib.rs:71)
- runtime exports: [src/lib.rs:75](D:/3D/API/src/lib.rs:75)
- scene exports: [src/lib.rs:80](D:/3D/API/src/lib.rs:80)
- script exports: [src/lib.rs:85](D:/3D/API/src/lib.rs:85)

The codebase is split into these layers:
- `scene`: runtime-facing scene description traits and draw data
- `runtime`: Vulkan renderer and execution path
- `engine`: `.nuscene` document parsing and compiled world model
- `editor`: scene editing, viewport, gizmos, play mode, undo/redo
- `ffi`: C ABI for importing `nu` into C/C++
- `resource`: asset handles and asset manager
- `event`: typed event bus
- `script`: NAScript parsing and script-side movement bindings

## Data Flow

The normal render path is:
1. A `Scene` implementation provides a `SceneConfig` and populates a `SceneFrame`.
2. The Vulkan runtime consumes that frame and turns it into internal draw batches.
3. Material data is packed into runtime parameters.
4. The shaders render the scene.

Main entry points:
- `SceneConfig`: [src/scene/mod.rs:10](D:/3D/API/src/scene/mod.rs:10)
- `Scene` trait: [src/scene/mod.rs:108](D:/3D/API/src/scene/mod.rs:108)
- `SceneFrame`: [src/scene/mod.rs:421](D:/3D/API/src/scene/mod.rs:421)
- `run_scene(...)`: [src/runtime/mod.rs:75](D:/3D/API/src/runtime/mod.rs:75)

## Scene API

The scene API is the main high-level rendering contract.

Core types in [src/scene/mod.rs](D:/3D/API/src/scene/mod.rs):
- `SceneConfig`: [src/scene/mod.rs:10](D:/3D/API/src/scene/mod.rs:10)
  - window title, lighting config, screenshot request, viewport behavior
- `Scene` trait: [src/scene/mod.rs:108](D:/3D/API/src/scene/mod.rs:108)
  - implement this to drive a scene through the runtime
- `MeshMaterial3D`: [src/scene/mod.rs:294](D:/3D/API/src/scene/mod.rs:294)
  - current material data for mesh draws
  - includes `color`, `roughness`, `metallic`, and texture path fields
- `MeshDraw3D`: [src/scene/mod.rs:311](D:/3D/API/src/scene/mod.rs:311)
  - a single submitted mesh draw
- `SceneFrame`: [src/scene/mod.rs:421](D:/3D/API/src/scene/mod.rs:421)
  - the frame-level container that the runtime consumes

If you are tracing why something appears in the renderer, start with `SceneFrame` and `MeshDraw3D`.

## Runtime Renderer

The Vulkan renderer lives in [src/runtime/mod.rs](D:/3D/API/src/runtime/mod.rs).

Key entry points:
- `run_scene(...)`: [src/runtime/mod.rs:75](D:/3D/API/src/runtime/mod.rs:75)
- internal mesh batch type: `MeshDrawBatch3D` at [src/runtime/mod.rs:227](D:/3D/API/src/runtime/mod.rs:227)
- material packing: `mesh_material_vertex_params(...)` at [src/runtime/mod.rs:692](D:/3D/API/src/runtime/mod.rs:692)
- descriptor setup for material data: `ensure_material_descriptor_set_3d(...)` at [src/runtime/mod.rs:1942](D:/3D/API/src/runtime/mod.rs:1942)

What the runtime does:
- turns submitted mesh draws into batchable internal draw data
- uploads material/light/object buffers
- binds pipelines and descriptor sets
- handles screenshot capture from the swapchain
- runs the 3D forward render path used by both the editor and the C++ preview host

## Current Material / PBR Path

The current material model is implemented across these files:
- material struct: [src/scene/mod.rs:294](D:/3D/API/src/scene/mod.rs:294)
- runtime packing: [src/runtime/mod.rs:692](D:/3D/API/src/runtime/mod.rs:692)
- fragment shader: [shaders/cube_3d.frag](D:/3D/API/shaders/cube_3d.frag)

Current material properties:
- `color`
- `roughness`
- `metallic`
- optional texture paths in scene/editor data

The current shader path uses a GGX-style PBR core in [shaders/cube_3d.frag](D:/3D/API/shaders/cube_3d.frag).
It replaced the older ad hoc shininess/specular path.

Current boundary:
- normal maps are not implemented yet
- tangent generation is not implemented yet
- `.mtl` material import is not implemented yet
- the texture-set runtime is still incomplete relative to a full PBR pipeline

## `.nuscene` Engine Layer

The scene document and compiled world model live under `engine`.

Main document types in [src/engine/mod.rs](D:/3D/API/src/engine/mod.rs):
- `NuMaterialSection`: [src/engine/mod.rs:333](D:/3D/API/src/engine/mod.rs:333)
- `NuSceneDocument`: [src/engine/mod.rs:353](D:/3D/API/src/engine/mod.rs:353)
- `build_scene_world(...)`: [src/engine/mod.rs:505](D:/3D/API/src/engine/mod.rs:505)
- `parse_scene_str(...)`: [src/engine/mod.rs:917](D:/3D/API/src/engine/mod.rs:917)

Compiled world type:
- `NuSceneWorld`: [src/engine/world.rs:54](D:/3D/API/src/engine/world.rs:54)

Use this layer when you need to understand:
- how `.nuscene` text becomes engine data
- how materials, transforms, lights, camera, and physics metadata are compiled
- how the editor/runtime work from a compiled scene world instead of raw source text

Template scene example:
- material section: [scenes/explicit_template.nuscene:71](D:/3D/API/scenes/explicit_template.nuscene:71)
- `metallic = 0.0`: [scenes/explicit_template.nuscene:76](D:/3D/API/scenes/explicit_template.nuscene:76)
- commented metallic example: [scenes/explicit_template.nuscene:84](D:/3D/API/scenes/explicit_template.nuscene:84)

## Editor Layer

The editor is split between document editing and the live viewport.

Core editor document type:
- `SceneEditor`: [src/editor/mod.rs:13](D:/3D/API/src/editor/mod.rs:13)

Viewport and scene implementation:
- material inspector: [src/editor/ui.rs:3045](D:/3D/API/src/editor/ui.rs:3045)
- `BasicEditorScene` scene implementation: [src/editor/ui.rs:3251](D:/3D/API/src/editor/ui.rs:3251)
- `BasicEditorScene::config(...)`: [src/editor/ui.rs:3252](D:/3D/API/src/editor/ui.rs:3252)

The editor layer owns:
- selection, gizmos, move/deform/pivot tools
- viewport camera/orbit behavior
- scene inspector and material inspector
- light selection and light gizmos
- play mode
- undo/redo history
- screenshot button and F12 capture path

If a bug is visible only in the editor viewport, start in [src/editor/ui.rs](D:/3D/API/src/editor/ui.rs).

## FFI / C API / C++ Import Path

The C ABI lives in [src/ffi/mod.rs](D:/3D/API/src/ffi/mod.rs).

Primary exports:
- create GL-style scratch context: [src/ffi/mod.rs:741](D:/3D/API/src/ffi/mod.rs:741)
- open preview window: [src/ffi/mod.rs:768](D:/3D/API/src/ffi/mod.rs:768)
- upload buffer bytes: [src/ffi/mod.rs:931](D:/3D/API/src/ffi/mod.rs:931)
- upload RGBA8 texture bytes: [src/ffi/mod.rs:1124](D:/3D/API/src/ffi/mod.rs:1124)

Public C header:
- [include/nu_ffi.h](D:/3D/API/include/nu_ffi.h)

C++ wrapper:
- `ScratchContext`: [examples/cpp/nu_gl_scratch.hpp:54](D:/3D/API/examples/cpp/nu_gl_scratch.hpp:54)
- `RunPreviewWindow(...)`: [examples/cpp/nu_gl_scratch.hpp:111](D:/3D/API/examples/cpp/nu_gl_scratch.hpp:111)
- templated `glBufferData(...)`: [examples/cpp/nu_gl_scratch.hpp:179](D:/3D/API/examples/cpp/nu_gl_scratch.hpp:179)
- `glTexImage2D(...)`: [examples/cpp/nu_gl_scratch.hpp:202](D:/3D/API/examples/cpp/nu_gl_scratch.hpp:202)

Minecraft-style sample:
- vertex upload: [examples/cpp/minecraft_block.cpp:157](D:/3D/API/examples/cpp/minecraft_block.cpp:157)
- index upload: [examples/cpp/minecraft_block.cpp:160](D:/3D/API/examples/cpp/minecraft_block.cpp:160)
- atlas upload: [examples/cpp/minecraft_block.cpp:171](D:/3D/API/examples/cpp/minecraft_block.cpp:171)
- preview launch: [examples/cpp/minecraft_block.cpp:188](D:/3D/API/examples/cpp/minecraft_block.cpp:188)

Current FFI boundary:
- the C++ side can record OpenGL-style scratch commands
- buffer/texture byte upload now crosses the ABI
- the preview window can render the submitted block sample
- this is still not a full persistent engine-host API with non-blocking frame lifecycle control

## Event System

The typed event bus lives in [src/event/mod.rs](D:/3D/API/src/event/mod.rs).

Key type:
- `EventBus`: [src/event/mod.rs:178](D:/3D/API/src/event/mod.rs:178)

This layer owns:
- immediate and queued delivery
- typed engine events
- listener registration
- editor/runtime integration points for hot reload and scene changes

## Asset System

The handle-based asset manager lives in [src/resource/asset.rs](D:/3D/API/src/resource/asset.rs).

Key type:
- `AssetManager`: [src/resource/asset.rs:41](D:/3D/API/src/resource/asset.rs:41)

This layer owns:
- stable asset handles
- generation invalidation
- type-specific asset tracking
- scene/shader/texture/mesh registration used by the editor and hot reload paths

## Script System

The current script parser lives in [src/script/mod.rs](D:/3D/API/src/script/mod.rs).

Key parser:
- `parse_na_script(...)`: [src/script/mod.rs:23](D:/3D/API/src/script/mod.rs:23)

Current script status:
- NAScript parsing exists
- script metadata is stored in scene/editor data
- play mode uses a built-in controller path
- the stored `cpp_script` attachment is not yet a full native execution bridge

## Where To Start For Common Tasks

Add a new runtime material property:
1. define it in `MeshMaterial3D`: [src/scene/mod.rs:294](D:/3D/API/src/scene/mod.rs:294)
2. propagate it through scene/editor data in `engine` and `editor`
3. pack it in `mesh_material_vertex_params(...)`: [src/runtime/mod.rs:692](D:/3D/API/src/runtime/mod.rs:692)
4. consume it in [shaders/cube_3d.frag](D:/3D/API/shaders/cube_3d.frag)

Trace a viewport-only bug:
1. inspect editor behavior in [src/editor/ui.rs](D:/3D/API/src/editor/ui.rs)
2. inspect generated `SceneConfig` in [src/editor/ui.rs:3252](D:/3D/API/src/editor/ui.rs:3252)
3. inspect runtime handling in [src/runtime/mod.rs](D:/3D/API/src/runtime/mod.rs)

Trace a `.nuscene` parsing issue:
1. parser in [src/engine/mod.rs:917](D:/3D/API/src/engine/mod.rs:917)
2. document model in [src/engine/mod.rs:353](D:/3D/API/src/engine/mod.rs:353)
3. compiled world in [src/engine/world.rs:54](D:/3D/API/src/engine/world.rs:54)

Trace a C++ sample issue:
1. exported ABI in [src/ffi/mod.rs](D:/3D/API/src/ffi/mod.rs)
2. public declarations in [include/nu_ffi.h](D:/3D/API/include/nu_ffi.h)
3. wrapper calls in [examples/cpp/nu_gl_scratch.hpp](D:/3D/API/examples/cpp/nu_gl_scratch.hpp)
4. sample usage in [examples/cpp/minecraft_block.cpp](D:/3D/API/examples/cpp/minecraft_block.cpp)

## Current Known Boundaries

These are still incomplete or intentionally limited:
- no normal maps yet
- no tangent generation yet
- no `.mtl` import yet
- no full texture-set PBR material runtime yet
- no non-blocking persistent FFI frame host yet
- `cpp_script` storage exists but is not yet a complete native script runtime
