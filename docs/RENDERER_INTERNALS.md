# Renderer Internals

This document maps the active 3D render path in `nu` to the files and lines that implement it.

It is not a theory document. It is a navigation file for:
- frame execution
- uniform packing
- descriptor layout
- shader input/output layout
- screenshot capture
- current lighting and shadow boundaries

## Entry Point

The runtime scene loop starts at:
- `run_scene(...)`: [src/runtime/mod.rs:75](D:/3D/API/src/runtime/mod.rs:75)

That function drives the app shell and eventually calls into the renderer frame path.

The main per-frame render call is:
- `draw_frame(...)`: [src/runtime/mod.rs:1488](D:/3D/API/src/runtime/mod.rs:1488)

## Frame Data Path

The high-level data flow is:
1. A `Scene` implementation fills a `SceneFrame`.
2. The runtime turns mesh draws into internal batches.
3. The runtime builds `CubeSceneUniforms`.
4. The runtime uploads object buffers and uniform buffers.
5. The 3D pipeline draws the scene.
6. If requested, the runtime copies the swapchain image to a PNG capture path.

Relevant types and functions:
- `SceneFrame`: [src/scene/mod.rs:421](D:/3D/API/src/scene/mod.rs:421)
- internal batch type `MeshDrawBatch3D`: [src/runtime/mod.rs:227](D:/3D/API/src/runtime/mod.rs:227)
- scene uniform block `CubeSceneUniforms`: [src/runtime/mod.rs:241](D:/3D/API/src/runtime/mod.rs:241)
- scene uniform upload: [src/runtime/mod.rs:2179](D:/3D/API/src/runtime/mod.rs:2179)

## Lighting Uniform Layout

The scene uniform block currently carries:
- point light positions
- point light colors
- point light shadow flags
- directional/fill light direction and intensity
- ambient terms
- shadow matrix and shadow tuning values

The runtime-side uniform data is assembled around:
- point light array initialization: [src/runtime/mod.rs:585](D:/3D/API/src/runtime/mod.rs:585)
- point light packing: [src/runtime/mod.rs:590](D:/3D/API/src/runtime/mod.rs:590)
- final `CubeSceneUniforms` construction: [src/runtime/mod.rs:624](D:/3D/API/src/runtime/mod.rs:624)

Lighting config source:
- `MAX_POINT_LIGHTS`: [src/lighting/mod.rs:1](D:/3D/API/src/lighting/mod.rs:1)
- `LightingConfig`: [src/lighting/mod.rs:80](D:/3D/API/src/lighting/mod.rs:80)

Current limit:
- fixed forward array of `4` point lights
- separate directional/fill path

## Material Packing

Per-draw material data is packed here:
- `mesh_material_vertex_params(...)`: [src/runtime/mod.rs:692](D:/3D/API/src/runtime/mod.rs:692)

Current packed material parameters:
- `x = roughness`
- `y = metallic`
- remaining lanes reserved

Material source type:
- `MeshMaterial3D`: [src/scene/mod.rs:294](D:/3D/API/src/scene/mod.rs:294)

Current material boundary:
- PBR core is present
- normal maps are not present
- tangent space is not present
- full texture-set binding is not complete yet

## Descriptor Set Layout

The main scene descriptor set layout is created here:
- `create_cube_descriptor_set_layout(...)`: [src/runtime/mod.rs:4484](D:/3D/API/src/runtime/mod.rs:4484)

The shader-side layout in the active 3D path is:

Set `0`
- binding `0`: scene uniform block in [shaders/cube_3d.frag:3](D:/3D/API/shaders/cube_3d.frag:3)
- binding `1`: object buffer in [shaders/cube_3d.vert:22](D:/3D/API/shaders/cube_3d.vert:22) and [shaders/cube_3d.frag:24](D:/3D/API/shaders/cube_3d.frag:24)

Set `1`
- binding `0`: albedo texture sampler in [shaders/cube_3d.frag:28](D:/3D/API/shaders/cube_3d.frag:28)

Set `2`
- binding `0`: shadow map sampler in [shaders/cube_3d.frag:29](D:/3D/API/shaders/cube_3d.frag:29)

Material texture descriptor creation/use:
- `ensure_material_descriptor_set_3d(...)`: [src/runtime/mod.rs:1942](D:/3D/API/src/runtime/mod.rs:1942)
- descriptor use during draw: [src/runtime/mod.rs:1861](D:/3D/API/src/runtime/mod.rs:1861)

## Vertex Layout

The active 3D vertex shader consumes:
- position: [shaders/cube_3d.vert:3](D:/3D/API/shaders/cube_3d.vert:3)
- normal: [shaders/cube_3d.vert:4](D:/3D/API/shaders/cube_3d.vert:4)
- uv: [shaders/cube_3d.vert:5](D:/3D/API/shaders/cube_3d.vert:5)
- albedo: [shaders/cube_3d.vert:6](D:/3D/API/shaders/cube_3d.vert:6)
- object index: [shaders/cube_3d.vert:7](D:/3D/API/shaders/cube_3d.vert:7)
- packed material params: [shaders/cube_3d.vert:8](D:/3D/API/shaders/cube_3d.vert:8)

