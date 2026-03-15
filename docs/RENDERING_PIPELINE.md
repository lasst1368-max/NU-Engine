# Rendering Pipeline

## Overview

A well-designed rendering pipeline is essential for a flexible and efficient engine.

For `nu`, the rendering pipeline needs to support:

- scene rendering
- editor rendering
- multi-pass lighting
- live shadows
- post-processing
- future techniques such as deferred lighting, Forward+, SSAO, and reflections

This document focuses on rendering architecture, not language-specific implementation.

## Implementation Context

`nu` is Vulkan-only at the renderer level.

Current binding layer note:

- Vulkan binding library in the current implementation: `ash 0.38`

That is an implementation detail, not the architecture itself.

The architecture should remain valid even if the binding layer changes.

## Pipeline Overview

A modern rendering pipeline in `nu` should be organized into the following stages:

1. Scene culling
2. Pass planning
3. Resource preparation
4. Command generation
5. GPU execution
6. Post-processing
7. Presentation

Supporting systems include:

- render graph or pass graph
- synchronization management
- pipeline cache
- transient resource allocation
- debug and profiling views

## Design Goals

The rendering pipeline must satisfy five requirements.

### Flexibility

It must support different rendering strategies without forcing a rewrite of the whole engine.

Examples:

- forward rendering
- deferred rendering
- Forward+
- shadow passes
- post-processing chains

### Performance

It must minimize unnecessary work.

Examples:

- culling invisible objects
- reducing redundant state changes
- batching compatible work
- using efficient synchronization

### Extensibility

It must be straightforward to add:

- new passes
- new material models
- new shadow techniques
- new post effects

### Maintainability

It must remain understandable under growth.

A rendering pipeline that becomes an opaque pile of ad hoc passes is not acceptable.

### Platform abstraction at the engine boundary

The engine should expose a clean rendering model to higher-level systems while keeping Vulkan-specific details inside the renderer.

## Scene Culling

Before rendering, the engine should determine what actually needs to be submitted.

This is the first major performance filter.

### Responsibilities

- reject objects outside the camera frustum
- reject invisible or disabled objects
- optionally sort or bucket visible objects for later passes
- prepare per-view visibility data

Pseudocode:

```text
system CullingSystem
    set_camera(active_camera)

    cull(scene_entities)
        visible = []
        for entity in scene_entities
            if not entity.active
                continue
            if not entity.has(Transform, MeshRenderer)
                continue
            if camera_frustum intersects entity.bounds
                visible.push(entity)
```

### Future extensions

`nu` should eventually support:

- occlusion culling
- light culling
- shadow caster culling
- editor-specific culling layers

## Render Pass Management

Modern rendering is not one pass. It is a sequence of passes with specific purposes.

Examples:

- shadow pass
- depth prepass optionally
- geometry pass
- lighting pass
- transparent pass
- UI/editor overlay pass
- post-processing pass
- final composite/present pass

Each pass should define:

- inputs
- outputs
- load/store behavior
- required pipeline state
- execution function

## Dynamic Rendering Direction

For `nu`, the preferred direction is dynamic rendering rather than a rigid old-style render pass architecture.

Why:

- simpler pass definition
- easier runtime configuration
- better fit for rapidly changing editor and post-process workflows
- fewer heavyweight compatibility objects to manage

This does not remove the need for pass planning. It only changes how the low-level render work is begun and ended.

## Render Graph

A render graph is the right long-term abstraction for `nu`.

A render graph should describe:

- resources
- passes
- dependencies
- execution order
- synchronization requirements

### What the render graph does

- tracks resource producers and consumers
- computes pass order
- inserts layout transitions and barriers
- allocates or aliases transient resources
- validates dependency correctness

### Core model

Pseudocode:

```text
resource ImageResource
    name
    format
    extent
    usage
    initial_layout
    final_layout

pass RenderPassNode
    name
    inputs
    outputs
    execute

graph RenderGraph
    resources
    passes
    compile()
    execute()
```

### Why this matters

Without a render graph, engines tend to accumulate:

- hidden pass dependencies
- redundant barriers
- duplicated transient resources
- fragile pass ordering

That is exactly the kind of rendering debt `nu` should avoid.

## Resource Preparation

Each pass depends on resources being in the correct state.

The pipeline should explicitly prepare:

- color targets
- depth targets
- sampled textures
- storage buffers
- uniform buffers
- shadow maps
- post-process inputs

This is where rendering architecture meets resource management.

