# FFI Usage

This document explains how the current C ABI works, how the C++ sample uses it, and where the implementation lives.

The active FFI surface is an `extern "C"` boundary over `nu`'s OpenGL-style scratch compatibility layer.

## Files

Core files:
- Rust FFI implementation: [src/ffi/mod.rs](D:/3D/API/src/ffi/mod.rs)
- Public C header: [include/nu_ffi.h](D:/3D/API/include/nu_ffi.h)
- C++ wrapper: [examples/cpp/nu_gl_scratch.hpp](D:/3D/API/examples/cpp/nu_gl_scratch.hpp)
- C++ sample: [examples/cpp/minecraft_block.cpp](D:/3D/API/examples/cpp/minecraft_block.cpp)
- Build script: [examples/cpp/build_minecraft_block.ps1](D:/3D/API/examples/cpp/build_minecraft_block.ps1)
- Existing sample readme: [examples/cpp/README.md](D:/3D/API/examples/cpp/README.md)

## What The ABI Is

The ABI gives C and C++ code:
- an opaque scratch rendering context
- OpenGL-style command recording functions
- byte upload for buffers and textures
- a preview window that renders the recorded scratch scene through `nu`

This is not yet a full engine embedding API with:
- non-blocking frame begin/end
- persistent host-managed windows
- general engine object lifetime control

## Core ABI Functions

The scratch context lifecycle:
- `nu_ffi_gl_context_create(...)`: [include/nu_ffi.h:45](D:/3D/API/include/nu_ffi.h:45), [src/ffi/mod.rs:741](D:/3D/API/src/ffi/mod.rs:741)
- `nu_ffi_gl_context_destroy(...)`: [include/nu_ffi.h:46](D:/3D/API/include/nu_ffi.h:46), [src/ffi/mod.rs:746](D:/3D/API/src/ffi/mod.rs:746)

The preview window:
- `nu_ffi_gl_preview_window(...)`: [include/nu_ffi.h:49](D:/3D/API/include/nu_ffi.h:49), [src/ffi/mod.rs:768](D:/3D/API/src/ffi/mod.rs:768)

Buffer upload:
- `nu_ffi_gl_buffer_data(...)`: [include/nu_ffi.h:70](D:/3D/API/include/nu_ffi.h:70), [src/ffi/mod.rs:931](D:/3D/API/src/ffi/mod.rs:931)
- `nu_ffi_gl_buffer_sub_data(...)`: [include/nu_ffi.h:77](D:/3D/API/include/nu_ffi.h:77), [src/ffi/mod.rs:973](D:/3D/API/src/ffi/mod.rs:973)

Texture upload:
- `nu_ffi_gl_tex_image_2d_rgba8(...)`: [include/nu_ffi.h:103](D:/3D/API/include/nu_ffi.h:103), [src/ffi/mod.rs:1124](D:/3D/API/src/ffi/mod.rs:1124)

Draw calls:
- `nu_ffi_gl_draw_arrays(...)`: [include/nu_ffi.h:116](D:/3D/API/include/nu_ffi.h:116), [src/ffi/mod.rs:1168](D:/3D/API/src/ffi/mod.rs:1168)
- `nu_ffi_gl_draw_elements(...)`: [include/nu_ffi.h:117](D:/3D/API/include/nu_ffi.h:117), [src/ffi/mod.rs:1193](D:/3D/API/src/ffi/mod.rs:1193)

Uniforms:
- `nu_ffi_gl_uniform_mat4(...)`: [include/nu_ffi.h:125](D:/3D/API/include/nu_ffi.h:125), [src/ffi/mod.rs:1223](D:/3D/API/src/ffi/mod.rs:1223)
- `nu_ffi_gl_uniform_vec3(...)`: [include/nu_ffi.h:126](D:/3D/API/include/nu_ffi.h:126), [src/ffi/mod.rs:1249](D:/3D/API/src/ffi/mod.rs:1249)

Handle generation:
- `nu_ffi_gl_gen_buffers(...)`: [include/nu_ffi.h:128](D:/3D/API/include/nu_ffi.h:128), [src/ffi/mod.rs:1287](D:/3D/API/src/ffi/mod.rs:1287)
- `nu_ffi_gl_gen_textures(...)`: [include/nu_ffi.h:129](D:/3D/API/include/nu_ffi.h:129), [src/ffi/mod.rs:1303](D:/3D/API/src/ffi/mod.rs:1303)
- `nu_ffi_gl_gen_vertex_arrays(...)`: [include/nu_ffi.h:130](D:/3D/API/include/nu_ffi.h:130), [src/ffi/mod.rs:1319](D:/3D/API/src/ffi/mod.rs:1319)

## C++ Wrapper

The C++ convenience wrapper is in [examples/cpp/nu_gl_scratch.hpp](D:/3D/API/examples/cpp/nu_gl_scratch.hpp).

