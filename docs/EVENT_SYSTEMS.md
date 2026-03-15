# Event Systems

## Overview

An event system gives engine subsystems a way to communicate without creating unnecessary direct dependencies.

For `nu`, this matters across both runtime and tooling.

Examples:

- input notifying gameplay or camera control
- physics notifying audio or gameplay logic
- resource management notifying the renderer about reloads
- editor tools notifying runtime views about selection or transform changes

This document describes the architectural direction for event systems in `nu`.

It is intentionally engine-focused and language-agnostic.

## Implementation Context

`nu` is Vulkan-only at the renderer level.

Current implementation note:

- Vulkan binding library in the current codebase: `ash 0.38`

That is only context. It should not shape the event architecture itself.

## Why Event Systems Matter

Without an event system, subsystems tend to accumulate direct references to each other.

That creates:

- tighter coupling
- harder testing
- less predictable ownership
- more fragile engine growth

A good event system helps the engine stay modular while still allowing information to move where it needs to go.

## Design Goals

An event system in `nu` should satisfy these goals.

### Decoupling

Producers should not need to know the concrete implementation of consumers.

### Type safety

The engine should be able to distinguish event kinds clearly and avoid ambiguous payload handling.

### Performance

High-frequency event paths must avoid unnecessary overhead.

### Flexibility

The system should support more than one delivery pattern.

### Debuggability

It should be possible to inspect event flow, especially in the editor and during hot reload work.

## Core Model

A useful event architecture has four parts:

- event type
- event bus or dispatcher
- listeners or subscribers
- delivery mode

Pseudocode:

```text
event BaseEvent
    event_type
    timestamp

service EventBus
    subscribe(event_type, listener)
    unsubscribe(event_type, listener)
    publish(event)
    queue(event)
    process_queued_events()
```

The important part is not the exact syntax.

The important part is that producers publish facts, and subscribers react to them.

## Event Design Principles

### Events should describe facts

An event should describe something that happened, not contain executable behavior.

Good examples:

- WindowResized
- KeyPressed
- CollisionStarted
- ResourceReloaded
- SelectionChanged

Bad examples:

- DoLightingNow
- ApplyPhysicsImmediately
- CallRendererAndFixThis

Events communicate state changes or occurrences. They should not become a disguised command system unless that is explicitly what the engine intends.

### Event payloads should stay focused

Payloads should contain the data needed for subscribers to react.

Examples:

- new width and height for a resize event
- key code and repeat state for an input event
- entity handles for a collision event
- resource handle and resource kind for a reload event

Keep event payloads compact. Very large payloads create avoidable copying and debugging problems.

## Event Types

The engine should define events as explicit, named types.

Examples likely needed in `nu`:

- WindowResized
- KeyPressed
- KeyReleased
- MouseMoved
- MouseButtonPressed
- MouseButtonReleased
- CollisionStarted
- CollisionEnded
- ResourceLoaded
- ResourceReloaded
- ResourceFailed
- SceneLoaded
- EntitySelected
- EntityTransformed
- PlayModeChanged

A typed event model is preferable to an unstructured string-based bus.

## Dispatch Model

There are two primary delivery modes the engine should support.

### Immediate dispatch

Immediate dispatch sends the event to listeners as soon as it is published.

Good for:

- input reactions
- immediate editor UI updates
- narrow synchronous state changes

Advantages:

- low latency
- simple mental model

Risks:

- harder to reason about ordering in large systems
- easier to create hidden call chains

### Queued dispatch

Queued dispatch stores events and processes them at a defined point later.

Good for:

- frame-stable gameplay events
- cross-system update coordination
- resource reload processing
- editor/runtime synchronization

Advantages:

- deterministic processing phase
- easier to reason about per-frame order
- safer for multithreaded producers

Risks:

- slightly higher latency
- requires explicit processing points

## Recommended Baseline for `nu`

`nu` should support both immediate and queued events.

The default guidance should be:

- use immediate dispatch for local low-latency UI and input reactions
- use queued dispatch for engine-level cross-system events

That gives the engine flexibility without forcing every event through one model.

## Event Bus

The event bus should be a scoped engine service, not an uncontrolled global.