The renderer should not assume resources are already usable. It should prepare or validate them through the graph and resource system.

## Command Generation

After culling and pass planning, the engine should generate GPU work.

### Responsibilities

- build draw lists
- bind pipelines and descriptors
- issue draws or dispatches
- minimize redundant state changes
- record work into command buffers

The command generation layer should operate on prepared render data, not raw scene authoring data.

That distinction matters.

The scene format is for authoring. The render submission model is for execution.

## GPU Execution

The GPU execution stage submits recorded command buffers in the correct order.

This includes:

- queue submission
- fence usage for CPU/GPU pacing
- semaphore usage for queue-to-queue ordering
- presentation synchronization

### Important point

Synchronization is not an optional cleanup detail. It is part of the rendering architecture.

A Vulkan renderer that treats synchronization as incidental will become unstable quickly.

## Synchronization Model

`nu` should treat synchronization at three levels.

### Resource-level synchronization

- image layout transitions
- buffer memory visibility
- attachment read/write hazards

### Pass-level synchronization

- pass A must complete before pass B reads its output
- shadow pass before main lighting pass
- lighting pass before post-processing

### Frame-level synchronization

- frames in flight
- swapchain image availability
- present completion

## Pipeline Barriers and Layout Transitions

At the Vulkan level, barriers and layout transitions are critical.

Architecturally, the important rule is this:

- do not scatter manual barriers across unrelated rendering code
- centralize them through pass compilation or graph execution

That keeps synchronization understandable and prevents duplicate or conflicting transitions.

## Deferred and Forward Paths

The rendering pipeline should be able to support both forward and deferred strategies.

### Deferred rendering

Useful when:

- there are many dynamic lights
- screen-space effects need structured intermediate data
- the engine wants a G-buffer-based post stack

Typical resources:

- position or depth-derived position
- normal buffer
- albedo/material buffer
- depth buffer
- final lighting buffer

### Forward or Forward+

Useful when:

- material simplicity matters
- transparency is important
- memory bandwidth should stay lower
- the engine wants a simpler baseline path

For `nu`, the correct path is not ideological. The engine should support more than one strategy where justified.

## Post-Processing

Post-processing should be treated as its own stage, not as an afterthought.

Examples:

- tone mapping
- bloom
- SSAO
- color grading
- vignette
- TAA later
- SSR later if architecture supports it well enough

A good post stack requires:

- offscreen render targets
- fullscreen or screen-space pass support
- stable resource bindings
- explicit pass ordering

## Live Shadows and Specialized Passes

The live shadow system already demonstrates why multi-pass architecture matters.

Shadow rendering is not just "part of the main pass."

It requires:

- shadow caster culling
- shadow map generation
- light-space resource setup
- sampling in later passes

This is exactly the kind of feature that validates a pass-driven architecture.

The same logic applies to:

- SSAO
- reflections
- depth prepass
- editor overlays
- debug visualizations

## Editor Rendering

`nu` has a custom editor renderer, so the pipeline must support both:

- game rendering
- editor rendering

That means the pipeline should be able to compose:

- world scene pass
- editor grid and helpers
- selection outlines
- gizmos
- physics debug overlays
- text and UI overlays

This is another reason the renderer needs explicit pass structure and resource ownership.

## Best Practices for `nu`

### Good direction

- render graph or pass graph as the orchestration model
- dynamic rendering for low-level pass execution where appropriate
- explicit synchronization
- explicit transient resource management
- scene culling before submission
- post-processing as a real pipeline stage
- separate authoring data from render submission data

### Bad direction

- hard-coded one-off pass ordering spread across the codebase
- manual ad hoc barriers everywhere
- mixing editor overlay logic directly into scene lighting logic
- treating pipelines, resources, and passes as one giant implicit state machine
- assuming one rendering strategy will solve every future feature

## Recommended Direction for the Next Phases

1. Formalize a render graph for runtime and editor rendering
2. Move all shadow, lighting, and post-processing passes under that graph
3. Add debug views for pass outputs and intermediate resources
4. Add SSAO on top of the new post-process path
5. Add a second renderer path such as deferred or Forward+ where justified
6. Keep Vulkan synchronization and resource transitions graph-driven

## Conclusion

A good rendering pipeline in `nu` should be:

- pass-driven
- graph-coordinated
- explicit in synchronization
- compatible with dynamic rendering
- extensible enough for modern lighting and post-processing
- shared between game and editor rendering where possible

That is the right foundation for a serious Vulkan-first engine.
