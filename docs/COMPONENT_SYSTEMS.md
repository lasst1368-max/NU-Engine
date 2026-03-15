# Component Systems

## Overview

The previous architecture notes established that `nu` should use component-based architecture as the world-facing model for the engine.

This document goes one level deeper and focuses on how component systems should be structured.

The goal is not to mimic a specific language or framework. The goal is to define a component model that is:

- flexible
- testable
- performant
- editor-friendly
- compatible with a Vulkan-first engine

## The Problem with Deep Inheritance

A traditional object hierarchy often grows into something like this:

```text
GameObject
  -> PhysicalObject
    -> Character
      -> Player
      -> Enemy
        -> FlyingEnemy
```

That model breaks down quickly.

### Problems

- New behavior combinations require new subclasses
- Similar logic gets duplicated across branches
- Base classes accumulate unrelated responsibilities
- Refactoring becomes risky because changes propagate too widely
- Editor tooling becomes harder because object capabilities are hidden inside class trees

This is exactly the kind of rigidity `nu` should avoid.

## Component-Based Design Principles

A component system replaces inheritance-heavy object design with composition.

### Core principles

- Single responsibility
  - each component owns one focused concern
- Encapsulation
  - component state stays inside the component
- Loose coupling
  - components should not depend directly on many other components
- Reusability
  - a component should be usable across many entity types
- Explicit ownership
  - entity ownership and system ownership should be clear

## Core Model

The engine should be built around three concepts:

- Entities
- Components
- Systems

### Entities

Entities are identifiers or lightweight containers.

They are not meant to contain large amounts of behavior.

Their purpose is to group components into one world object.

### Components

Components contain state or narrow behavior.

Examples for `nu`:

- Transform
- MeshRenderer
- MaterialInstance
- Camera
- DirectionalLight
- PointLight
- PhysicsBody
- Collider
- ScriptInstance
- AudioSource
- EditorSelection

### Systems

Systems operate on entities that contain the required component set.

Examples for `nu`:

- Render system
- Lighting system
- Physics system
- Animation system
- Scripting system
- Audio system
- Editor interaction system

## Basic Component Model

This is engine-oriented pseudocode, not Rust-specific code.

```text
component BaseComponent
    owner: EntityHandle
    state: ComponentState

    on_initialize()
    on_shutdown()

entity Entity
    id: EntityHandle
    name: String
    active: Bool
    components: ComponentStore

    add_component(type, data)
    get_component(type)
    remove_component(type)

system World
    update(delta_time)
    render(frame_context)
```

This model matters because it keeps responsibilities separated:

- the entity is a container
- the component stores domain state
- the system performs the work

## Common Components

### Transform

A transform component stores world or local spatial state.

Typical fields:

- position
- rotation
- scale
- cached transform matrix
- dirty flag

The transform component should support caching because transform recomputation is common and expensive when repeated unnecessarily.

### Mesh Renderer

A mesh renderer component should not contain rendering code in the object-oriented sense.

It should contain references such as:

- mesh handle
- material handle
- visibility flags
- render layer
- shadow casting flags
- shadow receiving flags

The actual draw work belongs in the render system.

### Camera

A camera component should define viewing state such as:

- projection mode
- field of view
- aspect ratio
- near/far planes
- exposure or debug overrides

The camera system or renderer should derive view and projection matrices from it.

### Light

A light component should contain light data, not shadow rendering logic.

Examples:

- light type
- color
- intensity
- range
- direction
- shadow settings

Shadow map allocation, atlases, and render passes remain renderer responsibilities.

### Physics Body and Collider

These should be separate components.

That separation is useful because:

- a collider is not always dynamic
- some objects need collision without rigid-body response
- body and shape data evolve at different rates

## Communication Between Components

Direct component-to-component lookups are easy, but they create tight coupling.

Example of the naive approach:

```text
mesh_renderer.update()
    transform = owner.get_component(Transform)
    if transform exists
        use transform
```