Vertex outputs passed into the fragment shader:
- world position: [shaders/cube_3d.vert:26](D:/3D/API/shaders/cube_3d.vert:26)
- world normal: [shaders/cube_3d.vert:27](D:/3D/API/shaders/cube_3d.vert:27)
- UV: [shaders/cube_3d.vert:28](D:/3D/API/shaders/cube_3d.vert:28)
- albedo: [shaders/cube_3d.vert:29](D:/3D/API/shaders/cube_3d.vert:29)
- object index: [shaders/cube_3d.vert:30](D:/3D/API/shaders/cube_3d.vert:30)
- packed material params: [shaders/cube_3d.vert:31](D:/3D/API/shaders/cube_3d.vert:31)

## PBR Shader Path

The fragment shader is:
- [shaders/cube_3d.frag](D:/3D/API/shaders/cube_3d.frag)

Important bindings and functions:
- scene uniforms: [shaders/cube_3d.frag:3](D:/3D/API/shaders/cube_3d.frag:3)
- albedo texture: [shaders/cube_3d.frag:28](D:/3D/API/shaders/cube_3d.frag:28)
- shadow map: [shaders/cube_3d.frag:29](D:/3D/API/shaders/cube_3d.frag:29)
- `geometry_schlick_ggx(...)`: [shaders/cube_3d.frag:272](D:/3D/API/shaders/cube_3d.frag:272)
- `geometry_smith(...)`: [shaders/cube_3d.frag:278](D:/3D/API/shaders/cube_3d.frag:278)
- `fresnel_schlick(...)`: [shaders/cube_3d.frag:283](D:/3D/API/shaders/cube_3d.frag:283)
- roughness read: [shaders/cube_3d.frag:298](D:/3D/API/shaders/cube_3d.frag:298)
- metallic read: [shaders/cube_3d.frag:299](D:/3D/API/shaders/cube_3d.frag:299)

Point-light accumulation loop:
- point light vector read: [shaders/cube_3d.frag:328](D:/3D/API/shaders/cube_3d.frag:328)
- range read: [shaders/cube_3d.frag:336](D:/3D/API/shaders/cube_3d.frag:336)

Current lighting model:
- GGX-style direct lighting
- directional/fill light shadow map
- point-light shadow approximation through the scene object buffer
- AO/contact darkening
- simple ambient/fake GI terms

## Object Buffer

Object/world transform data is exposed to the 3D shaders through the object buffer:
- vertex shader object buffer binding: [shaders/cube_3d.vert:22](D:/3D/API/shaders/cube_3d.vert:22)
- fragment shader object buffer binding: [shaders/cube_3d.frag:24](D:/3D/API/shaders/cube_3d.frag:24)

This buffer is also used by:
- point-light shadow testing
- AO approximation
- per-object lookup from `draw_object_index`

## Screenshots

Screenshot support is part of the runtime frame path.

Editor-facing screenshot request source:
- `SceneConfig`: [src/scene/mod.rs:10](D:/3D/API/src/scene/mod.rs:10)

Runtime capture happens from the swapchain image in the renderer path after drawing.
If you are tracing screenshot behavior, start with:
- frame draw path: [src/runtime/mod.rs:1488](D:/3D/API/src/runtime/mod.rs:1488)

## Editor Integration

The editor scene implementation that feeds the runtime is:
- `BasicEditorScene`: [src/editor/ui.rs:3251](D:/3D/API/src/editor/ui.rs:3251)
- `config(...)`: [src/editor/ui.rs:3252](D:/3D/API/src/editor/ui.rs:3252)

The material inspector that edits roughness and metallic is:
- [src/editor/ui.rs:3045](D:/3D/API/src/editor/ui.rs:3045)

## Where To Start For Common Renderer Changes

Add a new per-material scalar:
1. add it to `MeshMaterial3D`: [src/scene/mod.rs:294](D:/3D/API/src/scene/mod.rs:294)
2. propagate it through engine/editor scene data
3. pack it in [src/runtime/mod.rs:692](D:/3D/API/src/runtime/mod.rs:692)
4. consume it in [shaders/cube_3d.frag](D:/3D/API/shaders/cube_3d.frag)

Trace a lighting bug:
1. inspect `LightingConfig`: [src/lighting/mod.rs:80](D:/3D/API/src/lighting/mod.rs:80)
2. inspect `CubeSceneUniforms`: [src/runtime/mod.rs:241](D:/3D/API/src/runtime/mod.rs:241)
3. inspect point light packing: [src/runtime/mod.rs:590](D:/3D/API/src/runtime/mod.rs:590)
4. inspect shader logic in [shaders/cube_3d.frag](D:/3D/API/shaders/cube_3d.frag)

Trace a material texture bug:
1. inspect descriptor creation/use at [src/runtime/mod.rs:1942](D:/3D/API/src/runtime/mod.rs:1942)
2. inspect the sampler binding at [shaders/cube_3d.frag:28](D:/3D/API/shaders/cube_3d.frag:28)

## Current Boundaries

These are still incomplete:
- no normal-map descriptor path
- no tangent generation
- no `.mtl` material import
- no deferred renderer or render graph-backed post stack yet
- fixed `MAX_POINT_LIGHTS = 4`
- FFI preview host is still separate from a full persistent engine host API
