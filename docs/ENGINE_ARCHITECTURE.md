# Engine Architecture

## Architectural Patterns

This document outlines the architectural patterns used in `nu`.

`nu` is a Vulkan-powered engine. The renderer is Vulkan-only. Higher-level authoring syntax such as OpenGL-style compatibility is translated into the same Vulkan runtime.

While `nu` currently emphasizes rendering, editor tooling, scene management, and physics, the architectural choices here are meant to scale into a full engine.

## Overview

The engine uses a hybrid architecture:

- Layered architecture for system boundaries
- Component-based architecture for world composition
- Data-oriented design for performance-critical runtime paths
- Service-style registries for shared engine facilities

These are not competing choices. They solve different problems.

Related documents:

- [COMPONENT_SYSTEMS.md](COMPONENT_SYSTEMS.md)
- [RESOURCE_MANAGEMENT.md](RESOURCE_MANAGEMENT.md)
- [RENDERING_PIPELINE.md](RENDERING_PIPELINE.md)
- [EVENT_SYSTEMS.md](EVENT_SYSTEMS.md)
- [API_MAP.md](API_MAP.md)
- [RENDERER_INTERNALS.md](RENDERER_INTERNALS.md)
- [FFI_USAGE.md](FFI_USAGE.md)

## Layered Architecture

Layered architecture divides the engine into clear levels of responsibility.

Typical layers in `nu`:

1. Platform layer
   - windowing
   - input
   - file watching
   - OS integration
2. Core engine layer
   - scene loading
   - hot reload
   - asset management
   - configuration
3. Runtime systems layer
   - rendering
   - physics
   - editor runtime
   - scene playback
4. Application layer
   - game code
   - tools
   - editor workflows
   - demos

### Benefits

- Clear separation of concerns
- Easier maintenance
- Lower coupling between subsystems
- Safer replacement of internals over time

## Data-Oriented Design

Data-oriented design matters anywhere the engine processes large amounts of similar data.

In `nu`, this applies especially to:

- draw submission
- mesh batching
- pipeline compilation inputs
- collision checks
- scene playback
- hot reload dependency tracking

The rule is simple:

- organize code for clarity at the system level
- organize data for throughput at the runtime level

### Benefits

- Better cache locality
- Fewer scattered allocations
- Easier batching
- Better parallelization potential

## Service-Style Engine Facilities

Some engine systems need to be broadly accessible without turning everything into global state.

Examples:

- asset registry
- shader compiler/reload service
- pipeline cache
- physics world registry
- editor state bridge

`nu` should use explicit engine-owned services or registries, not uncontrolled globals.

### Benefits

- Decouples consumers from implementations
- Keeps system ownership explicit
- Makes testing and replacement easier

## Component-Based Architecture

Component-based architecture is the foundation for scene and gameplay-facing engine structure.

For deeper implementation guidance, see [COMPONENT_SYSTEMS.md](COMPONENT_SYSTEMS.md).

### Core Concepts

- Entities are identifiers or containers representing objects in the world
- Components store independent aspects of behavior or state
- Systems process entities that have the required components

Typical components in `nu` will include:

- Transform
- MeshRenderer
- Material
- Camera
- Light
- PhysicsBody
- Collider
- ScriptBehaviour
- AudioSource

Typical systems in `nu` will include:

- Render system
- Physics system
- Animation system
- Audio system
- Scripting system
- Editor selection/manipulation system

### Why Component Architecture Fits `nu`

1. Graphics flexibility
   - rendering features should be composable, not locked into inheritance trees
2. Editor friendliness
   - components map naturally to inspector panels and scene editing
3. Runtime extensibility
   - new systems can be introduced without rewriting object hierarchies
4. Better fit for Vulkan-era engines
   - Vulkan benefits from explicit ownership, explicit state, and predictable data flow
5. Compatibility with data-oriented optimization
   - component storage can evolve from simple containers to packed runtime data

## Base Direction for `nu`

The engine should use component-based architecture as the world model, layered architecture as the system boundary model, and data-oriented execution inside hot paths.

That means:

- world objects are assembled from components
- systems operate on component sets
- rendering and physics internals stay explicit and performance-conscious
- editor and runtime share the same scene model where possible

## Engine-Oriented Example

This example is intentionally engine-agnostic pseudocode, not Rust-specific code.

```text
component Transform
    position: Vec3
    rotation: Quaternion
    scale: Vec3

component MeshRenderer
    mesh: MeshHandle
    material: MaterialHandle

component Light
    kind: LightKind
    color: Vec3
    intensity: Float

entity CreateCube()
    add Transform
    add MeshRenderer

system RenderSystem
    for each entity with Transform + MeshRenderer
        submit mesh with transform and material

system LightSystem
    for each entity with Transform + Light
        publish light data to renderer
```

The point is composition:

- a visible object is not a subclass of a renderable base class
- it is an entity with a transform and a mesh renderer
- adding physics means adding `PhysicsBody` and `Collider`
- adding gameplay means attaching more components, not rewriting hierarchy roots

## Practical Guidance for `nu`

### Use components for scene-facing state

Good candidates:

- transforms
- mesh/material bindings
- lights
- cameras
- colliders
- rigid bodies
- editor metadata

### Do not force everything into components

Some systems are better represented as engine services or runtime managers:

- shader compiler
- pipeline cache
- swapchain runtime
- hot reload manager
- asset dependency graph

These are engine facilities, not world components.

### Keep the renderer explicit

Even with component architecture, the renderer should remain explicit internally:

- render graph or pass graph
- resource lifetime tracking
- pipeline cache
- descriptor management
- shadow systems
- post-processing

Component architecture is not a substitute for render architecture.

## Recommended Direction for the Next Phases

1. Introduce an entity/component scene runtime separate from file parsing
2. Keep `.nuscene` as the authoring format, but compile/load it into runtime scene data
3. Move renderable objects to component-driven scene submission
4. Move physics bindings onto the same entity/component scene model
5. Keep Vulkan runtime systems explicit and low-level under the component layer

## Conclusion

`nu` should not choose one pattern dogmatically.

The correct architecture is:

- layered at the engine boundary
- component-based at the world level
- data-oriented in performance-critical execution
- explicit in Vulkan rendering internals

That combination gives `nu` the best path toward:

- a credible Vulkan engine
- a usable custom editor
- scalable rendering features
- future gameplay systems without architectural debt
