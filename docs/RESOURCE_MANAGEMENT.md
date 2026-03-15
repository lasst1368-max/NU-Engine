# Resource Management

## Overview

Efficient resource management is a core engine responsibility.

For `nu`, resource management has to serve both runtime and tooling. That means the system must support:

- loading and unloading
- caching
- stable handles
- hot reloading
- asynchronous work
- streaming
- memory tracking
- GPU upload and residency decisions

This document describes the architectural direction for resource management in `nu`.

It is intentionally engine-focused and language-agnostic.

## Core Challenges

A real engine resource system has to solve several problems at once.

### Loading and unloading

Resources must be created from disk or generated data, then released when no longer needed.

### Caching

The same texture, mesh, or shader should not be loaded repeatedly if it is already resident.

### Stable referencing

World objects, renderer state, and editor tools need a safe way to refer to resources without holding raw pointers.

### Hot reloading

During development, shaders, textures, materials, and scene-linked assets need to reload while the engine is running.

### Streaming

Large textures, meshes, and future scene content should load incrementally or asynchronously without stalling the frame.

### Memory management

The engine must track both system memory and GPU memory, including transient upload resources.

## Resource Handles

The correct baseline is to expose handles, not raw resource pointers.

A handle should represent identity, not memory location.

Pseudocode:

```text
handle ResourceHandle<T>
    resource_id
    generation
    manager_reference

    get()
    is_valid()
```

### Why handles matter

- resources can move internally without breaking references
- handles can be validated before use
- the manager keeps ownership and lifetime control
- stale references can be detected with generation checks

For `nu`, this fits naturally with the engine’s existing handle-oriented renderer and registry direction.

## Base Resource Model

Every resource type should follow a common contract.

Pseudocode:

```text
resource BaseResource
    id
    state

    load()
    unload()
    reload()
```

Useful states include:

- Unloaded
- Loading
- Loaded
- Failed
- Reloading
- Evicted

The engine should distinguish clearly between:

- CPU-side resource state
- GPU-side residency state

Those are related, but not the same.

## Resource Categories in `nu`

The resource system should manage at least these categories:

- Textures
- Meshes
- Materials
- Shader modules
- Shader programs or pipeline definitions
- Fonts
- Scene documents
- Audio assets later
- Physics collision data later

Not every category should share identical loading logic, but all should share the same lifecycle model.

## Resource Manager Structure

The engine should use a central resource manager with type-aware storage and explicit registries.

Pseudocode:

```text
service ResourceManager
    resources_by_type
    metadata_by_type
    ref_counts
    residency_info

    load(type, id)
    get(type, id)
    has(type, id)
    release(type, id)
    reload(type, id)
    unload_all()
```

### Design goals

- fast lookup by type and ID
- explicit ownership
- predictable lifetime rules
- safe invalidation on reload
- room for async and streaming extensions

## Caching

Caching is required. Reloading the same asset repeatedly is not acceptable.

The simplest useful policy is:

- first load populates cache
- future requests return the cached resource handle
- reference count or retention policy prevents premature unload

For `nu`, caching should exist at multiple levels:

- source asset cache
- parsed asset cache
- GPU resource cache
- pipeline cache

These are related, but should not be collapsed into one undifferentiated map.

## Reference Tracking

The engine needs application-level lifetime tracking in addition to raw memory ownership.

That is because a resource may still be worth keeping resident even if no current world object references it directly.

Examples:

- editor preview repeatedly uses the same mesh
- a scene swap may come back immediately
- a shader is part of a hot reload loop

So `nu` should distinguish between:

- strong ownership
- live references
- cache residency policy

Reference counting is a valid baseline, but it should not be the only policy.

## Type-Safe Access

Resource retrieval should be type-safe.

Pseudocode:

```text
texture = resource_manager.get(Texture, "brick")
mesh = resource_manager.get(Mesh, "cube")
shader = resource_manager.get(ShaderModule, "lit.frag")
```

Type-aware storage prevents collisions such as:

- a texture called `crate`
- a mesh called `crate`

Those should be separate resources, not one namespace conflict.

## Hot Reloading

Hot reload is a first-class feature in `nu`, not a bolt-on.

A good hot reload system should:

- watch source files
- detect dependency relationships
- invalidate affected resources
- rebuild only what changed
- notify dependent systems explicitly

Examples:

