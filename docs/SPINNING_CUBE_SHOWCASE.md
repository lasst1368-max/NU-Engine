# Spinning Cube Showcase

The spinning cube demo is the current lightweight 3D showcase scene for `nu`.

Run it:

```bash
cargo run --example spinning_block_demo
```

Controls:

- `F12`: save a PNG screenshot

Screenshot output:

- `screenshots/nu_spinning_cube_<timestamp>.png`

What the scene demonstrates:

- auto-orbiting 3D camera
- multiple point lights
- live shadows
- PBR material response through `roughness` and `metallic`
- mixed primitive composition using `Cube`, `Plane`, and `Sphere`

Primary implementation:

- demo entry point: [src/demo/block.rs:13](D:/3D/API/src/demo/block.rs:13)
- scene config and camera orbit: [src/demo/block.rs:30](D:/3D/API/src/demo/block.rs:30)
- screenshot queueing: [src/demo/block.rs:240](D:/3D/API/src/demo/block.rs:240)
- lighting rig: [src/demo/block.rs:151](D:/3D/API/src/demo/block.rs:151)
- animated scene population: [src/demo/block.rs:78](D:/3D/API/src/demo/block.rs:78)

Runtime screenshot path:

- scene config screenshot field: [src/scene/mod.rs:18](D:/3D/API/src/scene/mod.rs:18)
- Vulkan screenshot capture: [src/runtime/mod.rs:1576](D:/3D/API/src/runtime/mod.rs:1576)
- PNG write path: [src/runtime/mod.rs:4391](D:/3D/API/src/runtime/mod.rs:4391)

About GIFs:

- PNG screenshots are implemented in-engine.
- GIF export is not a first-class runtime feature yet.
- The correct next step is frame-sequence export, then GIF/MP4 assembly from those frames.