### Responsibilities

- listener registration
- listener removal
- event publication
- event queueing
- queued event processing
- optional debug tracing

Pseudocode:

```text
service EventBus
    listeners_by_type
    queued_events

    subscribe(type, listener)
    unsubscribe(type, listener)
    publish_immediate(event)
    enqueue(event)
    flush()
```

## Filtering

Filtering is useful once the engine grows past trivial event counts.

Listeners should be able to subscribe narrowly.

Examples of filter dimensions:

- event type
- event category
- world or scene scope
- editor-only vs runtime-only
- entity scope later where appropriate

This matters because a large engine should not broadcast every event to every listener.

## Event Categories

Categories are useful as a secondary organizational layer.

Examples:

- Application
- Input
- Window
- Physics
- Resource
- Scene
- Editor
- Rendering
- Audio

A single event can belong to multiple categories.

That makes it easier to subscribe broadly when needed, without losing strong event types.

## Priorities

Listener priorities are useful, but should be introduced carefully.

They can be appropriate when:

- some listeners must handle input before others
- editor capture must happen before gameplay capture
- safety or shutdown handlers need deterministic precedence

They can also become a source of complexity if overused.

For `nu`, priorities should exist only where they solve a real ordering problem.

## Hierarchical Propagation

Some systems, especially UI and editor hierarchies, need propagation rules.

The two common models are:

- capturing: event moves from root toward target
- bubbling: event moves from target back toward ancestors

This is especially relevant for:

- editor viewport interactions
- widget hierarchies later
- gizmo and overlay interactions

This propagation model should be scoped to hierarchical systems, not forced onto all engine events.

## Event Consumption and Cancellation

Some events should be stoppable.

Examples:

- UI consumes mouse click before gameplay sees it
- editor gizmo captures drag before camera orbit logic reacts
- modal dialog consumes keyboard input before scene tools respond

That means the event system should support:

- handled state
- optional cancellation or stop-propagation

This is particularly important in the editor.

## Event Debugging

A real engine needs event visibility.

The system should support debugging tools such as:

- event trace logs
- event counters
- category filters
- per-frame event timeline views later
- listener registration inspection

This is not optional once hot reload, editor interactions, and runtime systems are all active at once.

## Threading Considerations

Multithreaded systems should not publish directly into unsafe shared listener execution paths.

A safer model is:

- worker threads publish into thread-safe queues
- the main thread or owning system drains and dispatches at known sync points

This is especially relevant for:

- async resource loading
- file watcher notifications
- future job-system driven physics or scene work

The event system should make thread ownership explicit.

## Event Systems and `nu`

`nu` should use events heavily in these areas.

### Input

Input should publish events rather than mutating many unrelated systems directly.

### Resource hot reload

Resource changes should produce explicit reload or invalidation events.

### Physics

Collision and trigger events should be published at the simulation boundary.

### Editor interactions

Selection, transform changes, tool mode changes, and preview invalidations should be event-driven.

### Renderer integration

The renderer should receive structured invalidation or rebuild events where necessary, rather than hidden direct dependencies from unrelated systems.

## Practical Rules

### Good event usage

- publishing state changes
- coordinating across subsystems
- decoupling editor and runtime systems
- handling resource invalidation and reload
- handling input and UI propagation cleanly

### Bad event usage

- replacing every direct function call with an event
- hiding critical control flow behind generic events
- broadcasting everything to everyone
- using events as a substitute for proper ownership
- mixing commands and facts without clear boundaries

## Recommended Direction for `nu`

1. Introduce a scoped engine event bus service
2. Support both immediate and queued delivery
3. Add event categories and typed subscriptions
4. Add handled/cancelled propagation for editor and UI interactions
5. Add debug tooling for event flow inspection
6. Route hot reload, editor selection, and runtime invalidation through events deliberately

## Conclusion

A good event system in `nu` should:

- reduce subsystem coupling
- stay type-safe
- support both immediate and queued delivery
- handle editor and runtime communication cleanly
- remain debuggable as the engine grows

That is the correct communication foundation for a Vulkan-first engine with a custom editor, hot reload, rendering systems, and future gameplay logic.