- changing a fragment shader invalidates material or pipeline state
- changing a texture invalidates sampled GPU resources and editor previews
- changing a mesh invalidates bounds, GPU buffers, and scene previews

### Important rule

Hot reload should not directly mutate arbitrary runtime state invisibly.

It should:

- reload the asset
- publish explicit invalidation/reload events
- let systems rebuild their own dependent state predictably

## Asynchronous Loading

The engine should support asynchronous resource loading wherever blocking would hurt frame pacing.

Good candidates:

- large textures
- imported meshes
- scene bundles
- shader compilation jobs

A reasonable architecture is:

- worker thread or task system loads/parses CPU-side asset data
- main thread or render thread performs required GPU submission and final swap-in

This separation is especially important in a Vulkan engine, because many GPU object creation and command submission paths need controlled synchronization.

## Streaming

Streaming should be understood as staged transfer of resource data, not just "loading later."

Examples:

- load lower mip levels first
- load simplified mesh first, then higher detail
- upload chunks gradually instead of stalling a frame
- page resources in and out based on visibility or editor focus

For `nu`, the most realistic early streaming targets are:

- texture mip streaming
- mesh LOD streaming
- chunked terrain or voxel mesh upload later

## GPU Memory Management

A Vulkan-first engine cannot treat GPU allocation as an afterthought.

The resource layer must work with:

- image creation
- buffer creation
- staging buffers
- upload scheduling
- memory reuse
- destruction ordering

This means the resource manager should cooperate with lower-level GPU allocators rather than owning all allocation logic itself.

The split should be:

- resource manager owns asset identity and lifecycle
- GPU allocator owns memory strategy
- renderer owns pass-time binding and usage

## Resource-Specific Notes

### Textures

Texture resources typically need:

- decoded source data or direct compressed data
- image format metadata
- mip information
- image view and sampler bindings
- streaming or residency policy later

### Meshes

Mesh resources typically need:

- CPU-side vertex/index data during load
- GPU vertex/index buffers
- bounds
- optional LODs
- optional collision or acceleration data later

### Shaders and pipelines

Shader source and GPU pipeline state should not be treated as the same thing.

A useful split is:

- shader module resource
- shader program or material definition resource
- pipeline cache entry derived from those plus render state

That separation matters because Vulkan pipeline lifetime and compatibility rules are stricter than typical high-level engine abstractions.

### Materials

Materials should reference other resources through handles:

- shader program handle
- texture handles
- parameter block

A material should not duplicate texture ownership.

## Failure Handling

Resource systems must fail predictably.

If loading fails:

- return an invalid handle or explicit failure result
- keep the cache coherent
- avoid partially initialized live resources
- prefer fallback resources where appropriate

Examples:

- missing texture -> use fallback white or checker texture
- missing shader -> use fallback debug shader
- missing mesh -> use fallback cube or placeholder mesh

This keeps the engine and editor usable even under asset failure.

## Editor and Runtime Integration

A usable engine editor depends on the same resource system as the runtime.

That means:

- the editor should not have a separate ad hoc asset path
- editor previews should resolve through the same resource manager
- hot reload should update both runtime and editor views
- resource state should be inspectable for tooling

This is especially important for `nu`, because its custom editor is part of the engine, not a separate UI stack glued on top.

## Recommended Direction for `nu`

1. Keep handles as the public-facing reference model
2. Split source asset caching from GPU residency tracking
3. Treat hot reload as dependency invalidation plus explicit rebuild events
4. Add async CPU-side loading with controlled render-thread GPU upload
5. Introduce streaming policies after the base cache and upload model are stable
6. Keep pipeline cache separate from general asset cache

## Practical Rules

### Good design choices

- stable typed handles
- explicit resource states
- dependency tracking
- fallback resources
- renderer-aware GPU upload flow
- editor/runtime sharing the same resource backend

### Bad design choices

- raw global pointers everywhere
- hidden implicit reloads with no invalidation model
- mixing source assets and GPU objects into one undifferentiated blob
- forcing Vulkan memory management decisions into gameplay-facing code
- making the editor use a different asset path than the runtime

## Conclusion

A good resource system in `nu` should do more than load files.

It should provide:

- stable handles
- caching
- controlled lifetime
- hot reload
- async loading
- streaming support
- explicit integration with Vulkan memory and upload workflows

That is the correct foundation for a serious renderer, a useful editor, and eventually a full engine.