Important wrapper entry points:
- `ScratchContext`: [examples/cpp/nu_gl_scratch.hpp:54](D:/3D/API/examples/cpp/nu_gl_scratch.hpp:54)
- `RunPreviewWindow(...)`: [examples/cpp/nu_gl_scratch.hpp:111](D:/3D/API/examples/cpp/nu_gl_scratch.hpp:111)
- templated `glBufferData(...)`: [examples/cpp/nu_gl_scratch.hpp:179](D:/3D/API/examples/cpp/nu_gl_scratch.hpp:179)
- `glTexImage2D(...)`: [examples/cpp/nu_gl_scratch.hpp:202](D:/3D/API/examples/cpp/nu_gl_scratch.hpp:202)

That wrapper intentionally looks OpenGL-like on the C++ side.

## Sample Flow

The Minecraft-style sample in [examples/cpp/minecraft_block.cpp](D:/3D/API/examples/cpp/minecraft_block.cpp) does this:

1. Build block vertex data:
   - [examples/cpp/minecraft_block.cpp:26](D:/3D/API/examples/cpp/minecraft_block.cpp:26)
2. Build block indices:
   - [examples/cpp/minecraft_block.cpp:88](D:/3D/API/examples/cpp/minecraft_block.cpp:88)
3. Build a grass/dirt atlas in memory:
   - [examples/cpp/minecraft_block.cpp:103](D:/3D/API/examples/cpp/minecraft_block.cpp:103)
4. Upload vertex data:
   - [examples/cpp/minecraft_block.cpp:157](D:/3D/API/examples/cpp/minecraft_block.cpp:157)
5. Upload index data:
   - [examples/cpp/minecraft_block.cpp:160](D:/3D/API/examples/cpp/minecraft_block.cpp:160)
6. Upload atlas pixels:
   - [examples/cpp/minecraft_block.cpp:171](D:/3D/API/examples/cpp/minecraft_block.cpp:171)
7. Open the preview window:
   - [examples/cpp/minecraft_block.cpp:188](D:/3D/API/examples/cpp/minecraft_block.cpp:188)

## Build Path

The sample build script uses:
- `g++.exe` from `C:\\msys64\\ucrt64\\bin`: [examples/cpp/build_minecraft_block.ps1:4](D:/3D/API/examples/cpp/build_minecraft_block.ps1:4)

It stages these beside the executable:
- `nu.dll`: [examples/cpp/build_minecraft_block.ps1:10](D:/3D/API/examples/cpp/build_minecraft_block.ps1:10)
- `nu.dll.lib`: [examples/cpp/build_minecraft_block.ps1:11](D:/3D/API/examples/cpp/build_minecraft_block.ps1:11)
- MinGW runtime DLLs:
  - [examples/cpp/build_minecraft_block.ps1:14](D:/3D/API/examples/cpp/build_minecraft_block.ps1:14)
  - [examples/cpp/build_minecraft_block.ps1:15](D:/3D/API/examples/cpp/build_minecraft_block.ps1:15)
  - [examples/cpp/build_minecraft_block.ps1:16](D:/3D/API/examples/cpp/build_minecraft_block.ps1:16)

It also prepends the compiler runtime path:
- [examples/cpp/build_minecraft_block.ps1:36](D:/3D/API/examples/cpp/build_minecraft_block.ps1:36)

And it fails hard if linking fails:
- [examples/cpp/build_minecraft_block.ps1:55](D:/3D/API/examples/cpp/build_minecraft_block.ps1:55)

Build command:

```powershell
cd D:\3D\API
cargo build
powershell -ExecutionPolicy Bypass -File .\examples\cpp\build_minecraft_block.ps1
```

Run command:

```powershell
cd D:\3D\API\examples\cpp
.\minecraft_block.exe
```

## How The Preview Works

The preview window does not execute native OpenGL.

What happens instead:
1. C++ records OpenGL-style scratch commands.
2. The Rust FFI layer stores the recorded commands and uploaded payloads.
3. `nu_ffi_gl_preview_window(...)` builds a `nu` scene from that scratch state.
4. The Vulkan runtime renders that scene in a native `nu` window.

That is why this path can stay OpenGL-like for API ergonomics while still using the Vulkan renderer internally.

## What Works Today

This path currently supports:
- opaque scratch context lifetime
- buffer byte upload
- RGBA8 texture upload
- OpenGL-style command recording
- preview window rendering
- the Minecraft-style block sample

## Current Boundaries

This path still does not provide:
- a non-blocking persistent window host API
- full engine object creation and update over FFI
- general-purpose material/shader management over FFI
- a true OpenGL backend
- a complete script/runtime bridge for native C++ gameplay code

## Recommended Next Steps

If you want to grow this into a serious embedding API, the next work should be:
1. a persistent `NuWindowHost` FFI surface with create/update/destroy lifecycle
2. explicit frame begin/end calls instead of one blocking preview call
3. texture/material binding that maps cleanly to the runtime PBR path
4. native script/component registration over FFI instead of only scratch rendering