That is acceptable in a prototype. It is not ideal as a long-term design.

### Why direct lookups become a problem

- harder to unit test
- hidden dependencies
- systems become less predictable
- component reuse declines

## Preferred Communication Model

`nu` should prefer system-driven coordination and events over direct component coupling.

### Better pattern

- systems query required components
- systems publish events when state changes matter to others
- components remain mostly passive data containers

Examples:

- physics system publishes collision events
- editor selection system publishes selection change events
- asset reload system publishes resource invalidation events
- scripting system reacts to those events at a higher layer

## Event System

An event system is useful when systems need to react without hard references.

Pseudocode:

```text
event CollisionEvent
    entity_a
    entity_b

event ResourceReloadedEvent
    resource_id
    resource_type

service EventBus
    subscribe(event_type, listener)
    unsubscribe(event_type, listener)
    dispatch(event)
```

### Benefits

- reduces direct dependencies
- improves testability
- keeps systems modular
- works well with editor/runtime integration

### Constraint

Do not turn events into an excuse for hidden global behavior.

Events should be explicit, scoped, and observable.

## Component Lifecycle

Lifecycle handling needs to be explicit.

A robust component model should define states such as:

- Uninitialized
- Initializing
- Active
- Destroying
- Destroyed

That matters because real engines need predictable order for:

- resource allocation
- registration with systems
- event subscription
- shutdown and cleanup

Pseudocode:

```text
component BaseComponent
    state: ComponentState

    initialize()
    destroy()
    is_active()
```

The engine, not arbitrary user code, should control lifecycle transitions.

## Optimizing Component Access

A naive `get_component(Type)` lookup based on dynamic casting or string matching does not scale well.

The engine should move toward stable component type identifiers and indexed storage.

### Baseline improvement

- assign a unique type ID per component type
- store a fast lookup table from component type ID to component instance

Pseudocode:

```text
component_type_id(Transform) -> 1
component_type_id(MeshRenderer) -> 2
component_type_id(Camera) -> 3

entity.component_map[type_id] -> component pointer or handle
```

### Why this matters

- faster lookup
- simpler system queries
- easier migration toward packed storage later

## Runtime Direction for `nu`

`nu` should not stop at an object-style component container.

That is only the first useful step.

The long-term runtime direction should be:

1. author scenes in `.nuscene`
2. load them into runtime entities/components
3. group hot-path components into packed system-owned storage
4. let systems process those packed datasets directly

That means `nu` can have both:

- a friendly authoring model
- a data-oriented execution model

## Practical Rules for `nu`

### Good uses of components

- transforms
- mesh/material assignment
- light parameters
- camera data
- physics bodies and colliders
- script bindings
- editor metadata tied to world objects

### Bad uses of components

Do not force engine services into entity components.

These should stay as engine systems or services:

- Vulkan device state
- swapchain management
- pipeline compiler/cache
- descriptor allocators
- shader hot reload service
- asset registry
- file watcher

Those are engine facilities, not world object state.

## Testing Guidance

A good component system is testable in isolation.

That means:

- component logic should not rely on deep global state
- systems should accept explicit dependencies
- events should be inspectable
- lifecycle transitions should be deterministic

This is one of the strongest reasons to avoid heavy direct coupling between components.

## Recommended Direction for the Next Steps

1. Add a real entity/component scene runtime on top of `.nuscene`
2. Move render submission to system-driven component queries
3. Move physics bindings onto runtime entities instead of mesh-only metadata
4. Add event-driven editor/runtime synchronization
5. Gradually migrate hot-path components into packed system-owned storage

## Conclusion

A good component system for `nu` should not be treated as an inheritance replacement only.

It should be treated as the engine's world model.

That world model needs to be:

- composition-first
- system-driven
- explicit in lifecycle
- testable
- compatible with data-oriented runtime optimization

That is the right path for a Vulkan-first engine that also wants a serious editor, live rendering features, physics, and long-term maintainability.
